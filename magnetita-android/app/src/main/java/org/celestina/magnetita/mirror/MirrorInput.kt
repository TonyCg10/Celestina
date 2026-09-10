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
        /** A segment is playing; the next waits for its end. */
        var busy = false
        /** Where the finger should be next, and whether it lifts there. */
        var pending: Pair<Float, Float>? = null
        var lift = false
    }

    private val fingers = HashMap<Int, Finger>()

    override fun onServiceConnected() {
        instance = this
    }

    override fun onDestroy() {
        if (instance === this) instance = null
        super.onDestroy()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}
    override fun onInterrupt() {}

    /** `phase` 0 down, 1 move, 2 up, in screen pixels. */
    fun touch(phase: Int, x: Float, y: Float, pointer: Int) {
        when (phase) {
            0 -> {
                val path = Path().apply { moveTo(x, y) }
                val stroke = GestureDescription.StrokeDescription(path, 0, DOWN_MS, true)
                val finger = Finger(x, y, stroke)
                fingers[pointer] = finger
                play(pointer, finger, stroke)
            }
            1 -> {
                val finger = fingers[pointer] ?: return
                finger.pending = x to y
                if (!finger.busy) advance(pointer, finger)
            }
            else -> {
                val finger = fingers[pointer] ?: return
                finger.pending = x to y
                finger.lift = true
                if (!finger.busy) advance(pointer, finger)
            }
        }
    }

    /** The next segment of a finger, from where it is to where it is wanted. */
    private fun advance(pointer: Int, finger: Finger) {
        val (x, y) = finger.pending ?: return
        finger.pending = null
        val previous = finger.stroke ?: return
        val path = Path().apply { moveTo(finger.x, finger.y); lineTo(x, y) }
        val stroke = previous.continueStroke(path, 0, SEGMENT_MS, !finger.lift)
        finger.x = x
        finger.y = y
        finger.stroke = stroke
        if (finger.lift) fingers.remove(pointer)
        play(pointer, finger, stroke)
    }

    private fun play(pointer: Int, finger: Finger, stroke: GestureDescription.StrokeDescription) {
        finger.busy = true
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        val done = object : GestureResultCallback() {
            override fun onCompleted(gestureDescription: GestureDescription?) { finished(pointer, finger) }
            override fun onCancelled(gestureDescription: GestureDescription?) { finished(pointer, finger) }
        }
        if (!dispatchGesture(gesture, done, null)) finished(pointer, finger)
    }

    private fun finished(pointer: Int, finger: Finger) {
        finger.busy = false
        if (finger.pending != null && fingers[pointer] === finger) advance(pointer, finger)
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
        private const val SEGMENT_MS = 8L
        /** A press before any move: long enough to register, short enough to tap. */
        private const val DOWN_MS = 1L
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
