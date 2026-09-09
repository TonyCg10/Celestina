package org.celestina.magnetita

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.celestina.magnetita.core.Core
import org.celestina.magnetita.ui.components.Canvas
import org.celestina.magnetita.ui.components.ChipTone
import org.celestina.magnetita.ui.components.Group
import org.celestina.magnetita.ui.components.GroupRow
import org.celestina.magnetita.ui.components.Header
import org.celestina.magnetita.ui.components.StateChip
import org.celestina.magnetita.ui.theme.MagnetitaTheme
import uniffi.magnetita_mobile.Identity
import uniffi.magnetita_mobile.PinnedDesktop

/** What the identity screen shows once the core has answered. */
private data class Shown(val identity: Identity?, val pinned: List<PinnedDesktop>, val failed: Boolean)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MagnetitaTheme {
                var shown by remember { mutableStateOf(Shown(null, emptyList(), false)) }
                LaunchedEffect(Unit) {
                    shown = withContext(Dispatchers.IO) {
                        runCatching { Shown(Core.identity(applicationContext), Core.pinned(applicationContext), false) }
                            .getOrElse { Shown(null, emptyList(), true) }
                    }
                }
                IdentityScreen(shown)
            }
        }
    }
}

@Composable
private fun IdentityScreen(shown: Shown) {
    val scroll = rememberScrollState()
    val progress = (1f - scroll.value / 300f).coerceIn(0f, 1f)
    Canvas {
        Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
            Header(
                title = stringResource(R.string.title_device),
                subtitle = shown.identity?.name,
                progress = progress,
            )
            Spacer(Modifier.height(8.dp))
            Group {
                GroupRow(
                    title = stringResource(R.string.subtitle_identity),
                    detail = null,
                    trailing = {
                        when {
                            shown.failed -> StateChip(stringResource(R.string.error_core), ChipTone.Refused)
                            shown.identity == null -> StateChip("…", ChipTone.Waiting)
                            else -> StateChip("OK", ChipTone.Alive)
                        }
                    },
                )
                GroupRow(title = stringResource(R.string.label_device_id), detail = shown.identity?.deviceId ?: "")
                GroupRow(title = stringResource(R.string.label_fingerprint), detail = shown.identity?.fingerprint ?: "", last = true)
            }
            Spacer(Modifier.height(16.dp))
            Group {
                if (shown.pinned.isEmpty()) {
                    GroupRow(title = stringResource(R.string.label_desktops), detail = stringResource(R.string.state_no_desktop), last = true)
                } else {
                    shown.pinned.forEachIndexed { i, d ->
                        GroupRow(title = d.name, detail = d.fingerprint, last = i == shown.pinned.lastIndex)
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
            Text(
                stringResource(R.string.hint_pair),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 32.dp),
            )
            Spacer(Modifier.height(120.dp))
        }
    }
}
