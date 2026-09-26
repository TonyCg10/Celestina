package org.celestina.magnetita.mirror

/** What the desktop's keys and navigation act on: the accessibility service, once enabled. */
interface MirrorSink {
    fun key(keycode: Int, pressed: Boolean)
    fun global(action: Int)
}

/**
 * The desktop's keys and global actions reach the phone only while the
 * mirror streams: the person consented to the capture, and its notification
 * shows that it runs. Outside a stream they are dropped, like touches
 * without the stream's geometry. Pure, so the JVM tests pin it; the service
 * gives it the real state and sink. Returns whether the input acted.
 */
class MirrorControl(private val streaming: () -> Boolean, private val sink: () -> MirrorSink?) {
    fun key(keycode: Int, pressed: Boolean): Boolean {
        if (!streaming()) return false
        val target = sink() ?: return false
        target.key(keycode, pressed)
        return true
    }

    fun global(action: Int): Boolean {
        if (!streaming()) return false
        val target = sink() ?: return false
        target.global(action)
        return true
    }
}
