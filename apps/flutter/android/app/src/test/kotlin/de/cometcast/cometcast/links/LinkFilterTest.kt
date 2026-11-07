// SPDX-License-Identifier: GPL-3.0-or-later
package de.cometcast.cometcast.links

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LinkFilterTest {
    @Test
    fun anHttpsLinkIsHandedToDart() {
        assertEquals(
            "https://relay.cometcast.de/K7QM2W",
            LinkFilter.acceptText("https://relay.cometcast.de/K7QM2W"),
        )
        assertEquals(
            "https://relay.cometcast.de/K7QM2W?server=cast.example.org",
            LinkFilter.acceptText("https://relay.cometcast.de/K7QM2W?server=cast.example.org"),
        )
    }

    @Test
    fun theJoinAddressFormIsHandedToDart() {
        assertEquals(
            "https://cometcast.de/app/K7QM2W",
            LinkFilter.acceptText("https://cometcast.de/app/K7QM2W"),
        )
    }

    @Test
    fun everythingElseIsDropped() {
        assertNull(LinkFilter.acceptText(null))
        assertNull(LinkFilter.acceptText(""))
        assertNull(LinkFilter.acceptText("http://relay.cometcast.de/K7QM2W"))
        assertNull(LinkFilter.acceptText("cometcast://join/K7QM2W"))
        assertNull(LinkFilter.acceptText("javascript:alert(1)"))
        assertNull(LinkFilter.acceptText("https://x/" + "A".repeat(LinkFilter.MAX_LENGTH)))
    }
}
