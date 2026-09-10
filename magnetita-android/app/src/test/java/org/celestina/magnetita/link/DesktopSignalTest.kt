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
        assertEquals(DesktopSignal.ShareOffered(4, "a.jpg", 12), DesktopSignal.of(LinkEvent(5, 1, "share: offer", text = "a.jpg", transfer = 4, size = 12)))
        assertEquals(DesktopSignal.ShareAccepted(4, 7), DesktopSignal.of(LinkEvent(5, 2, "share: accepted", transfer = 4, offset = 7)))
        assertEquals(DesktopSignal.ShareEnded(4, false), DesktopSignal.of(LinkEvent(5, 3, "share: rejected", transfer = 4, complete = false)))
        assertEquals(DesktopSignal.ShareEnded(4, true), DesktopSignal.of(LinkEvent(5, 4, "share: done", transfer = 4, complete = true)))
        assertEquals(DesktopSignal.FileReceived(4, "/x/a.jpg", true), DesktopSignal.of(LinkEvent(5, 100, "share: received", transfer = 4, complete = true, path = "/x/a.jpg")))
        assertEquals(DesktopSignal.ShareText("https://example.org"), DesktopSignal.of(LinkEvent(5, 5, "share: text", text = "https://example.org")))
        val state = MediaState("mpv", "T", "A", "", true, 1, 2, true, true, false, 30)
        assertEquals(DesktopSignal.DesktopMedia(state), DesktopSignal.of(LinkEvent(6, 1, "media: state", media = state)))
        assertEquals(DesktopSignal.MediaControl(MediaCommand("YT", 2, null, null)), DesktopSignal.of(LinkEvent(6, 2, "media: command", mediaCommand = MediaCommand("YT", 2, null, null))))
        assertEquals(DesktopSignal.MediaRequested, DesktopSignal.of(LinkEvent(6, 3, "media: requested")))
    }

    @Test
    fun onlyMagnetitaPairLinksAreAccepted() {
        assertTrue(PairLink.accepts("magnetita://pair?v=1&id=a&fp=b&secret=c&addr=10.0.0.1:1760"))
        assertFalse(PairLink.accepts("https://example.org"))
        assertFalse(PairLink.accepts(null))
        assertFalse(PairLink.accepts("magnetita://pair?" + "x".repeat(600)))
    }
}
