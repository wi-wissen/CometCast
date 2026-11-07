// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.net.wifi.WifiManager
import android.os.PowerManager
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
 * Since the M6 wiring the encoded frames reach the Rust sender through Dart (see
 * docs/android.md); this class only produces them.
 *
 * Size changes (rotation, Android 14 single-app resize): a `DisplayListener` and
 * `onCapturedContentResize` call [scheduleResize], which after a short debounce builds a new
 * encoder for the new size, points the existing VirtualDisplay at its surface (`resize` +
 * `setSurface`) and releases the old encoder. The new encoder emits a new config event and starts
 * with a key frame. UNCERTAIN: whether `resize` + `setSurface` on a MediaProjection display behave
 * like this on all devices; not run.
 *
 * Background: the foreground service keeps the process alive while the app is in the background;
 * a partial wake lock and a Wi-Fi lock keep the CPU and the network running while the screen is
 * off (the projection itself usually stops on lock, see [MediaProjection.Callback.onStop]).
 * UNCERTAIN: not run.
 */
class ScreenCaptureService : Service() {
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var encoder: ScreenEncoder? = null
    private val main = Handler(Looper.getMainLooper())
    private var captureW = 0
    private var captureH = 0
    private var maxDimension = 1280
    private var fps = 30
    private var bitrate = 4_000_000
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var displayListener: DisplayManager.DisplayListener? = null

    /** Size of the shared window in single-app mode (Android 14+), once the system reported it. */
    private var contentSize: Pair<Int, Int>? = null

    /** Size wanted by the last resize trigger; applied by [applyResize] after the debounce. */
    private var pendingSource: Pair<Int, Int>? = null
    private val resizeRunnable = Runnable { applyResize() }

    private val projectionCallback =
        object : MediaProjection.Callback() {
            override fun onStop() {
                // The user or the system ended the projection.
                CaptureEvents.emitStopped("projection_stopped")
                stopSelf()
            }

            // Android 14+. UNCERTAIN: not run; the callback may arrive with the size of the
            // captured window or of the display, whichever the platform reports.
            override fun onCapturedContentResize(
                width: Int,
                height: Int,
            ) {
                // Single-app sharing: the shared window changed its size. Follow its aspect ratio.
                if (width > 0 && height > 0) {
                    contentSize = Pair(width, height)
                    scheduleResize(Pair(width, height))
                }
            }

            // Not implemented: a hidden captured app is simply not drawn (frames repeat).
            override fun onCapturedContentVisibilityChanged(isVisible: Boolean) = Unit
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
        // Every start command that came from startForegroundService() must reach startForeground()
        // within about 5 s, even if we then ignore it; otherwise Android throws
        // ForegroundServiceDidNotStartInTimeException. So this happens first, on every path.
        val foreground =
            tryStartForeground(
                intent.getStringExtra(EXTRA_TITLE) ?: "Comet Cast",
                intent.getStringExtra(EXTRA_TEXT) ?: "",
                intent.getStringExtra(EXTRA_STOP_LABEL) ?: "Stop",
            )
        if (!foreground) {
            // Only report if no session runs; a running session keeps its own state.
            if (projection == null) {
                CaptureEvents.emitError("Could not start the capture notification")
                stopSelf()
            }
            return START_NOT_STICKY
        }
        // One session at a time: a second start leaves the running session untouched (the
        // notification above is the same one, so nothing visible changes).
        if (projection != null) return START_NOT_STICKY

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
        CaptureEvents.beginSession()
        contentSize = null
        maxDimension = intent.getIntExtra(EXTRA_MAX_DIMENSION, 1280)
        fps = intent.getIntExtra(EXTRA_FPS, 30)
        bitrate = intent.getIntExtra(EXTRA_BITRATE, 4_000_000)

        try {
            // 2. Projection from the one-time consent result, 3. callback, 4. virtual display.
            val manager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
            val p =
                manager.getMediaProjection(resultCode, consent)
                    ?: throw IllegalStateException("The system returned no MediaProjection")
            projection = p
            p.registerCallback(projectionCallback, main)

            val (w, h) = captureSize(screenSize())
            val enc = ScreenEncoder(w, h, fps, bitrate)
            enc.start() // releases its own resources if it throws
            encoder = enc
            captureW = enc.width // the encoder may have rounded the size to its alignment
            captureH = enc.height

            display =
                p.createVirtualDisplay(
                    "cometcast-capture",
                    captureW,
                    captureH,
                    densityDpi(),
                    DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                    enc.inputSurface,
                    null,
                    main,
                )
            instance = this
            acquireLocks()
            watchDisplay()
        } catch (e: Exception) {
            // SecurityException (consent invalid or reused), MediaCodec errors, ...
            // onDestroy (after stopSelf) releases whatever was already created.
            CaptureEvents.emitError("Could not start screen capture: ${e.message}")
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // The user swiped the app away: the Dart side that consumes the frames is gone.
        stopSelf()
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        instance = null
        main.removeCallbacks(resizeRunnable)
        unwatchDisplay()
        releaseLocks()
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

    // --- size changes ----------------------------------------------------------------------

    private fun watchDisplay() {
        val dm = getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
        val l =
            object : DisplayManager.DisplayListener {
                override fun onDisplayAdded(displayId: Int) = Unit

                override fun onDisplayRemoved(displayId: Int) = Unit

                override fun onDisplayChanged(displayId: Int) {
                    if (displayId == android.view.Display.DEFAULT_DISPLAY) scheduleResize(contentSize ?: screenSize())
                }
            }
        displayListener = l
        dm.registerDisplayListener(l, main)
    }

    private fun unwatchDisplay() {
        val l = displayListener ?: return
        displayListener = null
        (getSystemService(Context.DISPLAY_SERVICE) as DisplayManager).unregisterDisplayListener(l)
    }

    /** Remembers the wanted source size and applies it after 300 ms of quiet (rotations fire often). */
    private fun scheduleResize(source: Pair<Int, Int>) {
        pendingSource = source
        main.removeCallbacks(resizeRunnable)
        main.postDelayed(resizeRunnable, 300)
    }

    private fun applyResize() {
        val source = pendingSource ?: return
        val d = display ?: return
        val old = encoder ?: return
        val (w, h) = captureSize(source)
        // The encoder may round the size (alignment), so this only filters the obvious case; the
        // exact comparison happens below, with the size the new encoder really got.
        if (!CaptureGeometry.needsResize(captureW, captureH, w, h)) return
        try {
            val next = ScreenEncoder(w, h, fps, bitrate)
            next.start()
            if (!CaptureGeometry.needsResize(captureW, captureH, next.width, next.height)) {
                // Rounded to the same size as now: nothing changes.
                next.stop()
                return
            }
            d.resize(next.width, next.height, densityDpi())
            d.surface = next.inputSurface
            encoder = next
            captureW = next.width
            captureH = next.height
            old.stop()
        } catch (e: Exception) {
            // The old encoder keeps running at the old size; report and carry on.
            CaptureEvents.emitError("Could not follow the size change: ${e.message}")
        }
    }

    // --- wake locks --------------------------------------------------------------------------

    @Suppress("DEPRECATION")
    private fun acquireLocks() {
        try {
            val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
            wakeLock =
                pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "cometcast:capture").also {
                    it.setReferenceCounted(false)
                    // A safety timeout: a forgotten lock must not drain the battery for ever.
                    it.acquire(WAKE_LOCK_MS)
                }
            val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            val mode =
                if (Build.VERSION.SDK_INT >= 29) {
                    WifiManager.WIFI_MODE_FULL_LOW_LATENCY
                } else {
                    WifiManager.WIFI_MODE_FULL_HIGH_PERF
                }
            wifiLock =
                wm.createWifiLock(mode, "cometcast:capture").also {
                    it.setReferenceCounted(false)
                    it.acquire()
                }
        } catch (e: Exception) {
            // SecurityException (permission) or no Wi-Fi service (Ethernet-only TV): not fatal.
        }
    }

    private fun releaseLocks() {
        try {
            wakeLock?.takeIf { it.isHeld }?.release()
            wifiLock?.takeIf { it.isHeld }?.release()
        } catch (e: Exception) {
            // Already released.
        }
        wakeLock = null
        wifiLock = null
    }

    /** Guarded [startAsForeground]: SecurityException and friends must not crash the process. */
    private fun tryStartForeground(
        title: String,
        text: String,
        stopLabel: String,
    ): Boolean =
        try {
            startAsForeground(title, text, stopLabel)
            true
        } catch (e: Exception) {
            // SecurityException (missing permission or type), ForegroundServiceStartNotAllowed...
            false
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
                .setSmallIcon(R.drawable.ic_stat_capture)
                .setOngoing(true)
                .setCategory(Notification.CATEGORY_SERVICE)
                .setContentIntent(openAppIntent())
                .addAction(
                    Notification.Action.Builder(null, stopLabel, stopIntent).build(),
                ).also {
                    // Android 12+ delays the notification of a foreground service by up to 10 s
                    // unless asked otherwise. UNCERTAIN: not run.
                    if (Build.VERSION.SDK_INT >= 31) {
                        it.setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
                    }
                }.build()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    /** Tapping the notification brings the app back. */
    private fun openAppIntent(): PendingIntent? {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return null
        launch.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)
        return PendingIntent.getActivity(this, 1, launch, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    @Suppress("DEPRECATION")
    private fun displayMetrics(): DisplayMetrics {
        val metrics = DisplayMetrics()
        (getSystemService(Context.WINDOW_SERVICE) as WindowManager).defaultDisplay.getRealMetrics(metrics)
        return metrics
    }

    private fun densityDpi(): Int = displayMetrics().densityDpi

    /** Current size of the default display in pixels (what the projection would capture). */
    private fun screenSize(): Pair<Int, Int> {
        val m = displayMetrics()
        return Pair(m.widthPixels, m.heightPixels)
    }

    /** [source] scaled to [maxDimension] and made even (H.264); the encoder may round further. */
    private fun captureSize(source: Pair<Int, Int>): Pair<Int, Int> =
        CaptureGeometry.fit(source.first, source.second, maxDimension)

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
        private const val WAKE_LOCK_MS = 4L * 60 * 60 * 1000

        /** The running service, or null. Used to reach [requestKeyFrame]. */
        @Volatile
        var instance: ScreenCaptureService? = null
            private set
    }
}
