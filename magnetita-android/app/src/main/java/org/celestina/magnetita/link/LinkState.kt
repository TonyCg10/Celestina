package org.celestina.magnetita.link

/** What the link is doing, as the screens show it. One value at a time. */
sealed interface LinkState {
    /** No desktop pinned yet: the person must scan a QR. */
    data object NeedsPairing : LinkState

    /** Pinned, looking for the desktop's advertisement on the LAN. */
    data object Searching : LinkState

    /** An address was found; the handshake is in flight. */
    data class Connecting(val address: String) : LinkState

    /** A session is up with this desktop. */
    data class Connected(val desktopId: String, val desktopName: String, val address: String) : LinkState

    /** The session ended; the next attempt waits `retryMs`. */
    data class Waiting(val reason: String, val retryMs: Long) : LinkState
}
