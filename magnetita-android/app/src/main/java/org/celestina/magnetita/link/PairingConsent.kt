package org.celestina.magnetita.link

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * What a `magnetita://pair` link names, read only to show it to the person
 * before anything is dialled. The core parses the link again and its parse
 * is the one that pairs; this preview refuses every ambiguity the core would
 * resolve silently (a repeated id or fingerprint), so what the screen shows
 * is what the core uses.
 */
data class PairOffer(
    val uri: String,
    /** The id the desktop's QR names. */
    val deviceId: String,
    /** The pinned certificate's SHA-256 as colon-separated hex pairs, as the desktop prints fingerprints. */
    val fingerprint: String,
    /** Every `ip:port` the core would dial, in order; all on the LAN. */
    val addresses: List<String>,
)

/** Why a pairing link was refused before the person was asked. */
enum class PairRefusal { NotMagnetita, Malformed, NoAddress, NotLan }

/** A link read for the consent screen: an offer to show, or a refusal. */
sealed interface PairPreview {
    data class Offer(val offer: PairOffer) : PairPreview
    data class Refused(val reason: PairRefusal) : PairPreview

    companion object {
        /** The id bound of the wire (`MAX_IDENT`). */
        private const val MAX_ID = 128

        fun of(uri: String?): PairPreview {
            if (uri == null || !PairLink.accepts(uri)) return Refused(PairRefusal.NotMagnetita)
            var id: String? = null
            var fingerprint: String? = null
            val addresses = ArrayList<String>()
            for (field in uri.removePrefix(PairLink.PREFIX).split('&')) {
                val eq = field.indexOf('=')
                if (eq < 0) return Refused(PairRefusal.Malformed)
                val value = field.substring(eq + 1)
                when (field.substring(0, eq)) {
                    "id" -> {
                        if (id != null || value.isEmpty() || value.length > MAX_ID || !value.all(::isAsciiAlphanumeric)) {
                            return Refused(PairRefusal.Malformed)
                        }
                        id = value
                    }
                    "fp" -> {
                        if (fingerprint != null) return Refused(PairRefusal.Malformed)
                        fingerprint = fingerprintText(value) ?: return Refused(PairRefusal.Malformed)
                    }
                    "addr" -> addresses += value
                }
            }
            if (id == null || fingerprint == null) return Refused(PairRefusal.Malformed)
            if (addresses.isEmpty()) return Refused(PairRefusal.NoAddress)
            // The core dials every address in order, so one outside the LAN refuses the link.
            if (!addresses.all(LanAddress::isLan)) return Refused(PairRefusal.NotLan)
            return Offer(PairOffer(uri, id, fingerprint, addresses))
        }

        /** 64 hex digits as `aa:bb:…`, or null. */
        private fun fingerprintText(hex: String): String? {
            if (hex.length != 64 || !hex.all(LanAddress::isHexDigit)) return null
            return hex.lowercase().chunked(2).joinToString(":")
        }

        private fun isAsciiAlphanumeric(c: Char): Boolean = c in 'a'..'z' || c in 'A'..'Z' || c in '0'..'9'
    }
}

/**
 * Which `ip:port` literals a pairing link may name: RFC 1918 IPv4 and IPv6
 * unique-local (`fc00::/7`) addresses, the addresses of a home LAN. Loopback
 * (another app on this phone), link-local, public and shared (CGNAT)
 * addresses, host names and scope ids are refused. Parsing is strict so an
 * address accepted here is the one the core's `SocketAddr` parse reads.
 */
object LanAddress {
    fun isLan(address: String): Boolean {
        if (address.startsWith("[")) {
            val close = address.indexOf(']')
            if (close < 0 || address.getOrNull(close + 1) != ':' || !port(address.substring(close + 2))) return false
            val hextets = ipv6(address.substring(1, close)) ?: return false
            return (hextets[0] and 0xfe00) == 0xfc00
        }
        val colon = address.lastIndexOf(':')
        if (colon < 0 || !port(address.substring(colon + 1))) return false
        val v4 = ipv4(address.substring(0, colon)) ?: return false
        return v4[0] == 10 || (v4[0] == 172 && v4[1] in 16..31) || (v4[0] == 192 && v4[1] == 168)
    }

    internal fun isHexDigit(c: Char): Boolean = c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F'

    private fun port(text: String): Boolean =
        text.length in 1..5 && text.all { it in '0'..'9' } && text.toInt() in 1..65535

    /** Four decimal octets, no leading zeros, or null. */
    private fun ipv4(text: String): IntArray? {
        val parts = text.split('.')
        if (parts.size != 4) return null
        val out = IntArray(4)
        for ((i, part) in parts.withIndex()) {
            if (part.isEmpty() || part.length > 3 || !part.all { it in '0'..'9' }) return null
            if (part.length > 1 && part[0] == '0') return null
            val value = part.toInt()
            if (value > 255) return null
            out[i] = value
        }
        return out
    }

    /** The eight hextets of an IPv6 literal (with `::` and a trailing dotted IPv4), or null. */
    private fun ipv6(text: String): IntArray? {
        if (text.isEmpty() || text.length > 45) return null
        val halves = text.split("::")
        if (halves.size > 2) return null
        val compressed = halves.size == 2
        val head = words(halves[0], trailingV4 = !compressed) ?: return null
        val tail = if (compressed) words(halves[1], trailingV4 = true) ?: return null else emptyList()
        val count = head.size + tail.size
        if (if (compressed) count > 7 else count != 8) return null
        val out = IntArray(8)
        head.forEachIndexed { i, w -> out[i] = w }
        tail.forEachIndexed { i, w -> out[8 - tail.size + i] = w }
        return out
    }

    /** The 16-bit words of a `:`-separated run; an empty run is none. */
    private fun words(run: String, trailingV4: Boolean): List<Int>? {
        if (run.isEmpty()) return emptyList()
        val groups = run.split(':')
        val out = ArrayList<Int>(8)
        for ((i, group) in groups.withIndex()) {
            if (trailingV4 && i == groups.lastIndex && '.' in group) {
                val v4 = ipv4(group) ?: return null
                out += (v4[0] shl 8) or v4[1]
                out += (v4[2] shl 8) or v4[3]
            } else {
                if (group.isEmpty() || group.length > 4 || !group.all(::isHexDigit)) return null
                out += group.toInt(16)
            }
        }
        return out
    }
}

/** Where a pairing link stands with the person. */
sealed interface PairingState {
    /** Nothing to decide. */
    data object Idle : PairingState

    /** The screen shows this offer and waits for the person's answer. */
    data class Confirming(val offer: PairOffer) : PairingState

    /** A link was refused; the screen says why until dismissed. */
    data class Refused(val reason: PairRefusal) : PairingState
}

/**
 * The consent in front of pairing. Every link, whether from the exported
 * deep link or from the scanner, is offered here; nothing pairs until the
 * person confirms the very offer the screen shows. While one offer waits,
 * any other link is ignored, so a second intent cannot swap the desktop
 * under the person's finger. Pure, so the JVM tests pin it.
 */
class PairingConsent {
    private val _state = MutableStateFlow<PairingState>(PairingState.Idle)
    val state: StateFlow<PairingState> = _state.asStateFlow()

    /** Offers a link; false when ignored because another is awaiting the person. */
    @Synchronized
    fun offer(uri: String?): Boolean {
        if (_state.value is PairingState.Confirming) return false
        _state.value = when (val preview = PairPreview.of(uri)) {
            is PairPreview.Offer -> PairingState.Confirming(preview.offer)
            is PairPreview.Refused -> PairingState.Refused(preview.reason)
        }
        return true
    }

    /** The person confirmed `offer`: its link to pair, once, or null when it is not the one waiting. */
    @Synchronized
    fun confirm(offer: PairOffer): String? {
        val current = _state.value
        if (current !is PairingState.Confirming || current.offer != offer) return null
        _state.value = PairingState.Idle
        return offer.uri
    }

    /** The person declined the offer or dismissed a refusal. */
    @Synchronized
    fun dismiss() {
        _state.value = PairingState.Idle
    }
}
