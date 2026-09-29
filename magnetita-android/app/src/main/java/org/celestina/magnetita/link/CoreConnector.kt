package org.celestina.magnetita.link

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.magnetita_mobile.MobileContact
import uniffi.magnetita_mobile.MobileEntry
import uniffi.magnetita_mobile.MobileConversation
import uniffi.magnetita_mobile.MobileMediaCommand
import uniffi.magnetita_mobile.MobileSmsMessage
import uniffi.magnetita_mobile.MobileMediaState
import uniffi.magnetita_mobile.MobileNotification
import uniffi.magnetita_mobile.MobilePhone
import uniffi.magnetita_mobile.MobileSession
import uniffi.magnetita_mobile.ReconnectSchedule
import uniffi.magnetita_mobile.DesktopSignal as CoreSignal
import uniffi.magnetita_mobile.MobileStorageRequest

/** The [Connector] over the Rust core: every rule stays on the other side. */
class CoreConnector(private val phone: MobilePhone) : Connector {
    override fun pinnedIds(): List<String> = phone.pinned().map { it.deviceId }

    override fun connect(address: String): Result<LiveSession> =
        runCatching { CoreSession(phone.connect(address)) }

    override fun pair(uri: String): Result<LiveSession> =
        runCatching { CoreSession(phone.pair(uri)) }
}

private class CoreSession(private val inner: MobileSession) : LiveSession {
    override val desktopId: String = inner.desktopId()
    override val desktopName: String = inner.desktopName()
    override val gone: Boolean get() = inner.isClosed()

    override fun reportBattery(level: Int, charging: Boolean): Boolean =
        runCatching { inner.reportBattery(level.coerceIn(0, 100).toUByte(), charging) }.isSuccess

    override fun sendClipboard(text: String): Boolean =
        runCatching { inner.sendClipboard(text) }.isSuccess

    override fun sendNotification(note: PhoneNotification): Boolean = runCatching {
        inner.sendNotification(
            MobileNotification(
                key = note.key,
                appName = note.appName,
                title = note.title,
                body = note.body,
                timestampMs = note.timestampMs.toULong(),
                replyable = note.replyable,
                actions = note.actions,
                icon = note.icon,
                media = note.media,
            ),
        )
    }.isSuccess

    override fun sendNotificationGone(key: String): Boolean =
        runCatching { inner.sendNotificationGone(key) }.isSuccess

    override fun offerFile(name: String, size: Long, mime: String): Int? =
        runCatching { inner.offerFile(name, size.toULong(), mime).toInt() }.getOrNull()

    override fun writeTransfer(transfer: Int, bytes: ByteArray): Boolean =
        runCatching { inner.writeTransfer(transfer.toUInt(), bytes) }.isSuccess

    override fun finishTransfer(transfer: Int): Boolean =
        runCatching { inner.finishTransfer(transfer.toUInt()) }.isSuccess

    override fun abandonTransfer(transfer: Int): Boolean =
        runCatching { inner.abandonTransfer(transfer.toUInt()) }.isSuccess

    override fun acceptFile(transfer: Int, dir: String, usableBytes: Long): Boolean =
        runCatching { inner.acceptFile(transfer.toUInt(), dir, usableBytes.coerceAtLeast(0).toULong()) }.isSuccess

    override fun rejectFile(transfer: Int): Boolean =
        runCatching { inner.rejectFile(transfer.toUInt()) }.isSuccess

    override fun openStream(id: Int): Boolean = runCatching { inner.openStream(id.toUInt()) }.isSuccess
    override fun closeStream(id: Int) { runCatching { inner.closeStream(id.toUInt()) } }
    override fun sendMirrorStarted(width: Int, height: Int, codec: Int, audio: Boolean): Boolean =
        runCatching { inner.sendMirrorStarted(width.toUShort(), height.toUShort(), codec.toUByte(), audio) }.isSuccess
    override fun sendMirrorStop(): Boolean = runCatching { inner.sendMirrorStop() }.isSuccess

    override fun sendStorageState(available: Boolean): Boolean = runCatching { inner.sendStorageState(available) }.isSuccess
    override fun sendListing(request: Int, entries: List<StorageEntry>, more: Boolean, error: String): Boolean =
        runCatching { inner.sendListing(request.toUInt(), entries.map { e -> e.toMobile() }, more, error) }.isSuccess
    override fun sendStatReply(request: Int, entry: StorageEntry?): Boolean =
        runCatching { inner.sendStatReply(request.toUInt(), entry?.toMobile()) }.isSuccess
    override fun sendData(request: Int, bytes: ByteArray, error: String): Boolean =
        runCatching { inner.sendData(request.toUInt(), bytes, error) }.isSuccess
    override fun sendDone(request: Int, ok: Boolean, error: String): Boolean =
        runCatching { inner.sendDone(request.toUInt(), ok, error) }.isSuccess

    private fun StorageEntry.toMobile(): MobileEntry = MobileEntry(name, dir, size.toULong(), mtimeMs.toULong())

    override fun sendMediaState(state: MediaState): Boolean = runCatching {
        inner.sendMediaState(
            MobileMediaState(
                state.player, state.title, state.artist, state.album, state.playing,
                state.positionMs.coerceAtLeast(0).toULong(), state.lengthMs.coerceAtLeast(0).toULong(),
                state.canSeek, state.canNext, state.canPrevious, state.volume.coerceIn(0, 100).toUByte(),
            ),
        )
    }.isSuccess

    override fun sendMediaCommand(player: String, button: Int?, seekMs: Long?, volume: Int?): Boolean = runCatching {
        inner.sendMediaCommand(MobileMediaCommand(player, button?.toUByte(), seekMs?.toULong(), volume?.coerceIn(0, 100)?.toUByte()))
    }.isSuccess

    override fun requestMedia(): Boolean = runCatching { inner.requestMedia() }.isSuccess

    override fun sendContacts(version: Long, contacts: List<PhoneContact>, removed: List<Long>, complete: Boolean): Boolean = runCatching {
        inner.sendContacts(version.toULong(), contacts.map { MobileContact(it.id.toULong(), it.version.toULong(), it.vcard) }, removed.map { it.toULong() }, complete)
    }.isSuccess

    override fun sendSmsConversations(list: List<SmsConversation>): Boolean = runCatching {
        inner.sendSmsConversations(list.map { MobileConversation(it.thread.toULong(), it.addresses, it.snippet, it.timestampMs.toULong(), it.unread.coerceIn(0, 65535).toUShort()) })
    }.isSuccess

    override fun sendSmsThread(thread: Long, messages: List<SmsMessage>): Boolean = runCatching {
        inner.sendSmsThread(thread.toULong(), messages.map { it.toMobile() })
    }.isSuccess

    override fun sendSmsReceived(thread: Long, message: SmsMessage): Boolean = runCatching {
        inner.sendSmsReceived(thread.toULong(), message.toMobile())
    }.isSuccess

    override fun sendCallEvent(state: Int, number: String, name: String?, timestampMs: Long): Boolean = runCatching {
        inner.sendCallEvent(state.toUByte(), number, name, timestampMs.toULong())
    }.isSuccess

    override fun runCommand(id: Int): Boolean = runCatching { inner.runCommand(id.toUInt()) }.isSuccess
    override fun pointerMove(dx: Int, dy: Int): Boolean = runCatching { inner.pointerMove(dx.coerceIn(-32768, 32767).toShort(), dy.coerceIn(-32768, 32767).toShort()) }.isSuccess
    override fun pointerButton(button: Int, pressed: Boolean): Boolean = runCatching { inner.pointerButton(button.toUByte(), pressed) }.isSuccess
    override fun scroll(dx: Int, dy: Int): Boolean = runCatching { inner.scroll(dx.coerceIn(-32768, 32767).toShort(), dy.coerceIn(-32768, 32767).toShort()) }.isSuccess
    override fun key(code: Int, pressed: Boolean): Boolean = runCatching { inner.key(code.toUShort(), pressed) }.isSuccess
    override fun typeText(text: String): Boolean = runCatching { inner.typeText(text) }.isSuccess

    private fun SmsMessage.toMobile() = MobileSmsMessage(id.toULong(), fromMe, address, body, timestampMs.toULong(), attachmentMimes)

    override suspend fun next(timeoutMs: Long): DesktopEvent? = withContext(Dispatchers.IO) {
        inner.next(timeoutMs.toULong())?.let { DesktopEvent(it.description, it.signal.toApp()) }
    }

    override fun close(reason: String) = inner.close(reason)
}

/** The core's typed signal in the controller's form: a carry, no decision. */
private fun CoreSignal.toApp(): DesktopSignal = when (this) {
    CoreSignal.Ring -> DesktopSignal.Ring
    CoreSignal.StopRinging -> DesktopSignal.StopRinging
    CoreSignal.BatteryRequested -> DesktopSignal.BatteryRequested
    is CoreSignal.ClipboardText -> DesktopSignal.ClipboardText(text)
    CoreSignal.ClipboardRequested -> DesktopSignal.ClipboardRequested
    is CoreSignal.NotificationDismiss -> DesktopSignal.NotificationDismiss(key)
    is CoreSignal.NotificationAction -> DesktopSignal.NotificationAction(key, action.toInt())
    is CoreSignal.NotificationReply -> DesktopSignal.NotificationReply(key, text)
    is CoreSignal.ShareOffered -> DesktopSignal.ShareOffered(transfer.toInt(), name, size.toLong())
    is CoreSignal.ShareAccepted -> DesktopSignal.ShareAccepted(transfer.toInt(), offset.toLong())
    is CoreSignal.ShareEnded -> DesktopSignal.ShareEnded(transfer.toInt(), complete)
    is CoreSignal.FileReceived -> DesktopSignal.FileReceived(transfer.toInt(), path, complete)
    is CoreSignal.ShareText -> DesktopSignal.ShareText(text)
    is CoreSignal.DesktopMedia -> DesktopSignal.DesktopMedia(
        state.let { m -> MediaState(m.player, m.title, m.artist, m.album, m.playing, m.positionMs.toLong(), m.lengthMs.toLong(), m.canSeek, m.canNext, m.canPrevious, m.volume.toInt()) },
    )
    is CoreSignal.MediaControl -> DesktopSignal.MediaControl(
        command.let { c -> MediaCommand(c.player, c.button?.toInt(), c.seekMs?.toLong(), c.volume?.toInt()) },
    )
    CoreSignal.MediaRequested -> DesktopSignal.MediaRequested
    is CoreSignal.ContactsRequested -> DesktopSignal.ContactsRequested(since.toLong())
    CoreSignal.ConversationsRequested -> DesktopSignal.ConversationsRequested
    is CoreSignal.ThreadRequested -> DesktopSignal.ThreadRequested(thread.toLong(), beforeMs?.toLong(), limit.toInt())
    is CoreSignal.SmsSendRequested -> DesktopSignal.SmsSendRequested(thread.toLong(), body)
    is CoreSignal.CallCommand -> DesktopSignal.CallCommand(action.toInt())
    is CoreSignal.Commands -> DesktopSignal.Commands(list.map { it.id.toInt() to it.name })
    is CoreSignal.CommandResult -> DesktopSignal.CommandResult(id.toInt(), ok)
    is CoreSignal.MirrorStart -> DesktopSignal.MirrorStart(
        options.let { m -> MirrorOptions(m.maxSize.toInt(), m.fps.toInt(), m.bitrateKbps.toInt(), m.codec.toInt(), m.audio, m.screenOff) },
    )
    CoreSignal.MirrorStop -> DesktopSignal.MirrorStop
    CoreSignal.MirrorKeyframe -> DesktopSignal.MirrorKeyframe
    is CoreSignal.MirrorTouched -> DesktopSignal.MirrorTouched(touch.let { t -> MirrorTouch(t.phase.toInt(), t.x.toInt(), t.y.toInt(), t.pointer.toInt()) })
    is CoreSignal.MirrorKey -> DesktopSignal.MirrorKey(keycode.toInt(), pressed)
    is CoreSignal.MirrorGlobal -> DesktopSignal.MirrorGlobal(action.toInt())
    is CoreSignal.Storage -> DesktopSignal.Storage(request.toApp())
    is CoreSignal.Declined -> DesktopSignal.Declined(capability.toInt(), kind.toInt(), reason)
    is CoreSignal.Unhandled -> DesktopSignal.Unhandled(capability.toInt(), kind.toInt())
}

private fun MobileStorageRequest.toApp(): StorageRequest =
    StorageRequest(kind.toInt(), request.toInt(), path, to, offset.toLong(), len.toInt(), bytes, truncate)

/** The core's reconnection schedule, the desktop link's own numbers. */
class CoreSchedule : RetrySchedule {
    private val inner = ReconnectSchedule()

    override fun nextDelayMs(): Long = inner.nextDelayMs().toLong()

    override fun reset() = inner.reset()
}
