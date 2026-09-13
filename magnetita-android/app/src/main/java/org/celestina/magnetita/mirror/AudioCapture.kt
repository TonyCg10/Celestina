package org.celestina.magnetita.mirror

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.projection.MediaProjection
import org.celestina.magnetita.link.LiveSession
import org.celestina.magnetita.link.MirrorStreams

/**
 * The phone's playback, captured through the same projection the picture
 * uses and sent raw (PCM 16-bit, 48 kHz, stereo) down the link's audio
 * stream: no codec, so the desktop hears it as soon as it arrives. Needs
 * the microphone permission, which the consent screen asks for; apps that
 * opt out of capture stay silent, as Android decides.
 */
class AudioCapture(
    private val context: Context,
    private val projection: MediaProjection,
    private val live: LiveSession,
) {
    private var record: AudioRecord? = null
    private var thread: Thread? = null
    @Volatile private var running = false

    fun start(): Boolean {
        if (context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            android.util.Log.i(TAG, "no microphone permission: the sound stays on the phone")
            return false
        }
        val format = AudioFormat.Builder()
            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
            .setSampleRate(RATE)
            .setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
            .build()
        val capture = AudioPlaybackCaptureConfiguration.Builder(projection)
            .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
            .addMatchingUsage(AudioAttributes.USAGE_GAME)
            .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN)
            .build()
        val minimum = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT)
        val recorder = runCatching {
            AudioRecord.Builder()
                .setAudioFormat(format)
                .setAudioPlaybackCaptureConfig(capture)
                .setBufferSizeInBytes(maxOf(minimum, CHUNK_BYTES * 4))
                .build()
        }.getOrNull() ?: return false
        if (recorder.state != AudioRecord.STATE_INITIALIZED) {
            recorder.release()
            return false
        }
        if (!live.openStream(MirrorStreams.AUDIO)) {
            recorder.release()
            return false
        }
        record = recorder
        running = true
        recorder.startRecording()
        thread = Thread({
            val buffer = ByteArray(CHUNK_BYTES)
            while (running) {
                val n = recorder.read(buffer, 0, buffer.size)
                if (n <= 0) continue
                if (!live.writeTransfer(MirrorStreams.AUDIO, buffer.copyOf(n))) break
            }
        }, "mirror-audio").also { it.start() }
        android.util.Log.i(TAG, "capturing playback at $RATE Hz")
        return true
    }

    fun stop() {
        if (!running) return
        running = false
        runCatching { record?.stop() }
        runCatching { record?.release() }
        record = null
        runCatching { thread?.join(500) }
        thread = null
        live.closeStream(MirrorStreams.AUDIO)
    }

    companion object {
        private const val TAG = "AudioCapture"
        const val RATE = 48_000
        /** Ten milliseconds of stereo 16-bit at 48 kHz. */
        private const val CHUNK_BYTES = 48 * 2 * 2 * 10
    }
}
