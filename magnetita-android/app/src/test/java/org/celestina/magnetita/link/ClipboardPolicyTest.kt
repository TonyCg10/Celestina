package org.celestina.magnetita.link

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** What is worth the wire, and what would only echo. */
class ClipboardPolicyTest {
    @Test
    fun itSendsTextOnceAndNeverEchoesWhatTheDesktopSent() {
        val policy = ClipboardPolicy()
        assertFalse(policy.offer(null))
        assertFalse(policy.offer("   "))
        assertTrue(policy.offer("first"))
        assertFalse(policy.offer("first"))
        assertTrue(policy.offer("second"))
        policy.received("from the desk")
        assertFalse(policy.offer("from the desk"))
        assertTrue(policy.offer("typed after"))
    }

    @Test
    fun itRefusesTheProtocolBoundAndNulBytes() {
        val policy = ClipboardPolicy()
        assertFalse(policy.offer("x".repeat(ClipboardPolicy.MAX_BYTES + 1)))
        assertTrue(policy.offer("x".repeat(ClipboardPolicy.MAX_BYTES)))
        assertFalse(policy.offer("a\u0000b"))
    }
}
