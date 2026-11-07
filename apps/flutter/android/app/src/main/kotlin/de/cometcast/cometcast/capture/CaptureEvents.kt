// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.EventChannel
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/**
 * Hands encoded video from the capture service to Dart through the EventChannel
 * `de.cometcast/capture/frames`.
 *
 * Event payloads are maps (Flutter's standard codec; `ByteArray` arrives in Dart as `Uint8List`):
 *
 * - `{type: "config", mime: "video/avc", width: Int, height: Int, data: ByteArray}`:
 *   the H.264 codec config (SPS/PPS in Annex B form), sent once per session before frames. It is
 *   cached and replayed to every listener that subscribes later (together with a key frame
 *   request), so a late listener does not miss it.
 * - `{type: "frame", ptsUs: Long, keyFrame: Boolean, dropped: Long, data: ByteArray}`:
 *   one encoded access unit in Annex B form (start codes, as MediaCodec produces it). `dropped`
 *   is the number of frames dropped so far in this session (see below).
 * - `{type: "stopped", reason: String}`: the session ended (user stopped the projection, the
 *   system revoked it, or the service was stopped). Sent at most once per session.
 * - `{type: "error", message: String}`: the encoder or the projection failed.
 *
 * Size changes: every encoder carries a generation ([GenerationGate]); output of an encoder that
 * is not the active one is dropped, also while it waits for the main looper, so the old and the
 * new size never mix. The service activates the new generation just before it points the
 * VirtualDisplay at the new encoder, and the new encoder publishes its config only together with
 * its first key frame.
 *
 * Backpressure: frames are dropped (and counted) while nobody listens and while more than
 * [MAX_PENDING_FRAMES] frames wait for the main looper. After a drop the encoder discards delta
 * frames until it has produced a key frame (it asks for one), so the receiver never gets a
 * stream with a hole in it. The queue stays bounded; memory does not grow.
 *
 * UNCERTAIN: not run on a device. Whether the standard method codec copes with the data rate of a
 * 1080p30 stream (about 4 Mbit/s, one copy per frame) is untested; a native path straight into
 * the Rust core would avoid it.
 */
object CaptureEvents {
    /** Upper bound of encoded frames waiting for the main looper. */
    const val MAX_PENDING_FRAMES = 8

    private val main = Handler(Looper.getMainLooper())

    @Volatile
    private var sink: EventChannel.EventSink? = null

    @Volatile
    private var lastConfig: Map<String, Any?>? = null

    private val pendingFrames = AtomicInteger(0)
    private val droppedFrames = AtomicLong(0)
    private val stoppedEmitted = AtomicBoolean(false)

    /** Which encoder may deliver (see [GenerationGate]); the others are dropped. */
    val generations = GenerationGate()

    /** Number of frames dropped in the current session. */
    val dropped: Long get() = droppedFrames.get()

    /** The stream handler for the EventChannel. */
    val streamHandler =
        object : EventChannel.StreamHandler {
            override fun onListen(
                arguments: Any?,
                events: EventChannel.EventSink?,
            ) {
                sink = events
                // Late listener: replay the codec config and ask for a key frame (main thread).
                lastConfig?.let { events?.success(it) }
                ScreenCaptureService.instance?.requestKeyFrame()
            }

            override fun onCancel(arguments: Any?) {
                sink = null
            }
        }

    /** Called by the service when a new session starts: forget the previous one. */
    fun beginSession() {
        lastConfig = null
        droppedFrames.set(0)
        stoppedEmitted.set(false)
        generations.reset()
    }

    /** True if a frame produced now would be delivered (a listener exists and the queue has room). */
    fun wantsFrame(generation: Int): Boolean =
        generations.accepts(generation) && sink != null && pendingFrames.get() < MAX_PENDING_FRAMES

    fun countDropped() {
        droppedFrames.incrementAndGet()
    }

    /** Sends [event] to Dart on the main thread (EventSink must be used from there). */
    fun emit(event: Map<String, Any?>) {
        main.post { sink?.success(event) }
    }

    /** Caches the codec config for late listeners, then sends it (dropped for an old encoder). */
    fun emitConfig(
        generation: Int,
        event: Map<String, Any?>,
    ) {
        if (!generations.accepts(generation)) return
        lastConfig = event
        main.post { if (generations.accepts(generation)) sink?.success(event) }
    }

    /** Queues a frame unless nobody listens or the queue is full. Returns whether it was queued. */
    fun emitFrame(
        generation: Int,
        event: Map<String, Any?>,
    ): Boolean {
        if (!generations.accepts(generation)) return false // an old encoder: no count, no request
        if (sink == null) {
            countDropped()
            return false
        }
        if (pendingFrames.incrementAndGet() > MAX_PENDING_FRAMES) {
            pendingFrames.decrementAndGet()
            countDropped()
            return false
        }
        main.post {
            pendingFrames.decrementAndGet()
            // The generation may have changed while the frame waited for the looper.
            if (generations.accepts(generation)) sink?.success(event)
        }
        return true
    }

    /** Sends `stopped` once per session (the projection callback and onDestroy both report it). */
    fun emitStopped(reason: String) {
        if (stoppedEmitted.compareAndSet(false, true)) {
            emit(mapOf("type" to "stopped", "reason" to reason))
        }
    }

    fun emitError(message: String) = emit(mapOf("type" to "error", "message" to message))
}
