// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.app.Activity
import android.content.Context
import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
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
 * - `requestSyncFrame` (alias `requestKeyFrame`): asks the encoder for a sync frame
 *   (`MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME`). Called by Dart when the Rust core wants one.
 *
 * EventChannel `de.cometcast/capture/frames`: see [CaptureEvents].
 *
 * Android 13+ (API 33): `start` first asks for POST_NOTIFICATIONS (system dialog, only while it is
 * not granted), then shows the consent dialog whatever the answer was. The capture works without
 * the permission; the notification is then hidden. UNCERTAIN: not tried on a device.
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
    private var consentShown = false
    private var ignoreConsentResult = false
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
                if (ScreenCaptureService.instance != null) {
                    // One session at a time: refuse before asking for consent, which would be
                    // used for nothing (the running service ignores a second start).
                    result.success(false)
                    return
                }
                pendingResult = result
                pendingArgs = (call.arguments as? Map<*, *>) ?: emptyMap<Any, Any>()
                if (needsNotificationPermission()) {
                    // Asked first; the consent dialog follows whatever the user answers
                    // (see onRequestPermissionsResult). Denying only hides the notification.
                    activity.requestPermissions(
                        arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                        REQUEST_NOTIFICATIONS,
                    )
                } else {
                    launchConsent()
                }
            }
            "stop" -> {
                cancelPendingConsent()
                activity.stopService(Intent(activity, ScreenCaptureService::class.java))
                result.success(null)
            }
            // `requestSyncFrame` is the name the Dart wrapper uses; `requestKeyFrame` is the older
            // alias of the skeleton. Both set MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME.
            "requestSyncFrame", "requestKeyFrame" -> {
                ScreenCaptureService.instance?.requestKeyFrame()
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /**
     * A `stop` while a permission or consent dialog is open: answer the pending `start` with
     * `false`, close the consent dialog and remember that its result must be ignored, so the
     * service is not started late with nobody to use it. UNCERTAIN: not run on a device.
     */
    private fun cancelPendingConsent() {
        val result = pendingResult ?: return
        pendingResult = null
        if (consentShown) {
            ignoreConsentResult = true
            activity.finishActivity(REQUEST_CONSENT)
        }
        result.success(false)
    }

    private fun needsNotificationPermission(): Boolean =
        Build.VERSION.SDK_INT >= 33 &&
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED

    private fun launchConsent() {
        val manager = activity.getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        @Suppress("DEPRECATION")
        consentShown = true
        activity.startActivityForResult(manager.createScreenCaptureIntent(), REQUEST_CONSENT)
    }

    /**
     * Called from MainActivity.onRequestPermissionsResult. Returns true if the result was ours.
     * Granted or denied makes no difference for the capture: the foreground service runs without
     * the permission, only its notification is hidden by the system (Android 13+).
     */
    fun onRequestPermissionsResult(requestCode: Int): Boolean {
        if (requestCode != REQUEST_NOTIFICATIONS) return false
        if (pendingResult != null) launchConsent()
        return true
    }

    /** Called from MainActivity.onActivityResult. Returns true if the result was ours. */
    fun onActivityResult(
        requestCode: Int,
        resultCode: Int,
        data: Intent?,
    ): Boolean {
        if (requestCode != REQUEST_CONSENT) return false
        consentShown = false
        if (ignoreConsentResult) {
            // The dialog was cancelled by `stop`: whatever it answered, start nothing.
            ignoreConsentResult = false
            return true
        }
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
        private const val REQUEST_NOTIFICATIONS = 4713
    }
}
