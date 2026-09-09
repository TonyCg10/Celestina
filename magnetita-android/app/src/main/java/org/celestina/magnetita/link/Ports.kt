package org.celestina.magnetita.link

/**
 * The two things the controller needs from the outside, narrow enough to
 * fake on the JVM: who is pinned and how to reach them, and where the
 * desktop is right now. The real implementations wrap the Rust core and
 * `NsdManager`; the tests wrap nothing.
 */
interface Connector {
    /** Ids of the desktops this phone has pinned. */
    fun pinnedIds(): List<String>

    /** Dials `address`; a live session, or an error message. */
    fun connect(address: String): Result<LiveSession>

    /** Pins the desktop named by a `magnetita://pair` URI and opens its session. */
    fun pair(uri: String): Result<LiveSession>
}

/** One open session, as the controller drives it. */
interface LiveSession {
    val desktopId: String
    val desktopName: String

    /** Sends a battery report; false when the session is gone. */
    fun reportBattery(level: Int, charging: Boolean): Boolean

    /** Sends this phone's clipboard text; false when the session is gone. */
    fun sendClipboard(text: String): Boolean

    /**
     * Waits up to `timeoutMs` for the next envelope: a short description, or
     * null on timeout. Throws when the session is gone. Suspends, so the
     * wait is real time on the device and virtual time in a test.
     */
    suspend fun next(timeoutMs: Long): LinkEvent?

    fun close(reason: String)
}

/**
 * An envelope the desktop sent, described for a log and carried for a
 * handler; `text` is the clipboard text the core already decoded.
 */
data class LinkEvent(val capability: Int, val kind: Int, val description: String, val text: String? = null)

/** Where a desktop is advertising, one entry per (id, address). */
data class Advertised(val deviceId: String, val address: String)

/** The LAN's answer to "who is here", asked once per attempt. */
fun interface Discovery {
    suspend fun browse(): List<Advertised>
}

/** What the phone knows about itself that the desktop wants. */
fun interface BatterySource {
    fun read(): Pair<Int, Boolean>
}
