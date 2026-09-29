package org.celestina.magnetita.share

import android.content.ContentValues
import android.content.Context
import android.os.Environment
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import java.io.File

/** Received files become entries of the public Downloads collection. */
object Downloads {
    /**
     * Copies `file` into Downloads under its name; the name shown, or null.
     * A copy that fails removes the pending entry it inserted (AND-9), so no
     * half-written, hidden download is left behind; the source stays.
     */
    fun publish(context: Context, file: File): String? {
        val extension = file.extension.lowercase()
        val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension) ?: "application/octet-stream"
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, file.name)
            put(MediaStore.Downloads.MIME_TYPE, mime)
            put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/Magnetita")
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val resolver = context.contentResolver
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values) ?: return null
        val copied = runCatching {
            resolver.openOutputStream(uri)?.use { out -> file.inputStream().use { it.copyTo(out) } } ?: error("no output stream")
            resolver.update(uri, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null)
        }
        if (copied.isFailure) {
            runCatching { resolver.delete(uri, null, null) }
            return null
        }
        file.delete()
        return file.name
    }
}
