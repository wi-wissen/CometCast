// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast

import android.content.Intent
import de.cometcast.cometcast.capture.CaptureBridge
import de.cometcast.cometcast.capture.ScreenCaptureService
import de.cometcast.cometcast.platform.PlatformBridge
import de.cometcast.cometcast.video.VideoBridge
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    private var captureBridge: CaptureBridge? = null
    private var videoBridge: VideoBridge? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // None of these bridges was run on a device (docs/android.md).
        captureBridge = CaptureBridge(this, flutterEngine)
        videoBridge = VideoBridge(flutterEngine)
        PlatformBridge(this, flutterEngine)
    }

    override fun cleanUpFlutterEngine(flutterEngine: FlutterEngine) {
        videoBridge?.release()
        super.cleanUpFlutterEngine(flutterEngine)
    }

    override fun onDestroy() {
        // The engine (and with it the Dart side that consumes the encoded frames) dies with the
        // activity. A capture service that kept running would encode into nothing, with a
        // notification that says "sharing": stop it. A configuration change is handled by the
        // manifest's configChanges and does not get here.
        if (!isChangingConfigurations) {
            stopService(Intent(this, ScreenCaptureService::class.java))
        }
        super.onDestroy()
    }

    @Deprecated("Framework API; FlutterActivity still dispatches results through it")
    override fun onActivityResult(
        requestCode: Int,
        resultCode: Int,
        data: Intent?,
    ) {
        if (captureBridge?.onActivityResult(requestCode, resultCode, data) == true) return
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        captureBridge?.onRequestPermissionsResult(requestCode)
    }
}
