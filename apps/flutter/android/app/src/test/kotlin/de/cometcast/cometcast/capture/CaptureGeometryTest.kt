// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CaptureGeometryTest {
    @Test
    fun smallScreenIsNotEnlarged() {
        assertEquals(Pair(800, 480), CaptureGeometry.fit(800, 480, 1280))
    }

    @Test
    fun longSideIsLimitedAndAspectKept() {
        assertEquals(Pair(1280, 576), CaptureGeometry.fit(2560, 1152, 1280))
        // Portrait phone: the long side is the height.
        assertEquals(Pair(590, 1280), CaptureGeometry.fit(1080, 2340, 1280))
    }

    @Test
    fun sizesAreEven() {
        val (w, h) = CaptureGeometry.fit(1081, 2341, 1280)
        assertEquals(0, w % 2)
        assertEquals(0, h % 2)
    }

    @Test
    fun alignmentRoundsDown() {
        assertEquals(Pair(1280, 720), CaptureGeometry.fit(1280, 720, 4000, 16, 16))
        assertEquals(Pair(1360, 768), CaptureGeometry.fit(1366, 768, 4000, 16, 16))
    }

    @Test
    fun neverBelowTheAlignment() {
        assertEquals(Pair(2, 2), CaptureGeometry.fit(3, 3, 1))
        assertEquals(Pair(16, 16), CaptureGeometry.fit(5, 5, 1, 16, 16))
    }

    @Test
    fun rotationNeedsAResize() {
        assertTrue(CaptureGeometry.needsResize(1280, 590, 590, 1280))
        assertFalse(CaptureGeometry.needsResize(1280, 590, 1280, 590))
    }

    @Test(expected = IllegalArgumentException::class)
    fun zeroSizeIsRefused() {
        CaptureGeometry.fit(0, 100, 1280)
    }
}
