package org.celestina.magnetita.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.celestina.magnetita.R
import org.celestina.magnetita.link.LinkState
import org.celestina.magnetita.ui.components.Canvas
import org.celestina.magnetita.ui.components.ChipTone
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow
import org.celestina.magnetita.ui.components.Header
import org.celestina.magnetita.ui.components.StateChip
import uniffi.magnetita_mobile.Identity
import uniffi.magnetita_mobile.PinnedDesktop

/** Everything the device screen shows, gathered by the activity. */
data class DeviceShown(
    val identity: Identity?,
    val pinned: List<PinnedDesktop>,
    val coreFailed: Boolean,
    val link: LinkState,
    val batteryLevel: Int,
    val batteryCharging: Boolean,
    val ringing: Boolean,
    val clipboardNote: String = "",
    val notificationsEnabled: Boolean = false,
)

/**
 * The home screen: the desktop first (its name as the title, the link as a
 * chip, the address, the battery this phone sends), then this phone's
 * identity. Without a pin the title is the app's and the one action is to
 * scan. While the desktop is finding the phone the stop action leads.
 */
@Composable
fun DeviceScreen(
    shown: DeviceShown,
    onScan: () -> Unit,
    onForget: (String) -> Unit,
    onStopRinging: () -> Unit,
    onNotificationAccess: () -> Unit = {},
) {
    val scroll = rememberScrollState()
    val progress by remember { derivedStateOf { (1f - scroll.value / 300f).coerceIn(0f, 1f) } }
    val desktop = shown.pinned.firstOrNull()
    val link = shown.link
    Canvas {
        Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
            Header(
                title = desktop?.name ?: stringResource(R.string.title_desktop_none),
                subtitle = if (desktop != null) stringResource(R.string.label_pairing) else shown.identity?.name,
                progress = progress,
            )
            Spacer(Modifier.height(8.dp))
            if (shown.ringing) {
                Button(
                    onClick = onStopRinging,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error, contentColor = MaterialTheme.colorScheme.onError),
                ) { Text(stringResource(R.string.action_stop_ringing)) }
                Spacer(Modifier.height(16.dp))
            }
            Group {
                GroupRow(
                    title = stringResource(R.string.subtitle_link),
                    detail = when (link) {
                        LinkState.NeedsPairing -> stringResource(R.string.state_needs_pairing)
                        LinkState.Searching -> stringResource(R.string.state_searching)
                        is LinkState.Connecting -> stringResource(R.string.state_connecting)
                        is LinkState.Connected -> stringResource(R.string.state_connected_to, link.desktopName)
                        is LinkState.Waiting -> stringResource(R.string.state_waiting)
                    },
                    trailing = {
                        when (link) {
                            is LinkState.Connected -> StateChip(stringResource(R.string.chip_connected), ChipTone.Alive)
                            LinkState.NeedsPairing -> StateChip(stringResource(R.string.chip_pair), ChipTone.Waiting)
                            else -> StateChip("…", ChipTone.Waiting)
                        }
                    },
                )
                if (link is LinkState.Connected) {
                    GroupRow(title = stringResource(R.string.label_address), detail = link.address)
                }
                GroupRow(
                    title = stringResource(R.string.label_clipboard),
                    detail = shown.clipboardNote.ifBlank { stringResource(R.string.detail_clipboard) },
                )
                GroupRow(
                    title = stringResource(R.string.label_notifications),
                    detail = stringResource(if (shown.notificationsEnabled) R.string.detail_notifications_on else R.string.detail_notifications_off),
                    trailing = {
                        if (shown.notificationsEnabled) {
                            StateChip(stringResource(R.string.chip_on), ChipTone.Alive)
                        } else {
                            TextButton(onClick = onNotificationAccess) { Text(stringResource(R.string.action_allow)) }
                        }
                    },
                )
                GroupRow(
                    title = stringResource(R.string.label_phone_battery),
                    detail = stringResource(R.string.detail_battery_shared),
                    last = true,
                    trailing = {
                        val text = if (shown.batteryCharging) stringResource(R.string.battery_charging, shown.batteryLevel) else stringResource(R.string.battery_level, shown.batteryLevel)
                        StateChip(text, if (link is LinkState.Connected) ChipTone.Alive else ChipTone.Waiting)
                    },
                )
            }
            Spacer(Modifier.height(16.dp))
            if (desktop == null) {
                Button(onClick = onScan, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) { Text(stringResource(R.string.action_scan)) }
                Spacer(Modifier.height(8.dp))
                Text(
                    stringResource(R.string.hint_pair),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 32.dp),
                )
            } else {
                Group {
                    GroupRow(title = desktop.name, detail = desktop.fingerprint, last = true)
                }
                TextButton(onClick = { onForget(desktop.deviceId) }, modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp)) {
                    Text(stringResource(R.string.action_forget), color = MaterialTheme.colorScheme.error)
                }
            }
            Spacer(Modifier.height(16.dp))
            Group {
                GroupRow(
                    title = stringResource(R.string.title_device),
                    detail = shown.identity?.name,
                    trailing = {
                        when {
                            shown.coreFailed -> StateChip(stringResource(R.string.error_core), ChipTone.Refused)
                            shown.identity == null -> StateChip("…", ChipTone.Waiting)
                            else -> StateChip("OK", ChipTone.Alive)
                        }
                    },
                )
                GroupRow(title = stringResource(R.string.label_device_id), detail = shown.identity?.deviceId ?: "")
                GroupRow(title = stringResource(R.string.label_fingerprint), detail = shown.identity?.fingerprint ?: "", last = true)
            }
            Spacer(Modifier.height(120.dp))
        }
    }
}
