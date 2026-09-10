package org.celestina.magnetita.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow

/** The person's switches: what crosses to the desktop beyond the grants. */
@Composable
fun SettingsScreen(mediaNotifications: Boolean, onMediaNotifications: (Boolean) -> Unit) {
    Column(modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).statusBarsPadding()) {
        Text(
            text = stringResource(R.string.tab_settings),
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 20.dp),
        )
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
}
