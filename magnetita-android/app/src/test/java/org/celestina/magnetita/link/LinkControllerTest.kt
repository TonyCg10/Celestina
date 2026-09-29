package org.celestina.magnetita.link

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
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
        override val gone: Boolean get() = !alive
        var closed: String? = null
        val incoming = ArrayDeque<DesktopEvent>()

        /** Each event handed to the loop, with how many upload bytes had gone by then. */
        val handedAt = mutableListOf<Pair<DesktopSignal, Int>>()
        override fun reportBattery(level: Int, charging: Boolean): Boolean { reports += level to charging; return alive }
        val clips = mutableListOf<String>()
        override fun sendClipboard(text: String): Boolean { clips += text; return alive }
        val notes = mutableListOf<String>()
        /** The core refuses notifications, as against a desktop that did not negotiate them. */
        var declineNotes = false
        override fun sendNotification(note: PhoneNotification): Boolean { notes += "post:" + note.key; return alive && !declineNotes }
        override fun sendNotificationGone(key: String): Boolean { notes += "gone:$key"; return alive }
        val offers = mutableListOf<String>()
        val written = java.io.ByteArrayOutputStream()
        var finished = false
        val accepted = mutableListOf<Triple<Int, String, Long>>()
        override fun offerFile(name: String, size: Long, mime: String): Int? { offers += "$name:$size"; return 9 }
        override fun writeTransfer(transfer: Int, bytes: ByteArray): Boolean { written.write(bytes); return alive }
        override fun finishTransfer(transfer: Int): Boolean { finished = true; return alive }
        val abandoned = mutableListOf<Int>()
        override fun abandonTransfer(transfer: Int): Boolean { abandoned += transfer; return alive }
        override fun acceptFile(transfer: Int, dir: String, usableBytes: Long): Boolean { accepted += Triple(transfer, dir, usableBytes); return alive }
        override fun rejectFile(transfer: Int): Boolean = alive
        val media = mutableListOf<String>()
        override fun openStream(id: Int): Boolean = alive
        override fun closeStream(id: Int) {}
        override fun sendMirrorStarted(width: Int, height: Int, codec: Int, audio: Boolean): Boolean = alive
        override fun sendMirrorStop(): Boolean = alive
        override fun sendStorageState(available: Boolean): Boolean = alive
        override fun sendListing(request: Int, entries: List<StorageEntry>, more: Boolean, error: String): Boolean = alive
        override fun sendStatReply(request: Int, entry: StorageEntry?): Boolean = alive
        override fun sendData(request: Int, bytes: ByteArray, error: String): Boolean = alive
        override fun sendDone(request: Int, ok: Boolean, error: String): Boolean = alive
        override fun sendMediaState(state: MediaState): Boolean { media += "state:" + state.player; return alive }
        override fun sendMediaCommand(player: String, button: Int?, seekMs: Long?, volume: Int?): Boolean { media += "cmd:$player:$button"; return alive }
        override fun requestMedia(): Boolean { media += "request"; return alive }
        val phone = mutableListOf<String>()
        override fun sendContacts(version: Long, contacts: List<PhoneContact>, removed: List<Long>, complete: Boolean): Boolean { phone += "contacts:${contacts.size}:$complete"; return alive }
        override fun sendSmsConversations(list: List<SmsConversation>): Boolean { phone += "conversations:${list.size}"; return alive }
        override fun sendSmsThread(thread: Long, messages: List<SmsMessage>): Boolean { phone += "thread:$thread:${messages.size}"; return alive }
        override fun sendSmsReceived(thread: Long, message: SmsMessage): Boolean { phone += "received:$thread"; return alive }
        override fun sendCallEvent(state: Int, number: String, name: String?, timestampMs: Long): Boolean { phone += "call:$state:$number"; return alive }
        override fun runCommand(id: Int): Boolean { phone += "run:$id"; return alive }
        override fun pointerMove(dx: Int, dy: Int): Boolean = alive
        override fun pointerButton(button: Int, pressed: Boolean): Boolean = alive
        override fun scroll(dx: Int, dy: Int): Boolean = alive
        override fun key(code: Int, pressed: Boolean): Boolean = alive
        override fun typeText(text: String): Boolean = alive
        override suspend fun next(timeoutMs: Long): DesktopEvent? {
            if (!alive) throw IllegalStateException("connection lost")
            incoming.removeFirstOrNull()?.let { handedAt += it.signal to written.size(); return it }
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

    /** A schedule that hands out the delays it was given, then the last. */
    private class ScriptedSchedule(private val delays: List<Long> = listOf(250L)) : RetrySchedule {
        var next = 0
        var resets = 0
        override fun nextDelayMs(): Long = delays[next.coerceAtMost(delays.lastIndex)].also { next += 1 }
        override fun reset() { next = 0; resets += 1 }
    }

    private fun event(signal: DesktopSignal) = DesktopEvent(signal.toString(), signal)

    @Test
    fun aScannedUriPairsHoldsThatSessionAndThenReconnectsThroughDiscovery() = runTest {
        val first = FakeSession("desk", "Celestina")
        val second = FakeSession("desk", "Celestina")
        val connector = FakeConnector(emptyList(), ArrayDeque(listOf(first, second)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 9 to false }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
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
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 50 to false }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
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
        val controller = LinkController(connector, discovery, { 42 to true }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
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
    fun aRefusedDialWaitsTheSchedulesDelaysAndAnEstablishedSessionResetsIt() = runTest {
        val connector = FakeConnector(listOf("desk"), ArrayDeque())
        val schedule = ScriptedSchedule(listOf(300L, 700L))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 1 to false }, schedule, io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(100)
        val waiting = controller.state.first { it is LinkState.Waiting } as LinkState.Waiting
        assertEquals(300L, waiting.retryMs)
        advanceTimeBy(300 + 50)
        val again = controller.state.first { it is LinkState.Waiting } as LinkState.Waiting
        assertEquals(700L, again.retryMs)
        // A session that comes up starts the schedule over.
        connector.sessions += FakeSession("desk", "Celestina")
        advanceTimeBy(800)
        assertTrue(controller.state.value is LinkState.Connected)
        assertEquals(1, schedule.resets)
        job.cancel()
    }

    @Test
    fun aBatteryRequestIsAnsweredAndAForgetClosesTheSession() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(200)
        session.incoming += event(DesktopSignal.BatteryRequested)
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
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
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
        controller.send(Outbound.RunCommand(4))
        advanceTimeBy(200)
        assertEquals(listOf("contacts:1:true", "received:9", "call:0:600", "run:4"), session.phone)
        assertTrue("the held session is exposed for input", controller.live === session)
        assertEquals(listOf("post:k1", "gone:k1"), session.notes)
        assertEquals(listOf("state:YT", "cmd:mpv:3", "request"), session.media)
        session.incoming += event(DesktopSignal.ClipboardText("hello"))
        session.incoming += event(DesktopSignal.ClipboardRequested)
        session.incoming += event(DesktopSignal.NotificationAction("k1", 0))
        session.incoming += event(DesktopSignal.NotificationReply("k1", "on my way"))
        session.incoming += event(DesktopSignal.NotificationDismiss("k1"))
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
            connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(),
            files = { uri -> if (uri == "content://photo") payload.inputStream() else null },
            freeSpace = { 5_000L },
            io = coroutineContext, pollMs = 100,
        )
        controller.receiveDir = "/tmp/received"
        val job = launch { controller.run() }
        advanceTimeBy(200)
        controller.send(Outbound.File("content://photo", "photo.bin", payload.size.toLong(), "application/octet-stream"))
        advanceTimeBy(200)
        assertEquals(listOf("photo.bin:200000"), session.offers)
        session.incoming += event(DesktopSignal.ShareAccepted(9, 150_000))
        advanceTimeBy(300)
        assertTrue(session.finished)
        assertEquals(50_000, session.written.size())
        assertTrue(payload.copyOfRange(150_000, 200_000).contentEquals(session.written.toByteArray()))
        session.incoming += event(DesktopSignal.ShareOffered(3, "doc.pdf", 10))
        advanceTimeBy(300)
        assertEquals(listOf(Triple(3, "/tmp/received", 5_000L)), session.accepted)
        job.cancel()
    }

    /** AND-7: the sender suspends on its queue; nothing waits for a poll. */
    @Test
    fun aQueuedMessageGoesOutWithoutWaitingForAPoll() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(), io = coroutineContext, pollMs = 60_000)
        val job = launch { controller.run() }
        runCurrent()
        assertTrue(controller.state.value is LinkState.Connected)
        controller.sendClipboard("now")
        controller.batteryChanged()
        runCurrent()
        assertEquals(listOf("now"), session.clips)
        assertEquals(2, session.reports.size)
        controller.disconnect()
        runCurrent()
        assertEquals("forgotten", session.closed)
        job.cancel()
    }

    /**
     * AND-2: an accepted file streams in its own coroutine, so what the
     * desktop sends next is handled before the upload has gone; an upload
     * the desktop abandons is stopped.
     */
    @Test
    fun anUploadDoesNotHoldTheReceiveLoopAndStopsWhenAbandoned() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val payload = ByteArray(300_000) { 1 }
        val controller = LinkController(
            connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(),
            files = { payload.inputStream() },
            io = coroutineContext, pollMs = 100,
        )
        val job = launch { controller.run() }
        advanceTimeBy(200)
        controller.send(Outbound.File("content://big", "big.bin", payload.size.toLong(), "application/octet-stream"))
        advanceTimeBy(200)
        session.incoming += event(DesktopSignal.ShareAccepted(9, 0))
        session.incoming += event(DesktopSignal.Ring)
        advanceTimeBy(300)
        assertEquals("the loop took the ring before the upload ran", 0, session.handedAt.single { it.first == DesktopSignal.Ring }.second)
        assertEquals(300_000, session.written.size())
        assertTrue(session.finished)

        // A second upload the desktop abandons at once never streams.
        session.finished = false
        session.written.reset()
        controller.send(Outbound.File("content://big", "again.bin", payload.size.toLong(), "application/octet-stream"))
        advanceTimeBy(200)
        session.incoming += event(DesktopSignal.ShareAccepted(9, 0))
        session.incoming += event(DesktopSignal.ShareEnded(9, false))
        advanceTimeBy(300)
        assertEquals(0, session.written.size())
        assertTrue(!session.finished)
        job.cancel()
    }

    /**
     * A send the core refuses (a capability the desktop did not negotiate)
     * drops that message only: the sender goes on, and a forget still closes.
     */
    @Test
    fun aDeclinedSendDoesNotStopTheSender() = runTest {
        val live = FakeSession("desk", "Celestina")
        live.declineNotes = true
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(live)))
        val controller = LinkController(connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 42 to true }, ScriptedSchedule(), io = coroutineContext, pollMs = 100)
        val job = launch { controller.run() }
        advanceTimeBy(300)
        assertEquals(1, live.reports.size)
        controller.send(Outbound.Notification(PhoneNotification("k", "app", "t", "b", 0L, false, emptyList(), null, false)))
        advanceTimeBy(50)
        controller.sendClipboard("after the declined note")
        controller.batteryChanged()
        advanceTimeBy(300)
        assertEquals(listOf("post:k"), live.notes)
        assertEquals(listOf("after the declined note"), live.clips)
        assertEquals(2, live.reports.size)
        controller.disconnect()
        advanceTimeBy(300)
        assertEquals("forgotten", live.closed)
        job.cancel()
    }

    /** A read error in an uploaded file ends that upload alone, told to the desktop. */
    @Test
    fun anUploadReadErrorEndsOnlyThatUpload() = runTest {
        val session = FakeSession("desk", "Celestina")
        val connector = FakeConnector(listOf("desk"), ArrayDeque(listOf(session)))
        val broken = object : java.io.InputStream() {
            override fun read(): Int = throw java.io.IOException("provider gone")
            override fun read(b: ByteArray, off: Int, len: Int): Int = throw java.io.IOException("provider gone")
        }
        val controller = LinkController(
            connector, { listOf(Advertised("desk", "10.0.0.1:1760")) }, { 30 to false }, ScriptedSchedule(),
            files = { broken },
            io = coroutineContext, pollMs = 100,
        )
        var failure: Throwable? = null
        val job = launch { try { controller.run() } catch (e: Throwable) { failure = e } }
        advanceTimeBy(200)
        controller.send(Outbound.File("content://big", "big.bin", 10L, "application/octet-stream"))
        advanceTimeBy(200)
        session.incoming += event(DesktopSignal.ShareAccepted(9, 0))
        session.incoming += event(DesktopSignal.Ring)
        advanceTimeBy(300)
        assertEquals(null, failure)
        assertTrue(job.isActive)
        assertEquals(listOf(9), session.abandoned)
        assertTrue(!session.finished)
        assertEquals(null, session.closed)
        assertTrue(controller.state.value is LinkState.Connected)
        // The session goes on after the failed upload.
        controller.sendClipboard("still here")
        advanceTimeBy(100)
        assertEquals(listOf("still here"), session.clips)
        job.cancel()
    }
}
