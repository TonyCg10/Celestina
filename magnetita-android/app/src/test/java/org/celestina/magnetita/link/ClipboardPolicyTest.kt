package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * What would only echo, and what the wire's rule refuses. The rule itself
 * (bound, NUL) is the core's and is tested there; here it is a fake.
 */
class ClipboardPolicyTest {
    private val policy = ClipboardPolicy { it != "refused by the rule" }

    @Test
    fun itSendsTextOnceAndNeverEchoesWhatTheDesktopSent() {
        assertEquals(ClipboardPolicy.Offer.Nothing, policy.offer(null))
        assertEquals(ClipboardPolicy.Offer.Nothing, policy.offer("   "))
        assertEquals(ClipboardPolicy.Offer.Send, policy.offer("first"))
        assertEquals(ClipboardPolicy.Offer.Nothing, policy.offer("first"))
        assertEquals(ClipboardPolicy.Offer.Send, policy.offer("second"))
        policy.received("from the desk")
        assertEquals(ClipboardPolicy.Offer.Nothing, policy.offer("from the desk"))
        assertEquals(ClipboardPolicy.Offer.Send, policy.offer("typed after"))
    }

    @Test
    fun whatTheWireRefusesIsSaidAndNotRecorded() {
        assertEquals(ClipboardPolicy.Offer.Refused, policy.offer("refused by the rule"))
        // Not recorded as exchanged: the same text is judged again next time.
        assertEquals(ClipboardPolicy.Offer.Refused, policy.offer("refused by the rule"))
    }
}
