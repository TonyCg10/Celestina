package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The wire ids the screens depend on, pinned to the protocol document. */
class DesktopSignalTest {
    @Test
    fun findAndBatteryEnvelopesMapToTheirSignals() {
        assertEquals(DesktopSignal.Ring, DesktopSignal.of(LinkEvent(4, 1, "find: ring")))
        assertEquals(DesktopSignal.StopRinging, DesktopSignal.of(LinkEvent(4, 2, "find: stop")))
        assertEquals(DesktopSignal.BatteryRequested, DesktopSignal.of(LinkEvent(1, 2, "battery: requested")))
        assertEquals(DesktopSignal.ClipboardText("hello"), DesktopSignal.of(LinkEvent(2, 1, "clipboard: 5 bytes", "hello")))
        assertEquals(DesktopSignal.Other(2, 1), DesktopSignal.of(LinkEvent(2, 1, "clipboard: undecodable")))
        assertEquals(DesktopSignal.ClipboardRequested, DesktopSignal.of(LinkEvent(2, 2, "clipboard: requested")))
        assertEquals(DesktopSignal.Other(3, 1), DesktopSignal.of(LinkEvent(3, 1, "notification")))
    }

    @Test
    fun onlyMagnetitaPairLinksAreAccepted() {
        assertTrue(PairLink.accepts("magnetita://pair?v=1&id=a&fp=b&secret=c&addr=10.0.0.1:1760"))
        assertFalse(PairLink.accepts("https://example.org"))
        assertFalse(PairLink.accepts(null))
        assertFalse(PairLink.accepts("magnetita://pair?" + "x".repeat(600)))
    }
}
