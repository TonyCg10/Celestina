package org.celestina.magnetita

import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.celestina.magnetita.core.Core
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.LinkState
import org.celestina.magnetita.ui.screens.DeviceScreen
import org.celestina.magnetita.ui.screens.DeviceShown
import org.celestina.magnetita.ui.screens.ScanScreen
import org.celestina.magnetita.ui.theme.MagnetitaTheme
import uniffi.magnetita_mobile.Identity
import uniffi.magnetita_mobile.PinnedDesktop

/** What the core answered once, off the UI thread. */
private data class FromCore(val identity: Identity?, val pinned: List<PinnedDesktop>, val failed: Boolean)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        LinkService.start(this)
        offerPairing(intent)
        setContent {
            MagnetitaTheme {
                var scanning by remember { mutableStateOf(false) }
                var core by remember { mutableStateOf(FromCore(null, emptyList(), false)) }
                val link by LinkService.state.collectAsState()
                val ringing by LinkService.ringing.collectAsState()
                // Pins change on pair and forget; both show in the link state.
                LaunchedEffect(link::class) { core = readCore() }
                BackHandler(enabled = scanning) { scanning = false }
                if (scanning) {
                    ScanScreen(
                        onLink = { uri -> LinkService.pair(this, uri); scanning = false },
                        onBack = { scanning = false },
                    )
                } else {
                    val battery = readBattery()
                    DeviceScreen(
                        shown = DeviceShown(core.identity, core.pinned, core.failed, link, battery.first, battery.second, ringing),
                        onScan = { scanning = true },
                        onForget = { id -> LinkService.forget(this, id) },
                        onStopRinging = { LinkService.stopRinging(this) },
                    )
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        offerPairing(intent)
    }

    private suspend fun readCore(): FromCore = withContext(Dispatchers.IO) {
        runCatching { FromCore(Core.identity(applicationContext), Core.pinned(applicationContext), false) }
            .getOrElse { FromCore(null, emptyList(), true) }
    }

    @Composable
    private fun readBattery(): Pair<Int, Boolean> {
        val status = applicationContext.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val level = status?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
        val scale = status?.getIntExtra(BatteryManager.EXTRA_SCALE, 100) ?: 100
        val plugged = status?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0
        return (if (level >= 0 && scale > 0) level * 100 / scale else 0) to (plugged != 0)
    }

    /** A `magnetita://pair` link, from the system camera or a test, pairs. */
    private fun offerPairing(intent: Intent?) {
        val data = intent?.data ?: return
        if (intent.action == Intent.ACTION_VIEW && data.scheme == "magnetita" && data.host == "pair") {
            LinkService.pair(this, data.toString())
        }
    }
}
