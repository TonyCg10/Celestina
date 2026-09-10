package org.celestina.magnetita.mirror

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.Context
import android.graphics.Path
import android.view.accessibility.AccessibilityEvent

/**
 * The desktop's touches and navigation on the mirrored screen, played
 * through the accessibility gesture API: the one way an app may act on
 * another app's screen without `adb`. Enabled by the person in the
 * system settings; the device screen offers the shortcut.
 *
 * A finger is a stroke continued segment by segment: `down` opens it,
 * each `move` extends it from the last point, `up` closes it. A segment
 * is dispatched only when the previous one completed, to the latest point
 * the desktop named meanwhile, so a fast drag lands where the pointer is
 * and not where it was. Key events
 * cannot be injected this way, so a key becomes text set on the focused
 * field (a character appended, Backspace removing one, Enter the field's
 * action), and back, home and recents are the global actions.
 */
class MirrorInput : AccessibilityService() {
    private class Finger(var x: Float, var y: Float, var stroke: GestureDescription.StrokeDescription?) {
        /** When the last segment was dispatched, in uptime millis. */
        var lastDispatch = 0L
        /** Where the finger should be next, and whether it lifts there. */
        var pending: Pair<Float, Float>? = null
        var lift = false
        var scheduled = false
    }

    private val fingers = HashMap<Int, Finger>()
    private val main = android.os.Handler(android.os.Looper.getMainLooper())

    override fun onServiceConnected() {
        instance = this
    }

    override fun onDestroy() {
        if (instance === this) instance = null
        super.onDestroy()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}
    override fun onInterrupt() {}

    /** `phase` 0 down, 1 move, 2 up, in screen pixels. Any thread. */
    fun touch(phase: Int, x: Float, y: Float, pointer: Int) {
        main.post {
            when (phase) {
                0 -> {
                    val path = Path().apply { moveTo(x, y) }
                    val stroke = GestureDescription.StrokeDescription(path, 0, DOWN_MS, true)
                    val finger = Finger(x, y, stroke)
                    fingers[pointer] = finger
                    finger.lastDispatch = android.os.SystemClock.uptimeMillis()
                    dispatch(stroke)
                }
                1 -> {
                    val finger = fingers[pointer] ?: return@post
                    finger.pending = x to y
                    flush(pointer, finger)
                }
                else -> {
                    val finger = fingers[pointer] ?: return@post
                    finger.pending = x to y
                    finger.lift = true
                    flush(pointer, finger)
                }
            }
        }
    }

    /**
     * One segment per [SEGMENT_MS]: the latest point the desktop named
     * goes out when the running segment ends, never a queue of stale ones.
     * The framework reports a continuing stroke's end only when the whole
     * gesture ends, so the pacing is by the clock, not by callbacks.
     */
    private fun flush(pointer: Int, finger: Finger) {
        val now = android.os.SystemClock.uptimeMillis()
        val wait = finger.lastDispatch + SEGMENT_MS - now
        if (wait > 0) {
            if (!finger.scheduled) {
                finger.scheduled = true
                main.postDelayed({ finger.scheduled = false; flush(pointer, finger) }, wait)
            }
            return
        }
        val (x, y) = finger.pending ?: return
        finger.pending = null
        val previous = finger.stroke ?: return
        val path = Path().apply { moveTo(finger.x, finger.y); lineTo(x, y) }
        val stroke = previous.continueStroke(path, 0, SEGMENT_MS, !finger.lift)
        finger.x = x
        finger.y = y
        finger.stroke = stroke
        finger.lastDispatch = now
        if (finger.lift) fingers.remove(pointer)
        dispatch(stroke)
    }

    private fun dispatch(stroke: GestureDescription.StrokeDescription) {
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        dispatchGesture(gesture, null, null)
    }

    /** An Android key code on the focused field, the way accessibility allows. */
    fun key(keycode: Int, pressed: Boolean) {
        if (!pressed) return
        val focused = rootInActiveWindow?.findFocus(android.view.accessibility.AccessibilityNodeInfo.FOCUS_INPUT) ?: return
        if (!focused.isEditable) return
        val current = focused.text?.toString() ?: ""
        val next = when (keycode) {
            KEYCODE_DEL -> if (current.isEmpty()) return else current.dropLast(1)
            KEYCODE_ENTER -> {
                if (!focused.performAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_IME_ENTER.id)) {
                    focused.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK)
                }
                return
            }
            else -> current + (character(keycode) ?: return)
        }
        val arguments = android.os.Bundle().apply {
            putCharSequence(android.view.accessibility.AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, next)
        }
        focused.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT, arguments)
    }

    /** 0 back, 1 home, 2 recents. */
    fun global(action: Int) {
        val which = when (action) {
            0 -> GLOBAL_ACTION_BACK
            1 -> GLOBAL_ACTION_HOME
            2 -> GLOBAL_ACTION_RECENTS
            else -> return
        }
        performGlobalAction(which)
    }

    companion object {
        /** A segment plays this long; the finger catches up in one. */
        private const val SEGMENT_MS = 16L
        /** The press before any move. */
        private const val DOWN_MS = 16L
        private const val KEYCODE_ENTER = 66
        private const val KEYCODE_DEL = 67

        /** The printable character of a key code, or null. */
        fun character(keycode: Int): Char? = when (keycode) {
            in 7..16 -> '0' + (keycode - 7)
            in 29..54 -> 'a' + (keycode - 29)
            62 -> ' '
            55 -> ','
            56 -> '.'
            69 -> '-'
            70 -> '='
            71 -> '['
            72 -> ']'
            73 -> '\\'
            74 -> ';'
            75 -> '\''
            76 -> '/'
            else -> null
        }

        @Volatile var instance: MirrorInput? = null
            private set

        fun enabled(context: Context): Boolean =
            android.provider.Settings.Secure.getString(context.contentResolver, "enabled_accessibility_services")
                ?.split(':')?.any { it.startsWith(context.packageName + "/") } == true
    }
}
