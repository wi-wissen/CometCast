// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast

import android.content.Intent
import de.cometcast.cometcast.capture.CaptureBridge
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    private var captureBridge: CaptureBridge? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // Screen capture skeleton (not run on a device, see docs/android.md).
        captureBridge = CaptureBridge(this, flutterEngine)
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
