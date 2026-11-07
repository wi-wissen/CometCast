// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

/**
 * Pure size arithmetic of the screen capture (no Android classes, so it is covered by a plain
 * JVM unit test, see `app/src/test`).
 */
object CaptureGeometry {
    /**
     * Scales a [srcW] x [srcH] screen so that its longer side is at most [maxDimension] (never
     * enlarged) and rounds both sides down to a multiple of [alignW] / [alignH] (H.264 needs
     * even sizes; many hardware encoders want 16 or more). Never returns less than the alignment.
     */
    fun fit(
        srcW: Int,
        srcH: Int,
        maxDimension: Int,
        alignW: Int = 2,
        alignH: Int = 2,
    ): Pair<Int, Int> {
        require(srcW > 0 && srcH > 0) { "screen size must be positive" }
        val aw = maxOf(alignW, 1)
        val ah = maxOf(alignH, 1)
        val longSide = maxOf(srcW, srcH)
        val limit = maxOf(maxDimension, 1)
        val scale = if (longSide > limit) limit.toDouble() / longSide else 1.0
        val w = (srcW * scale).toInt()
        val h = (srcH * scale).toInt()
        return Pair(maxOf(w - w % aw, aw), maxOf(h - h % ah, ah))
    }

    /** True if a capture of [oldW] x [oldH] has to be rebuilt for [newW] x [newH]. */
    fun needsResize(
        oldW: Int,
        oldH: Int,
        newW: Int,
        newH: Int,
    ): Boolean = oldW != newW || oldH != newH
}
