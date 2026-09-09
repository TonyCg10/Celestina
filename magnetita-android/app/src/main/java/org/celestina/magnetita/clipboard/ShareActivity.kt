package org.celestina.magnetita.clipboard

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import org.celestina.magnetita.R
import org.celestina.magnetita.link.LinkService

/** The share target: text shared to Magnetita becomes the desktop's clipboard. */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = intent?.takeIf { it.action == Intent.ACTION_SEND }?.getStringExtra(Intent.EXTRA_TEXT)
        if (text.isNullOrBlank()) {
            Toast.makeText(this, R.string.clipboard_empty, Toast.LENGTH_SHORT).show()
        } else {
            LinkService.sendClipboard(this, text)
            Toast.makeText(this, R.string.clipboard_sent, Toast.LENGTH_SHORT).show()
        }
        finish()
    }
}
