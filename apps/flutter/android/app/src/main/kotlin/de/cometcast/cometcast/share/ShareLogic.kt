// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.share

import java.io.InputStream
import java.io.OutputStream

/**
 * The pure rules of the Android share target (native-apps.md 21.2, spec 15.3): which item kinds
 * exist, their size limits, the first link in shared text, a safe file name, and a copy that
 * refuses to go beyond a limit. No Android classes, so it is unit tested on the JVM.
 */
object ShareLogic {
    const val MAX_IMAGE_BYTES = 10L * 1024 * 1024
    const val MAX_PDF_BYTES = 64L * 1024 * 1024
    const val MAX_OFFICE_BYTES = 50L * 1024 * 1024

    /** Longest link handed to Dart (the core's link rules are stricter and decide). */
    const val MAX_LINK_LENGTH = 2048

    private val officeMimes =
        setOf(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
    private val imageMimes = setOf("image/png", "image/jpeg", "image/webp", "image/gif")

    /** `image`, `pdf` or `office` for a MIME type of the manifest's filter, else null. */
    fun kindFor(mime: String?): String? {
        val m = mime?.lowercase()?.substringBefore(';')?.trim() ?: return null
        return when {
            m in imageMimes -> "image"
            m == "application/pdf" -> "pdf"
            m in officeMimes -> "office"
            else -> null
        }
    }

    fun limitFor(kind: String): Long =
        when (kind) {
            "image" -> MAX_IMAGE_BYTES
            "pdf" -> MAX_PDF_BYTES
            else -> MAX_OFFICE_BYTES
        }

    private val linkPattern = Regex("""https?://[^\s<>"']+""", RegexOption.IGNORE_CASE)

    /** The first `http://` or `https://` address of the text, or null ("no link"). */
    fun firstLink(text: String?): String? {
        if (text == null) return null
        val raw = linkPattern.find(text)?.value ?: return null
        // A sentence ends after a link: ". , ; : ! ? )" are not part of it.
        val link = raw.trimEnd('.', ',', ';', ':', '!', '?', ')', ']')
        return if (link.length in 8..MAX_LINK_LENGTH) link else null
    }

    /** A file name without folders or control characters, never empty. */
    fun safeName(name: String?): String {
        val last = (name ?: "").substringAfterLast('/').substringAfterLast('\\')
        val cleaned = last.filter { !it.isISOControl() }.trim().trimStart('.')
        return cleaned.take(120).ifEmpty { "shared" }
    }

    /**
     * Copies [input] to [output] but stops as soon as more than [limit] bytes came.
     * Returns the number of bytes copied, or -1 when the limit was exceeded (the caller deletes
     * the partial copy).
     */
    fun copyLimited(
        input: InputStream,
        output: OutputStream,
        limit: Long,
    ): Long {
        val buf = ByteArray(64 * 1024)
        var total = 0L
        while (true) {
            val n = input.read(buf)
            if (n < 0) return total
            total += n
            if (total > limit) return -1
            output.write(buf, 0, n)
        }
    }
}
