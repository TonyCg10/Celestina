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
 * each `move` extends it from the last point, `up` closes it. Key events
 * cannot be injected this way; only back, home and recents are honoured.
 */
class MirrorInput : AccessibilityService() {
    private data class Finger(var x: Float, var y: Float, var stroke: GestureDescription.StrokeDescription?)

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
                val stroke = GestureDescription.StrokeDescription(path, 0, SEGMENT_MS, true)
                fingers[pointer] = Finger(x, y, stroke)
                dispatch(stroke)
            }
            1 -> {
                val finger = fingers[pointer] ?: return
                val previous = finger.stroke ?: return
                val path = Path().apply { moveTo(finger.x, finger.y); lineTo(x, y) }
                val stroke = previous.continueStroke(path, 0, SEGMENT_MS, true)
                finger.x = x
                finger.y = y
                finger.stroke = stroke
                dispatch(stroke)
            }
            else -> {
                val finger = fingers.remove(pointer) ?: return
                val previous = finger.stroke ?: return
                val path = Path().apply { moveTo(finger.x, finger.y); lineTo(x, y) }
                dispatch(previous.continueStroke(path, 0, SEGMENT_MS, false))
            }
        }
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

    private fun dispatch(stroke: GestureDescription.StrokeDescription) {
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        dispatchGesture(gesture, null, null)
    }

    companion object {
        private const val SEGMENT_MS = 16L

        @Volatile var instance: MirrorInput? = null
            private set

        fun enabled(context: Context): Boolean =
            android.provider.Settings.Secure.getString(context.contentResolver, "enabled_accessibility_services")
                ?.split(':')?.any { it.startsWith(context.packageName + "/") } == true
    }
}
