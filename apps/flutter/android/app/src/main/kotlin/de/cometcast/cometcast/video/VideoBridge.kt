// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

import android.os.Handler
import android.os.Looper
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.view.TextureRegistry

/**
 * Hardware video decoding for the receiver on Android (see docs/android.md, "Receiver video").
 *
 * MethodChannel `de.cometcast/video`:
 * - `start` -> `{textureId: Long}`: creates a Flutter [TextureRegistry.SurfaceProducer] and the
 *   [HardwareVideoDecoder] that renders into it. Dart shows it with `Texture(textureId: ...)`.
 *   Throws (PlatformException) if no texture can be made.
 * - `queue` `{data: ByteArray, ptsUs: Long, key: Boolean}`: one Annex B access unit. Never blocks;
 *   returns `false` when the unit was dropped.
 * - `stop`: releases decoder and texture. Safe when nothing runs.
 *
 * EventChannel `de.cometcast/video/events`: maps `{type: "size", width, height}` (picture size,
 * for the aspect ratio), `{type: "needKeyFrame"}` (a unit was dropped or the surface is new: ask
 * the sender for a key frame) and `{type: "error", message}` (the decoder failed: Dart falls back
 * to the software decoder).
 *
 * UNCERTAIN: never run on a device. The SurfaceProducer callbacks (`onSurfaceAvailable`,
 * `onSurfaceCleanup`) are used as the Flutter engine of this build declares them.
 */
class VideoBridge(
    private val engine: FlutterEngine,
) : MethodChannel.MethodCallHandler,
    HardwareVideoDecoder.Listener {
    private val main = Handler(Looper.getMainLooper())
    @Volatile
    private var producer: TextureRegistry.SurfaceProducer? = null
    private var decoder: HardwareVideoDecoder? = null

    /** The surface for the worker thread; set and cleared on the platform thread only. */
    @Volatile
    private var surface: android.view.Surface? = null

    @Volatile
    private var sink: EventChannel.EventSink? = null

    init {
        MethodChannel(engine.dartExecutor.binaryMessenger, METHOD_CHANNEL).setMethodCallHandler(this)
        EventChannel(engine.dartExecutor.binaryMessenger, EVENT_CHANNEL).setStreamHandler(
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
            },
        )
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "start" -> {
                try {
                    release()
                    val p = engine.renderer.createSurfaceProducer()
                    val d = HardwareVideoDecoder({ surface }, this)
                    surface = p.surface
                    p.setCallback(
                        object : io.flutter.view.TextureRegistry.SurfaceProducer.Callback {
                            override fun onSurfaceAvailable() {
                                surface = p.surface
                                d.surfaceAvailable()
                            }

                            // Flutter may destroy the surface when this returns: the decoder has
                            // released its codec (bounded wait) before it does.
                            override fun onSurfaceCleanup() {
                                surface = null
                                d.surfaceDestroyed()
                            }
                        },
                    )
                    producer = p
                    decoder = d
                    d.start()
                    result.success(mapOf("textureId" to p.id()))
                } catch (e: Exception) {
                    release()
                    result.error("video_start", e.message, null)
                }
            }
            "queue" -> {
                val d = decoder
                val data = call.argument<ByteArray>("data")
                val pts = (call.argument<Number>("ptsUs"))?.toLong() ?: 0L
                val key = call.argument<Boolean>("key") ?: false
                result.success(d != null && data != null && d.queue(data, pts, key))
            }
            "stop" -> {
                release()
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** Called when the engine goes away (MainActivity.cleanUpFlutterEngine). */
    fun release() {
        decoder?.stop()
        decoder = null
        surface = null
        producer?.let {
            it.setCallback(null)
            it.release()
        }
        producer = null
    }

    private fun emit(event: Map<String, Any?>) {
        main.post { sink?.success(event) }
    }

    override fun onSize(
        width: Int,
        height: Int,
    ) {
        // Called on the decoder's worker thread; the producer belongs to the platform thread.
        main.post { producer?.setSize(width, height) }
        emit(mapOf("type" to "size", "width" to width, "height" to height))
    }

    override fun onNeedKeyFrame() = emit(mapOf("type" to "needKeyFrame"))

    override fun onError(message: String) = emit(mapOf("type" to "error", "message" to message))

    companion object {
        const val METHOD_CHANNEL = "de.cometcast/video"
        const val EVENT_CHANNEL = "de.cometcast/video/events"
    }
}
