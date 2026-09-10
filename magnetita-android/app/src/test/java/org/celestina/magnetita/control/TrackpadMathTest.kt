package org.celestina.magnetita.control

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Fractions accumulate, gain rises with speed, a field change is a diff. */
class TrackpadMathTest {
    @Test
    fun smallDragsAccumulateIntoWholePixelsAndFastOnesGainMore() {
        val math = TrackpadMath(gain = 1f)
        var x = 0
        repeat(10) { x += math.move(0.3f, 0f).first }
        assertTrue("ten drags of 0.3 px reach the pointer: $x", x in 1..3)
        val slow = TrackpadMath(gain = 1f).move(10f, 0f).first
        val fast = TrackpadMath(gain = 1f).move(40f, 0f).first
        assertTrue("faster drags gain more: $slow vs $fast", fast > slow * 2)
    }

    @Test
    fun twoFingerDragsScrollInWheelSteps() {
        val math = TrackpadMath()
        assertEquals(-120, math.scroll(48f))
        assertEquals(60, math.scroll(-24f))
    }

    @Test
    fun aFieldChangeIsAppendedTextOrDeletions() {
        assertEquals("lo" to 0, TrackpadMath.diff("ho", "holo"))
        assertEquals("" to 2, TrackpadMath.diff("hola", "ho"))
        assertEquals("y" to 1, TrackpadMath.diff("hola", "holy"))
    }
}
