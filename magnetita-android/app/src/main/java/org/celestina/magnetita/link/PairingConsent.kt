package org.celestina.magnetita.link

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * What a `magnetita://pair` link names, read only to show it to the person
 * before anything is dialled. The core reads it ([CorePairing]) with the
 * very parse that pairs, so the screen never shows an offer the core would
 * read differently.
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
 * Pure, so the JVM tests pin it with their own `preview`; the rules that
 * read a link are the core's and are tested there. `now` is a monotonic clock in ms; the
 * app passes `SystemClock.elapsedRealtime`, which counts deep sleep, so an
 * offer's two minutes are wall time. The default serves the JVM tests.
 */
class PairingConsent(
    /** Reads a link against this phone's own addresses; the app passes [CorePairing.preview]. */
    private val preview: (String?, Set<String>) -> PairPreview,
    private val now: () -> Long = { System.nanoTime() / 1_000_000 },
) {
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
        _state.value = when (val read = preview(uri, local)) {
            is PairPreview.Offer -> PairingState.Confirming(read.offer).also { offeredAt = now() }
            is PairPreview.Refused -> PairingState.Refused(read.reason)
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
