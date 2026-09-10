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
import org.celestina.magnetita.notifications.PhoneNotifications

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
    private val media by lazy { org.celestina.magnetita.media.PhoneMedia(applicationContext) }
    private val book by lazy { org.celestina.magnetita.phone.PhoneBook(applicationContext) }
    private val messages by lazy { org.celestina.magnetita.phone.Messages(applicationContext) }
    private val calls by lazy { org.celestina.magnetita.phone.Calls(applicationContext) }
    private val storage by lazy { org.celestina.magnetita.storage.PhoneStorage(applicationContext) }
    private val storageExecutor = java.util.concurrent.Executors.newSingleThreadExecutor()
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
        media.start()
        val phone = Core.phone(applicationContext)
        val c = LinkController(
            connector = CoreConnector(phone),
            discovery = NsdDiscovery(applicationContext),
            battery = { readBattery() },
            files = { uri -> runCatching { contentResolver.openInputStream(android.net.Uri.parse(uri)) }.getOrNull() },
            log = { android.util.Log.i(TAG, it) },
        )
        c.receiveDir = java.io.File(cacheDir, "received").absolutePath
        controller = c
        controllerRef = c
        loop = lifecycleScope.launch {
            launch {
                c.state.collect { s ->
                    _state.value = s
                    update(notification(describe(s)))
                    // The desktop mounts the phone only while a root is shared.
                    if (s is LinkState.Connected) c.send(Outbound.StorageState(storage.available()))
                }
            }
            launch {
                c.signals.collect { signal ->
                    when (signal) {
                        DesktopSignal.Ring -> { ringer.start(); _ringing.value = true }
                        DesktopSignal.StopRinging -> { ringer.stop(); _ringing.value = false }
                        is DesktopSignal.ClipboardText -> receiveClipboard(signal.text)
                        DesktopSignal.ClipboardRequested -> offerClipboard()
                        is DesktopSignal.NotificationDismiss -> PhoneNotifications.instance?.dismiss(signal.key)
                        is DesktopSignal.NotificationAction -> PhoneNotifications.instance?.press(signal.key, signal.action)
                        is DesktopSignal.NotificationReply -> PhoneNotifications.instance?.reply(signal.key, signal.text)
                        is DesktopSignal.FileReceived -> if (signal.complete) publishReceived(signal.path)
                        is DesktopSignal.DesktopMedia -> _desktopMedia.value = signal.state.takeIf { it.player.isNotEmpty() } ?: (if (_desktopMedia.value?.player == signal.state.player) null else _desktopMedia.value)
                        is DesktopSignal.MediaControl -> media.drive(signal.command)
                        DesktopSignal.MediaRequested -> media.report()
                        is DesktopSignal.ContactsRequested -> offMain { book.send(signal.since) }
                        DesktopSignal.ConversationsRequested -> offMain { messages.sendConversations() }
                        is DesktopSignal.ThreadRequested -> offMain { messages.sendThread(signal.thread, signal.beforeMs, signal.limit) }
                        is DesktopSignal.SmsSendRequested -> offMain { messages.send(signal.thread, signal.body) }
                        is DesktopSignal.CallCommand -> calls.act(signal.action)
                        is DesktopSignal.Commands -> _commands.value = signal.list
                        is DesktopSignal.CommandResult -> _commandNote.value = getString(if (signal.ok) R.string.command_ok else R.string.command_failed)
                        is DesktopSignal.ShareText -> receiveClipboard(signal.text)
                        is DesktopSignal.MirrorStart -> askMirror(signal.options)
                        DesktopSignal.MirrorStop -> org.celestina.magnetita.mirror.MirrorService.stop(this@LinkService)
                        is DesktopSignal.MirrorTouched -> org.celestina.magnetita.mirror.MirrorService.touch(signal.touch.phase, signal.touch.x, signal.touch.y, signal.touch.pointer)
                        is DesktopSignal.MirrorGlobal -> org.celestina.magnetita.mirror.MirrorInput.instance?.global(signal.action)
                        is DesktopSignal.MirrorKey -> {}
                        is DesktopSignal.Storage -> {
                            val live = controllerRef?.live
                            if (live != null) storageExecutor.execute { storage.answer(signal.request, live) }
                        }
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
            ACTION_OUTBOUND -> { while (true) { val op = pending.pollFirst() ?: break; controller?.send(op) } }
            ACTION_CALL_ENDED -> calls.restoreRinger()
            ACTION_PHONE_GRANTED -> offMain { book.send(0); messages.sendConversations() }
            ACTION_STORAGE_GRANTED -> controller?.send(Outbound.StorageState(storage.available()))
            ACTION_FOCUS -> { inFront = true; offerClipboard() }
            ACTION_BLUR -> inFront = false
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
        storageExecutor.shutdownNow()
        loop?.cancel()
        runCatching { clipboard.removePrimaryClipChangedListener(clipListener) }
        media.stop()
        ringer.stop()
        _ringing.value = false
        runCatching { unregisterReceiver(batteryReceiver) }
        _state.value = LinkState.NeedsPairing
        super.onDestroy()
    }

    /** The desktop's clipboard arrived: it becomes this phone's, once. */
    /**
     * The desktop wants the screen: the consent activity opens straight
     * away when this app is in front, else a notification offers it; a
     * service may not raise an activity from the background.
     */
    private fun askMirror(options: org.celestina.magnetita.link.MirrorOptions) {
        val consent = org.celestina.magnetita.mirror.MirrorConsentActivity.intent(this, options)
        if (inFront) {
            runCatching { startActivity(consent) }.onSuccess { return }
        }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(MIRROR_CHANNEL, getString(R.string.channel_mirror_ask), NotificationManager.IMPORTANCE_HIGH))
        val open = PendingIntent.getActivity(this, 2, consent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        manager.notify(MIRROR_ASK_ID, Notification.Builder(this, MIRROR_CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(getString(R.string.mirror_ask_title))
            .setContentText(getString(R.string.mirror_ask_text))
            .setContentIntent(open)
            .setAutoCancel(true)
            .setTimeoutAfter(60_000)
            .build())
    }

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

    /** Provider reads and sends never run on the main thread. */
    private fun offMain(work: () -> Unit) {
        lifecycleScope.launch(kotlinx.coroutines.Dispatchers.IO) { runCatching(work).onFailure { android.util.Log.w(TAG, "phone adapter: $it") } }
    }

    /** A complete file in the cache becomes a public download, then a notification. */
    private fun publishReceived(path: String) {
        lifecycleScope.launch(kotlinx.coroutines.Dispatchers.IO) {
            val saved = runCatching { org.celestina.magnetita.share.Downloads.publish(applicationContext, java.io.File(path)) }.getOrNull()
            _clipboardNote.value = if (saved != null) getString(R.string.file_received, saved) else getString(R.string.file_failed)
            if (saved != null) {
                val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                manager.createNotificationChannel(NotificationChannel(FILES_CHANNEL, getString(R.string.channel_files), NotificationManager.IMPORTANCE_DEFAULT))
                manager.notify(saved.hashCode(), Notification.Builder(this@LinkService, FILES_CHANNEL)
                    .setContentTitle(getString(R.string.file_received_title))
                    .setContentText(saved)
                    .setSmallIcon(R.drawable.ic_launcher_monochrome)
                    .setAutoCancel(true)
                    .build())
            }
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
        private const val FILES_CHANNEL = "files"
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
        private const val ACTION_CALL_ENDED = "org.celestina.magnetita.CALL_ENDED"
        private const val ACTION_PHONE_GRANTED = "org.celestina.magnetita.PHONE_GRANTED"

        /** A call ended: undo a mute the desktop asked for. */
        fun callEnded(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_CALL_ENDED))
        }

        /** The phone grants arrived: contacts and conversations go out now. */
        fun phoneGranted(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_PHONE_GRANTED))
        }

        private val _clipboardNote = MutableStateFlow("")

        private val _commands = MutableStateFlow<List<Pair<Int, String>>>(emptyList())

        /** The desktop's registered commands, ids and names. */
        val commands: StateFlow<List<Pair<Int, String>>> = _commands.asStateFlow()

        private val _commandNote = MutableStateFlow("")
        val commandNote: StateFlow<String> = _commandNote.asStateFlow()

        @Volatile private var controllerRef: LinkController? = null

        /** Input that cannot wait for a poll goes straight to the held session, off the main thread. */
        fun input(work: (org.celestina.magnetita.link.LiveSession) -> Unit) {
            val live = controllerRef?.live ?: return
            inputExecutor.execute { runCatching { work(live) } }
        }

        private val inputExecutor = java.util.concurrent.Executors.newSingleThreadExecutor()

        private val _desktopMedia = MutableStateFlow<MediaState?>(null)

        /** The desktop's player the screen shows, or null when none. */
        val desktopMedia: StateFlow<MediaState?> = _desktopMedia.asStateFlow()

        /** The last clipboard exchange, in the person's words, for the screen. */
        val clipboardNote: StateFlow<String> = _clipboardNote.asStateFlow()

        // Notifications are objects, not intent extras: the listener drops them
        // here and pokes the service, which hands them to the loop in order.
        private const val ACTION_OUTBOUND = "org.celestina.magnetita.OUTBOUND"
        private val pending = java.util.concurrent.ConcurrentLinkedDeque<Outbound>()

        /** Queue anything for the desktop from another component. */
        fun send(context: Context, op: Outbound) {
            pending.addLast(op)
            while (pending.size > 64) pending.pollFirst()
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_OUTBOUND))
        }

        /** Sends `text` as this phone's clipboard, from the tile or a share. */
        fun sendClipboard(context: Context, text: String) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_SEND_CLIPBOARD).putExtra(EXTRA_TEXT, text))
        }

        /** The app came to the front: the clipboard may be read now. */
        fun focused(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_FOCUS))
        }

        private const val ACTION_STORAGE_GRANTED = "org.celestina.magnetita.STORAGE_GRANTED"

        /** A root was shared (or withdrawn): the desktop learns at once. */
        fun storageGranted(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_STORAGE_GRANTED))
        }

        /** The app left the front. */
        fun blurred(context: Context) {
            context.startForegroundService(Intent(context, LinkService::class.java).setAction(ACTION_BLUR))
        }

        @Volatile private var inFront = false
        private const val ACTION_BLUR = "org.celestina.magnetita.BLUR"
        private const val MIRROR_CHANNEL = "mirror-ask"
        private const val MIRROR_ASK_ID = 4

        /** The held session, for the mirror's own writer thread; null when down. */
        fun liveSession(): LiveSession? = controllerRef?.live

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
