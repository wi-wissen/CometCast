// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.EventChannel

/**
 * Hands encoded video from the capture service to Dart through the EventChannel
 * `de.cometcast/capture/frames`.
 *
 * Event payloads are maps (Flutter's standard codec; `ByteArray` arrives in Dart as `Uint8List`):
 *
 * - `{type: "config", mime: "video/avc", width: Int, height: Int, data: ByteArray}`:
 *   the H.264 codec config (SPS/PPS in Annex B form), sent once per session before frames.
 * - `{type: "frame", ptsUs: Long, keyFrame: Boolean, data: ByteArray}`:
 *   one encoded access unit in Annex B form (start codes, as MediaCodec produces it).
 * - `{type: "stopped", reason: String}`: the session ended (user stopped the projection, the
 *   system revoked it, or the service was stopped).
 * - `{type: "error", message: String}`: the encoder or the projection failed.
 *
 * UNCERTAIN: not run on a device. Whether the standard method codec copes with the data rate of a
 * 1080p30 stream (about 4 Mbit/s, one copy per frame) is untested; a native path straight into
 * the Rust core would avoid it.
 */
object CaptureEvents {
    private val main = Handler(Looper.getMainLooper())

    @Volatile
    private var sink: EventChannel.EventSink? = null

    /** The stream handler for the EventChannel. */
    val streamHandler =
        object : EventChannel.StreamHandler {
            override fun onListen(
                arguments: Any?,
                events: EventChannel.EventSink?,
            ) {
                sink = events
            }

            override fun onCancel(arguments: Any?) {
                sink = null
            }
        }

    /** Sends [event] to Dart on the main thread (EventSink must be used from there). */
    fun emit(event: Map<String, Any?>) {
        main.post { sink?.success(event) }
    }

    fun emitStopped(reason: String) = emit(mapOf("type" to "stopped", "reason" to reason))

    fun emitError(message: String) = emit(mapOf("type" to "error", "message" to message))
}
