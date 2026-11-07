// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.view.Surface
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock

/**
 * H.264 decoding with the device's MediaCodec into a [Surface] (the Flutter texture). Fed with
 * Annex B access units by [VideoBridge]; one worker thread owns the codec.
 *
 * STATUS: compiled only, NEVER RUN on a device (docs/android.md). Everything about real decoder
 * behaviour is UNCERTAIN.
 *
 * Design:
 * - The codec is created at the first key frame, with the size read from its SPS ([H264Util.spsSize])
 *   and the SPS/PPS as `csd-0` / `csd-1`. A key frame with another size (the sender rotated) makes
 *   the worker release the codec and configure a new one: no reliance on adaptive playback.
 * - Backpressure: at most [QUEUE] units wait (the oldest delta frame goes first, see [UnitQueue]).
 *   When the queue overflowed or the codec has no free input buffer for [INPUT_WAIT_MS], delta
 *   frames are skipped until the next key frame. [Listener.onNeedKeyFrame] asks the sender for
 *   one, and again once per second ([DecoderState]) up to [MAX_KEY_REQUESTS] times; if none comes
 *   the decoder reports an error (the app then decodes in software).
 * - Surface teardown is synchronous (see [surfaceDestroyed]); the pure decisions are in [DecoderState].
 * - Output buffers are released with `render = true` as soon as they come: lowest latency, no
 *   pacing (screen sharing, not film playback).
 * - Errors (CodecException, IllegalStateException, a missing decoder) end the codec and call
 *   [Listener.onError]; the Dart side then switches to the software decoder.
 */
class HardwareVideoDecoder(
    private val surfaceProvider: () -> Surface?,
    private val listener: Listener,
) {
    interface Listener {
        fun onSize(
            width: Int,
            height: Int,
        )

        fun onNeedKeyFrame()

        fun onError(message: String)
    }

    private class Frame(
        val data: ByteArray,
        val ptsUs: Long,
        val key: Boolean,
    )

    private val queue = UnitQueue<Frame>(QUEUE) { it.key }
    private val state = DecoderState()
    private val pts = PtsUnwrapper()

    /** Guards `codec` and everything done with it (worker, teardown from the platform thread). */
    private val codecLock = ReentrantLock()

    @Volatile
    private var running = false
    private var thread: Thread? = null

    // Under codecLock only.
    private var codec: MediaCodec? = null
    private var width = 0
    private var height = 0
    private var lastSps: ByteArray? = null

    @Volatile
    var dropped = 0L
        private set

    @Volatile
    var decodedUnits = 0L
        private set

    fun start() {
        if (running) return
        state.reset()
        running = true
        thread =
            Thread({ loop() }, "cometcast-video-decode").also {
                it.isDaemon = true
                it.start()
            }
    }

    /** From any thread. Returns false if the unit was dropped (queue full). */
    fun queue(
        data: ByteArray,
        ptsUs: Long,
        key: Boolean,
    ): Boolean {
        if (!running) return false
        if (queue.offer(Frame(data, pts.unwrap(ptsUs), key))) {
            // The oldest delta frame was dropped: the stream has a hole until the next key frame.
            dropped++
            state.requireKeyFrame()
            return false
        }
        return true
    }

    /**
     * The texture's surface is gone (Flutter's `onSurfaceCleanup`). Synchronous: when this returns
     * the codec is released and nothing renders any more, unless the worker is stuck inside the
     * codec for longer than [TEARDOWN_WAIT_MS] (then the worker releases it at its next step and
     * [DecoderState.mayRender] already keeps it from rendering).
     */
    fun surfaceDestroyed() {
        state.surfaceDestroyed()
        state.beginTeardown()
        queue.clear()
        if (codecLock.tryLock(TEARDOWN_WAIT_MS, TimeUnit.MILLISECONDS)) {
            try {
                releaseCodec()
            } finally {
                codecLock.unlock()
            }
        }
    }

    /** A new surface exists: decoding restarts at the next key frame. */
    fun surfaceAvailable() {
        state.surfaceAvailable()
        listener.onNeedKeyFrame()
    }

    fun stop() {
        state.beginTeardown()
        running = false
        queue.clear()
        thread?.join(1_000)
        thread = null
        if (codecLock.tryLock(TEARDOWN_WAIT_MS, TimeUnit.MILLISECONDS)) {
            try {
                releaseCodec()
            } finally {
                codecLock.unlock()
            }
        }
    }

    private fun loop() {
        val info = MediaCodec.BufferInfo()
        try {
            while (running) {
                if (state.keyRequestDue()) listener.onNeedKeyFrame()
                if (state.exhausted()) {
                    listener.onError("No key frame arrived after $MAX_KEY_REQUESTS requests")
                    return
                }
                val u = queue.poll(10)
                codecLock.lock()
                try {
                    if (state.isSurfaceLost() && codec != null) releaseCodec()
                    if (u != null && !state.isSurfaceLost()) feed(u)
                    drain(info)
                } finally {
                    codecLock.unlock()
                }
            }
        } catch (e: Exception) {
            if (running) listener.onError("Video decoder failed: ${e.message}")
        } finally {
            // The worker is gone: later queue() calls must not pretend to feed it.
            running = false
            codecLock.lock()
            try {
                releaseCodec()
            } finally {
                codecLock.unlock()
            }
        }
    }

    private fun feed(u: Frame) {
        val nals = H264Util.splitNals(u.data)
        val isKey = u.key || H264Util.hasIdr(nals)
        if (!state.admit(isKey)) {
            dropped++
            return
        }
        if (isKey) {
            val sets = H264Util.parameterSets(u.data, nals)
            if (sets != null) {
                val spsNal = nals.first { it.type == H264Util.NAL_SPS }
                val size = H264Util.spsSize(u.data.copyOfRange(spsNal.start, spsNal.end))
                val changed =
                    codec == null ||
                        (size != null && (size.first != width || size.second != height)) ||
                        !sets.first.contentEquals(lastSps ?: ByteArray(0))
                if (changed) {
                    releaseCodec()
                    configure(sets, size)
                }
            } else if (codec == null) {
                // A key frame without SPS/PPS cannot start a decoder; ask again (backoff).
                state.requireKeyFrame()
                return
            }
        }
        val c = codec ?: return
        val index = c.dequeueInputBuffer(INPUT_WAIT_MS * 1000L)
        if (index < 0) {
            dropped++
            state.requireKeyFrame()
            return
        }
        val buf = c.getInputBuffer(index)
        if (buf == null || buf.capacity() < u.data.size) {
            c.queueInputBuffer(index, 0, 0, u.ptsUs, 0)
            dropped++
            // Asking again for the same too large unit would never end: after a few tries the
            // decoder reports an error and the app falls back to the software decoder.
            if (state.oversize() == DecoderState.Overflow.GIVE_UP) {
                throw IllegalStateException(
                    "access unit of ${u.data.size} bytes does not fit the input buffer (${buf?.capacity()})",
                )
            }
            return
        }
        buf.clear()
        buf.put(u.data)
        c.queueInputBuffer(index, 0, u.data.size, u.ptsUs, if (isKey) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0)
        decodedUnits++
    }

    private fun configure(
        sets: Pair<ByteArray, ByteArray>,
        size: Pair<Int, Int>?,
    ) {
        val surface = surfaceProvider()
        if (surface == null || !surface.isValid) {
            state.requireKeyFrame()
            return
        }
        // Without a parsed size a generous default is used; the decoder reports the real one.
        val w = size?.first ?: DEFAULT_W
        val h = size?.second ?: DEFAULT_H
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w, h)
        format.setByteBuffer("csd-0", java.nio.ByteBuffer.wrap(sets.first))
        format.setByteBuffer("csd-1", java.nio.ByteBuffer.wrap(sets.second))
        format.setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, MAX_INPUT)
        if (Build.VERSION.SDK_INT >= 30) {
            // Hint for low latency; decoders may ignore it. UNCERTAIN: effect not measured.
            format.setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
        }
        if (Build.VERSION.SDK_INT >= 23) {
            format.setInteger(MediaFormat.KEY_PRIORITY, 0) // real time
        }
        val c = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        try {
            c.configure(format, surface, null, 0)
            c.start()
        } catch (e: Exception) {
            c.release()
            throw e
        }
        codec = c
        width = w
        height = h
        lastSps = sets.first
        if (size != null) listener.onSize(w, h)
    }

    private fun drain(info: MediaCodec.BufferInfo) {
        val c = codec ?: return
        var first = true
        while (true) {
            val index = c.dequeueOutputBuffer(info, if (first) 2_000L else 0L)
            first = false
            when {
                index >= 0 -> c.releaseOutputBuffer(index, state.mayRender())
                index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                    val f = c.outputFormat
                    val ow = f.getInteger(MediaFormat.KEY_WIDTH)
                    val oh = f.getInteger(MediaFormat.KEY_HEIGHT)
                    // Crop keys, when present, give the visible size (API 23+ constants are strings).
                    val cropW =
                        if (f.containsKey("crop-right") && f.containsKey("crop-left")) {
                            f.getInteger("crop-right") - f.getInteger("crop-left") + 1
                        } else {
                            ow
                        }
                    val cropH =
                        if (f.containsKey("crop-bottom") && f.containsKey("crop-top")) {
                            f.getInteger("crop-bottom") - f.getInteger("crop-top") + 1
                        } else {
                            oh
                        }
                    listener.onSize(cropW, cropH)
                }
                else -> return
            }
        }
    }

    private fun releaseCodec() {
        val c = codec
        codec = null
        if (c != null) {
            try {
                c.stop()
            } catch (e: IllegalStateException) {
                // Already stopped.
            }
            c.release()
        }
        lastSps = null
        width = 0
        height = 0
    }

    companion object {
        const val QUEUE = 8
        const val INPUT_WAIT_MS = 20L
        const val DEFAULT_W = 1920
        const val DEFAULT_H = 1080
        /** Same limit as the RTP assembler (`MAX_ACCESS_UNIT_BYTES` in cc-media): nothing it passes on is refused here. */
        const val MAX_INPUT = 8 * 1024 * 1024
        const val TEARDOWN_WAIT_MS = 500L
        const val MAX_KEY_REQUESTS = DecoderState.MAX_KEY_REQUESTS
    }
}
