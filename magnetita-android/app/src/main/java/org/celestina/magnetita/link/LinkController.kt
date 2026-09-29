package org.celestina.magnetita.link

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withContext
import kotlin.coroutines.CoroutineContext

/**
 * The link's one loop: while pinned, find the desktop, connect, hold the
 * session, and come back when it drops. Pure Kotlin over the ports, so
 * the JVM tests drive it with fakes; the service gives it the real ones.
 *
 * Rules the loop keeps:
 * - Only advertised ids that are pinned are dialled; a stranger on the LAN
 *   is never contacted.
 * - A battery report goes out on connect and whenever the source changes.
 * - A dropped session waits the schedule's delay and starts over; a session
 *   that was up resets the schedule.
 * - Everything blocking runs on `io`, never on the caller's dispatcher.
 * - The sender suspends on its queue and wakes for work, never on a timer
 *   (AND-7); an accepted file streams in a coroutine of its own, so the
 *   receive loop keeps answering the desktop while it goes (AND-2).
 */
class LinkController(
    private val connector: Connector,
    private val discovery: Discovery,
    private val battery: BatterySource,
    /** The wait between attempts; the core's schedule on the device. */
    private val schedule: RetrySchedule,
    private val files: FileSource = FileSource { null },
    /** Free bytes under a receive directory, which it creates; the core decides what fits. */
    private val freeSpace: (String) -> Long = { dir -> java.io.File(dir).apply { mkdirs() }.usableSpace },
    private val io: CoroutineContext = Dispatchers.IO,
    private val pollMs: Long = 1_000,
    private val log: (String) -> Unit = {},
) {
    private val _state = MutableStateFlow<LinkState>(LinkState.NeedsPairing)
    val state: StateFlow<LinkState> = _state.asStateFlow()

    private val _signals = MutableSharedFlow<DesktopSignal>(extraBufferCapacity = 16)

    /** What the desktop asked, one per envelope; the service reacts to it. */
    val signals: SharedFlow<DesktopSignal> = _signals.asSharedFlow()

    private val dropRequests = Channel<Unit>(Channel.CONFLATED)
    private val batteryChanges = Channel<Unit>(Channel.CONFLATED)
    private val outbound = Channel<Outbound>(capacity = 64, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** Hand the loop a clipboard text; the held session sends it at once. */
    fun sendClipboard(text: String) {
        outbound.trySend(Outbound.Clipboard(text))
    }

    /** Queue anything for the desktop; the held session sends in order. */
    fun send(op: Outbound) {
        outbound.trySend(op)
    }

    /** Ends the current session (after a forget); the loop decides what follows. */
    fun disconnect() {
        dropRequests.trySend(Unit)
    }

    private val pairRequest = MutableStateFlow<String?>(null)

    /** Hand the loop a scanned `magnetita://pair` URI; it pairs on its next turn. */
    fun pair(uri: String) {
        pairRequest.value = uri
    }

    /** Tell the loop the battery moved; the held session reports it. */
    fun batteryChanged() {
        batteryChanges.trySend(Unit)
    }

    /** The first address a pairing link names, for the screen; the core parses the rest. */
    private fun pairAddress(uri: String): String =
        uri.substringAfter("addr=", "").substringBefore('&').substringBefore(',').ifBlank { "pairing" }

    /** Offers waiting for the desktop's answer; written by the sender, read by the receive loop. */
    private val offered = java.util.concurrent.ConcurrentHashMap<Int, Outbound.File>()

    /** The session being held, for input that cannot wait for a poll. */
    @Volatile var live: LiveSession? = null
        private set

    /** Where offered files are received; null declines every offer. */
    @Volatile var receiveDir: String? = null

    /**
     * Streams one accepted file from `offset`, off the loop's dispatcher. A
     * source that fails (the provider went away, a read error) ends only this
     * upload: the desktop is told it was abandoned and the session goes on.
     */
    private suspend fun pump(live: LiveSession, transfer: Int, file: Outbound.File, offset: Long) {
        try {
            stream(live, transfer, file, offset)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            log("upload of ${file.name} failed: ${e.message}")
            withContext(io) { live.abandonTransfer(transfer) }
        }
    }

    private suspend fun stream(live: LiveSession, transfer: Int, file: Outbound.File, offset: Long) {
        withContext(io) {
            // A file that can no longer be opened is abandoned, not reported complete.
            val source = files.open(file.uri) ?: error("the file can no longer be read")
            source.use { input ->
                var skipped = 0L
                while (skipped < offset) {
                    val n = input.skip(offset - skipped)
                    if (n <= 0) break
                    skipped += n
                }
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    // A cancelled transfer (the desktop gave up, the session ended) stops here.
                    ensureActive()
                    val n = input.read(buffer)
                    if (n < 0) break
                    if (n > 0 && !live.writeTransfer(transfer, buffer.copyOf(n))) {
                        // Gone: nothing to tell. Refused: this upload fails alone.
                        if (live.gone) return@withContext
                        error("the core refused the transfer's bytes")
                    }
                }
            }
            live.finishTransfer(transfer)
            log("sent ${file.name}")
        }
    }

    /** Runs until cancelled. */
    suspend fun run() = coroutineScope {

        while (isActive) {
            val uri = pairRequest.value
            if (uri != null) {
                pairRequest.value = null
                val address = pairAddress(uri)
                _state.value = LinkState.Connecting(address)
                val paired = withContext(io) { connector.pair(uri) }
                paired.onSuccess { live ->
                    schedule.reset()
                    _state.value = LinkState.Connected(live.desktopId, live.desktopName, address)
                    val reason = hold(live)
                    _state.value = LinkState.Waiting(reason, pollMs)
                    delay(pollMs)
                }.onFailure { log("pair: ${it.message}") }
                continue
            }
            val pinned = withContext(io) { connector.pinnedIds() }
            if (pinned.isEmpty()) {
                _state.value = LinkState.NeedsPairing
                delay(pollMs)
                continue
            }
            _state.value = LinkState.Searching
            val candidates = discovery.browse().filter { it.deviceId in pinned }
            if (candidates.isEmpty()) {
                val wait = schedule.nextDelayMs()
                _state.value = LinkState.Waiting("desktop not advertised", wait)
                delay(wait)
                continue
            }
            var session: LiveSession? = null
            var address = ""
            for (c in candidates) {
                _state.value = LinkState.Connecting(c.address)
                val attempt = withContext(io) { connector.connect(c.address) }
                attempt.onSuccess { session = it; address = c.address }.onFailure { log("connect ${c.address}: ${it.message}") }
                if (session != null) break
            }
            val live = session
            if (live == null) {
                val wait = schedule.nextDelayMs()
                _state.value = LinkState.Waiting("no address answered", wait)
                delay(wait)
                continue
            }
            schedule.reset()
            _state.value = LinkState.Connected(live.desktopId, live.desktopName, address)
            val reason = hold(live)
            val wait = schedule.nextDelayMs()
            _state.value = LinkState.Waiting(reason, wait)
            delay(wait)
        }
    }

    /** What wakes the sender. */
    private sealed interface Wake {
        data object Drop : Wake
        data object Battery : Wake
        data class Send(val op: Outbound) : Wake
    }

    /** Sends one queued operation; false when the session is gone. */
    private fun sendNow(live: LiveSession, op: Outbound): Boolean = when (op) {
        is Outbound.Clipboard -> live.sendClipboard(op.text)
        is Outbound.Notification -> live.sendNotification(op.note)
        is Outbound.NotificationGone -> live.sendNotificationGone(op.key)
        is Outbound.File -> {
            val id = live.offerFile(op.name, op.size, op.mime)
            if (id != null) offered[id] = op
            id != null
        }
        is Outbound.Media -> live.sendMediaState(op.state)
        is Outbound.MediaControl -> live.sendMediaCommand(op.command.player, op.command.button, op.command.seekMs, op.command.volume)
        Outbound.MediaWanted -> live.requestMedia()
        is Outbound.StorageState -> live.sendStorageState(op.available)
        is Outbound.Contacts -> live.sendContacts(op.version, op.contacts, op.removed, op.complete)
        is Outbound.Conversations -> live.sendSmsConversations(op.list)
        is Outbound.Thread -> live.sendSmsThread(op.thread, op.messages)
        is Outbound.Received -> live.sendSmsReceived(op.thread, op.message)
        is Outbound.Call -> live.sendCallEvent(op.state, op.number, op.name, op.timestampMs)
        is Outbound.RunCommand -> live.runCommand(op.id)
    }

    /** Pumps one session until it ends; returns why. */
    private suspend fun hold(live: LiveSession): String = coroutineScope {
        this@LinkController.live = live
        // A forget or a battery change seen before this session is not its own.
        dropRequests.tryReceive()
        batteryChanges.tryReceive()
        val (level, charging) = battery.read()
        withContext(io) { live.reportBattery(level, charging) }
        // Suspends until there is work: a forget first, then the battery, then the queue.
        val reporter = launch {
            while (isActive) {
                val wake = select<Wake> {
                    dropRequests.onReceive { Wake.Drop }
                    batteryChanges.onReceive { Wake.Battery }
                    outbound.onReceive { Wake.Send(it) }
                }
                val sent = when (wake) {
                    Wake.Drop -> {
                        withContext(io) { live.close("forgotten") }
                        break
                    }
                    Wake.Battery -> {
                        val (l, c) = battery.read()
                        withContext(io) { live.reportBattery(l, c) }
                    }
                    is Wake.Send -> withContext(io) { sendNow(live, wake.op) }
                }
                // Only the session's end stops the sender; a refusal (a capability
                // the desktop declined, a bound) drops that one message.
                if (!sent) {
                    if (live.gone) break
                    val what = if (wake is Wake.Send) wake.op::class.simpleName else wake::class.simpleName
                    log("not sent, refused by the core: $what")
                }
            }
        }
        // Accepted files, each streaming in its own coroutine of this session.
        val pumps = HashMap<Int, Job>()
        val reason = try {
            while (isActive) {
                val event = live.next(pollMs) ?: continue
                log("desktop: ${event.description}")
                val signal = event.signal
                if (signal == DesktopSignal.BatteryRequested) {
                    val (l, c) = battery.read()
                    withContext(io) { live.reportBattery(l, c) }
                }
                if (signal is DesktopSignal.ShareAccepted) {
                    offered.remove(signal.transfer)?.let { file ->
                        pumps.values.removeAll { it.isCompleted }
                        pumps[signal.transfer] = launch { pump(live, signal.transfer, file, signal.offset) }
                    }
                }
                if (signal is DesktopSignal.ShareEnded && !signal.complete) {
                    offered.remove(signal.transfer)
                    pumps.remove(signal.transfer)?.cancel()
                }
                if (signal is DesktopSignal.ShareOffered) {
                    val dir = receiveDir
                    withContext(io) { if (dir != null) live.acceptFile(signal.transfer, dir, freeSpace(dir)) else live.rejectFile(signal.transfer) }
                }
                _signals.tryEmit(signal)
            }
            "cancelled"
        } catch (e: CancellationException) {
            withContext(io) { live.close("stopping") }
            throw e
        } catch (e: Exception) {
            e.message ?: "session ended"
        } finally {
            this@LinkController.live = null
            reporter.cancel()
            pumps.values.forEach { it.cancel() }
        }
        reason
    }
}
