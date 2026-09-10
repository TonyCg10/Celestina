package org.celestina.magnetita.mirror

import org.celestina.magnetita.link.DesktopSignal
import org.celestina.magnetita.link.LinkEvent
import org.celestina.magnetita.link.MirrorOptions
import org.celestina.magnetita.link.MirrorTouch
import org.junit.Assert.assertEquals
import org.junit.Test

class MirrorGeometryTest {
    @Test
    fun the_picture_keeps_the_shape_under_the_cap_with_even_sides() {
        assertEquals(664 to 1440, MirrorGeometry.fit(1080, 2340, 1440))
        assertEquals(1080 to 2340, MirrorGeometry.fit(1080, 2340, 0))
        assertEquals(1080 to 2340, MirrorGeometry.fit(1080, 2340, 4000))
        assertEquals(1920 to 1080, MirrorGeometry.fit(3840, 2160, 1920))
    }

    @Test
    fun a_touch_on_the_picture_lands_on_the_screen() {
        assertEquals(1079f to 2339f, MirrorGeometry.toScreen(664, 1440, 664 to 1440, 1080 to 2340))
        assertEquals(0f to 0f, MirrorGeometry.toScreen(0, 0, 664 to 1440, 1080 to 2340))
        assertEquals(1079f to 2339f, MirrorGeometry.toScreen(9999, 9999, 664 to 1440, 1080 to 2340))
    }

    @Test
    fun the_mirror_envelopes_become_signals() {
        val options = MirrorOptions(1440, 60, 6000, 0, false)
        assertEquals(DesktopSignal.MirrorStart(options), DesktopSignal.of(LinkEvent(9, 1, "", mirrorStart = options)))
        assertEquals(DesktopSignal.MirrorStop, DesktopSignal.of(LinkEvent(9, 3, "", mirrorStop = true)))
        val touch = MirrorTouch(0, 10, 20, 0)
        assertEquals(DesktopSignal.MirrorTouched(touch), DesktopSignal.of(LinkEvent(9, 4, "", mirrorTouch = touch)))
        assertEquals(DesktopSignal.MirrorGlobal(1), DesktopSignal.of(LinkEvent(9, 6, "", mirrorGlobal = 1)))
        assertEquals(DesktopSignal.Other(9, 2), DesktopSignal.of(LinkEvent(9, 2, "")))
    }
}

class MirrorKeysTest {
    @org.junit.Test
    fun key_codes_become_the_characters_the_field_gets() {
        org.junit.Assert.assertEquals('a', MirrorInput.character(29))
        org.junit.Assert.assertEquals('9', MirrorInput.character(16))
        org.junit.Assert.assertEquals(' ', MirrorInput.character(62))
        org.junit.Assert.assertNull(MirrorInput.character(19))
    }
}
