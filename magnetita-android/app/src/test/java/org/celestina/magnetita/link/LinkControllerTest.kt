package org.celestina.magnetita.link

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The loop's rules, driven with fakes and virtual time. */
@OptIn(ExperimentalCoroutinesApi::class)
class LinkControllerTest {
    private class FakeSession(override val desktopId: String, override val desktopName: String) : LiveSession {
        val reports = mutableListOf<Pair<Int, Boolean>>()
        var alive = true
        var closed: String? = null
        val incoming = ArrayDeque<LinkEvent>()
        override fun reportBattery(level: Int, charging: Boolean): Boolean { reports += level to charging; return alive }
        val clips = mutableListOf<String>()
        override fun sendClipboard(text: String): Boolean { clips += text; return alive }
        override suspend fun next(timeoutMs: Long): LinkEvent? {
            if (!alive) throw IllegalStateException("connection lost")
            incoming.removeFirstOrNull()?.let { return it }
            delay(timeoutMs)
            if (!alive) throw IllegalStateException("connection lost")
            return null
        }
        override fun close(reason: String) { closed = reason; alive = false }
    }

    private class FakeConnector(var pins: List<String>, val sessions: ArrayDeque<FakeSession>) : Connector {
        val dialled = mutableListOf<String>()
        override fun pinnedIds() = pins
        override fun connect(address: String): Result<LiveSession> {
            dialled += address
            return sessions.removeFirstOrNull()?.let { Result.success(it) } ?: Result.failure(IllegalStateException("refused"))
        }
        val paired = mutableListOf<String>()
        override fun pair(uri: String): Result<LiveSession> {
            paired += uri
            pins = pins + "desk"
            return sessions.removeFirstOrNull()?.let { Result.success(it) } ?: Result.failure(IllegalStateException("refused"))
        }
    }

    @Test
    fun aScannedUriPairsHoldsThatSessionAndThenReconnectsThroughDiscovery() = runTest {
        val first = FakeSession("desk", "Celestina")
        val second = FakeSession("desk", "Celestina")
        val connector = FakeConnector(emptyList(), ArrayDeque(listOf(first, second)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 9 to false }, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(150)
        assertEquals(LinkState.NeedsPairing, controller.state.value)
        controller.pair("magnetita://pair?v=1&id=desk&addr=10.0.0.1:1760")
        advanceTimeBy(300)
        assertEquals(listOf("magnetita://pair?v=1&id=desk&addr=10.0.0.1:1760"), connector.paired)
        assertEquals(LinkState.Connected("desk", "Celestina", "10.0.0.1:1760"), controller.state.value)
        first.alive = false
        advanceTimeBy(1_000)
        assertEquals(listOf("10.0.0.1:1760"), connector.dialled)
        assertEquals(LinkState.Connected("desk", "Celestina", "10.0.0.1:1760"), controller.state.value)
        job.cancel()
    }

    @Test
    fun withoutPinsItAsksForPairingAndDialsNobody() = runTest {
        val connector = FakeConnector(emptyList(), ArrayDeque())
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 50 to false }, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(1_000)
        assertEquals(LinkState.NeedsPairing, controller.state.value)
        assertTrue(connector.dialled.isEmpty())
        job.cancel()
    }

    @Test
    fun itDialsOnlyPinnedDesktopsReportsBatteryAndRetriesAfterADrop() = runTest {
        val first = FakeSession("desk", "Celestina")
        val second = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(first, second)))
        val discovery = Discovery { listOf(Advertised("stranger", "10.0.0.9:1760"), Advertised("desk", "10.0.0.1:1760")) }
        val controller = LinkController(connector, discovery, { 42 to true }, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(500)
        assertEquals(listOf("10.0.0.1:1760"), connector.dialled)
        assertEquals(LinkState.Connected("desk", "Celestina", "10.0.0.1:1760"), controller.state.value)
        assertEquals(listOf(42 to true), first.reports)

        // The battery moves: one more report on the next poll.
        controller.batteryChanged()
        advanceTimeBy(300)
        assertEquals(2, first.reports.size)

        // The desktop drops the session: the loop waits, then dials again.
        first.alive = false
        advanceTimeBy(200)
        assertTrue(controller.state.value is LinkState.Waiting)
        advanceTimeBy(2_000)
        assertEquals(2, connector.dialled.size)
        assertEquals(LinkState.Connected("desk", "Celestina", "10.0.0.1:1760"), controller.state.value)
        job.cancel()
    }

    @Test
    fun aRefusedDialBacksOffWithTheSchedule() = runTest {
        val connector = FakeConnector(listOf("desk"), ArrayDeque())
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 1 to false }, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(100)
        val waiting = controller.state.first { it is LinkState.Waiting } as LinkState.Waiting
        assertEquals(Backoff.FIRST_MS, waiting.retryMs)
        advanceTimeBy(Backoff.FIRST_MS + 100)
        val again = controller.state.first { it is LinkState.Waiting } as LinkState.Waiting
        assertEquals(Backoff.FIRST_MS * 2, again.retryMs)
        job.cancel()
    }

    @Test
    fun aBatteryRequestIsAnsweredAndAForgetClosesTheSession() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(200)
        session.incoming += LinkEvent(1, 2, "battery: requested")
        advanceTimeBy(200)
        assertEquals(listOf(30 to false, 30 to false), session.reports)
        controller.disconnect()
        advanceTimeBy(200)
        assertEquals("forgotten", session.closed)
        assertTrue(controller.state.value is LinkState.Waiting)
        job.cancel()
    }

    @Test
    fun aClipboardHandedToTheLoopGoesOutOnTheNextPollAndSignalsArriveDecoded() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, io = coroutineContext, pollMs = 100)
        val seen = mutableListOf<DesktopSignal>()
        val job = launch { controller.run() }
        val watcher = launch { controller.signals.collect { seen += it } }
        advanceTimeBy(200)
        controller.sendClipboard("copied on the phone")
        advanceTimeBy(200)
        assertEquals(listOf("copied on the phone"), session.clips)
        session.incoming += LinkEvent(2, 1, "clipboard: 5 bytes", "hello")
        session.incoming += LinkEvent(2, 2, "clipboard: requested")
        advanceTimeBy(300)
        assertEquals(listOf<DesktopSignal>(DesktopSignal.ClipboardText("hello"), DesktopSignal.ClipboardRequested), seen)
        watcher.cancel()
        job.cancel()
    }

    @Test
    fun theScheduleDoublesToAMinuteAndResets() {
        val b = Backoff()
        val delays = (0..10).map { b.nextDelayMs() }
        assertEquals(listOf(250L, 500L, 1000L, 2000L, 4000L), delays.take(5))
        assertEquals(60_000L, delays[9])
        b.reset()
        assertEquals(250L, b.nextDelayMs())
    }
}
