// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.scanner

import android.app.Activity
import android.content.pm.PackageManager
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * The system QR scanner for Dart (native-apps.md section 8), MethodChannel `de.cometcast/qr`:
 * - `isAvailable` -> Boolean: a camera exists and Google Play services are there.
 * - `scan` -> String?: opens the Google code scanner (`GmsBarcodeScanning`, QR codes only) and
 *   returns the text of the first code; null when the person cancelled or the scanner could not
 *   run. The scanner is a part of Google Play services: **the app asks for no camera permission**
 *   (the scanner runs in Google's process).
 *
 * The result is only text; Dart checks it (`parseLinkUrl`) and shows `app.scanWrong` for
 * anything that is not a link of ours. UNCERTAIN: never run on a device. The module is
 * downloaded by Play services on first use; without it `scan` answers null.
 */
class QrBridge(
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
            "isAvailable" -> result.success(available())
            "scan" -> scan(result)
            else -> result.notImplemented()
        }
    }

    private fun available(): Boolean =
        activity.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) &&
            GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(activity) ==
            ConnectionResult.SUCCESS

    private fun scan(result: MethodChannel.Result) {
        var answered = false

        fun answer(value: String?) {
            if (answered) return
            answered = true
            result.success(value)
        }
        try {
            val options =
                GmsBarcodeScannerOptions
                    .Builder()
                    .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                    .build()
            GmsBarcodeScanning
                .getClient(activity, options)
                .startScan()
                .addOnSuccessListener { answer(it.rawValue) }
                .addOnCanceledListener { answer(null) }
                .addOnFailureListener { answer(null) }
        } catch (e: Exception) {
            answer(null)
        }
    }

    companion object {
        const val CHANNEL = "de.cometcast/qr"
    }
}
