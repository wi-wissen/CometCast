// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.media.projection.MediaProjectionManager
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * Dart-facing side of the screen capture skeleton.
 *
 * MethodChannel `de.cometcast/capture`:
 * - `start` (map: maxDimension Int = 1280, fps Int = 30, bitrate Int = 4000000, notificationTitle,
 *   notificationText, notificationStop: String, all optional; the texts come from the ARB keys
 *   captureNotification*): shows the system consent dialog and, if the user agrees, starts
 *   [ScreenCaptureService]. Replies `true` (started) or `false` (user said no).
 * - `stop`: stops the service.
 * - `requestKeyFrame`: asks the encoder for a key frame.
 *
 * EventChannel `de.cometcast/capture/frames`: see [CaptureEvents].
 *
 * Android 14 rule: consent is asked for EVERY session. The consent result is passed to the service
 * once and dropped; nothing here caches it.
 *
 * UNCERTAIN: never run on a device.
 */
class CaptureBridge(
    private val activity: Activity,
    engine: FlutterEngine,
) : MethodChannel.MethodCallHandler {
    private var pendingResult: MethodChannel.Result? = null
    private var pendingArgs: Map<*, *> = emptyMap<Any, Any>()

    init {
        MethodChannel(engine.dartExecutor.binaryMessenger, METHOD_CHANNEL).setMethodCallHandler(this)
        EventChannel(engine.dartExecutor.binaryMessenger, EVENT_CHANNEL).setStreamHandler(CaptureEvents.streamHandler)
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "start" -> {
                if (pendingResult != null) {
                    result.error("busy", "A consent dialog is already open", null)
                    return
                }
                pendingResult = result
                pendingArgs = (call.arguments as? Map<*, *>) ?: emptyMap<Any, Any>()
                val manager = activity.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
                @Suppress("DEPRECATION")
                activity.startActivityForResult(manager.createScreenCaptureIntent(), REQUEST_CONSENT)
            }
            "stop" -> {
                activity.stopService(Intent(activity, ScreenCaptureService::class.java))
                result.success(null)
            }
            "requestKeyFrame" -> {
                ScreenCaptureService.instance?.requestKeyFrame()
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** Called from MainActivity.onActivityResult. Returns true if the result was ours. */
    fun onActivityResult(
        requestCode: Int,
        resultCode: Int,
        data: Intent?,
    ): Boolean {
        if (requestCode != REQUEST_CONSENT) return false
        val result = pendingResult ?: return true
        pendingResult = null
        if (resultCode != Activity.RESULT_OK || data == null) {
            result.success(false)
            return true
        }
        val args = pendingArgs
        val intent =
            Intent(activity, ScreenCaptureService::class.java)
                .putExtra(ScreenCaptureService.EXTRA_RESULT_CODE, resultCode)
                .putExtra(ScreenCaptureService.EXTRA_CONSENT_DATA, data)
                .putExtra(ScreenCaptureService.EXTRA_MAX_DIMENSION, (args["maxDimension"] as? Int) ?: 1280)
                .putExtra(ScreenCaptureService.EXTRA_FPS, (args["fps"] as? Int) ?: 30)
                .putExtra(ScreenCaptureService.EXTRA_BITRATE, (args["bitrate"] as? Int) ?: 4_000_000)
                .putExtra(ScreenCaptureService.EXTRA_TITLE, args["notificationTitle"] as? String)
                .putExtra(ScreenCaptureService.EXTRA_TEXT, args["notificationText"] as? String)
                .putExtra(ScreenCaptureService.EXTRA_STOP_LABEL, args["notificationStop"] as? String)
        activity.startForegroundService(intent)
        result.success(true)
        return true
    }

    companion object {
        const val METHOD_CHANNEL = "de.cometcast/capture"
        const val EVENT_CHANNEL = "de.cometcast/capture/frames"
        private const val REQUEST_CONSENT = 4712
    }
}
