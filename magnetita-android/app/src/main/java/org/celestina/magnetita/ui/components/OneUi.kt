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
                Brush.radialGradient(
                    colors = listOf(accent.copy(alpha = 0.35f), Color.Transparent),
                    center = androidx.compose.ui.geometry.Offset(0f, 0f),
                    radius = 1400f,
                ),
            ),
    ) { content() }
}

/**
 * The large title that collapses into a toolbar. `progress` is 1 when the
 * page is at the top and 0 when scrolled; the caller drives it.
 */
@Composable
fun Header(
    title: String,
    subtitle: String? = null,
    progress: Float,
    maxHeight: Dp = 168.dp,
    minHeight: Dp = 64.dp,
    actions: (@Composable () -> Unit)? = null,
) {
    val height = minHeight + (maxHeight - minHeight) * progress
    val largeAlpha = ((progress - 0.35f) / 0.65f).coerceIn(0f, 1f)
    val smallAlpha = (1f - progress / 0.35f).coerceIn(0f, 1f)
    Box(modifier = Modifier.fillMaxWidth().height(height)) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(minHeight + 48.dp)
                .graphicsLayer { alpha = smallAlpha }
                .background(Brush.verticalGradient(listOf(MaterialTheme.colorScheme.background, Color.Transparent))),
        )
        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer { alpha = largeAlpha; scaleX = 0.85f + 0.15f * progress; scaleY = scaleX }
                .padding(horizontal = 24.dp),
            contentAlignment = Alignment.Center,
        ) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(title, style = MaterialTheme.typography.displayMedium, color = MaterialTheme.colorScheme.onBackground)
                if (subtitle != null) {
                    Text(subtitle, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold), color = MaterialTheme.colorScheme.primary)
                }
            }
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(top = 8.dp, start = 24.dp, end = 16.dp)
                .graphicsLayer { alpha = smallAlpha },
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.align(Alignment.TopStart)) {
                Text(title, style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.onBackground)
                if (subtitle != null) {
                    Spacer(Modifier.width(6.dp))
                    Text(subtitle, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold), color = MaterialTheme.colorScheme.primary)
                }
            }
        }
        if (actions != null) {
            Box(modifier = Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars).padding(top = 8.dp, end = 12.dp), contentAlignment = Alignment.TopEnd) { actions() }
        }
    }
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
