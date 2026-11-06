// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.DisplayMetrics
import android.view.WindowManager
import de.cometcast.cometcast.R

/**
 * Foreground service that captures the screen with MediaProjection and encodes it to H.264.
 *
 * STATUS: skeleton. It compiles in `flutter build apk --debug` but was NEVER RUN (no device or
 * emulator here). Everything below that talks about Android behaviour comes from the Android
 * documentation as the author understood it and is UNCERTAIN until tested on a device.
 *
 * Rules this service follows (Android 14 / API 34 and newer):
 * - The manifest declares `foregroundServiceType="mediaProjection"` and the permissions
 *   FOREGROUND_SERVICE and FOREGROUND_SERVICE_MEDIA_PROJECTION.
 * - `startForeground(..., FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)` is called BEFORE
 *   `MediaProjectionManager.getMediaProjection(...)`.
 * - A `MediaProjection.Callback` is registered BEFORE `createVirtualDisplay(...)`.
 * - Consent is per session: the consent result (resultCode + data Intent) is used once, for one
 *   projection, and never stored. Every new session asks the user again (see [CaptureBridge]). The
 *   system may stop the projection at any time (user taps "stop sharing", screen lock, ...); that
 *   ends the session via [MediaProjection.Callback.onStop].
 *
 * TODO (not wired): the encoded frames only reach Dart (EventChannel, see [CaptureEvents]).
 * `cc-sender` currently sends a synthetic VP8 test stream only; there is no H.264 path into the
 * Rust core (no RTP packetisation for H.264, no way to pass external frames through
 * `crates/cc-ffi`). Until that exists the captured video goes nowhere useful.
 */
class ScreenCaptureService : Service() {
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var encoder: ScreenEncoder? = null
    private val main = Handler(Looper.getMainLooper())

    private val projectionCallback =
        object : MediaProjection.Callback() {
            override fun onStop() {
                // The user or the system ended the projection.
                CaptureEvents.emitStopped("projection_stopped")
                stopSelf()
            }
        }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        if (intent == null || intent.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        if (projection != null) return START_NOT_STICKY // one session at a time

        val resultCode = intent.getIntExtra(EXTRA_RESULT_CODE, 0)
        val consent: Intent? =
            if (Build.VERSION.SDK_INT >= 33) {
                intent.getParcelableExtra(EXTRA_CONSENT_DATA, Intent::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(EXTRA_CONSENT_DATA)
            }
        if (consent == null) {
            CaptureEvents.emitError("Missing screen capture consent")
            stopSelf()
            return START_NOT_STICKY
        }

        // 1. Foreground first (Android 14: before getMediaProjection).
        startAsForeground(
            intent.getStringExtra(EXTRA_TITLE) ?: "Comet Cast",
            intent.getStringExtra(EXTRA_TEXT) ?: "",
            intent.getStringExtra(EXTRA_STOP_LABEL) ?: "Stop",
        )

        try {
            // 2. Projection from the one-time consent result, 3. callback, 4. virtual display.
            val manager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            val p =
                manager.getMediaProjection(resultCode, consent)
                    ?: throw IllegalStateException("The system returned no MediaProjection")
            projection = p
            p.registerCallback(projectionCallback, main)

            val (w, h) = captureSize(intent.getIntExtra(EXTRA_MAX_DIMENSION, 1280))
            val fps = intent.getIntExtra(EXTRA_FPS, 30)
            val enc = ScreenEncoder(w, h, fps, intent.getIntExtra(EXTRA_BITRATE, 4_000_000))
            enc.start()
            encoder = enc

            display =
                p.createVirtualDisplay(
                    "cometcast-capture",
                    w,
                    h,
                    densityDpi(),
                    DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                    enc.inputSurface,
                    null,
                    main,
                )
            instance = this
        } catch (e: Exception) {
            // SecurityException (consent invalid or reused), MediaCodec errors, ...
            CaptureEvents.emitError("Could not start screen capture: ${e.message}")
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        instance = null
        display?.release()
        display = null
        encoder?.stop()
        encoder = null
        projection?.let {
            it.unregisterCallback(projectionCallback)
            it.stop()
        }
        projection = null
        CaptureEvents.emitStopped("service_stopped")
        super.onDestroy()
    }

    /** Asks the encoder for a key frame (for example after a receiver joined). */
    fun requestKeyFrame() {
        encoder?.requestKeyFrame()
    }

    private fun startAsForeground(
        title: String,
        text: String,
        stopLabel: String,
    ) {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= 26) {
            // The channel name is a technical label; the visible texts come from Dart (ARB).
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Screen capture", NotificationManager.IMPORTANCE_LOW),
            )
        }
        val stopIntent =
            PendingIntent.getService(
                this,
                0,
                Intent(this, ScreenCaptureService::class.java).setAction(ACTION_STOP),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        val builder =
            if (Build.VERSION.SDK_INT >= 26) {
                Notification.Builder(this, CHANNEL_ID)
            } else {
                @Suppress("DEPRECATION")
                Notification.Builder(this)
            }
        val notification =
            builder
                .setContentTitle(title)
                .setContentText(text)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setOngoing(true)
                .addAction(
                    Notification.Action.Builder(null, stopLabel, stopIntent).build(),
                ).build()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    @Suppress("DEPRECATION")
    private fun displayMetrics(): DisplayMetrics {
        val metrics = DisplayMetrics()
        (getSystemService(Context.WINDOW_SERVICE) as WindowManager).defaultDisplay.getRealMetrics(metrics)
        return metrics
    }

    private fun densityDpi(): Int = displayMetrics().densityDpi

    /** Scales the screen so its longer side is at most [maxDimension]; both sides even (H.264). */
    private fun captureSize(maxDimension: Int): Pair<Int, Int> {
        val m = displayMetrics()
        val longSide = maxOf(m.widthPixels, m.heightPixels)
        val scale = if (longSide > maxDimension) maxDimension.toFloat() / longSide else 1f
        val w = (m.widthPixels * scale).toInt() and 1.inv()
        val h = (m.heightPixels * scale).toInt() and 1.inv()
        return Pair(maxOf(w, 2), maxOf(h, 2))
    }

    companion object {
        const val ACTION_STOP = "de.cometcast.cometcast.capture.STOP"
        const val EXTRA_RESULT_CODE = "resultCode"
        const val EXTRA_CONSENT_DATA = "consentData"
        const val EXTRA_MAX_DIMENSION = "maxDimension"
        const val EXTRA_FPS = "fps"
        const val EXTRA_BITRATE = "bitrate"
        const val EXTRA_TITLE = "title"
        const val EXTRA_TEXT = "text"
        const val EXTRA_STOP_LABEL = "stopLabel"
        private const val CHANNEL_ID = "cometcast_capture"
        private const val NOTIFICATION_ID = 4711

        /** The running service, or null. Used to reach [requestKeyFrame]. */
        @Volatile
        var instance: ScreenCaptureService? = null
            private set
    }
}
