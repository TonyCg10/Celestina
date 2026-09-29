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

    /**
     * The newest conversations, at most [MAX_CONVERSATIONS]: one row per
     * thread from the threads provider, never a walk over every message
     * (AND-3). The addresses come from the canonical addresses the threads
     * name, and the unread counts from the unread inbox rows only.
     */
    fun sendConversations() {
        if (!canRead) {
            LinkService.send(context, Outbound.Conversations(emptyList()))
            return
        }
        val resolver = context.contentResolver
        val threads = ArrayList<Triple<Long, List<Long>, Pair<String, Long>>>()
        resolver.query(
            THREADS,
            arrayOf(Telephony.Threads._ID, Telephony.Threads.RECIPIENT_IDS, Telephony.Threads.SNIPPET, Telephony.Threads.DATE),
            null, null, "${Telephony.Threads.DATE} DESC LIMIT $MAX_CONVERSATIONS",
        )?.use { c ->
            while (threads.size < MAX_CONVERSATIONS && c.moveToNext()) {
                val recipients = c.getString(1).orEmpty().split(' ').mapNotNull { it.toLongOrNull() }
                threads += Triple(c.getLong(0), recipients, c.getString(2).orEmpty() to c.getLong(3))
            }
        }
        val wanted = threads.flatMap { it.second }.distinct()
        val addresses = HashMap<Long, String>()
        wanted.chunked(QUERY_ARGS).forEach { ids ->
            resolver.query(
                CANONICAL_ADDRESSES, arrayOf("_id", "address"),
                "_id IN (${ids.joinToString(",") { "?" }})", ids.map { it.toString() }.toTypedArray(), null,
            )?.use { c -> while (c.moveToNext()) addresses[c.getLong(0)] = c.getString(1).orEmpty() }
        }
        val unread = HashMap<Long, Int>()
        resolver.query(
            Telephony.Sms.CONTENT_URI, arrayOf(Telephony.Sms.THREAD_ID),
            "${Telephony.Sms.READ} = 0 AND ${Telephony.Sms.TYPE} = ?", arrayOf(Telephony.Sms.MESSAGE_TYPE_INBOX.toString()), null,
        )?.use { c -> while (c.moveToNext()) unread.merge(c.getLong(0), 1, Int::plus) }
        val list = threads.map { (thread, recipients, last) ->
            SmsConversation(thread, recipients.mapNotNull { addresses[it] }, last.first, last.second, unread[thread] ?: 0)
        }
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

    companion object {
        /** The threads provider, one row per conversation. */
        private val THREADS = Telephony.Threads.CONTENT_URI.buildUpon().appendQueryParameter("simple", "true").build()
        private val CANONICAL_ADDRESSES = android.net.Uri.parse("content://mms-sms/canonical-addresses")

        /** The wire's bound on one conversation list. */
        private const val MAX_CONVERSATIONS = 256

        /** Bind arguments per `IN` query, well under SQLite's limit. */
        private const val QUERY_ARGS = 256
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
