package org.celestina.magnetita.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp

private val DarkScheme = darkColorScheme(
    primary = SamsungBlueDark,
    onPrimary = OnSamsungBlueDark,
    secondary = SoftBlueDark,
    onSecondary = OnSamsungBlueDark,
    tertiary = LinkAliveDark,
    onTertiary = CanvasDark,
    error = RefusalDark,
    onError = CanvasDark,
    background = CanvasDark,
    onBackground = InkDark,
    surface = SurfaceDark,
    onSurface = InkDark,
    surfaceVariant = SurfaceVariantDark,
    onSurfaceVariant = InkMutedDark,
    outlineVariant = HairlineDark,
)

private val LightScheme = lightColorScheme(
    primary = SamsungBlueLight,
    onPrimary = OnSamsungBlueLight,
    secondary = SoftBlueLight,
    onSecondary = OnSamsungBlueLight,
    tertiary = LinkAliveLight,
    onTertiary = SurfaceLight,
    error = RefusalLight,
    onError = SurfaceLight,
    background = CanvasLight,
    onBackground = InkLight,
    surface = SurfaceLight,
    onSurface = InkLight,
    surfaceVariant = SurfaceVariantLight,
    onSurfaceVariant = InkMutedLight,
    outlineVariant = HairlineLight,
)

// Shapes say what a thing is: 26 for grouped cards and dialogs, 32 for
// sheets, 18 for buttons, 12 for small items. Pills are switches, chips and
// the floating navigation strip only.
val MagnetitaShapes = Shapes(
    extraSmall = RoundedCornerShape(12.dp),
    small = RoundedCornerShape(18.dp),
    medium = RoundedCornerShape(20.dp),
    large = RoundedCornerShape(26.dp),
    extraLarge = RoundedCornerShape(32.dp),
)

@Composable
fun MagnetitaTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    // Dynamic colour is deliberately off: the accent is the product's, not
    // the wallpaper's.
    MaterialTheme(
        colorScheme = if (darkTheme) DarkScheme else LightScheme,
        typography = MagnetitaTypography,
        shapes = MagnetitaShapes,
        content = content,
    )
}
