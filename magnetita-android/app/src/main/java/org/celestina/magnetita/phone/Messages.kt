package org.celestina.magnetita.phone

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.provider.Telephony
import android.telephony.SmsManager
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.Outbound
import org.celestina.magnetita.link.SmsConversation
import org.celestina.magnetita.link.SmsMessage

/**
 * The phone's SMS for the desktop, read from the provider; a send through
 * the system's SMS manager. Only the default messaging app may write the
 * provider, so a message sent from the desktop reaches the desktop's thread
 * through the wire and the phone's own app through its own path.
 */
class Messages(private val context: Context) {
    private val canRead get() = PhonePermissions.granted(context, android.Manifest.permission.READ_SMS)

    /** Every conversation, newest first: the last message of each thread. */
    fun sendConversations() {
        if (!canRead) {
            LinkService.send(context, Outbound.Conversations(emptyList()))
            return
        }
        val seen = LinkedHashMap<Long, SmsConversation>()
        val unread = HashMap<Long, Int>()
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI,
            arrayOf(Telephony.Sms.THREAD_ID, Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE, Telephony.Sms.READ, Telephony.Sms.TYPE),
            null, null, "${Telephony.Sms.DATE} DESC",
        )?.use { c ->
            while (c.moveToNext()) {
                val thread = c.getLong(0)
                val address = c.getString(1).orEmpty()
                if (c.getInt(4) == 0 && c.getInt(5) == Telephony.Sms.MESSAGE_TYPE_INBOX) unread[thread] = (unread[thread] ?: 0) + 1
                if (thread !in seen) seen[thread] = SmsConversation(thread, listOf(address), c.getString(2).orEmpty(), c.getLong(3), 0)
            }
        }
        val list = seen.values.map { it.copy(unread = unread[it.thread] ?: 0) }.take(256)
        LinkService.send(context, Outbound.Conversations(list))
    }

    /** A page of one thread, oldest first, older than `beforeMs` when given. */
    fun sendThread(thread: Long, beforeMs: Long?, limit: Int) {
        if (!canRead) return
        val where = if (beforeMs != null) "${Telephony.Sms.THREAD_ID} = ? AND ${Telephony.Sms.DATE} < ?" else "${Telephony.Sms.THREAD_ID} = ?"
        val args = if (beforeMs != null) arrayOf(thread.toString(), beforeMs.toString()) else arrayOf(thread.toString())
        val page = ArrayList<SmsMessage>()
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI,
            arrayOf(Telephony.Sms._ID, Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE, Telephony.Sms.TYPE),
            where, args, "${Telephony.Sms.DATE} DESC LIMIT ${limit.coerceIn(1, 256)}",
        )?.use { c ->
            while (c.moveToNext()) {
                val type = c.getInt(4)
                page += SmsMessage(c.getLong(0), type != Telephony.Sms.MESSAGE_TYPE_INBOX, c.getString(1).orEmpty(), c.getString(2).orEmpty(), c.getLong(3))
            }
        }
        LinkService.send(context, Outbound.Thread(thread, page.reversed()))
    }

    /** Sends `body` to the thread's address and tells the desktop it went. */
    fun send(thread: Long, body: String) {
        if (!PhonePermissions.granted(context, android.Manifest.permission.SEND_SMS)) return
        val address = addressOf(thread) ?: return
        val manager = context.getSystemService(SmsManager::class.java) ?: return
        val parts = manager.divideMessage(body)
        runCatching { manager.sendMultipartTextMessage(address, null, parts, null, null) }.onSuccess {
            LinkService.send(context, Outbound.Received(thread, SmsMessage(System.currentTimeMillis(), true, address, body, System.currentTimeMillis())))
        }
    }

    private fun addressOf(thread: Long): String? {
        if (!canRead) return null
        context.contentResolver.query(
            Telephony.Sms.CONTENT_URI, arrayOf(Telephony.Sms.ADDRESS),
            "${Telephony.Sms.THREAD_ID} = ?", arrayOf(thread.toString()), "${Telephony.Sms.DATE} DESC LIMIT 1",
        )?.use { c -> if (c.moveToFirst()) return c.getString(0) }
        return null
    }

    /** An SMS arrived: the system tells every listener, default app or not. */
    class Receiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action != Telephony.Sms.Intents.SMS_RECEIVED_ACTION) return
            val parts = Telephony.Sms.Intents.getMessagesFromIntent(intent) ?: return
            if (parts.isEmpty()) return
            val address = parts[0].displayOriginatingAddress ?: return
            val body = parts.joinToString("") { it.messageBody.orEmpty() }
            val thread = runCatching { Telephony.Threads.getOrCreateThreadId(context, address) }.getOrDefault(0L)
            val at = parts[0].timestampMillis
            LinkService.send(context, Outbound.Received(thread, SmsMessage(at, false, address, body, at)))
        }
    }
}
