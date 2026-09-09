package org.celestina.magnetita.link

/**
 * What an envelope from the desktop asks of the phone, decided from the
 * capability and kind ids of the wire document (`magnetita/docs/protocol.md`).
 * Pure, so it is tested on the JVM; the bodies stay opaque here because
 * the ones this screen set handles are empty.
 */
sealed interface DesktopSignal {
    data object Ring : DesktopSignal
    data object StopRinging : DesktopSignal
    data object BatteryRequested : DesktopSignal

    /** The desktop's clipboard now holds this text. */
    data class ClipboardText(val text: String) : DesktopSignal

    /** The desktop asks for this phone's clipboard; answered only in front. */
    data object ClipboardRequested : DesktopSignal
    data class Other(val capability: Int, val kind: Int) : DesktopSignal

    companion object {
        const val CAPABILITY_BATTERY = 1
        const val CAPABILITY_CLIPBOARD = 2
        const val CAPABILITY_FIND = 4
        const val KIND_BATTERY_REQUEST = 2
        const val KIND_CLIPBOARD_TEXT = 1
        const val KIND_CLIPBOARD_REQUEST = 2
        const val KIND_FIND_RING = 1
        const val KIND_FIND_STOP = 2

        fun of(event: LinkEvent): DesktopSignal = when (event.capability to event.kind) {
            CAPABILITY_FIND to KIND_FIND_RING -> Ring
            CAPABILITY_FIND to KIND_FIND_STOP -> StopRinging
            CAPABILITY_BATTERY to KIND_BATTERY_REQUEST -> BatteryRequested
            CAPABILITY_CLIPBOARD to KIND_CLIPBOARD_TEXT ->
                event.text?.let { ClipboardText(it) } ?: Other(event.capability, event.kind)
            CAPABILITY_CLIPBOARD to KIND_CLIPBOARD_REQUEST -> ClipboardRequested
            else -> Other(event.capability, event.kind)
        }
    }
}

/** The pairing links the scanner accepts; the core parses the rest. */
object PairLink {
    const val PREFIX = "magnetita://pair?"

    fun accepts(text: String?): Boolean = text != null && text.startsWith(PREFIX) && text.length <= 512
}
