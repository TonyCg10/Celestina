package org.celestina.magnetita.mirror

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The desktop's keys and global actions need a streaming mirror (audit finding AND-6). */
class MirrorControlTest {
    private class Recorder : MirrorSink {
        val seen = ArrayList<String>()
        override fun key(keycode: Int, pressed: Boolean) { seen += "key $keycode $pressed" }
        override fun global(action: Int) { seen += "global $action" }
    }

    @Test
    fun a_key_without_a_stream_is_ignored() {
        val sink = Recorder()
        val control = MirrorControl(streaming = { false }, sink = { sink })
        assertFalse(control.key(29, true))
        assertFalse(control.global(1))
        assertEquals(emptyList<String>(), sink.seen)
    }

    @Test
    fun keys_and_global_actions_reach_the_phone_while_streaming() {
        val sink = Recorder()
        var streaming = true
        val control = MirrorControl(streaming = { streaming }, sink = { sink })
        assertTrue(control.key(29, true))
        assertTrue(control.global(0))
        streaming = false
        assertFalse(control.key(30, true))
        assertEquals(listOf("key 29 true", "global 0"), sink.seen)
    }

    @Test
    fun no_accessibility_service_means_nothing_acts() {
        val control = MirrorControl(streaming = { true }, sink = { null })
        assertFalse(control.key(29, true))
        assertFalse(control.global(2))
    }
}
