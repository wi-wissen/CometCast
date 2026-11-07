// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.share

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileOutputStream
import java.security.SecureRandom
import java.util.concurrent.Callable
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.Future

/**
 * The share target of the main activity (native-apps.md 21.2, spec 15.3), MethodChannel
 * `cometcast/share`:
 * - `takeSharedItem` -> Map? : the item of the intent that opened or re-opened the activity, once,
 *   or null. `{kind: file|link|noLink|tooLarge, path, name, size, mime, url}`.
 *   `file`: the stream was copied to `cacheDir/share/<random>/<name>`; `link`: the first
 *   `http(s)://` address of `EXTRA_TEXT`; `noLink`: text without one (or a type that is not
 *   registered); `tooLarge`: more than the limit of its kind (10 MiB image, 64 MiB PDF, 50 MiB
 *   Office), found **while copying**, nothing is left on the disk.
 * - `discardSharedItem` (path): deletes that copy. Done when the presentation of the item ends;
 *   `cacheDir/share` is also emptied when the app starts.
 * - `returnToCaller`: Cancel and system Back on the share page (spec 15.14 N112). The activity moves
 *   to the back (`moveTaskToBack(true)`: the app's previous page and any join are kept, the
 *   person is back in the app that shared); only an activity that the share intent started cold
 *   is finished.
 * - Dart receives `shared` (no argument) when a new item arrives while the app runs
 *   ([onNewIntent]); it then calls `takeSharedItem`.
 *
 * The rules are in [ShareLogic]. UNCERTAIN: never run on a device (docs/android.md).
 */
class ShareBridge(
    private val activity: Activity,
    engine: FlutterEngine,
) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(engine.dartExecutor.binaryMessenger, CHANNEL)
    private val main = Handler(Looper.getMainLooper())
    private val worker: ExecutorService = Executors.newSingleThreadExecutor()
    private val root = File(activity.cacheDir, "share")
    private var pending: Future<Map<String, Any?>?>? = null

    /** The share intent started this activity (cold start): the only case `finish()` is allowed. */
    private var startedByShare = false

    init {
        // Copies of an earlier run are not needed any more.
        root.deleteRecursively()
        channel.setMethodCallHandler(this)
        startedByShare = accept(activity.intent)
    }

    /** A new item while the activity runs (launch mode singleTop). */
    fun onNewIntent(intent: Intent) {
        if (accept(intent)) channel.invokeMethod("shared", null)
    }

    fun release() {
        worker.shutdownNow()
    }

    override fun onMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
    ) {
        when (call.method) {
            "takeSharedItem" -> {
                val f = pending
                pending = null
                if (f == null) {
                    result.success(null)
                    return
                }
                // The copy may still run: wait for it off the main thread.
                worker.execute {
                    val item = try { f.get() } catch (e: Exception) { null }
                    main.post { result.success(item) }
                }
            }
            "returnToCaller" -> {
                returnToCaller()
                result.success(null)
            }
            "discardSharedItem" -> {
                discard(call.arguments as? String)
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** Back to the app that shared, without ending Comet Cast (N112). */
    private fun returnToCaller() {
        if (startedByShare) {
            activity.finish()
        } else {
            activity.moveTaskToBack(true)
        }
    }

    private fun discard(path: String?) {
        if (path == null) return
        val f = File(path)
        // Only what this bridge made: inside cacheDir/share, one folder per item.
        val dir = f.parentFile ?: return
        if (dir.parentFile?.canonicalPath == root.canonicalPath) dir.deleteRecursively()
    }

    /** Takes the item of an intent. True if it is a share intent of ours. */
    private fun accept(intent: Intent?): Boolean {
        if (intent?.action != Intent.ACTION_SEND) return false
        val stream: Uri? =
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(Intent.EXTRA_STREAM)
        val text = intent.getStringExtra(Intent.EXTRA_TEXT)
        val mime = intent.type
        // An older item that was never taken is replaced; its copy goes away.
        pending?.let { old -> worker.execute { (try { old.get() } catch (e: Exception) { null })?.get("path")?.let { discard(it as String) } } }
        pending =
            worker.submit(
                Callable {
                    when {
                        stream != null -> copyStream(stream, mime)
                        else -> linkItem(text)
                    }
                },
            )
        return true
    }

    private fun linkItem(text: String?): Map<String, Any?> {
        val url = ShareLogic.firstLink(text)
        return if (url == null) {
            mapOf("kind" to "noLink")
        } else {
            mapOf("kind" to "link", "url" to url)
        }
    }

    private fun copyStream(
        uri: Uri,
        intentMime: String?,
    ): Map<String, Any?> {
        val resolver = activity.contentResolver
        val mime = resolver.getType(uri)?.takeIf { ShareLogic.kindFor(it) != null } ?: intentMime
        val kind = ShareLogic.kindFor(mime) ?: return mapOf("kind" to "noLink")
        val name = ShareLogic.safeName(displayName(uri))
        val dir = File(root, randomName())
        dir.mkdirs()
        val file = File(dir, name)
        val limit = ShareLogic.limitFor(kind)
        try {
            val input = resolver.openInputStream(uri) ?: return fail(dir)
            val size =
                input.use { i -> FileOutputStream(file).use { o -> ShareLogic.copyLimited(i, o, limit) } }
            if (size < 0) {
                dir.deleteRecursively()
                return mapOf("kind" to "tooLarge")
            }
            return mapOf(
                "kind" to "file",
                "path" to file.absolutePath,
                "name" to name,
                "size" to size,
                "mime" to mime,
            )
        } catch (e: Exception) {
            return fail(dir)
        }
    }

    private fun fail(dir: File): Map<String, Any?> {
        dir.deleteRecursively()
        return mapOf("kind" to "noLink")
    }

    private fun displayName(uri: Uri): String? =
        try {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        } catch (e: Exception) {
            null
        } ?: uri.lastPathSegment

    private fun randomName(): String {
        val b = ByteArray(12)
        SecureRandom().nextBytes(b)
        return b.joinToString("") { "%02x".format(it) }
    }

    companion object {
        const val CHANNEL = "cometcast/share"
    }
}
