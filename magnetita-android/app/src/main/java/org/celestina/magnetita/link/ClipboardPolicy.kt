package org.celestina.magnetita.link

/**
 * What this phone sends as clipboard, and what it writes when the desktop
 * sends one. Android lets an app read the clipboard only while it has the
 * focus, so the outbound half is fed by the screens (in front), the tile
 * and the share target; this class only keeps the echo rule: nothing blank,
 * and never the value last exchanged in either direction (which would echo
 * forever). Whether a text may go at all is the wire's one rule (`syncable`,
 * the core's `clipboardSyncable`), so its bound is never copied here.
 */
class ClipboardPolicy(private val syncable: (String) -> Boolean) {
    /** What to do with an offered value. */
    enum class Offer {
        /** Send it; it is recorded as the last exchanged. */
        Send,

        /** The wire's rule refuses it: say so, send nothing. */
        Refused,

        /** Nothing to send: no text, blank, or an echo. */
        Nothing,
    }

    private var lastExchanged: String? = null

    /** Whether `text` should go to the desktop; records it if so. */
    fun offer(text: String?): Offer {
        if (text == null || text.isBlank() || text == lastExchanged) return Offer.Nothing
        if (!syncable(text)) return Offer.Refused
        lastExchanged = text
        return Offer.Send
    }

    /** The desktop sent `text`: remember it so it is not sent straight back. */
    fun received(text: String) {
        lastExchanged = text
    }
}
