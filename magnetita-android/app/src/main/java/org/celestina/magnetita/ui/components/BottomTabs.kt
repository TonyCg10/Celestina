package org.celestina.magnetita.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Phone
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Build
import androidx.compose.material.icons.outlined.Phone
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.Brush
import androidx.compose.foundation.background
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R

/** The three pages: the phone, the control, the settings. */
private class Tab(val title: Int, val selected: ImageVector, val unselected: ImageVector)

private val TABS = listOf(
    Tab(R.string.tab_device, Icons.Filled.Phone, Icons.Outlined.Phone),
    Tab(R.string.tab_control, Icons.Filled.Build, Icons.Outlined.Build),
    Tab(R.string.tab_settings, Icons.Filled.Settings, Icons.Outlined.Settings),
)

/**
 * The floating pill of icons at the bottom, the one MilaHub carries: a
 * circle-shaped surface over the page, icons only, the selected one on the
 * accent with a soft indicator.
 */
@Composable
fun BottomTabs(selected: Int, onSelect: (Int) -> Unit) {
    Box(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 64.dp).padding(bottom = 24.dp).navigationBarsPadding(),
        contentAlignment = Alignment.Center,
    ) {
        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.background, tonalElevation = 4.dp, shadowElevation = 8.dp) {
            NavigationBar(
                containerColor = Color.Transparent,
                tonalElevation = 0.dp,
                modifier = Modifier.height(56.dp),
                windowInsets = WindowInsets(0, 0, 0, 0),
            ) {
                TABS.forEachIndexed { index, tab ->
                    val isSelected = selected == index
                    NavigationBarItem(
                        icon = {
                            Icon(
                                imageVector = if (isSelected) tab.selected else tab.unselected,
                                contentDescription = stringResource(tab.title),
                                modifier = Modifier.size(28.dp),
                            )
                        },
                        label = null,
                        alwaysShowLabel = false,
                        selected = isSelected,
                        onClick = { onSelect(index) },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = MaterialTheme.colorScheme.primary,
                            indicatorColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f),
                            unselectedIconColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                        ),
                    )
                }
            }
        }
    }
}

/**
 * The glass under the pill: the surface rising from the bottom edge as a
 * blurred gradient, so the page fades into it instead of ending at a line.
 */
@Composable
fun GlassVeil(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(110.dp)
            .background(
                brush = Brush.verticalGradient(
                    colors = listOf(
                        Color.Transparent,
                        MaterialTheme.colorScheme.surface.copy(alpha = 0.5f),
                        MaterialTheme.colorScheme.surface,
                    ),
                ),
            )
            .blur(8.dp),
    )
}
