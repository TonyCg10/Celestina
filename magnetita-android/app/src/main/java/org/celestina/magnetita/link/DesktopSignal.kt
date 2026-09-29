package org.celestina.magnetita.link

/**
 * What the desktop asked of the phone, one value per message. The core
 * (`magnetita-mobile`) decides every signal from the wire: the capability
 * and kind ids, the bodies, the defaults and the refusals stay on that side,
 * and [CoreConnector] only carries its typed signal across into this form,
 * which the pure controller and its JVM tests use.
 */
sealed interface DesktopSignal {
    data object Ring : DesktopSignal
    data object StopRinging : DesktopSignal
    data object BatteryRequested : DesktopSignal

    /** The desktop's clipboard now holds this text. */
    data class ClipboardText(val text: String) : DesktopSignal

    /** The desktop asks for this phone's clipboard; answered only in front. */
    data object ClipboardRequested : DesktopSignal

    /** The desktop dismissed one of this phone's notifications. */
    data class NotificationDismiss(val key: String) : DesktopSignal

    /** The desktop pressed button `action` of one of this phone's notifications. */
    data class NotificationAction(val key: String, val action: Int) : DesktopSignal

    /** The desktop answered one of this phone's notifications inline. */
    data class NotificationReply(val key: String, val text: String) : DesktopSignal

    /** The desktop offers a file. */
    data class ShareOffered(val transfer: Int, val name: String, val size: Long) : DesktopSignal

    /** The desktop accepted this phone's offer; send from `offset`. */
    data class ShareAccepted(val transfer: Int, val offset: Long) : DesktopSignal

    /** The desktop declined this phone's offer, or a transfer ended (`complete`). */
    data class ShareEnded(val transfer: Int, val complete: Boolean) : DesktopSignal

    /** A file the core finished receiving on this phone. */
    data class FileReceived(val transfer: Int, val path: String, val complete: Boolean) : DesktopSignal

    /** The desktop shared a URL or a snippet. */
    data class ShareText(val text: String) : DesktopSignal

    /** One of the desktop's players; an empty player name means it left. */
    data class DesktopMedia(val state: MediaState) : DesktopSignal

    /** The desktop drives one of this phone's players. */
    data class MediaControl(val command: MediaCommand) : DesktopSignal

    /** The desktop asks for this phone's players. */
    data object MediaRequested : DesktopSignal

    /** Contacts changed since `since` are wanted (0 for all). */
    data class ContactsRequested(val since: Long) : DesktopSignal
    data object ConversationsRequested : DesktopSignal
    data class ThreadRequested(val thread: Long, val beforeMs: Long?, val limit: Int) : DesktopSignal
    data class SmsSendRequested(val thread: Long, val body: String) : DesktopSignal

    /** 0 mute, 1 answer, 2 hang up. */
    data class CallCommand(val action: Int) : DesktopSignal

    /** The desktop's registered commands, ids and names. */
    data class Commands(val list: List<Pair<Int, String>>) : DesktopSignal

    /** How a run ended. */
    data class CommandResult(val id: Int, val ok: Boolean) : DesktopSignal

    /** The desktop wants to see this screen. */
    data class MirrorStart(val options: MirrorOptions) : DesktopSignal
    data object MirrorStop : DesktopSignal

    /** The desktop wants a key frame now. */
    data object MirrorKeyframe : DesktopSignal
    data class MirrorTouched(val touch: MirrorTouch) : DesktopSignal
    data class MirrorKey(val keycode: Int, val pressed: Boolean) : DesktopSignal

    /** 0 back, 1 home, 2 recents. */
    data class MirrorGlobal(val action: Int) : DesktopSignal

    /** The desktop browses the shared root. */
    data class Storage(val request: StorageRequest) : DesktopSignal

    /**
     * Refused by the core, never acted on: a capability this session did not
     * negotiate, or an offer past the phone's bounds, already declined.
     */
    data class Declined(val capability: Int, val kind: Int, val reason: String) : DesktopSignal

    /** A message this build does not handle, or whose body did not decode. */
    data class Unhandled(val capability: Int, val kind: Int) : DesktopSignal
}
