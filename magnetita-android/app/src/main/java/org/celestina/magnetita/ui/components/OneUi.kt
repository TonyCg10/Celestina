package org.celestina.magnetita.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.dp

// The One UI pieces of DESIGN.md: the glow, the collapsing header, the
// group, the row and the state chip. Screens compose these; they never paint
// a colour or a radius of their own.

/** The canvas with the accent glow from the top-left: the only decoration. */
@Composable
fun Canvas(content: @Composable () -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .background(
                // The light: one spot in the middle of the screen, as MilaHub.
                Brush.radialGradient(
                    colors = listOf(accent.copy(alpha = 0.35f), Color.Transparent),
                    radius = 1100f,
                ),
            ),
    ) { content() }
}

/**
 * The large title that collapses into a toolbar, MilaHub's `OneUIHeader`
 * token for token: 280 dp open, 110 dp closed; the large title at 32 sp,
 * medium weight, letter spacing -1.5, scaling 0.82..1 and sliding away to
 * the left and up; the small title bold at the toolbar's start with the
 * subtitle in the accent. `progress` is 1 at the top and 0 scrolled.
 */
@Composable
fun Header(
    progress: Float,
    title: String,
    subtitle: String? = null,
    maxHeight: Dp = HEADER_MAX,
    minHeight: Dp = HEADER_MIN,
    actions: (@Composable BoxScope.() -> Unit)? = null,
) {
    val density = LocalDensity.current
    val currentHeight = minHeight + (maxHeight - minHeight) * progress
    Surface(
        modifier = Modifier.fillMaxWidth().height(currentHeight),
        color = MaterialTheme.colorScheme.surface.copy(alpha = (1f - progress * 0.02f).coerceIn(0.98f, 1f)),
    ) {
        Box(modifier = Modifier.fillMaxSize()) {
            val titleAlphaLarge = ((progress - 0.35f) / 0.65f).coerceIn(0f, 1f)
            val titleAlphaSmall = (1f - (progress / 0.35f)).coerceIn(0f, 1f)
            val titleScale = 0.82f + (progress * 0.18f)
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        alpha = titleAlphaLarge
                        scaleX = titleScale
                        scaleY = titleScale
                        translationX = -(1f - progress) * with(density) { 90.dp.toPx() }
                        translationY = (1f - progress) * with(density) { -24.dp.toPx() }
                    }
                    .padding(horizontal = 24.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.displayMedium.copy(
                        fontWeight = FontWeight.Medium,
                        fontSize = 32.sp,
                        letterSpacing = (-1.5).sp,
                        color = MaterialTheme.colorScheme.onSurface,
                    ),
                )
            }
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .windowInsetsPadding(WindowInsets.statusBars)
                    .padding(horizontal = 24.dp)
                    .graphicsLayer {
                        alpha = titleAlphaSmall
                        translationX = progress * with(density) { 24.dp.toPx() }
                        translationY = progress * with(density) { 12.dp.toPx() }
                    },
                contentAlignment = Alignment.CenterStart,
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = title,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.onSurface),
                    )
                    if (!subtitle.isNullOrEmpty()) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(
                            text = subtitle,
                            style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.primary),
                        )
                    }
                }
            }
            if (actions != null) {
                Box(
                    modifier = Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars).padding(horizontal = 16.dp),
                    contentAlignment = Alignment.CenterEnd,
                ) { actions() }
            }
        }
    }
}

/** The header's two heights, MilaHub's. */
val HEADER_MAX: Dp = 280.dp
val HEADER_MIN: Dp = 110.dp

/** The header's collapse for a scroll position, 1 at the top and 0 once the range is scrolled. */
fun headerProgress(scrollPx: Float, density: androidx.compose.ui.unit.Density): Float {
    val range = with(density) { (HEADER_MAX - HEADER_MIN).toPx() }
    return (1f - scrollPx / range).coerceIn(0f, 1f)
}

/** A grouped card: 26 dp radius in `surface`, rows separated by hairlines. */
@Composable
fun Group(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Surface(
        modifier = modifier.fillMaxWidth().padding(horizontal = 16.dp),
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surface,
    ) { Column { content() } }
}

/** One row of a group: a title, one line of detail, a control on the right. */
@Composable
fun GroupRow(
    title: String,
    detail: String? = null,
    last: Boolean = false,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            if (detail != null) {
                Text(detail, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (trailing != null) trailing()
    }
    if (!last) {
        HorizontalDivider(modifier = Modifier.padding(horizontal = 20.dp), color = MaterialTheme.colorScheme.outlineVariant)
    }
}

/** A state as a pill: mint for a live link, variant for waiting, red for a refusal. */
@Composable
fun StateChip(text: String, tone: ChipTone) {
    val (container, ink) = when (tone) {
        ChipTone.Alive -> MaterialTheme.colorScheme.tertiary to MaterialTheme.colorScheme.onTertiary
        ChipTone.Waiting -> MaterialTheme.colorScheme.surfaceVariant to MaterialTheme.colorScheme.onSurfaceVariant
        ChipTone.Refused -> MaterialTheme.colorScheme.error to MaterialTheme.colorScheme.onError
    }
    Surface(shape = CircleShape, color = container) {
        Text(text, style = MaterialTheme.typography.labelLarge, color = ink, modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp))
    }
}

enum class ChipTone { Alive, Waiting, Refused }

private fun Modifier.width(dp: Dp) = this.then(Modifier.padding(start = dp))
