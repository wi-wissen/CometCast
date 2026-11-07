// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.links

import android.app.Activity
import android.content.Intent
import com.android.installreferrer.api.InstallReferrerClient
import com.android.installreferrer.api.InstallReferrerStateListener
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * App Links and the Google Play install referrer for Dart (native-apps.md 20.4), MethodChannel
 * `de.cometcast/links`:
 * - `initialLink` -> String?: the https link that started the activity (an App Link of the
 *   built-in relay's host, see the intent filter in AndroidManifest.xml), or null.
 * - `installReferrer` -> String?: the `referrer` string of the Google Play install referrer, or
 *   null (not installed from Google Play, no referrer, service not available).
 * - Dart receives `link` (String) when a link arrives while the app runs ([onNewIntent]).
 *
 * The pure URL rules are in [LinkFilter]; Dart decides what a link means (code, server, trust).
 * UNCERTAIN: never run on a device. The install referrer is an assumption of the design (spec
 * 12): without it Android behaves like iOS.
 */
class LinkBridge(
    private val activity: Activity,
    engine: FlutterEngine,
) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(engine.dartExecutor.binaryMessenger, CHANNEL)
    private var launchLink: String? = LinkFilter.accept(activity.intent)

    init {
        channel.setMethodCallHandler(this)
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "initialLink" -> {
                // Handed out once: a later call (a new Dart isolate after a configuration
                // change) must not replay a link that was handled.
                val l = launchLink
                launchLink = null
                result.success(l)
            }
            "installReferrer" -> readReferrer(result)
            else -> result.notImplemented()
        }
    }

    /** A new link while the activity runs (launch mode singleTop). */
    fun onNewIntent(intent: Intent) {
        LinkFilter.accept(intent)?.let { channel.invokeMethod("link", it) }
    }

    private fun readReferrer(result: MethodChannel.Result) {
        val client =
            try {
                InstallReferrerClient.newBuilder(activity).build()
            } catch (e: Exception) {
                result.success(null)
                return
            }
        var answered = false

        fun answer(value: String?) {
            if (answered) return
            answered = true
            try {
                client.endConnection()
            } catch (_: Exception) {
            }
            result.success(value)
        }
        try {
            client.startConnection(
                object : InstallReferrerStateListener {
                    override fun onInstallReferrerSetupFinished(responseCode: Int) {
                        if (responseCode != InstallReferrerClient.InstallReferrerResponse.OK) {
                            answer(null)
                            return
                        }
                        answer(
                            try {
                                client.installReferrer.installReferrer
                            } catch (_: Exception) {
                                null
                            },
                        )
                    }

                    override fun onInstallReferrerServiceDisconnected() {
                        answer(null)
                    }
                },
            )
        } catch (_: Exception) {
            answer(null)
        }
    }

    companion object {
        const val CHANNEL = "de.cometcast/links"
    }
}

/** The pure part: which intents carry a link for the app. */
object LinkFilter {
    /** Longest link handed to Dart (the protocol's codes are 6 characters). */
    const val MAX_LENGTH = 2048

    /** The https link of a VIEW intent, or null for anything else. */
    fun accept(intent: Intent?): String? {
        if (intent?.action != Intent.ACTION_VIEW) return null
        return acceptText(intent.dataString)
    }

    fun acceptText(text: String?): String? {
        if (text == null || text.length > MAX_LENGTH) return null
        return if (text.startsWith("https://")) text else null
    }
}
