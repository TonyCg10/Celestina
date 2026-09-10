package org.celestina.magnetita.storage

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.LiveSession
import org.celestina.magnetita.link.StorageEntry
import org.celestina.magnetita.link.StorageRequest
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.RandomAccessFile

/**
 * The desktop's browse of this phone through the document tree the person
 * chose once (the storage access framework's tree grant, persisted). Each
 * request is answered on the storage thread; nothing here runs on the loop.
 * The framework's limits the wire document states: a listing is one query
 * of the whole directory, and a move between providers is refused.
 */
class PhoneStorage(private val context: Context) {
    private val prefs by lazy { context.getSharedPreferences("storage", Context.MODE_PRIVATE) }

    /** The shared tree, or null until the person picks one. */
    fun tree(): Uri? = prefs.getString(KEY_TREE, null)?.let(Uri::parse)?.takeIf { hasGrant(it) }

    /** Whether the whole phone is shared: the all-files grant the settings give. */
    fun wholePhone(): Boolean = android.os.Environment.isExternalStorageManager()

    fun available(): Boolean = wholePhone() || tree() != null

    private fun hasGrant(tree: Uri): Boolean =
        context.contentResolver.persistedUriPermissions.any { it.uri == tree && it.isReadPermission && it.isWritePermission }

    /** Keeps the tree the picker returned. */
    fun grant(tree: Uri) {
        context.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        prefs.edit().putString(KEY_TREE, tree.toString()).apply()
    }

    /** Answers one request on `live`. */
    fun answer(request: StorageRequest, live: LiveSession) {
        if (wholePhone()) {
            answerFiles(android.os.Environment.getExternalStorageDirectory(), request, live)
            return
        }
        val tree = tree()
        if (tree == null) {
            fail(request, live, "no shared root")
            return
        }
        val treeId = DocumentsContract.getTreeDocumentId(tree)
        val id = DocumentPaths.childId(treeId, request.path)
        if (id == null) {
            fail(request, live, "bad path")
            return
        }
        val uri = DocumentsContract.buildDocumentUriUsingTree(tree, id)
        runCatching {
            when (request.kind) {
                StorageRequest.LIST -> {
                    val all = list(tree, id)
                    val page = all.drop(request.offset.toInt()).take(PAGE)
                    live.sendListing(request.request, page, all.size > request.offset.toInt() + PAGE, "")
                }
                StorageRequest.STAT -> live.sendStatReply(request.request, stat(uri))
                StorageRequest.READ -> live.sendData(request.request, read(uri, request.offset, request.len), "")
                StorageRequest.WRITE -> {
                    val target = if (stat(uri) == null) create(tree, treeId, request.path, false) else uri
                    write(target, request.offset, request.bytes, request.truncate)
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.MKDIR -> {
                    create(tree, treeId, request.path, true)
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.RENAME -> {
                    rename(tree, treeId, request.path, request.to)
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.DELETE -> {
                    if (!DocumentsContract.deleteDocument(context.contentResolver, uri)) error("not deleted")
                    live.sendDone(request.request, true, "")
                }
                else -> error("unknown request")
            }
        }.onFailure { fail(request, live, it.message ?: "failed") }
    }

    /** The same requests over plain files when the whole phone is shared. */
    private fun answerFiles(root: File, request: StorageRequest, live: LiveSession) {
        if (!DocumentPaths.valid(request.path) || !DocumentPaths.valid(request.to)) {
            fail(request, live, "bad path")
            return
        }
        val file = if (request.path.isEmpty()) root else File(root, request.path)
        runCatching {
            when (request.kind) {
                StorageRequest.LIST -> {
                    val all = (file.listFiles() ?: error("not a directory")).map { entryOf(it) }.sortedBy { it.name }
                    val page = all.drop(request.offset.toInt()).take(PAGE)
                    live.sendListing(request.request, page, all.size > request.offset.toInt() + PAGE, "")
                }
                StorageRequest.STAT -> live.sendStatReply(request.request, if (file.exists()) entryOf(file) else null)
                StorageRequest.READ -> RandomAccessFile(file, "r").use { raf ->
                    raf.seek(request.offset)
                    val buffer = ByteArray(request.len)
                    var filled = 0
                    while (filled < buffer.size) {
                        val n = raf.read(buffer, filled, buffer.size - filled)
                        if (n < 0) break
                        filled += n
                    }
                    live.sendData(request.request, buffer.copyOf(filled), "")
                }
                StorageRequest.WRITE -> {
                    RandomAccessFile(file, "rw").use { raf ->
                        raf.seek(request.offset)
                        raf.write(request.bytes)
                        if (request.truncate) raf.setLength(request.offset + request.bytes.size)
                    }
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.MKDIR -> {
                    if (!file.mkdir()) error("not created")
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.RENAME -> {
                    if (!file.renameTo(File(root, request.to))) error("not moved")
                    live.sendDone(request.request, true, "")
                }
                StorageRequest.DELETE -> {
                    if (!file.delete()) error("not deleted")
                    live.sendDone(request.request, true, "")
                }
                else -> error("unknown request")
            }
        }.onFailure { fail(request, live, it.message ?: "failed") }
    }

    private fun entryOf(file: File): StorageEntry =
        StorageEntry(file.name, file.isDirectory, if (file.isDirectory) 0 else file.length(), file.lastModified())

    private fun fail(request: StorageRequest, live: LiveSession, why: String) {
        when (request.kind) {
            StorageRequest.LIST -> live.sendListing(request.request, emptyList(), false, why)
            StorageRequest.STAT -> live.sendStatReply(request.request, null)
            StorageRequest.READ -> live.sendData(request.request, ByteArray(0), why)
            else -> live.sendDone(request.request, false, why)
        }
    }

    private fun list(tree: Uri, id: String): List<StorageEntry> {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, id)
        val out = ArrayList<StorageEntry>()
        context.contentResolver.query(children, COLUMNS, null, null, null)?.use { c ->
            while (c.moveToNext()) {
                val name = c.getString(1) ?: continue
                if ('/' in name || name == "." || name == "..") continue
                val dir = c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                out += StorageEntry(name, dir, if (dir) 0 else c.getLong(3), c.getLong(4))
            }
        }
        out.sortBy { it.name }
        return out
    }

    private fun stat(uri: Uri): StorageEntry? =
        runCatching {
            context.contentResolver.query(uri, COLUMNS, null, null, null)?.use { c ->
                if (!c.moveToFirst()) return null
                val dir = c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                StorageEntry(c.getString(1) ?: "", dir, if (dir) 0 else c.getLong(3), c.getLong(4))
            }
        }.getOrNull()

    private fun read(uri: Uri, offset: Long, len: Int): ByteArray {
        context.contentResolver.openFileDescriptor(uri, "r")?.use { fd ->
            FileInputStream(fd.fileDescriptor).use { input ->
                input.channel.position(offset)
                val buffer = ByteArray(len)
                var filled = 0
                while (filled < len) {
                    val n = input.read(buffer, filled, len - filled)
                    if (n < 0) break
                    filled += n
                }
                return buffer.copyOf(filled)
            }
        } ?: error("cannot open")
    }

    private fun write(uri: Uri, offset: Long, bytes: ByteArray, truncate: Boolean) {
        context.contentResolver.openFileDescriptor(uri, "rw")?.use { fd ->
            FileOutputStream(fd.fileDescriptor).use { out ->
                out.channel.position(offset)
                out.write(bytes)
                if (truncate) out.channel.truncate(offset + bytes.size)
            }
        } ?: error("cannot open for writing")
    }

    private fun create(tree: Uri, treeId: String, path: String, dir: Boolean): Uri {
        val (parent, name) = DocumentPaths.split(path)
        val parentId = DocumentPaths.childId(treeId, parent) ?: error("bad path")
        val parentUri = DocumentsContract.buildDocumentUriUsingTree(tree, parentId)
        val mime = if (dir) DocumentsContract.Document.MIME_TYPE_DIR else mimeOf(name)
        return DocumentsContract.createDocument(context.contentResolver, parentUri, mime, name) ?: error("not created")
    }

    private fun rename(tree: Uri, treeId: String, from: String, to: String) {
        val (fromParent, fromName) = DocumentPaths.split(from)
        val (toParent, toName) = DocumentPaths.split(to)
        var uri = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentPaths.childId(treeId, from) ?: error("bad path"))
        if (fromParent != toParent) {
            val source = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentPaths.childId(treeId, fromParent) ?: error("bad path"))
            val target = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentPaths.childId(treeId, toParent) ?: error("bad path"))
            uri = DocumentsContract.moveDocument(context.contentResolver, uri, source, target) ?: error("not moved")
        }
        if (fromName != toName) {
            DocumentsContract.renameDocument(context.contentResolver, uri, toName) ?: error("not renamed")
        }
    }

    private fun mimeOf(name: String): String {
        val extension = name.substringAfterLast('.', "").lowercase()
        return android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension) ?: "application/octet-stream"
    }

    companion object {
        private const val KEY_TREE = "tree"
        private const val PAGE = 256
        private val COLUMNS = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )

        /** The system page where the person grants access to all files. */
        fun allFilesIntent(context: Context): Intent =
            Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:" + context.packageName))

        /** The picker's result: keep the grant and tell the desktop. */
        fun granted(context: Context, tree: Uri) {
            PhoneStorage(context).grant(tree)
            LinkService.storageGranted(context)
        }
    }
}
