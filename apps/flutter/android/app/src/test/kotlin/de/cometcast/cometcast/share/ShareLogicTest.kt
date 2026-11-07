// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.share

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream

class ShareLogicTest {
    @Test
    fun kindsAndLimits() {
        assertEquals("image", ShareLogic.kindFor("image/PNG"))
        assertEquals("image", ShareLogic.kindFor("image/gif; charset=x"))
        assertEquals("pdf", ShareLogic.kindFor("application/pdf"))
        assertEquals(
            "office",
            ShareLogic.kindFor("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        )
        assertNull(ShareLogic.kindFor("application/zip"))
        assertNull(ShareLogic.kindFor("video/mp4"))
        assertNull(ShareLogic.kindFor("image/svg+xml"))
        assertNull(ShareLogic.kindFor(null))
        assertEquals(10L * 1024 * 1024, ShareLogic.limitFor("image"))
        assertEquals(64L * 1024 * 1024, ShareLogic.limitFor("pdf"))
        assertEquals(50L * 1024 * 1024, ShareLogic.limitFor("office"))
    }

    @Test
    fun theFirstLinkOfSharedText() {
        assertEquals("https://example.org/a?b=1", ShareLogic.firstLink("Look: https://example.org/a?b=1."))
        assertEquals("http://x.de/p", ShareLogic.firstLink("a http://x.de/p, and https://y.de"))
        // Closing brackets at the end belong to the sentence (the core validates the rest).
        assertEquals("https://example.org/wiki/A_(b", ShareLogic.firstLink("(https://example.org/wiki/A_(b))"))
        assertNull(ShareLogic.firstLink("no address here, www.example.org"))
        assertNull(ShareLogic.firstLink("ftp://example.org/x"))
        assertNull(ShareLogic.firstLink(null))
        assertNull(ShareLogic.firstLink("https://" + "a".repeat(ShareLogic.MAX_LINK_LENGTH)))
    }

    @Test
    fun fileNamesAreSafe() {
        assertEquals("a.png", ShareLogic.safeName("/storage/emulated/0/a.png"))
        assertEquals("evil.pdf", ShareLogic.safeName("..\\..\\evil.pdf"))
        assertEquals("x.pdf", ShareLogic.safeName("\u0000x.pdf\n"))
        assertEquals("shared", ShareLogic.safeName(""))
        assertEquals("shared", ShareLogic.safeName(null))
        assertEquals("shared", ShareLogic.safeName("..."))
        assertEquals(120, ShareLogic.safeName("a".repeat(500)).length)
    }

    @Test
    fun theCopyRefusesToGoBeyondTheLimit() {
        val data = ByteArray(1000) { it.toByte() }
        val ok = ByteArrayOutputStream()
        assertEquals(1000L, ShareLogic.copyLimited(ByteArrayInputStream(data), ok, 1000))
        assertEquals(1000, ok.size())
        val big = ByteArrayOutputStream()
        assertEquals(-1L, ShareLogic.copyLimited(ByteArrayInputStream(data), big, 999))
        // It stopped writing at the limit: never more than limit + one buffer reached the output.
        assert(big.size() <= 999 + 64 * 1024)
    }
}
