package org.celestina.magnetita.mirror

/**
 * The picture's size for a screen and the desktop's `maxSize` (the longer
 * side's cap, 0 for native), and where a touch on that picture lands on
 * the screen. Pure, so the JVM tests pin it.
 */
object MirrorGeometry {
    /** Both sides even, as the encoders want, the longer one at most `maxSize`. */
    fun fit(width: Int, height: Int, maxSize: Int): Pair<Int, Int> {
        val longer = maxOf(width, height)
        val scale = if (maxSize in 1 until longer) maxSize.toDouble() / longer else 1.0
        val w = (width * scale).toInt() and 1.inv()
        val h = (height * scale).toInt() and 1.inv()
        return maxOf(w, 2) to maxOf(h, 2)
    }

    /** A point of the picture on the screen. */
    fun toScreen(x: Int, y: Int, picture: Pair<Int, Int>, screen: Pair<Int, Int>): Pair<Float, Float> {
        if (picture.first <= 0 || picture.second <= 0) return x.toFloat() to y.toFloat()
        val sx = x * screen.first.toFloat() / picture.first
        val sy = y * screen.second.toFloat() / picture.second
        return sx.coerceIn(0f, (screen.first - 1).toFloat()) to sy.coerceIn(0f, (screen.second - 1).toFloat())
    }
}
