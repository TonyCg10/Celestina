package org.celestina.magnetita.link

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.magnetita_mobile.MobileNotification
import uniffi.magnetita_mobile.MobilePhone
import uniffi.magnetita_mobile.MobileSession

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
            ),
        )
    }.isSuccess

    override fun sendNotificationGone(key: String): Boolean =
        runCatching { inner.sendNotificationGone(key) }.isSuccess

    override suspend fun next(timeoutMs: Long): LinkEvent? = withContext(Dispatchers.IO) {
        inner.next(timeoutMs.toULong())?.let { LinkEvent(it.capability.toInt(), it.kind.toInt(), it.description, it.text, it.key, it.action?.toInt()) }
    }

    override fun close(reason: String) = inner.close(reason)
}
