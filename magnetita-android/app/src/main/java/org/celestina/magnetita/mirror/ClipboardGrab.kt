package org.celestina.magnetita.mirror

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.content.Context
import android.graphics.PixelFormat
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import org.celestina.magnetita.link.LinkService

/**
 * Reads the clipboard from the background, the one way Android allows it
 * without `adb`: an accessibility overlay of one pixel, focusable but not
 * touchable, takes window focus for an instant; a focused window may read
 * the clipboard. The text goes to the link's clipboard path, which
 * applies the same policy as the tile and the share target, and the
 * overlay is gone before the person notices it.
 */
object ClipboardGrab {
    @Volatile private var busy = false

    fun read(service: AccessibilityService) {
        if (busy) return
        busy = true
        val manager = service.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val view = View(service)
        val params = WindowManager.LayoutParams(
            1, 1,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT,
        ).apply { gravity = Gravity.TOP or Gravity.START }
        var done = false
        val finish = {
            if (!done) {
                done = true
                runCatching { manager.removeView(view) }
                busy = false
            }
        }
        view.viewTreeObserver.addOnWindowFocusChangeListener { focused ->
            if (!focused || done) return@addOnWindowFocusChangeListener
            val clipboard = service.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val text = runCatching { clipboard.primaryClip?.getItemAt(0)?.coerceToText(service)?.toString() }.getOrNull()
            android.util.Log.i(TAG, "clipboard read: ${text?.length ?: -1} chars")
            if (!text.isNullOrEmpty()) LinkService.sendClipboard(service, text)
            finish()
        }
        if (runCatching { manager.addView(view, params) }.isFailure) {
            busy = false
            return
        }
        // Never hold the focus: if it never arrives, let go anyway.
        view.postDelayed({ finish() }, 700)
    }

    private const val TAG = "ClipboardGrab"
}
