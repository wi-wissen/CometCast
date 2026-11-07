// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.capture

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GenerationGateTest {
    @Test
    fun onlyTheActiveGenerationDelivers() {
        val gate = GenerationGate()
        val first = gate.allocate()
        gate.activate(first)
        assertTrue(gate.accepts(first))
        // The new encoder starts: it delivers nothing yet.
        val second = gate.allocate()
        assertFalse(gate.accepts(second))
        // The swap: the old encoder's frames in flight are dropped from now on.
        gate.activate(second)
        assertFalse(gate.accepts(first))
        assertTrue(gate.accepts(second))
    }

    @Test
    fun aFailedSwapCanReturnToTheOldGeneration() {
        val gate = GenerationGate()
        val first = gate.allocate()
        gate.activate(first)
        val second = gate.allocate()
        gate.activate(second)
        gate.activate(first)
        assertTrue(gate.accepts(first))
        assertFalse(gate.accepts(second))
    }

    @Test
    fun resetStartsOver() {
        val gate = GenerationGate()
        gate.activate(gate.allocate())
        gate.reset()
        assertEquals(1, gate.allocate())
    }

    @Test
    fun configWaitsForTheFirstKeyFrameAndIsSentOnce() {
        val hold = ConfigHold<String>()
        assertNull(hold.takeForKeyFrame())
        hold.hold("sps-new")
        assertEquals("sps-new", hold.takeForKeyFrame())
        assertNull(hold.takeForKeyFrame())
    }
}
