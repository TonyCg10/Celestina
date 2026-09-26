package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The consent in front of pairing (audit finding AND-1): a link, from the
 * exported deep link or the scanner, pairs only after the person confirms
 * the desktop it names, and only when every address is on the LAN.
 */
class PairingConsentTest {
    private val fp = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF"
    private val secret = "ab".repeat(32)

    private fun link(vararg addresses: String, id: String = "desk1"): String =
        "magnetita://pair?v=1&id=$id&fp=$fp&secret=$secret" + addresses.joinToString("") { "&addr=$it" }

    @Test
    fun an_intent_without_confirmation_never_pairs() {
        val consent = PairingConsent()
        assertTrue(consent.offer(link("192.168.1.20:1760")))
        val shown = consent.state.value as PairingState.Confirming
        assertEquals("desk1", shown.offer.deviceId)
        assertEquals(listOf("192.168.1.20:1760"), shown.offer.addresses)
        assertEquals(
            "00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff",
            shown.offer.fingerprint,
        )
        // Declining is the only other way out, and it pairs nothing.
        consent.dismiss()
        assertEquals(PairingState.Idle, consent.state.value)
        assertNull(consent.confirm(shown.offer))
    }

    @Test
    fun only_the_confirmed_offer_pairs_and_only_once() {
        val consent = PairingConsent()
        val uri = link("10.0.0.5:1760")
        consent.offer(uri)
        val shown = (consent.state.value as PairingState.Confirming).offer
        assertEquals(uri, consent.confirm(shown))
        assertEquals(PairingState.Idle, consent.state.value)
        assertNull(consent.confirm(shown))
    }

    @Test
    fun a_second_link_while_one_is_pending_is_ignored() {
        val consent = PairingConsent()
        val first = link("192.168.1.20:1760")
        consent.offer(first)
        val shown = (consent.state.value as PairingState.Confirming).offer
        assertFalse(consent.offer(link("192.168.1.66:1760", id = "other")))
        assertFalse(consent.offer(link("8.8.8.8:1760")))
        assertEquals(PairingState.Confirming(shown), consent.state.value)
        // A screen still holding another offer cannot confirm it.
        val stale = shown.copy(uri = link("192.168.1.66:1760", id = "other"), deviceId = "other")
        assertNull(consent.confirm(stale))
        assertEquals(first, consent.confirm(shown))
    }

    @Test
    fun a_public_address_is_refused_before_the_person_is_asked() {
        val consent = PairingConsent()
        assertTrue(consent.offer(link("8.8.8.8:1760")))
        assertEquals(PairingState.Refused(PairRefusal.NotLan), consent.state.value)
        // One public address among LAN ones refuses the whole link: the core dials them all.
        assertTrue(consent.offer(link("192.168.1.20:1760", "203.0.113.9:1760")))
        assertEquals(PairingState.Refused(PairRefusal.NotLan), consent.state.value)
        // A refusal is not pending: the next valid link is shown.
        assertTrue(consent.offer(link("172.16.4.2:1760")))
        assertTrue(consent.state.value is PairingState.Confirming)
    }

    @Test
    fun links_that_are_not_well_formed_are_refused() {
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of(null))
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of("https://example.org/?addr=192.168.1.2:1"))
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of(link("192.168.1.2:1760") + "x".repeat(600)))
        assertEquals(PairPreview.Refused(PairRefusal.NoAddress), PairPreview.of(link()))
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of("magnetita://pair?v=1&fp=$fp&secret=$secret&addr=192.168.1.2:1760"))
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of(link("192.168.1.2:1760", id = "bad-id")))
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of(link("192.168.1.2:1760") + "&"))
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of("magnetita://pair?v=1&id=desk1&fp=abc&secret=$secret&addr=192.168.1.2:1760"))
        // A repeated id or fingerprint would let the screen show one value and the core use another.
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of(link("192.168.1.2:1760") + "&id=evil"))
        assertEquals(PairPreview.Refused(PairRefusal.Malformed), PairPreview.of(link("192.168.1.2:1760") + "&fp=" + "0".repeat(64)))
    }

    @Test
    fun only_private_and_unique_local_addresses_are_on_the_lan() {
        for (lan in listOf("10.0.0.1:1760", "10.255.255.254:1", "172.16.0.1:1760", "172.31.255.1:1760", "192.168.0.10:65535", "[fd12:3456::1]:1760", "[fc00::1]:1760", "[fd00::192.168.1.2]:1760")) {
            assertTrue(lan, LanAddress.isLan(lan))
        }
        val refused = listOf(
            "8.8.8.8:1760", "172.15.0.1:1760", "172.32.0.1:1760", "192.169.0.1:1760", "100.64.0.1:1760",
            "127.0.0.1:1760", "169.254.1.1:1760", "0.0.0.0:1760", "255.255.255.255:1760",
            "[2001:db8::1]:1760", "[fe80::1]:1760", "[::1]:1760", "[::ffff:192.168.1.2]:1760", "[fd00::1%2]:1760",
            "192.168.1.2", "192.168.1.2:0", "192.168.1.2:65536", "192.168.1.2:+1", "192.168.01.2:1760", "192.168.1:1760",
            "192.168.1.2.3:1760", "fd00::1:1760", "[fd00::1]1760", "[fd00:::1]:1760", "[fd00::1::2]:1760",
            "[1:2:3:4:5:6:7:8:9]:1760", "desktop.local:1760", "", ":1760",
        )
        for (address in refused) {
            assertFalse(address, LanAddress.isLan(address))
        }
    }
}
