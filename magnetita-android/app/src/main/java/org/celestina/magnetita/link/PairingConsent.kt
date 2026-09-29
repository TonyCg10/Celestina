package org.celestina.magnetita.link

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * What a `magnetita://pair` link names, read only to show it to the person
 * before anything is dialled. The core parses the link again and its parse
 * is the one that pairs; this preview refuses everything the core refuses
 * and every ambiguity the core would resolve silently (a repeated field),
 * so the screen never shows an offer the core would read differently.
 */
data class PairOffer(
    val uri: String,
    /** The id the desktop's QR names. */
    val deviceId: String,
    /** The pinned certificate's SHA-256 as colon-separated hex pairs, as the desktop prints fingerprints. */
    val fingerprint: String,
    /** Every `ip:port` the core would dial, in order; all on the LAN and none this phone's own. */
    val addresses: List<String>,
)

/** Why a pairing link was refused, or why a waiting offer ended, without pairing. */
enum class PairRefusal {
    NotMagnetita,
    Malformed,
    NoAddress,
    NotLan,
    /** An address is one of this phone's own: another app here would answer as the desktop. */
    ThisPhone,
    /** A second, different link arrived while one waited: both are dropped. */
    Conflict,
    /** The offer waited longer than [PairingConsent.TTL_MS]. */
    Expired,
    /** This phone's own addresses could not be read, so a link to itself could not be told apart. */
    LocalUnknown,
}

/** A link read for the consent screen: an offer to show, or a refusal. */
sealed interface PairPreview {
    data class Offer(val offer: PairOffer) : PairPreview
    data class Refused(val reason: PairRefusal) : PairPreview

    companion object {
        /** The wire's bounds on an id or an address (`MAX_IDENT`) and on a list (`MAX_LIST`). */
        private const val MAX_IDENT = 128
        private const val MAX_LIST = 256

        /**
         * Reads `uri` for the screen. `local` holds this phone's own
         * interface addresses as host literals; a link naming one of them is
         * refused, and so is every link when none of them could be read.
         */
        fun of(uri: String?, local: Set<String>): PairPreview {
            if (uri == null || !PairLink.accepts(uri)) return Refused(PairRefusal.NotMagnetita)
            var version: String? = null
            var id: String? = null
            var fingerprint: String? = null
            var secret = false
            val addresses = ArrayList<String>()
            for (field in uri.removePrefix(PairLink.PREFIX).split('&')) {
                val eq = field.indexOf('=')
                if (eq < 0) return Refused(PairRefusal.Malformed)
                val value = field.substring(eq + 1)
                when (field.substring(0, eq)) {
                    "v" -> {
                        if (version != null) return Refused(PairRefusal.Malformed)
                        version = value
                    }
                    "id" -> {
                        if (id != null || value.isEmpty() || value.length > MAX_IDENT || !value.all(::isAsciiAlphanumeric)) {
                            return Refused(PairRefusal.Malformed)
                        }
                        id = value
                    }
                    "fp" -> {
                        if (fingerprint != null) return Refused(PairRefusal.Malformed)
                        fingerprint = fingerprintText(value) ?: return Refused(PairRefusal.Malformed)
                    }
                    "secret" -> {
                        if (secret || fingerprintText(value) == null) return Refused(PairRefusal.Malformed)
                        secret = true
                    }
                    "addr" -> {
                        if (value.isEmpty() || value.length > MAX_IDENT || addresses.size >= MAX_LIST) {
                            return Refused(PairRefusal.Malformed)
                        }
                        addresses += value
                    }
                }
            }
            if (version != "1" || id == null || fingerprint == null || !secret) return Refused(PairRefusal.Malformed)
            if (addresses.isEmpty()) return Refused(PairRefusal.NoAddress)
            // The core dials every address in order, so one bad address refuses the link.
            if (!addresses.all(LanAddress::isLan)) return Refused(PairRefusal.NotLan)
            val own = local.mapNotNull(LanAddress::canonicalHost).toSet()
            // Fail closed: without this phone's addresses a link to itself would pass.
            if (own.isEmpty()) return Refused(PairRefusal.LocalUnknown)
            if (addresses.any { LanAddress.hostOf(it) in own }) return Refused(PairRefusal.ThisPhone)
            return Offer(PairOffer(uri, id, fingerprint, addresses))
        }

        /** 64 hex digits (32 bytes) as `aa:bb:…`, or null. */
        private fun fingerprintText(hex: String): String? {
            if (hex.length != 64 || !hex.all(LanAddress::isHexDigit)) return null
            return hex.lowercase().chunked(2).joinToString(":")
        }

        private fun isAsciiAlphanumeric(c: Char): Boolean = c in 'a'..'z' || c in 'A'..'Z' || c in '0'..'9'
    }
}

/**
 * Which `ip:port` literals a pairing link may name: RFC 1918 IPv4 and IPv6
 * unique-local (`fc00::/7`) addresses, the addresses of a home LAN. Loopback,
 * link-local, public and shared (CGNAT) addresses, host names and scope ids
 * are refused. Parsing is strict so an address accepted here is the one the
 * core's `SocketAddr` parse reads. This phone's own LAN address passes here;
 * [PairPreview.of] refuses it against the interface addresses it is given.
 */
object LanAddress {
    fun isLan(address: String): Boolean {
        val host = parse(address) ?: return false
        return if (host.size == 4) {
            host[0] == 10 || (host[0] == 172 && host[1] in 16..31) || (host[0] == 192 && host[1] == 168)
        } else {
            (host[0] and 0xfe00) == 0xfc00
        }
    }

    /** The canonical host of an `ip:port` literal, comparable with [canonicalHost], or null. */
    fun hostOf(address: String): String? = parse(address)?.let(::canonical)

    /**
     * The canonical form of a bare host literal as an interface reports it
     * (`192.168.1.20`, `fd00:0:0:0:0:0:0:5%wlan0`), or null. The scope id is
     * dropped: a local address is this phone's on any interface.
     */
    fun canonicalHost(host: String): String? {
        val bare = host.removePrefix("/").substringBefore('%')
        val words = if (':' in bare) ipv6(bare) else ipv4(bare)
        return words?.let(::canonical)
    }

    internal fun isHexDigit(c: Char): Boolean = c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F'

    /** Four octets for IPv4, eight hextets for IPv6, or null. */
    private fun parse(address: String): IntArray? {
        if (address.startsWith("[")) {
            val close = address.indexOf(']')
            if (close < 0 || address.getOrNull(close + 1) != ':' || !port(address.substring(close + 2))) return null
            return ipv6(address.substring(1, close))
        }
        val colon = address.lastIndexOf(':')
        if (colon < 0 || !port(address.substring(colon + 1))) return null
        return ipv4(address.substring(0, colon))
    }

    private fun canonical(words: IntArray): String =
        if (words.size == 4) words.joinToString(".") else words.joinToString(":") { it.toString(16) }

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

    /** A link was refused, or an offer ended unanswered; the screen says why until dismissed. */
    data class Refused(val reason: PairRefusal) : PairingState
}

/**
 * The consent in front of pairing. Every link, whether from the exported
 * deep link or from the scanner, is offered here; nothing pairs until the
 * person confirms the very offer the screen shows, within [TTL_MS].
 *
 * - A different link arriving while one waits drops both and says so
 *   ([PairRefusal.Conflict]): neither is trusted. The conflict then holds
 *   and every further link is ignored until the person dismisses it, so a
 *   third link cannot replace the message before it is read. The same link
 *   again is the same offer and changes nothing.
 * - A waiting offer expires after [TTL_MS] and is dismissed when no consent
 *   screen is in the foreground any more, so a link planted while the
 *   person looked away cannot wait for a later, genuine pairing.
 *
 * Pure, so the JVM tests pin it. `now` is a monotonic clock in ms; the
 * app passes `SystemClock.elapsedRealtime`, which counts deep sleep, so an
 * offer's two minutes are wall time. The default serves the JVM tests.
 */
class PairingConsent(private val now: () -> Long = { System.nanoTime() / 1_000_000 }) {
    private val _state = MutableStateFlow<PairingState>(PairingState.Idle)
    val state: StateFlow<PairingState> = _state.asStateFlow()
    private var offeredAt = 0L
    private var screens = 0

    /**
     * Offers a link, refusing it against this phone's own addresses `local`.
     * False when the link was ignored: a conflict is on screen, or the same
     * link already waits.
     */
    @Synchronized
    fun offer(uri: String?, local: Set<String>): Boolean {
        expire()
        val waiting = _state.value
        if (waiting == PairingState.Refused(PairRefusal.Conflict)) return false
        if (waiting is PairingState.Confirming) {
            if (waiting.offer.uri == uri) return false
            _state.value = PairingState.Refused(PairRefusal.Conflict)
            return true
        }
        _state.value = when (val preview = PairPreview.of(uri, local)) {
            is PairPreview.Offer -> PairingState.Confirming(preview.offer).also { offeredAt = now() }
            is PairPreview.Refused -> PairingState.Refused(preview.reason)
        }
        return true
    }

    /** The person confirmed `offer`: its link to pair, once, or null when it is not the one waiting. */
    @Synchronized
    fun confirm(offer: PairOffer): String? {
        expire()
        val current = _state.value
        if (current !is PairingState.Confirming || current.offer != offer) return null
        _state.value = PairingState.Idle
        return offer.uri
    }

    /** How long the waiting offer has left, 0 when none waits. */
    @Synchronized
    fun remainingMs(): Long =
        if (_state.value is PairingState.Confirming) (offeredAt + TTL_MS - now()).coerceAtLeast(0) else 0

    /** Ends a waiting offer whose time is up; the screen then says it expired. */
    @Synchronized
    fun expire() {
        if (_state.value is PairingState.Confirming && now() - offeredAt >= TTL_MS) {
            _state.value = PairingState.Refused(PairRefusal.Expired)
        }
    }

    /** The person declined the offer or dismissed a refusal; the only way out of a conflict. */
    @Synchronized
    fun dismiss() {
        _state.value = PairingState.Idle
    }

    /** A screen that shows the consent came to the foreground. */
    @Synchronized
    fun screenStarted() {
        screens += 1
    }

    /**
     * A screen that shows the consent left the foreground. When none is left
     * the offer is dropped, unless the screen is only being recreated
     * (`changingConfigurations`), which starts it again at once.
     */
    @Synchronized
    fun screenStopped(changingConfigurations: Boolean) {
        screens = (screens - 1).coerceAtLeast(0)
        if (screens == 0 && !changingConfigurations) _state.value = PairingState.Idle
    }

    companion object {
        /** How long an offer waits for the person. */
        const val TTL_MS = 120_000L
    }
}
