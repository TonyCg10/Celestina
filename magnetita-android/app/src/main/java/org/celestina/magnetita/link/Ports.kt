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

    /** Reports one of this phone's players; an empty player name clears it. */
    fun sendMediaState(state: MediaState): Boolean

    /** Drives one of the desktop's players. */
    fun sendMediaCommand(player: String, button: Int?, seekMs: Long?, volume: Int?): Boolean

    /** Asks the desktop for its players' states. */
    fun requestMedia(): Boolean

    fun sendContacts(version: Long, contacts: List<PhoneContact>, removed: List<Long>, complete: Boolean): Boolean
    fun sendSmsConversations(list: List<SmsConversation>): Boolean
    fun sendSmsThread(thread: Long, messages: List<SmsMessage>): Boolean
    fun sendSmsReceived(thread: Long, message: SmsMessage): Boolean

    /** 0 ringing, 1 answered, 2 missed, 3 ended. */
    fun sendCallEvent(state: Int, number: String, name: String?, timestampMs: Long): Boolean

    /** Runs the desktop's registered command. */
    fun runCommand(id: Int): Boolean

    /** Pointer motion, unreliable and cheap; called often, off the main thread. */
    fun pointerMove(dx: Int, dy: Int): Boolean

    /** 0 left, 1 right, 2 middle. */
    fun pointerButton(button: Int, pressed: Boolean): Boolean

    /** Scroll in 1/120 wheel steps. */
    fun scroll(dx: Int, dy: Int): Boolean

    /** A Linux evdev key code, down or up. */
    fun key(code: Int, pressed: Boolean): Boolean

    fun typeText(text: String): Boolean

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
    val media: MediaState? = null,
    val mediaCommand: MediaCommand? = null,
    val contactsSince: Long? = null,
    val conversationsWanted: Boolean = false,
    val thread: Long? = null,
    val beforeMs: Long? = null,
    val limit: Int? = null,
    val smsSend: Boolean = false,
    val callAction: Int? = null,
    val commands: List<Pair<Int, String>>? = null,
    val commandId: Int? = null,
    val commandOk: Boolean? = null,
)

/** One contact as its vCard, versioned by the phone's last update. */
data class PhoneContact(val id: Long, val version: Long, val vcard: String)

data class SmsConversation(val thread: Long, val addresses: List<String>, val snippet: String, val timestampMs: Long, val unread: Int)

data class SmsMessage(val id: Long, val fromMe: Boolean, val address: String, val body: String, val timestampMs: Long, val attachmentMimes: List<String> = emptyList())

/** One player's state, either side's. */
data class MediaState(
    val player: String,
    val title: String,
    val artist: String,
    val album: String,
    val playing: Boolean,
    val positionMs: Long,
    val lengthMs: Long,
    val canSeek: Boolean,
    val canNext: Boolean,
    val canPrevious: Boolean,
    val volume: Int,
)

/** A button (0 play, 1 pause, 2 play/pause, 3 next, 4 previous, 5 stop), a seek or a volume. */
data class MediaCommand(val player: String, val button: Int?, val seekMs: Long?, val volume: Int?)

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

    /** One of this phone's players changed. */
    data class Media(val state: MediaState) : Outbound

    /** A button, seek or volume for one of the desktop's players. */
    data class MediaControl(val command: MediaCommand) : Outbound

    /** The desktop's players are wanted now. */
    data object MediaWanted : Outbound

    data class Contacts(val version: Long, val contacts: List<PhoneContact>, val removed: List<Long>, val complete: Boolean) : Outbound
    data class Conversations(val list: List<SmsConversation>) : Outbound
    data class Thread(val thread: Long, val messages: List<SmsMessage>) : Outbound
    data class Received(val thread: Long, val message: SmsMessage) : Outbound

    /** A call changed: 0 ringing, 1 answered, 2 missed, 3 ended. */
    data class Call(val state: Int, val number: String, val name: String?, val timestampMs: Long) : Outbound

    /** Run the desktop's registered command. */
    data class RunCommand(val id: Int) : Outbound
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
