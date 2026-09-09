package org.celestina.magnetita

import org.celestina.magnetita.ui.theme.CanvasDark
import org.celestina.magnetita.ui.theme.SamsungBlueDark
import org.celestina.magnetita.ui.theme.SurfaceDark
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The tokens are the ones DESIGN.md names; a drift here is a design change. */
class ThemeTest {
    @Test
    fun theCanvasIsPureBlackAndTheSurfaceIsTintedNotGrey() {
        assertEquals(0xFF000000L, argb(CanvasDark))
        // A tint has a blue channel above its red channel; grey would be equal.
        val red = (argb(SurfaceDark) shr 16) and 0xFF
        val blue = argb(SurfaceDark) and 0xFF
        assertTrue(blue > red)
    }

    @Test
    fun theAccentIsSamsungBlue() {
        assertEquals(0xFF4D9FFFL, argb(SamsungBlueDark))
    }

    /** The colour's ARGB word: Compose keeps it in the top 32 bits of `value`. */
    private fun argb(color: androidx.compose.ui.graphics.Color): Long = (color.value shr 32).toLong()
}
