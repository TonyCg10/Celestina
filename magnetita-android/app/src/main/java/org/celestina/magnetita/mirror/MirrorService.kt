package org.celestina.magnetita.mirror

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.projection.MediaProjectionManager
import android.os.IBinder
import androidx.core.content.IntentCompat
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.celestina.magnetita.R
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.MirrorOptions

/**
 * The mirror's foreground service, of the `mediaProjection` type Android
 * requires while the screen is captured. It starts with the consent the
 * [MirrorConsentActivity] collected and stops when the desktop, the
 * person or the projection says so.
 */
class MirrorService : Service() {
    private var mirror: ScreenMirror? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> { stopMirror("stopped"); return START_NOT_STICKY }
        }
        val consent = intent?.let { IntentCompat.getParcelableExtra(it, EXTRA_CONSENT, Intent::class.java) }
        val options = intent?.let { readOptions(it) }
        if (consent == null || options == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        startForeground(NOTIFICATION_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
        val live = LinkService.liveSession()
        if (live == null) {
            stopMirror("no link")
            return START_NOT_STICKY
        }
        val manager = getSystemService(Context.MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        val projection = runCatching { manager.getMediaProjection(android.app.Activity.RESULT_OK, consent) }.getOrNull()
        if (projection == null) {
            stopMirror("no projection")
            return START_NOT_STICKY
        }
        mirror?.stop("replaced")
        val started = ScreenMirror(applicationContext, projection, live, options) { reason ->
            _state.value = MirrorState.Idle
            mirror = null
            current = null
            LinkService.input { it.sendMirrorStop() }
            stopSelf()
            android.util.Log.i(TAG, "mirror ended: $reason")
        }
        if (started.start()) {
            mirror = started
            current = started
            _state.value = MirrorState.Streaming(started.picture.first, started.picture.second)
            geometry = started.picture to started.screen
        } else {
            stopMirror("could not start")
        }
        return START_NOT_STICKY
    }

    private fun stopMirror(reason: String) {
        val m = mirror
        mirror = null
        current = null
        geometry = null
        if (m != null) m.stop(reason) else { _state.value = MirrorState.Idle; stopSelf() }
    }

    override fun onDestroy() {
        mirror?.stop("service destroyed")
        mirror = null
        geometry = null
        super.onDestroy()
    }

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, getString(R.string.channel_mirror), NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(
            this, 1, Intent(this, MirrorService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(getString(R.string.mirror_streaming))
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, getString(R.string.mirror_stop), stop).build())
            .build()
    }

    sealed interface MirrorState {
        data object Idle : MirrorState
        data class Streaming(val width: Int, val height: Int) : MirrorState
    }

    companion object {
        private const val TAG = "MirrorService"
        private const val CHANNEL = "mirror"
        private const val NOTIFICATION_ID = 3
        const val ACTION_STOP = "org.celestina.magnetita.MIRROR_STOP"
        const val EXTRA_CONSENT = "consent"
        private const val EXTRA_MAX = "max"
        private const val EXTRA_FPS = "fps"
        private const val EXTRA_BITRATE = "bitrate"
        private const val EXTRA_CODEC = "codec"
        private const val EXTRA_AUDIO = "audio"

        private val _state = MutableStateFlow<MirrorState>(MirrorState.Idle)
        val state: StateFlow<MirrorState> = _state.asStateFlow()

        /** The streaming mirror's picture and screen sizes, for touches. */
        @Volatile private var geometry: Pair<Pair<Int, Int>, Pair<Int, Int>>? = null
        @Volatile private var current: ScreenMirror? = null

        fun putOptions(intent: Intent, options: MirrorOptions): Intent = intent
            .putExtra(EXTRA_MAX, options.maxSize).putExtra(EXTRA_FPS, options.fps)
            .putExtra(EXTRA_BITRATE, options.bitrateKbps).putExtra(EXTRA_CODEC, options.codec)
            .putExtra(EXTRA_AUDIO, options.audio)

        fun readOptions(intent: Intent): MirrorOptions? {
            if (!intent.hasExtra(EXTRA_FPS)) return null
            return MirrorOptions(
                intent.getIntExtra(EXTRA_MAX, 1440), intent.getIntExtra(EXTRA_FPS, 60),
                intent.getIntExtra(EXTRA_BITRATE, 6000), intent.getIntExtra(EXTRA_CODEC, 0),
                intent.getBooleanExtra(EXTRA_AUDIO, false),
            )
        }

        /** Starts with the consent result and the desktop's options. */
        fun start(context: Context, consent: Intent, options: MirrorOptions) {
            context.startForegroundService(
                putOptions(Intent(context, MirrorService::class.java), options).putExtra(EXTRA_CONSENT, consent),
            )
        }

        /** The desktop's window opened mid-stream: the next frame is a key frame. */
        fun keyframe() {
            current?.requestKeyframe()
        }

        fun stop(context: Context) {
            if (_state.value is MirrorState.Idle) return
            context.startService(Intent(context, MirrorService::class.java).setAction(ACTION_STOP))
        }

        /** A touch from the desktop, in the picture's pixels, to the accessibility service. */
        fun touch(phase: Int, x: Int, y: Int, pointer: Int) {
            val (picture, screen) = geometry ?: run {
                android.util.Log.w(TAG, "touch with no mirror geometry")
                return
            }
            val (sx, sy) = MirrorGeometry.toScreen(x, y, picture, screen)
            val input = MirrorInput.instance
            if (input == null) {
                android.util.Log.w(TAG, "touch with no accessibility service")
                return
            }
            input.touch(phase, sx, sy, pointer)
        }
    }
}
