package org.celestina.magnetita.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Switch
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R
import org.celestina.magnetita.ui.components.Canvas
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow
import org.celestina.magnetita.ui.components.HEADER_MAX
import org.celestina.magnetita.ui.components.Header
import org.celestina.magnetita.ui.components.headerProgress

/** The person's switches: what crosses to the desktop beyond the grants. */
@Composable
fun SettingsScreen(mediaNotifications: Boolean, onMediaNotifications: (Boolean) -> Unit) {
    val scroll = rememberScrollState()
    val density = LocalDensity.current
    val progress by remember { derivedStateOf { headerProgress(scroll.value.toFloat(), density) } }
    Canvas {
        Box(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
                Spacer(Modifier.height(HEADER_MAX))
                Group {
                    GroupRow(
                        title = stringResource(R.string.label_media_notifications),
                        detail = stringResource(R.string.detail_media_notifications),
                        last = true,
                        trailing = { Switch(checked = mediaNotifications, onCheckedChange = onMediaNotifications) },
                    )
                }
                Spacer(Modifier.height(120.dp))
            }
            Header(progress = progress, title = stringResource(R.string.tab_settings))
        }
    }
}
