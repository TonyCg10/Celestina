package org.celestina.magnetita.media

import android.content.ComponentName
import android.content.Context
import android.media.MediaMetadata
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.MediaCommand
import org.celestina.magnetita.link.MediaState
import org.celestina.magnetita.link.Outbound
import org.celestina.magnetita.notifications.PhoneNotifications

/**
 * This phone's players for the desktop, and the desktop's buttons back.
 * Android hands the active `MediaSession`s to whoever holds notification
 * access, the same grant the notification listener needs, so this rides on
 * it: no grant, no players, and the device screen already says so.
 *
 * The first active session is the one the desktop's card shows; every
 * change of its metadata or playback state goes out, and a command names
 * the player so a later one still lands on the right session.
 */
class PhoneMedia(private val context: Context) {
    private val manager = context.getSystemService(Context.MEDIA_SESSION_SERVICE) as MediaSessionManager
    private val listenerComponent = ComponentName(context, PhoneNotifications::class.java)
    private var controllers: List<MediaController> = emptyList()
    private val callbacks = HashMap<MediaController, MediaController.Callback>()
    private var lastSent: MediaState? = null

    private val sessionsListener = MediaSessionManager.OnActiveSessionsChangedListener { list -> adopt(list.orEmpty()) }

    /** Starts watching; safe without the grant, it simply sees nothing. */
    fun start() {
        runCatching {
            manager.addOnActiveSessionsChangedListener(sessionsListener, listenerComponent)
            adopt(manager.getActiveSessions(listenerComponent))
        }
    }

    fun stop() {
        runCatching { manager.removeOnActiveSessionsChangedListener(sessionsListener) }
        adopt(emptyList())
    }

    /** Sends the current state again, on the desktop's request. */
    fun report() {
        LinkService.send(context, Outbound.Media(current()))
    }

    /** The desktop's button, seek or volume, on the player it names. */
    fun drive(command: MediaCommand) {
        val controller = controllers.firstOrNull { label(it) == command.player } ?: controllers.firstOrNull() ?: return
        val transport = controller.transportControls
        when (command.button) {
            0 -> transport.play()
            1 -> transport.pause()
            2 -> if (controller.playbackState?.state == PlaybackState.STATE_PLAYING) transport.pause() else transport.play()
            3 -> transport.skipToNext()
            4 -> transport.skipToPrevious()
            5 -> transport.stop()
        }
        command.seekMs?.let { transport.seekTo(it) }
        command.volume?.let { volume ->
            val info = controller.playbackInfo ?: return@let
            controller.setVolumeTo((volume.coerceIn(0, 100) * info.maxVolume / 100), 0)
        }
    }

    private fun adopt(list: List<MediaController>) {
        callbacks.forEach { (c, cb) -> runCatching { c.unregisterCallback(cb) } }
        callbacks.clear()
        controllers = list
        list.forEach { c ->
            val cb = object : MediaController.Callback() {
                override fun onMetadataChanged(metadata: MediaMetadata?) = changed()
                override fun onPlaybackStateChanged(state: PlaybackState?) = changed()
                override fun onSessionDestroyed() = changed()
            }
            runCatching { c.registerCallback(cb) }
            callbacks[c] = cb
        }
        changed()
    }

    private fun changed() {
        val state = current()
        if (state != lastSent) {
            lastSent = state
            LinkService.send(context, Outbound.Media(state))
        }
    }

    /** The first active player's state, or the empty state when none. */
    fun current(): MediaState {
        val c = controllers.firstOrNull() ?: return EMPTY
        return snapshot(c)
    }

    private fun snapshot(c: MediaController): MediaState {
        val m = c.metadata
        val p = c.playbackState
        val actions = p?.actions ?: 0L
        val info = c.playbackInfo
        val volume = if (info != null && info.maxVolume > 0) info.currentVolume * 100 / info.maxVolume else 0
        return MediaState(
            player = label(c),
            title = m?.getString(MediaMetadata.METADATA_KEY_TITLE).orEmpty(),
            artist = m?.getString(MediaMetadata.METADATA_KEY_ARTIST).orEmpty(),
            album = m?.getString(MediaMetadata.METADATA_KEY_ALBUM).orEmpty(),
            playing = p?.state == PlaybackState.STATE_PLAYING,
            positionMs = p?.position ?: 0L,
            lengthMs = m?.getLong(MediaMetadata.METADATA_KEY_DURATION) ?: 0L,
            canSeek = actions and PlaybackState.ACTION_SEEK_TO != 0L,
            canNext = actions and PlaybackState.ACTION_SKIP_TO_NEXT != 0L,
            canPrevious = actions and PlaybackState.ACTION_SKIP_TO_PREVIOUS != 0L,
            volume = volume,
        )
    }

    private fun label(c: MediaController): String = runCatching {
        context.packageManager.getApplicationLabel(context.packageManager.getApplicationInfo(c.packageName, 0)).toString()
    }.getOrDefault(c.packageName)

    companion object {
        val EMPTY = MediaState("", "", "", "", false, 0, 0, false, false, false, 0)
    }
}
