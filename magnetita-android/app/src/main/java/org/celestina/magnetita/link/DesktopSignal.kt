package org.celestina.magnetita.link

/**
 * What an envelope from the desktop asks of the phone, decided from the
 * capability and kind ids of the wire document (`magnetita/docs/protocol.md`).
 * Pure, so it is tested on the JVM; the bodies stay opaque here because
 * the ones this screen set handles are empty.
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
    data class Other(val capability: Int, val kind: Int) : DesktopSignal

    companion object {
        const val CAPABILITY_BATTERY = 1
        const val CAPABILITY_CLIPBOARD = 2
        const val CAPABILITY_NOTIFICATIONS = 3
        const val CAPABILITY_FIND = 4
        const val CAPABILITY_SHARE = 5
        const val CAPABILITY_MEDIA = 6
        const val CAPABILITY_SMS = 10
        const val CAPABILITY_CONTACTS = 11
        const val CAPABILITY_TELEPHONY = 12
        const val KIND_MEDIA_STATE = 1
        const val KIND_MEDIA_COMMAND = 2
        const val KIND_MEDIA_REQUEST = 3
        const val KIND_SHARE_OFFER = 1
        const val KIND_SHARE_ACCEPT = 2
        const val KIND_SHARE_REJECT = 3
        const val KIND_SHARE_DONE = 4
        const val KIND_SHARE_TEXT = 5
        /** The core's own kind for a file it finished receiving; not on the wire. */
        const val KIND_FILE_RECEIVED = 100
        const val KIND_NOTIFICATION_DISMISSED = 2
        const val KIND_NOTIFICATION_ACTION = 3
        const val KIND_NOTIFICATION_REPLY = 4
        const val KIND_BATTERY_REQUEST = 2
        const val KIND_CLIPBOARD_TEXT = 1
        const val KIND_CLIPBOARD_REQUEST = 2
        const val KIND_FIND_RING = 1
        const val KIND_FIND_STOP = 2

        fun of(event: LinkEvent): DesktopSignal = when (event.capability to event.kind) {
            CAPABILITY_FIND to KIND_FIND_RING -> Ring
            CAPABILITY_FIND to KIND_FIND_STOP -> StopRinging
            CAPABILITY_BATTERY to KIND_BATTERY_REQUEST -> BatteryRequested
            CAPABILITY_CLIPBOARD to KIND_CLIPBOARD_TEXT ->
                event.text?.let { ClipboardText(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_CLIPBOARD to KIND_CLIPBOARD_REQUEST -> ClipboardRequested
            CAPABILITY_NOTIFICATIONS to KIND_NOTIFICATION_DISMISSED ->
                event.key?.let { NotificationDismiss(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_NOTIFICATIONS to KIND_NOTIFICATION_ACTION ->
                if (event.key != null && event.action != null) NotificationAction(event.key, event.action) else Other(event.capability, event.kind)
            CAPABILITY_NOTIFICATIONS to KIND_NOTIFICATION_REPLY ->
                if (event.key != null && event.text != null) NotificationReply(event.key, event.text) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_SHARE_OFFER ->
                if (event.transfer != null && event.text != null && event.size != null) ShareOffered(event.transfer, event.text, event.size) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_SHARE_ACCEPT ->
                if (event.transfer != null) ShareAccepted(event.transfer, event.offset ?: 0) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_SHARE_REJECT ->
                if (event.transfer != null) ShareEnded(event.transfer, false) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_SHARE_DONE ->
                if (event.transfer != null) ShareEnded(event.transfer, event.complete ?: false) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_FILE_RECEIVED ->
                if (event.transfer != null && event.path != null) FileReceived(event.transfer, event.path, event.complete ?: false) else Other(event.capability, event.kind)
            CAPABILITY_SHARE to KIND_SHARE_TEXT ->
                event.text?.let { ShareText(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_MEDIA to KIND_MEDIA_STATE ->
                event.media?.let { DesktopMedia(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_MEDIA to KIND_MEDIA_COMMAND ->
                event.mediaCommand?.let { MediaControl(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_MEDIA to KIND_MEDIA_REQUEST -> MediaRequested
            CAPABILITY_CONTACTS to 1 -> ContactsRequested(event.contactsSince ?: 0)
            CAPABILITY_SMS to 1 -> if (event.conversationsWanted) ConversationsRequested else Other(event.capability, event.kind)
            CAPABILITY_SMS to 2 ->
                if (event.thread != null) ThreadRequested(event.thread, event.beforeMs, event.limit ?: 50) else Other(event.capability, event.kind)
            CAPABILITY_SMS to 4 ->
                if (event.smsSend && event.thread != null && event.text != null) SmsSendRequested(event.thread, event.text) else Other(event.capability, event.kind)
            CAPABILITY_TELEPHONY to 2 -> event.callAction?.let { CallCommand(it) } ?: Other(event.capability, event.kind)
            else -> Other(event.capability, event.kind)
        }
    }
}

/** The pairing links the scanner accepts; the core parses the rest. */
object PairLink {
    const val PREFIX = "magnetita://pair?"

    fun accepts(text: String?): Boolean = text != null && text.startsWith(PREFIX) && text.length <= 512
}
