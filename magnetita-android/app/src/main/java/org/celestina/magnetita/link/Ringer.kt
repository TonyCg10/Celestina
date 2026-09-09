package org.celestina.magnetita.link

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.os.VibrationEffect
import android.os.VibratorManager

/**
 * The find capability on the phone: the alarm sound at full volume plus a
 * vibration pattern, until the desktop or the person stops it.
 *
 * A `MediaPlayer` in this process, not a `Ringtone`: on Samsung a ringtone
 * plays through the system's remote player, reports `isPlaying` false, and
 * a second ring leaked a first one that nothing could stop. Here the state
 * is ours, one player at a time, and `stop` always wins.
 */
class Ringer(private val context: Context) {
    private var player: MediaPlayer? = null
    private var restoreVolume: Int? = null
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
    }

    fun stop() {
        player?.let { runCatching { it.stop() }; it.release() }
        player = null
        vibrator.cancel()
        restoreVolume?.let { audio.setStreamVolume(AudioManager.STREAM_ALARM, it, 0) }
        restoreVolume = null
    }
}
