package org.celestina.magnetita.link

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.coroutines.CoroutineContext

/**
 * The link's one loop: while pinned, find the desktop, connect, hold the
 * session, and come back when it drops. Pure Kotlin over the two ports, so
 * the JVM tests drive it with fakes; the service gives it the real ones.
 *
 * Rules the loop keeps:
 * - Only advertised ids that are pinned are dialled; a stranger on the LAN
 *   is never contacted.
 * - A battery report goes out on connect and whenever the source changes.
 * - A dropped session waits the schedule's delay and starts over; a session
 *   that was up resets the schedule.
 * - Everything blocking runs on `io`, never on the caller's dispatcher.
 */
class LinkController(
    private val connector: Connector,
    private val discovery: Discovery,
    private val battery: BatterySource,
    private val io: CoroutineContext = Dispatchers.IO,
    private val pollMs: Long = 1_000,
    private val log: (String) -> Unit = {},
) {
    private val _state = MutableStateFlow<LinkState>(LinkState.NeedsPairing)
    val state: StateFlow<LinkState> = _state.asStateFlow()

    private val _signals = MutableSharedFlow<DesktopSignal>(extraBufferCapacity = 16)

    /** What the desktop asked, one per envelope; the service reacts to it. */
    val signals: SharedFlow<DesktopSignal> = _signals.asSharedFlow()

    private val dropRequested = MutableStateFlow(false)
    private val outboundClipboard = MutableStateFlow<String?>(null)

    /** Hand the loop a clipboard text; the session sends it on the next poll. */
    fun sendClipboard(text: String) {
        outboundClipboard.value = text
    }

    /** Ends the current session (after a forget); the loop decides what follows. */
    fun disconnect() {
        dropRequested.value = true
    }

    private val batteryChanged = MutableStateFlow(0)
    private val pairRequest = MutableStateFlow<String?>(null)

    /** Hand the loop a scanned `magnetita://pair` URI; it pairs on its next turn. */
    fun pair(uri: String) {
        pairRequest.value = uri
    }

    /** Tell the loop the battery moved; it reports on the next poll. */
    fun batteryChanged() {
        batteryChanged.value += 1
    }

    /** The first address a pairing link names, for the screen; the core parses the rest. */
    private fun pairAddress(uri: String): String =
        uri.substringAfter("addr=", "").substringBefore('&').substringBefore(',').ifBlank { "pairing" }

    /** Runs until cancelled. */
    suspend fun run() = coroutineScope {
        val backoff = Backoff()
        while (isActive) {
            val uri = pairRequest.value
            if (uri != null) {
                pairRequest.value = null
                val address = pairAddress(uri)
                _state.value = LinkState.Connecting(address)
                val paired = withContext(io) { connector.pair(uri) }
                paired.onSuccess { live ->
                    backoff.reset()
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
                val wait = backoff.nextDelayMs()
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
                val wait = backoff.nextDelayMs()
                _state.value = LinkState.Waiting("no address answered", wait)
                delay(wait)
                continue
            }
            backoff.reset()
            _state.value = LinkState.Connected(live.desktopId, live.desktopName, address)
            val reason = hold(live)
            val wait = backoff.nextDelayMs()
            _state.value = LinkState.Waiting(reason, wait)
            delay(wait)
        }
    }

    /** Pumps one session until it ends; returns why. */
    private suspend fun hold(live: LiveSession): String = coroutineScope {
        val (level, charging) = battery.read()
        withContext(io) { live.reportBattery(level, charging) }
        var seen = batteryChanged.value
        dropRequested.value = false
        val reporter = launch {
            while (isActive) {
                delay(pollMs)
                if (dropRequested.value) {
                    withContext(io) { live.close("forgotten") }
                    break
                }
                outboundClipboard.value?.let { text ->
                    outboundClipboard.value = null
                    if (!withContext(io) { live.sendClipboard(text) }) return@launch
                }
                val now = batteryChanged.value
                if (now != seen) {
                    seen = now
                    val (l, c) = battery.read()
                    if (!withContext(io) { live.reportBattery(l, c) }) break
                }
            }
        }
        val reason = try {
            while (isActive) {
                val event = live.next(pollMs) ?: continue
                log("desktop: ${event.description}")
                val signal = DesktopSignal.of(event)
                if (signal == DesktopSignal.BatteryRequested) {
                    val (l, c) = battery.read()
                    withContext(io) { live.reportBattery(l, c) }
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
            reporter.cancel()
        }
        reason
    }
}
