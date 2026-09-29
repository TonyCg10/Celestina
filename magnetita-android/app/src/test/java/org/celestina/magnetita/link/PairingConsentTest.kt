package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The consent in front of pairing (audit finding AND-1): a link, from the
 * exported deep link or the scanner, pairs only after the person confirms
 * the offer it shows, in time. How a link is read (the parse, the LAN rule,
 * this phone's own addresses) is the core's `preview_pairing`, tested in
 * `magnetita-mobile`; here a fake reads the links these tests write.
 */
class PairingConsentTest {
    private val fp = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF"
    private val secret = "ab".repeat(32)
    private var clock = 1_000L
    /** Reads the links [link] writes: every address public in 8.0.0.0/8 is refused. */
    private val preview: (String?, Set<String>) -> PairPreview = { uri, _ ->
        val fields = uri?.substringAfter('?')?.split('&')?.map { it.substringBefore('=') to it.substringAfter('=') }.orEmpty()
        val addresses = fields.filter { it.first == "addr" }.map { it.second }
        when {
            uri == null -> PairPreview.Refused(PairRefusal.NotMagnetita)
            addresses.any { it.startsWith("8.") } -> PairPreview.Refused(PairRefusal.NotLan)
            else -> PairPreview.Offer(
                PairOffer(
                    uri,
                    fields.first { it.first == "id" }.second,
                    fields.first { it.first == "fp" }.second.lowercase().chunked(2).joinToString(":"),
                    addresses,
                ),
            )
        }
    }
    private val consent = PairingConsent(preview) { clock }

    private fun link(vararg addresses: String, id: String = "desk1"): String =
        "magnetita://pair?v=1&id=$id&fp=$fp&secret=$secret" + addresses.joinToString("") { "&addr=$it" }

    /** This phone's own addresses, as the interfaces report them. */
    private val phone = setOf("127.0.0.1", "192.168.1.30")

    private fun offer(uri: String): Boolean = consent.offer(uri, phone)

    private fun waiting(): PairOffer = (consent.state.value as PairingState.Confirming).offer

    @Test
    fun an_intent_without_confirmation_never_pairs() {
        offer(link("192.168.1.20:1760"))
        val shown = waiting()
        assertEquals("desk1", shown.deviceId)
        assertEquals(listOf("192.168.1.20:1760"), shown.addresses)
        assertEquals(
            "00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff",
            shown.fingerprint,
        )
        // Declining is the only other way out, and it pairs nothing.
        consent.dismiss()
        assertEquals(PairingState.Idle, consent.state.value)
        assertNull(consent.confirm(shown))
    }

    @Test
    fun only_the_confirmed_offer_pairs_and_only_once() {
        val uri = link("10.0.0.5:1760")
        offer(uri)
        val shown = waiting()
        // A screen holding a different offer than the waiting one cannot confirm it.
        val other = shown.copy(uri = link("192.168.1.66:1760", id = "other"), deviceId = "other")
        assertNull(consent.confirm(other))
        assertEquals(PairingState.Confirming(shown), consent.state.value)
        assertEquals(uri, consent.confirm(shown))
        assertEquals(PairingState.Idle, consent.state.value)
        assertNull(consent.confirm(shown))
    }

    @Test
    fun a_different_link_while_one_waits_drops_both_and_says_so() {
        val planted = link("192.168.1.66:1760", id = "planted")
        offer(planted)
        val shown = waiting()
        assertTrue(offer(link("192.168.1.20:1760")))
        assertEquals(PairingState.Refused(PairRefusal.Conflict), consent.state.value)
        // Neither link can be confirmed any more.
        assertNull(consent.confirm(shown))
        // The person dismisses the message (a scan is reachable only from Idle) and scans again.
        consent.dismiss()
        offer(link("192.168.1.20:1760"))
        assertEquals("desk1", waiting().deviceId)
    }

    @Test
    fun a_conflict_holds_until_the_person_dismisses_it() {
        offer(link("192.168.1.20:1760"))
        offer(link("192.168.1.66:1760", id = "planted"))
        // A third link cannot replace the message before the person reads it.
        assertFalse(offer(link("192.168.1.77:1760", id = "third")))
        assertEquals(PairingState.Refused(PairRefusal.Conflict), consent.state.value)
        consent.dismiss()
        assertTrue(offer(link("192.168.1.77:1760", id = "third")))
        assertEquals("third", waiting().deviceId)
    }

    @Test
    fun the_same_link_again_keeps_the_waiting_offer() {
        val uri = link("192.168.1.20:1760")
        offer(uri)
        val shown = waiting()
        assertFalse(offer(uri))
        assertEquals(PairingState.Confirming(shown), consent.state.value)
        assertEquals(uri, consent.confirm(shown))
    }

    @Test
    fun a_waiting_offer_expires() {
        offer(link("192.168.1.20:1760"))
        val shown = waiting()
        clock += PairingConsent.TTL_MS - 1
        assertEquals(1L, consent.remainingMs())
        consent.expire()
        assertEquals(PairingState.Confirming(shown), consent.state.value)
        clock += 1
        assertEquals(0L, consent.remainingMs())
        // A confirm that comes too late pairs nothing.
        assertNull(consent.confirm(shown))
        assertEquals(PairingState.Refused(PairRefusal.Expired), consent.state.value)
    }

    @Test
    fun an_expired_offer_is_replaced_by_the_next_link() {
        offer(link("192.168.1.66:1760", id = "planted"))
        clock += PairingConsent.TTL_MS
        val genuine = link("192.168.1.20:1760")
        offer(genuine)
        assertEquals("desk1", waiting().deviceId)
        assertEquals(genuine, consent.confirm(waiting()))
    }

    @Test
    fun leaving_the_foreground_drops_the_offer_but_a_recreation_does_not() {
        consent.screenStarted()
        offer(link("192.168.1.20:1760"))
        // Rotation: the old screen stops while changing configurations, the new one starts.
        consent.screenStopped(changingConfigurations = true)
        consent.screenStarted()
        assertTrue(consent.state.value is PairingState.Confirming)
        // A second screen on top, then the first one stops: one is still in front.
        consent.screenStarted()
        consent.screenStopped(changingConfigurations = false)
        assertTrue(consent.state.value is PairingState.Confirming)
        // Home: the last screen stops.
        consent.screenStopped(changingConfigurations = false)
        assertEquals(PairingState.Idle, consent.state.value)
    }

    @Test
    fun a_refused_link_is_shown_as_refused_and_is_not_pending() {
        offer(link("8.8.8.8:1760"))
        assertEquals(PairingState.Refused(PairRefusal.NotLan), consent.state.value)
        assertEquals(0L, consent.remainingMs())
        // A refusal is not pending: the next valid link is shown.
        offer(link("172.16.4.2:1760"))
        assertTrue(consent.state.value is PairingState.Confirming)
    }
}
