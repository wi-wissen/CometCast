// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.platform

import android.app.Activity
import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.view.WindowManager
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * Small platform facts for Dart, MethodChannel `de.cometcast/platform`:
 * - `isTelevision` -> Boolean: Android TV / Google TV (UI mode television or the leanback
 *   feature). Used to switch the app into receiver-only, remote-friendly mode.
 * - `nativeLibraryDir` -> String: where the package manager put the native libraries; the PDF and
 *   Office workers are shipped there as `libcc_docs_worker.so` / `libcc_office_worker.so`.
 * - `setKeepScreenOn` (Boolean): sets or clears `FLAG_KEEP_SCREEN_ON` on the window.
 * - `deviceInfo` -> map (sdkInt, manufacturer, model, abis, tv): written to the log by Dart so a
 *   device test can say what it ran on.
 *
 * UNCERTAIN: never run on a device.
 */
class PlatformBridge(
    private val activity: Activity,
    engine: FlutterEngine,
) : MethodChannel.MethodCallHandler {
    init {
        MethodChannel(engine.dartExecutor.binaryMessenger, CHANNEL).setMethodCallHandler(this)
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "isTelevision" -> result.success(isTelevision(activity))
            "nativeLibraryDir" -> result.success(activity.applicationInfo.nativeLibraryDir)
            "setKeepScreenOn" -> {
                val on = call.arguments as? Boolean ?: false
                activity.runOnUiThread {
                    if (on) {
                        activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    } else {
                        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    }
                }
                result.success(null)
            }
            "deviceInfo" ->
                result.success(
                    mapOf(
                        "sdkInt" to Build.VERSION.SDK_INT,
                        "release" to Build.VERSION.RELEASE,
                        "manufacturer" to Build.MANUFACTURER,
                        "model" to Build.MODEL,
                        "abis" to Build.SUPPORTED_ABIS.toList(),
                        "tv" to isTelevision(activity),
                    ),
                )
            else -> result.notImplemented()
        }
    }

    companion object {
        const val CHANNEL = "de.cometcast/platform"

        fun isTelevision(context: Context): Boolean {
            val ui = context.getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager
            return ui?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION ||
                context.packageManager.hasSystemFeature("android.software.leanback")
        }
    }
}
