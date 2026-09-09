package org.celestina.magnetita.link

/**
 * What this phone sends as clipboard, and what it writes when the desktop
 * sends one. Android lets an app read the clipboard only while it has the
 * focus, so the outbound half is fed by the screens (in front), the tile
 * and the share target; this class only decides whether a value is worth
 * the wire: text, not empty, within the protocol's bound, not the value
 * last exchanged in either direction (which would echo forever).
 */
class ClipboardPolicy {
    private var lastExchanged: String? = null

    /** True when `text` should go to the desktop; records it if so. */
    fun offer(text: String?): Boolean {
        if (text == null || text.isBlank() || text.toByteArray().size > MAX_BYTES || text.contains('\u0000')) return false
        if (text == lastExchanged) return false
        lastExchanged = text
        return true
    }

    /** The desktop sent `text`: remember it so it is not sent straight back. */
    fun received(text: String) {
        lastExchanged = text
    }

    companion object {
        /** The protocol's clipboard bound, 256 KiB. */
        const val MAX_BYTES = 256 * 1024
    }
}
