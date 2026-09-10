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
        val notes = mutableListOf<String>()
        override fun sendNotification(note: PhoneNotification): Boolean { notes += "post:" + note.key; return alive }
        override fun sendNotificationGone(key: String): Boolean { notes += "gone:$key"; return alive }
        val offers = mutableListOf<String>()
        val written = java.io.ByteArrayOutputStream()
        var finished = false
        val accepted = mutableListOf<Pair<Int, String>>()
        override fun offerFile(name: String, size: Long, mime: String): Int? { offers += "$name:$size"; return 9 }
        override fun writeTransfer(transfer: Int, bytes: ByteArray): Boolean { written.write(bytes); return alive }
        override fun finishTransfer(transfer: Int): Boolean { finished = true; return alive }
        override fun acceptFile(transfer: Int, dir: String): Boolean { accepted += transfer to dir; return alive }
        override fun rejectFile(transfer: Int): Boolean = alive
        val media = mutableListOf<String>()
        override fun sendMediaState(state: MediaState): Boolean { media += "state:" + state.player; return alive }
        override fun sendMediaCommand(player: String, button: Int?, seekMs: Long?, volume: Int?): Boolean { media += "cmd:$player:$button"; return alive }
        override fun requestMedia(): Boolean { media += "request"; return alive }
        val phone = mutableListOf<String>()
        override fun sendContacts(version: Long, contacts: List<PhoneContact>, removed: List<Long>, complete: Boolean): Boolean { phone += "contacts:${contacts.size}:$complete"; return alive }
        override fun sendSmsConversations(list: List<SmsConversation>): Boolean { phone += "conversations:${list.size}"; return alive }
        override fun sendSmsThread(thread: Long, messages: List<SmsMessage>): Boolean { phone += "thread:$thread:${messages.size}"; return alive }
        override fun sendSmsReceived(thread: Long, message: SmsMessage): Boolean { phone += "received:$thread"; return alive }
        override fun sendCallEvent(state: Int, number: String, name: String?, timestampMs: Long): Boolean { phone += "call:$state:$number"; return alive }
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
        controller.send(Outbound.Notification(PhoneNotification("k1", "Messages", "Ana", "hi", 1, true, listOf("Mark read"))))
        controller.send(Outbound.NotificationGone("k1"))
        controller.send(Outbound.Media(MediaState("YT", "Song", "Band", "", true, 0, 0, false, true, true, 50)))
        controller.send(Outbound.MediaControl(MediaCommand("mpv", 3, null, null)))
        controller.send(Outbound.MediaWanted)
        controller.send(Outbound.Contacts(1, listOf(PhoneContact(1, 1, "BEGIN:VCARD")), emptyList(), true))
        controller.send(Outbound.Received(9, SmsMessage(1, false, "600", "hi", 1)))
        controller.send(Outbound.Call(0, "600", null, 1))
        advanceTimeBy(200)
        assertEquals(listOf("contacts:1:true", "received:9", "call:0:600"), session.phone)
        assertEquals(listOf("post:k1", "gone:k1"), session.notes)
        assertEquals(listOf("state:YT", "cmd:mpv:3", "request"), session.media)
        session.incoming += LinkEvent(2, 1, "clipboard: 5 bytes", "hello")
        session.incoming += LinkEvent(2, 2, "clipboard: requested")
        session.incoming += LinkEvent(3, 3, "notification: action", key = "k1", action = 0)
        session.incoming += LinkEvent(3, 4, "notification: reply", text = "on my way", key = "k1")
        session.incoming += LinkEvent(3, 2, "notification: dismiss", key = "k1")
        advanceTimeBy(600)
        assertEquals(
            listOf<DesktopSignal>(
                DesktopSignal.ClipboardText("hello"),
                DesktopSignal.ClipboardRequested,
                DesktopSignal.NotificationAction("k1", 0),
                DesktopSignal.NotificationReply("k1", "on my way"),
                DesktopSignal.NotificationDismiss("k1"),
            ),
            seen,
        )
        watcher.cancel()
        job.cancel()
    }

    @Test
    fun anOfferedFileIsStreamedFromTheAcceptedOffsetAndOffersAreAcceptedIntoTheDir() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val payload = ByteArray(200_000) { (it % 251).toByte() }
        val controller = LinkController(
            connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false },
            files = { uri -> if (uri == "content://photo") payload.inputStream() else null },
            io = coroutineContext, pollMs = 100,
        )
        controller.receiveDir = "/tmp/received"
        val job = launch { controller.run() }
        advanceTimeBy(200)
        controller.send(Outbound.File("content://photo", "photo.bin", payload.size.toLong(), "application/octet-stream"))
        advanceTimeBy(200)
        assertEquals(listOf("photo.bin:200000"), session.offers)
        session.incoming += LinkEvent(5, 2, "share: accepted", transfer = 9, offset = 150_000)
        advanceTimeBy(300)
        assertTrue(session.finished)
        assertEquals(50_000, session.written.size())
        assertTrue(payload.copyOfRange(150_000, 200_000).contentEquals(session.written.toByteArray()))
        session.incoming += LinkEvent(5, 1, "share: offer", text = "doc.pdf", transfer = 3, size = 10)
        advanceTimeBy(300)
        assertEquals(listOf(3 to "/tmp/received"), session.accepted)
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
