package org.celestina.magnetita.clipboard

import android.app.Activity
import android.content.ClipboardManager
import android.widget.Toast
import org.celestina.magnetita.R
import org.celestina.magnetita.link.LinkService

/**
 * A window with nothing in it, so the clipboard can be read: Android hands
 * the clipboard only to the app in front, and the quick-settings tile is
 * not in front. The tile opens this, it reads the clip once it has the
 * focus, sends, and closes.
 */
class SendClipboardActivity : Activity() {
    private var sent = false

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || sent) return
        sent = true
        val clip = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
        val text = clip.primaryClip?.getItemAt(0)?.coerceToText(this)?.toString()
        if (text.isNullOrBlank()) {
            Toast.makeText(this, R.string.clipboard_empty, Toast.LENGTH_SHORT).show()
        } else {
            LinkService.sendClipboard(this, text)
            Toast.makeText(this, R.string.clipboard_sent, Toast.LENGTH_SHORT).show()
        }
        finish()
    }
}
