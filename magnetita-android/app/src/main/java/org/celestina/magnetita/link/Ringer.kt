package org.celestina.magnetita.link

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.VibratorManager

/**
 * The find capability on the phone: the alarm sound at full volume plus a
 * vibration pattern, until the desktop or the person stops it, or
 * [RING_TIMEOUT_MS] passes.
 *
 * A `MediaPlayer` in this process, not a `Ringtone`: on Samsung a ringtone
 * plays through the system's remote player, reports `isPlaying` false, and
 * a second ring leaked a first one that nothing could stop. Here the state
 * is ours, one player at a time, and `stop` always wins.
 *
 * The timeout is the lesson of an incident (2026-09-24): the desktop's
 * stop command never reached a phone whose link had dropped, and the ring
 * ran until the author force-stopped the app over `adb`. A phone is meant
 * to be found within a couple of minutes; ringing forever helps no one and
 * once turned an alarm into an incident of its own.
 */
class Ringer(private val context: Context, private val onStopped: () -> Unit = {}) {
    private var player: MediaPlayer? = null
    private var restoreVolume: Int? = null
    private val main = Handler(Looper.getMainLooper())
    private val timeout = Runnable { stop() }
    private val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private val vibrator = (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as VibratorManager).defaultVibrator

    val ringing: Boolean get() = player != null

    fun start() {
        if (ringing) return
        val uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM)
            ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE) ?: return
        val made = runCatching {
            MediaPlayer().apply {
                setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_ALARM)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                        .build(),
                )
                setDataSource(context, uri)
                isLooping = true
                prepare()
                start()
            }
        }.getOrNull() ?: return
        restoreVolume = audio.getStreamVolume(AudioManager.STREAM_ALARM)
        audio.setStreamVolume(AudioManager.STREAM_ALARM, audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)
        player = made
        vibrator.vibrate(VibrationEffect.createWaveform(longArrayOf(0, 600, 300), 0))
        main.postDelayed(timeout, RING_TIMEOUT_MS)
    }

    /** Idempotent: a stop with nothing ringing does not notify [onStopped] again. */
    fun stop() {
        val wasRinging = ringing
        main.removeCallbacks(timeout)
        player?.let { runCatching { it.stop() }; it.release() }
        player = null
        vibrator.cancel()
        restoreVolume?.let { audio.setStreamVolume(AudioManager.STREAM_ALARM, it, 0) }
        restoreVolume = null
        if (wasRinging) onStopped()
    }

    private companion object {
        /** A phone is findable by ear well inside this; see the class doc. */
        const val RING_TIMEOUT_MS = 90_000L
    }
}
