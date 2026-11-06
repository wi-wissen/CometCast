// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Bundle
import android.view.Surface

/**
 * H.264 encoder with surface input: the VirtualDisplay of the MediaProjection renders into
 * [inputSurface], MediaCodec encodes, and a drain thread passes every output buffer to
 * [CaptureEvents].
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
    private lateinit var codec: MediaCodec

    /** Surface the VirtualDisplay renders into. Valid after [start]. */
    lateinit var inputSurface: Surface
        private set

    @Volatile
    private var running = false
    private var drainThread: Thread? = null

    fun start() {
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

        codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        inputSurface = codec.createInputSurface()
        codec.start()

        running = true
        drainThread =
            Thread({ drainLoop() }, "cometcast-encoder-drain").also {
                it.isDaemon = true
                it.start()
            }
    }

    /** Asks the encoder for a key frame soon (for example when a receiver joins). */
    fun requestKeyFrame() {
        if (!running) return
        val params = Bundle()
        params.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0)
        try {
            codec.setParameters(params)
        } catch (e: IllegalStateException) {
            // Codec already stopped; nothing to do.
        }
    }

    fun stop() {
        running = false
        drainThread?.join(1_000)
        try {
            codec.stop()
        } catch (e: IllegalStateException) {
            // Already stopped.
        }
        codec.release()
        inputSurface.release()
    }

    private fun drainLoop() {
        val info = MediaCodec.BufferInfo()
        try {
            while (running) {
                val index = codec.dequeueOutputBuffer(info, 10_000)
                if (index < 0) continue // timeout, format change or similar: nothing to send
                val buffer = codec.getOutputBuffer(index)
                if (buffer != null && info.size > 0) {
                    buffer.position(info.offset)
                    buffer.limit(info.offset + info.size)
                    val data = ByteArray(info.size)
                    buffer.get(data)
                    if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) {
                        CaptureEvents.emit(
                            mapOf(
                                "type" to "config",
                                "mime" to MediaFormat.MIMETYPE_VIDEO_AVC,
                                "width" to width,
                                "height" to height,
                                "data" to data,
                            ),
                        )
                    } else {
                        CaptureEvents.emit(
                            mapOf(
                                "type" to "frame",
                                "ptsUs" to info.presentationTimeUs,
                                "keyFrame" to (info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0),
                                "data" to data,
                            ),
                        )
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
