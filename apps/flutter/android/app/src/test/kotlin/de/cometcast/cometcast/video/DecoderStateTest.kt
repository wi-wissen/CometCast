// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.video

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DecoderStateTest {
    private var now = 0L
    private fun state(
        interval: Long = 1000,
        maxRequests: Int = 3,
        maxOversize: Int = 3,
    ) = DecoderState({ now }, interval, maxRequests, maxOversize)

    @Test
    fun deltaFramesAreRefusedUntilAKeyFrame() {
        val s = state()
        assertFalse(s.admit(false))
        assertTrue(s.admit(true))
        assertTrue(s.admit(false))
        s.requireKeyFrame()
        assertFalse(s.admit(false))
    }

    @Test
    fun keyFrameIsRequestedAgainWithBackoffUntilItArrives() {
        val s = state()
        assertTrue(s.keyRequestDue())
        assertFalse(s.keyRequestDue())
        now = 999
        assertFalse(s.keyRequestDue())
        now = 1000
        assertTrue(s.keyRequestDue())
        // A key frame ends the requests.
        assertTrue(s.admit(true))
        now = 5000
        assertFalse(s.keyRequestDue())
    }

    @Test
    fun requestsStopAfterTheLimitAndTheDecoderGivesUp() {
        val s = state(maxRequests = 3)
        var sent = 0
        repeat(10) {
            if (s.keyRequestDue()) sent++
            assertFalse(s.exhausted() && sent < 3)
            now += 1000
        }
        assertEquals(3, sent)
        assertTrue(s.exhausted())
    }

    @Test
    fun noRequestsWhileTheSurfaceIsGone() {
        val s = state()
        s.surfaceDestroyed()
        assertFalse(s.keyRequestDue())
        s.surfaceAvailable()
        assertTrue(s.keyRequestDue())
    }

    @Test
    fun renderingStopsWhenTeardownBeginsAndOnlyReturnsWithANewSurface() {
        val s = state()
        s.reset()
        assertTrue(s.mayRender())
        s.beginTeardown()
        assertFalse(s.mayRender())
        s.surfaceDestroyed()
        assertFalse(s.mayRender())
        s.surfaceAvailable()
        assertTrue(s.mayRender())
        assertTrue(s.needsKeyFrame())
        s.surfaceDestroyed()
        assertFalse(s.mayRender())
    }

    @Test
    fun oversizeUnitsGiveUpInsteadOfAKeyFrameStorm() {
        val s = state(maxOversize = 3)
        assertEquals(DecoderState.Overflow.NEED_KEY, s.oversize())
        assertEquals(DecoderState.Overflow.NEED_KEY, s.oversize())
        assertEquals(DecoderState.Overflow.GIVE_UP, s.oversize())
        // A good key frame in between starts the count again.
        val t = state(maxOversize = 2)
        t.oversize()
        t.admit(true)
        assertEquals(DecoderState.Overflow.NEED_KEY, t.oversize())
    }

    @Test
    fun timestampsAreUnwrappedTo64Bit() {
        val u = PtsUnwrapper(1000)
        assertEquals(100, u.unwrap(100))
        assertEquals(900, u.unwrap(900))
        assertEquals(1050, u.unwrap(50)) // wrapped
        assertEquals(1900, u.unwrap(900))
        // A small step back (reordering) is not a wrap.
        assertEquals(1890, u.unwrap(890))
        assertEquals(2010, u.unwrap(10)) // second wrap
        assertEquals(-1, u.unwrap(-1))
    }

    @Test
    fun theRealWrapIsTheRtpClock() {
        val u = PtsUnwrapper()
        val last = PtsUnwrapper.WRAP_US - 1
        assertEquals(last, u.unwrap(last))
        assertEquals(PtsUnwrapper.WRAP_US + 5, u.unwrap(5))
    }

    private data class U(val id: Int, val key: Boolean)

    @Test
    fun fullQueueDropsTheOldestDeltaFrameFirst() {
        val q = UnitQueue<U>(3) { it.key }
        assertFalse(q.offer(U(1, true)))
        assertFalse(q.offer(U(2, false)))
        assertFalse(q.offer(U(3, false)))
        assertTrue(q.offer(U(4, false))) // 2 goes, the key frame 1 stays
        assertEquals(listOf(1, 3, 4), generateSequence { q.poll(0) }.map { it.id }.toList())
        assertNull(q.poll(0))
    }

    @Test
    fun fullQueueOfKeyFramesDropsTheOldest() {
        val q = UnitQueue<U>(2) { it.key }
        q.offer(U(1, true))
        q.offer(U(2, true))
        assertTrue(q.offer(U(3, true)))
        assertEquals(listOf(2, 3), generateSequence { q.poll(0) }.map { it.id }.toList())
    }
}
