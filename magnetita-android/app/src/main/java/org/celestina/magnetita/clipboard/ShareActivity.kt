package org.celestina.magnetita.clipboard

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.core.content.IntentCompat
import org.celestina.magnetita.link.Outbound
import android.os.Bundle
import android.widget.Toast
import org.celestina.magnetita.R
import org.celestina.magnetita.link.LinkService

/** The share target: text becomes the desktop's clipboard; files travel on their own streams. */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val send = intent?.takeIf { it.action == Intent.ACTION_SEND || it.action == Intent.ACTION_SEND_MULTIPLE }
        val streams: List<Uri> = when (send?.action) {
            Intent.ACTION_SEND -> listOfNotNull(IntentCompat.getParcelableExtra(send, Intent.EXTRA_STREAM, Uri::class.java))
            Intent.ACTION_SEND_MULTIPLE -> IntentCompat.getParcelableArrayListExtra(send, Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
            else -> emptyList()
        }
        val text = send?.getStringExtra(Intent.EXTRA_TEXT)
        when {
            streams.isNotEmpty() -> {
                streams.forEach { uri -> describe(uri)?.let { LinkService.send(this, it) } }
                Toast.makeText(this, resources.getQuantityString(R.plurals.files_offered, streams.size, streams.size), Toast.LENGTH_SHORT).show()
            }
            !text.isNullOrBlank() -> {
                LinkService.sendClipboard(this, text)
                Toast.makeText(this, R.string.clipboard_sent, Toast.LENGTH_SHORT).show()
            }
            else -> Toast.makeText(this, R.string.clipboard_empty, Toast.LENGTH_SHORT).show()
        }
        finish()
    }

    /** Name, size and type from the provider; null when it will not say. */
    private fun describe(uri: Uri): Outbound.File? {
        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { cursor ->
            if (!cursor.moveToFirst()) return null
            val name = cursor.getString(cursor.getColumnIndexOrThrow(OpenableColumns.DISPLAY_NAME)) ?: return null
            val sizeIndex = cursor.getColumnIndexOrThrow(OpenableColumns.SIZE)
            val size = if (cursor.isNull(sizeIndex)) -1L else cursor.getLong(sizeIndex)
            if (size < 0) return null
            return Outbound.File(uri.toString(), name, size, mime)
        }
        return null
    }
}
