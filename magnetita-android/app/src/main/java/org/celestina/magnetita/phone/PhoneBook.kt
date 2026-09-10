package org.celestina.magnetita.phone

import android.content.Context
import android.provider.ContactsContract
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.Outbound
import org.celestina.magnetita.link.PhoneContact

/**
 * The phone's contacts as vCard 4.0, one per contact with its name and
 * numbers, versioned by the contact's last update so the desktop asks only
 * for what changed. Deletions are not tracked by the provider in a way a
 * one-way reader can see, so a removed contact stays on the desktop until
 * the daemon restarts and asks from zero.
 */
class PhoneBook(private val context: Context) {
    /** Sends every contact changed after `since`, in pages of 200. */
    fun send(since: Long) {
        if (!PhonePermissions.granted(context, android.Manifest.permission.READ_CONTACTS)) {
            LinkService.send(context, Outbound.Contacts(since, emptyList(), emptyList(), true))
            return
        }
        val numbers = HashMap<Long, MutableList<String>>()
        context.contentResolver.query(
            ContactsContract.CommonDataKinds.Phone.CONTENT_URI,
            arrayOf(ContactsContract.CommonDataKinds.Phone.CONTACT_ID, ContactsContract.CommonDataKinds.Phone.NUMBER),
            null, null, null,
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getLong(0)
                val number = c.getString(1) ?: continue
                numbers.getOrPut(id) { mutableListOf() }.add(number)
            }
        }
        val contacts = ArrayList<PhoneContact>()
        var version = since
        context.contentResolver.query(
            ContactsContract.Contacts.CONTENT_URI,
            arrayOf(ContactsContract.Contacts._ID, ContactsContract.Contacts.DISPLAY_NAME, ContactsContract.Contacts.CONTACT_LAST_UPDATED_TIMESTAMP),
            "${ContactsContract.Contacts.CONTACT_LAST_UPDATED_TIMESTAMP} > ?",
            arrayOf(since.toString()),
            null,
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getLong(0)
                val name = c.getString(1) ?: continue
                val updated = c.getLong(2)
                if (updated > version) version = updated
                contacts += PhoneContact(id, updated, vcard(name, numbers[id].orEmpty()))
            }
        }
        val pages = contacts.chunked(200)
        if (pages.isEmpty()) {
            LinkService.send(context, Outbound.Contacts(version, emptyList(), emptyList(), true))
            return
        }
        pages.forEachIndexed { i, page ->
            LinkService.send(context, Outbound.Contacts(version, page, emptyList(), i == pages.lastIndex))
        }
    }

    companion object {
        /** A minimal vCard 4.0: the name and the numbers, nothing else leaves the phone. */
        fun vcard(name: String, numbers: List<String>): String = buildString {
            append("BEGIN:VCARD\r\nVERSION:4.0\r\n")
            append("FN:").append(escape(name)).append("\r\n")
            numbers.forEach { append("TEL:").append(it.replace("\\s".toRegex(), "")).append("\r\n") }
            append("END:VCARD\r\n")
        }

        private fun escape(text: String) = text.replace("\\", "\\\\").replace("\n", "\\n").replace(",", "\\,").replace(";", "\\;")
    }
}
