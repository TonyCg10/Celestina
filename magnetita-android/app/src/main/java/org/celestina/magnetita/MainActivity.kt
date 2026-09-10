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
import org.celestina.magnetita.link.MediaCommand
import org.celestina.magnetita.link.Outbound
import org.celestina.magnetita.notifications.PhoneNotifications
import org.celestina.magnetita.phone.PhonePermissions
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import org.celestina.magnetita.ui.screens.ControlScreen
import org.celestina.magnetita.ui.screens.DeviceScreen
import org.celestina.magnetita.ui.screens.Remote
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
                var page by remember { mutableStateOf(0) }
                val commands by LinkService.commands.collectAsState()
                val commandNote by LinkService.commandNote.collectAsState()
                var core by remember { mutableStateOf(FromCore(null, emptyList(), false)) }
                val link by LinkService.state.collectAsState()
                val ringing by LinkService.ringing.collectAsState()
                val clipboardNote by LinkService.clipboardNote.collectAsState()
                val desktopMedia by LinkService.desktopMedia.collectAsState()
                var phoneGranted by remember { mutableStateOf(PhonePermissions.allGranted(this)) }
                val mirrorInput = remember(link, phoneGranted) { org.celestina.magnetita.mirror.MirrorInput.enabled(this) }
                var storageShared by remember { mutableStateOf(org.celestina.magnetita.storage.PhoneStorage(this).available()) }
                var wholePhone by remember { mutableStateOf(org.celestina.magnetita.storage.PhoneStorage(this).wholePhone()) }
                // The all-files grant is given on a system page; re-read it when the app comes back.
                LaunchedEffect(link, phoneGranted) {
                    wholePhone = org.celestina.magnetita.storage.PhoneStorage(this@MainActivity).wholePhone()
                    storageShared = org.celestina.magnetita.storage.PhoneStorage(this@MainActivity).available()
                }
                val pickTree = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
                    if (tree != null) org.celestina.magnetita.storage.PhoneStorage.granted(this, tree)
                    storageShared = org.celestina.magnetita.storage.PhoneStorage(this).available()
                }
                val askPhone = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
                    phoneGranted = PhonePermissions.allGranted(this)
                    if (phoneGranted) LinkService.phoneGranted(this)
                }
                // While the screen is in front, the desktop's players are wanted.
                LaunchedEffect(link) {
                    if (link is LinkState.Connected) {
                        while (true) {
                            LinkService.send(this@MainActivity, Outbound.MediaWanted)
                            kotlinx.coroutines.delay(60_000)
                        }
                    }
                }
                // Pins change on pair and forget; both show in the link state.
                LaunchedEffect(link::class) { core = readCore() }
                BackHandler(enabled = scanning) { scanning = false }
                if (scanning) {
                    ScanScreen(
                        onLink = { uri -> LinkService.pair(this, uri); scanning = false },
                        onBack = { scanning = false },
                    )
                } else if (page == 1) {
                    BackHandler { page = 0 }
                    val remote = remember {
                        object : Remote {
                            override fun move(dx: Int, dy: Int) = LinkService.input { it.pointerMove(dx, dy) }
                            override fun button(button: Int, pressed: Boolean) = LinkService.input { it.pointerButton(button, pressed) }
                            override fun scroll(dx: Int, dy: Int) = LinkService.input { it.scroll(dx, dy) }
                            override fun key(code: Int, pressed: Boolean) = LinkService.input { it.key(code, pressed) }
                            override fun type(text: String) = LinkService.input { it.typeText(text) }
                            override fun run(id: Int) = LinkService.send(this@MainActivity, Outbound.RunCommand(id))
                        }
                    }
                    ControlScreen(remote, commands, commandNote, link is LinkState.Connected)
                } else {
                    val battery = readBattery()
                    DeviceScreen(
                        shown = DeviceShown(core.identity, core.pinned, core.failed, link, battery.first, battery.second, ringing, clipboardNote, notificationsEnabled(link), desktopMedia, phoneGranted, mirrorInput, storageShared, wholePhone),
                        onScan = { scanning = true },
                        onForget = { id -> LinkService.forget(this, id) },
                        onStopRinging = { LinkService.stopRinging(this) },
                        onNotificationAccess = { startActivity(Intent(android.provider.Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)) },
                        onPhoneAccess = { askPhone.launch(PhonePermissions.ALL) },
                        onMirrorInput = { startActivity(Intent(android.provider.Settings.ACTION_ACCESSIBILITY_SETTINGS)) },
                        onStorage = { pickTree.launch(null) },
                        onWholePhone = { startActivity(org.celestina.magnetita.storage.PhoneStorage.allFilesIntent(this)) },
                        onControl = { page = 1 },
                        onMedia = { button -> desktopMedia?.let { LinkService.send(this, Outbound.MediaControl(MediaCommand(it.player, button, null, null))) } },
                    )
                }
            }
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) LinkService.focused(this) else LinkService.blurred(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        offerPairing(intent)
    }

    private suspend fun readCore(): FromCore = withContext(Dispatchers.IO) {
        runCatching { FromCore(Core.identity(applicationContext), Core.pinned(applicationContext), false) }
            .getOrElse { FromCore(null, emptyList(), true) }
    }

    /** Re-read on every link change, which is also when the person comes back from Settings. */
    @Composable
    private fun notificationsEnabled(@Suppress("UNUSED_PARAMETER") link: LinkState): Boolean =
        PhoneNotifications.enabled(applicationContext)

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
