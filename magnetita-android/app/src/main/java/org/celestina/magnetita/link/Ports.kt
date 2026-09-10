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

    /** A notification appeared or changed on this phone. */
    fun sendNotification(note: PhoneNotification): Boolean

    /** A notification left this phone. */
    fun sendNotificationGone(key: String): Boolean

    /** Offers a file; the transfer id, or null when the session is gone. */
    fun offerFile(name: String, size: Long, mime: String): Int?

    /** Writes the next bytes of an accepted transfer. */
    fun writeTransfer(transfer: Int, bytes: ByteArray): Boolean

    /** Ends an accepted transfer. */
    fun finishTransfer(transfer: Int): Boolean

    /** Accepts an offered file into `dir`; its end arrives as a signal. */
    fun acceptFile(transfer: Int, dir: String): Boolean

    fun rejectFile(transfer: Int): Boolean

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
data class LinkEvent(
    val capability: Int,
    val kind: Int,
    val description: String,
    val text: String? = null,
    val key: String? = null,
    val action: Int? = null,
    val transfer: Int? = null,
    val size: Long? = null,
    val offset: Long? = null,
    val complete: Boolean? = null,
    val path: String? = null,
)

/** One of this phone's notifications, as the wire carries it. */
data class PhoneNotification(
    val key: String,
    val appName: String,
    val title: String,
    val body: String,
    val timestampMs: Long,
    val replyable: Boolean,
    val actions: List<String>,
    val icon: ByteArray? = null,
)

/** Something this phone wants to tell the desktop, in order. */
sealed interface Outbound {
    data class Clipboard(val text: String) : Outbound
    data class Notification(val note: PhoneNotification) : Outbound
    data class NotificationGone(val key: String) : Outbound

    /** A file to offer: what the content resolver knows about it. */
    data class File(val uri: String, val name: String, val size: Long, val mime: String) : Outbound
}

/** Where a desktop is advertising, one entry per (id, address). */
data class Advertised(val deviceId: String, val address: String)

/** The LAN's answer to "who is here", asked once per attempt. */
fun interface Discovery {
    suspend fun browse(): List<Advertised>
}

/** Opens the bytes of a file this phone offers, by the URI the share gave. */
fun interface FileSource {
    fun open(uri: String): java.io.InputStream?
}

/** What the phone knows about itself that the desktop wants. */
fun interface BatterySource {
    fun read(): Pair<Int, Boolean>
}
