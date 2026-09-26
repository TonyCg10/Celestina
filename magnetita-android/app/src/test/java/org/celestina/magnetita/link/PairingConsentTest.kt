package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The consent in front of pairing (audit finding AND-1): a link, from the
 * exported deep link or the scanner, pairs only after the person confirms
 * the desktop it names, in time, and only when every address is on the LAN
 * and none is this phone's own.
 */
class PairingConsentTest {
    private val fp = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF"
    private val secret = "ab".repeat(32)
    private var clock = 1_000L
    private val consent = PairingConsent { clock }

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
    fun a_public_address_is_refused_before_the_person_is_asked() {
        offer(link("8.8.8.8:1760"))
        assertEquals(PairingState.Refused(PairRefusal.NotLan), consent.state.value)
        // One public address among LAN ones refuses the whole link: the core dials them all.
        offer(link("192.168.1.20:1760", "203.0.113.9:1760"))
        assertEquals(PairingState.Refused(PairRefusal.NotLan), consent.state.value)
        // A refusal is not pending: the next valid link is shown.
        offer(link("172.16.4.2:1760"))
        assertTrue(consent.state.value is PairingState.Confirming)
    }

    @Test
    fun an_address_of_this_phone_is_refused() {
        val local = setOf("192.168.1.30", "/fd00:0:0:0:0:0:0:30%wlan0", "127.0.0.1", "not-an-address")
        consent.offer(link("192.168.1.30:1760"), local)
        assertEquals(PairingState.Refused(PairRefusal.ThisPhone), consent.state.value)
        consent.offer(link("[fd00::30]:1760"), local)
        assertEquals(PairingState.Refused(PairRefusal.ThisPhone), consent.state.value)
        consent.offer(link("192.168.1.20:1760", "192.168.1.30:1760"), local)
        assertEquals(PairingState.Refused(PairRefusal.ThisPhone), consent.state.value)
        consent.offer(link("192.168.1.20:1760"), local)
        assertTrue(consent.state.value is PairingState.Confirming)
    }

    @Test
    fun no_readable_address_of_this_phone_refuses_every_link() {
        assertEquals(PairPreview.Refused(PairRefusal.LocalUnknown), PairPreview.of(link("192.168.1.20:1760"), emptySet()))
        assertEquals(PairPreview.Refused(PairRefusal.LocalUnknown), PairPreview.of(link("192.168.1.20:1760"), setOf("wlan0")))
        consent.offer(link("192.168.1.20:1760"), emptySet())
        assertEquals(PairingState.Refused(PairRefusal.LocalUnknown), consent.state.value)
    }

    @Test
    fun links_the_core_would_refuse_are_refused() {
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of(null, phone))
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of("https://example.org/?addr=192.168.1.2:1", phone))
        assertEquals(PairPreview.Refused(PairRefusal.NotMagnetita), PairPreview.of(link("192.168.1.2:1760") + "x".repeat(600), phone))
        assertEquals(PairPreview.Refused(PairRefusal.NoAddress), PairPreview.of(link(), phone))
        val malformed = listOf(
            "magnetita://pair?v=1&fp=$fp&secret=$secret&addr=192.168.1.2:1760",
            "magnetita://pair?v=2&id=desk1&fp=$fp&secret=$secret&addr=192.168.1.2:1760",
            "magnetita://pair?id=desk1&fp=$fp&secret=$secret&addr=192.168.1.2:1760",
            "magnetita://pair?v=1&id=desk1&fp=$fp&addr=192.168.1.2:1760",
            "magnetita://pair?v=1&id=desk1&fp=$fp&secret=abc&addr=192.168.1.2:1760",
            "magnetita://pair?v=1&id=desk1&fp=abc&secret=$secret&addr=192.168.1.2:1760",
            link("192.168.1.2:1760", id = "bad-id"),
            link("192.168.1.2:1760") + "&",
            link("192.168.1.2:1760") + "&addr=",
            // A repeated field would let the screen show one value and the core use another.
            link("192.168.1.2:1760") + "&id=evil",
            link("192.168.1.2:1760") + "&fp=" + "0".repeat(64),
            link("192.168.1.2:1760") + "&v=1",
            link("192.168.1.2:1760") + "&secret=$secret",
        )
        for (uri in malformed) {
            assertEquals(uri, PairPreview.Refused(PairRefusal.Malformed), PairPreview.of(uri, phone))
        }
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

    @Test
    fun hosts_compare_in_one_canonical_form() {
        assertEquals("fd00:0:0:0:0:0:0:30", LanAddress.hostOf("[fd00::30]:1760"))
        assertEquals("fd00:0:0:0:0:0:0:30", LanAddress.canonicalHost("fd00:0:0:0:0:0:0:30%wlan0"))
        assertEquals("192.168.1.30", LanAddress.canonicalHost("/192.168.1.30"))
        assertNull(LanAddress.canonicalHost("wlan0"))
    }
}
