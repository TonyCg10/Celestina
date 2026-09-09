package org.celestina.magnetita.link

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.os.BatteryManager
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.celestina.magnetita.MainActivity
import org.celestina.magnetita.R
import org.celestina.magnetita.core.Core

/**
 * The session lives here, in a foreground service of the `connectedDevice`
 * type, so Doze and app switches do not cut it — the spikes of `MAG-P0`
 * lost every long session that lived in an activity.
 *
 * The service owns one [LinkController] and feeds it the real ports: the
 * Rust core, `NsdManager`, and the battery broadcast. Screens read
 * [state] and never touch the session.
 */
class LinkService : LifecycleService() {
    private var loop: Job? = null
    private var controller: LinkController? = null
    private val ringer by lazy { Ringer(applicationContext) }
    private val clipboardPolicy = ClipboardPolicy()
    private val clipboard by lazy { getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager }

    // Fires only while this app has the focus: the in-front half of the
    // outbound clipboard. The tile and the share target cover the rest.
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener { offerClipboard() }

    private val batteryReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            controller?.batteryChanged()
        }
    }

    override fun onCreate() {
        super.onCreate()
        startForeground(NOTIFICATION_ID, notification(getString(R.string.state_searching)), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        registerReceiver(batteryReceiver, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        clipboard.addPrimaryClipChangedListener(clipListener)
        val phone = Core.phone(applicationContext)
        val c = LinkController(
            connector = CoreConnector(phone),
            discovery = NsdDiscovery(applicationContext),
            battery = { readBattery() },
            log = { android.util.Log.i(TAG, it) },
        )
        controller = c
        loop = lifecycleScope.launch {
            launch { c.state.collect { s -> _state.value = s; update(notification(describe(s))) } }
            launch {
                c.signals.collect { signal ->
                    when (signal) {
                        DesktopSignal.Ring -> { ringer.start(); _ringing.value = true }
                        DesktopSignal.StopRinging -> { ringer.stop(); _ringing.value = false }
                        is DesktopSignal.ClipboardText -> receiveClipboard(signal.text)
                        DesktopSignal.ClipboardRequested -> offerClipboard()
                        else -> {}
                    }
                }
            }
            c.run()
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        intent?.getStringExtra(EXTRA_PAIR_URI)?.let { controller?.pair(it) }
        when (intent?.action) {
            ACTION_STOP_RINGING -> { ringer.stop(); _ringing.value = false }
            ACTION_SEND_CLIPBOARD -> offerClipboard(intent.getStringExtra(EXTRA_TEXT))
            ACTION_FOCUS -> offerClipboard()
            ACTION_FORGET -> intent.getStringExtra(EXTRA_DEVICE_ID)?.let { id ->
                lifecycleScope.launch(kotlinx.coroutines.Dispatchers.IO) {
                    runCatching { Core.forget(applicationContext, id) }
                    controller?.disconnect()
                }
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        loop?.cancel()
        runCatching { clipboard.removePrimaryClipChangedListener(clipListener) }
        ringer.stop()
        _ringing.value = false
        runCatching { unregisterReceiver(batteryReceiver) }
        _state.value = LinkState.NeedsPairing
        super.onDestroy()
    }

    /** The desktop's clipboard arrived: it becomes this phone's, once. */
    private fun receiveClipboard(text: String) {
        clipboardPolicy.received(text)
        runCatching { clipboard.setPrimaryClip(ClipData.newPlainText("Magnetita", text)) }
        _clipboardNote.value = getString(R.string.clipboard_received)
    }

    /**
     * Sends `text`, or the primary clip when null; the read yields nothing
     * unless this app is in front, which is Android's rule, not ours.
     */
    private fun offerClipboard(text: String? = null) {
        val value = text ?: runCatching { clipboard.primaryClip?.getItemAt(0)?.coerceToText(this)?.toString() }.getOrNull()
        if (clipboardPolicy.offer(value)) {
            controller?.sendClipboard(value!!)
            _clipboardNote.value = getString(R.string.clipboard_sent)
        }
    }

    private fun readBattery(): Pair<Int, Boolean> {
        val status = applicationContext.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val level = status?.getIntExtra(BatteryManager.EXTRA_LEVEL, -1) ?: -1
        val scale = status?.getIntExtra(BatteryManager.EXTRA_SCALE, 100) ?: 100
        val plugged = status?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0
        val percent = if (level >= 0 && scale > 0) (level * 100 / scale) else 0
        return percent to (plugged != 0)
    }

    private fun describe(s: LinkState): String = when (s) {
        LinkState.NeedsPairing -> getString(R.string.state_needs_pairing)
        LinkState.Searching -> getString(R.string.state_searching)
        is LinkState.Connecting -> getString(R.string.state_connecting)
        is LinkState.Connected -> getString(R.string.state_connected_to, s.desktopName)
        is LinkState.Waiting -> getString(R.string.state_waiting)
    }

    private fun notification(text: String): Notification {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(NotificationChannel(CHANNEL, getString(R.string.channel_link), NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    private fun update(n: Notification) {
        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).notify(NOTIFICATION_ID, n)
    }

    companion object {
        private const val TAG = "magnetita-link"
        private const val CHANNEL = "link"
        private const val NOTIFICATION_ID = 1

        private val _state = MutableStateFlow<LinkState>(LinkState.NeedsPairing)

        /** The link's state for the screens; the service is its only writer. */
        val state: StateFlow<LinkState> = _state.asStateFlow()

        private const val EXTRA_PAIR_URI = "pair_uri"
        private const val EXTRA_DEVICE_ID = "device_id"
        private const val ACTION_STOP_RINGING = "org.celestina.magnetita.STOP_RINGING"
        private const val ACTION_FORGET = "org.celestina.magnetita.FORGET"
        private const val ACTION_SEND_CLIPBOARD = "org.celestina.magnetita.SEND_CLIPBOARD"
        private const val ACTION_FOCUS = "org.celestina.magnetita.FOCUS"
        private const val EXTRA_TEXT = "text"

        private val _clipboardNote = MutableStateFlow("")

        /** The last clipboard exchange, in the person's words, for the screen. */
        val clipboardNote: StateFlow<String> = _clipboardNote.asStateFlow()

        /** Sends `text` as this phone's clipboard, from the tile or a share. */
        fun sendClipboard(context: Context, text: String) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_SEND_CLIPBOARD).putExtra(EXTRA_TEXT, text))
        }

        /** The app came to the front: the clipboard may be read now. */
        fun focused(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_FOCUS))
        }

        private val _ringing = MutableStateFlow(false)

        /** True while the find sound plays. */
        val ringing: StateFlow<Boolean> = _ringing.asStateFlow()

        fun stopRinging(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_STOP_RINGING))
        }

        /** Drops the pin and the session of one desktop. */
        fun forget(context: Context, deviceId: String) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_FORGET).putExtra(EXTRA_DEVICE_ID, deviceId))
        }

        fun start(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java))
        }

        /** Pairs with the desktop a QR (or the system camera's link) named. */
        fun pair(context: Context, uri: String) {
            context.startForegroundService(Intent(context, LinkService::class.java).putExtra(EXTRA_PAIR_URI, uri))
        }
    }
}
