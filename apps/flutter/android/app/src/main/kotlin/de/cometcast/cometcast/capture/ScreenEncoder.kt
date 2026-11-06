// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Build
import android.os.Bundle
import android.view.Surface

/**
 * H.264 encoder with surface input: the VirtualDisplay of the MediaProjection renders into
 * [inputSurface], MediaCodec encodes, and a drain thread passes every output buffer to
 * [CaptureEvents] (which applies backpressure, see there).
 *
 * UNCERTAIN: compiled only. Not run on any device, so none of these are verified: that the
 * chosen format (baseline-compatible defaults, CBR) is accepted by hardware encoders of TV boxes,
 * the output frame pacing when the screen does not change (surface input may then produce no
 * frames; a repeat-previous-frame setting is used, see below), and the bitrate quality.
 */
class ScreenEncoder(
    private val width: Int,
    private val height: Int,
    private val fps: Int,
    private val bitrate: Int,
) {
    private var codec: MediaCodec? = null

    /** Surface the VirtualDisplay renders into. Valid after [start]. */
    lateinit var inputSurface: Surface
        private set

    private var surfaceCreated = false

    @Volatile
    private var running = false
    private var drainThread: Thread? = null

    // Drain thread only: after a dropped frame, delta frames are discarded until a key frame.
    private var needKeyFrame = false
    private var lastKeyRequestMs = 0L

    private fun makeFormat(withProfile: Boolean): MediaFormat {
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, width, height)
        format.setInteger(
            MediaFormat.KEY_COLOR_FORMAT,
            MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface,
        )
        format.setInteger(MediaFormat.KEY_BIT_RATE, bitrate)
        format.setInteger(MediaFormat.KEY_FRAME_RATE, fps)
        format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2)
        format.setInteger(
            MediaFormat.KEY_BITRATE_MODE,
            MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR,
        )
        // With surface input the encoder only gets frames when the screen changes. Repeat the last
        // frame after 1 s so that a still screen still produces output (key for a receiver that
        // joins late). Value in microseconds.
        format.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 1_000_000L)
        // The WebRTC offer of the Rust core says Constrained Baseline (profile-level-id 42e01f), so
        // ask for Baseline; if the encoder rejects it the caller retries without (and the output
        // may then be a higher profile than announced). UNCERTAIN: not run on a device.
        if (withProfile) {
            format.setInteger(
                MediaFormat.KEY_PROFILE,
                MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline,
            )
        }
        // Asks for SPS/PPS in front of every key frame (API 29+). Encoders may ignore it; the
        // Rust side prepends cached SPS/PPS to key frames that lack them anyway.
        if (Build.VERSION.SDK_INT >= 29) {
            format.setInteger(MediaFormat.KEY_PREPEND_HEADER_TO_SYNC_FRAMES, 1)
        }
        return format
    }

    /** Starts the encoder. If this throws, everything it created has been released again. */
    fun start() {
        try {
            var c = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            codec = c
            try {
                c.configure(makeFormat(withProfile = true), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            } catch (e: RuntimeException) {
                // Baseline not supported (IllegalArgumentException or CodecException): try again
                // without a profile on a fresh codec. UNCERTAIN: never run on a device.
                c.release()
                c = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
                codec = c
                c.configure(makeFormat(withProfile = false), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            }
            inputSurface = c.createInputSurface()
            surfaceCreated = true
            c.start()

            running = true
            drainThread =
                Thread({ drainLoop(c) }, "cometcast-encoder-drain").also {
                    it.isDaemon = true
                    it.start()
                }
        } catch (e: Exception) {
            // The caller never gets the encoder, so it cannot release it: do it here.
            stop()
            throw e
        }
    }

    /** Asks the encoder for a key frame soon (for example when a receiver joins). */
    fun requestKeyFrame() {
        if (!running) return
        val params = Bundle()
        params.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0)
        try {
            codec?.setParameters(params)
        } catch (e: IllegalStateException) {
            // Codec already stopped; nothing to do.
        }
    }

    /** Releases everything. Safe to call when [start] failed half way, and more than once. */
    fun stop() {
        running = false
        drainThread?.join(1_000)
        drainThread = null
        val c = codec
        codec = null
        if (c != null) {
            try {
                c.stop()
            } catch (e: IllegalStateException) {
                // Never started or already stopped.
            }
            c.release()
        }
        if (surfaceCreated) {
            surfaceCreated = false
            inputSurface.release()
        }
    }

    /**
     * Decides before copying whether a frame can be delivered. Nobody listening or a full queue
     * drops the frame (counted); until the next key frame all delta frames are dropped as well.
     */
    private fun shouldSend(isKey: Boolean): Boolean {
        if (!CaptureEvents.wantsFrame()) {
            CaptureEvents.countDropped()
            needKeyFrame = true
            return false
        }
        if (needKeyFrame) {
            if (isKey) {
                needKeyFrame = false
            } else {
                CaptureEvents.countDropped()
                val now = System.currentTimeMillis()
                if (now - lastKeyRequestMs > 500) {
                    lastKeyRequestMs = now
                    requestKeyFrame()
                }
                return false
            }
        }
        return true
    }

    private fun drainLoop(codec: MediaCodec) {
        val info = MediaCodec.BufferInfo()
        try {
            while (running) {
                val index = codec.dequeueOutputBuffer(info, 10_000)
                if (index < 0) continue // timeout, format change or similar: nothing to send
                val buffer = codec.getOutputBuffer(index)
                if (buffer != null && info.size > 0) {
                    val isConfig = info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0
                    val isKey = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
                    if (isConfig || shouldSend(isKey)) {
                        buffer.position(info.offset)
                        buffer.limit(info.offset + info.size)
                        val data = ByteArray(info.size)
                        buffer.get(data)
                        if (isConfig) {
                            CaptureEvents.emitConfig(
                                mapOf(
                                    "type" to "config",
                                    "mime" to MediaFormat.MIMETYPE_VIDEO_AVC,
                                    "width" to width,
                                    "height" to height,
                                    "data" to data,
                                ),
                            )
                        } else {
                            val queued =
                                CaptureEvents.emitFrame(
                                    mapOf(
                                        "type" to "frame",
                                        "ptsUs" to info.presentationTimeUs,
                                        "keyFrame" to isKey,
                                        "dropped" to CaptureEvents.dropped,
                                        "data" to data,
                                    ),
                                )
                            if (!queued) needKeyFrame = true
                        }
                    }
                }
                codec.releaseOutputBuffer(index, false)
                if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) break
            }
        } catch (e: IllegalStateException) {
            if (running) CaptureEvents.emitError("Encoder failed: ${e.message}")
        }
    }
}
