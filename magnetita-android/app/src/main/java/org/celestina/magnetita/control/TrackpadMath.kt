package org.celestina.magnetita.control

/**
 * The arithmetic between a finger and the desktop's pointer, pure so the
 * JVM tests pin it: fractional motion accumulates until a whole pixel
 * moves, gain scales a slow drag down and a fast one up, and two fingers
 * scroll in the wire's 1/120 steps.
 */
class TrackpadMath(private val gain: Float = 1.6f) {
    private var restX = 0f
    private var restY = 0f

    /** The whole pixels to send for a drag of (`dx`, `dy`) screen pixels. */
    fun move(dx: Float, dy: Float): Pair<Int, Int> {
        val speed = kotlin.math.sqrt(dx * dx + dy * dy)
        val factor = gain * (0.6f + kotlin.math.min(speed, 40f) / 40f)
        restX += dx * factor
        restY += dy * factor
        val x = restX.toInt()
        val y = restY.toInt()
        restX -= x
        restY -= y
        return x to y
    }

    /** Scroll steps for a two-finger drag of `dy` pixels: a finger-height is a wheel notch. */
    fun scroll(dy: Float): Int = (-dy * 120f / 48f).toInt()

    companion object {
        const val KEY_BACKSPACE = 14
        const val KEY_TAB = 15
        const val KEY_ENTER = 28
        const val KEY_ESC = 1
        const val KEY_UP = 103
        const val KEY_LEFT = 105
        const val KEY_RIGHT = 106
        const val KEY_DOWN = 108

        /** What a text field change means: appended text, or deletions. */
        fun diff(before: String, after: String): Pair<String, Int> {
            val common = before.commonPrefixWith(after).length
            return after.substring(common) to (before.length - common)
        }
    }
}
