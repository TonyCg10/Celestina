package org.celestina.magnetita.mirror

import android.content.Context
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.projection.MediaProjection
import android.os.Handler
import android.os.HandlerThread
import android.view.Surface
import org.celestina.magnetita.link.LiveSession
import org.celestina.magnetita.link.MirrorOptions
import org.celestina.magnetita.link.MirrorStreams

/**
 * One mirror: a virtual display fed by the projection into an HEVC (or
 * H.264) encoder whose output goes, as it comes, down the link's video
 * stream. The encoder writes on its own thread; a blocking write stalls
 * the encoder rather than piling frames up, which is the pacing the
 * desktop wants (`MAG-P6`).
 *
 * The encoder is tuned the way scrcpy tunes its own: a key frame every
 * ten seconds, the last frame repeated every 100 ms while the screen is
 * still, at most the asked frame rate. Every access unit is followed by
 * an access unit delimiter, so the desktop's parser emits the frame the
 * moment it ends instead of waiting for the next one. A rotation restarts
 * the capture at the new size and tells the desktop again.
 */
class ScreenMirror(
    private val context: Context,
    private val projection: MediaProjection,
    private val live: LiveSession,
    private val options: MirrorOptions,
    private val onStopped: (reason: String) -> Unit,
) {
    private var codec: MediaCodec? = null
    private var display: VirtualDisplay? = null
    private var surface: Surface? = null
    private val thread = HandlerThread("mirror-encoder").apply { start() }
    @Volatile private var open = false

    /** The picture's size once started, in the phone's pixels before scaling. */
    var screen: Pair<Int, Int> = 0 to 0
        private set

    /** The streamed size. */
    var picture: Pair<Int, Int> = 0 to 0
        private set

    private val displayManager by lazy { context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager }
    private val rotationListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(displayId: Int) {}
        override fun onDisplayRemoved(displayId: Int) {}
        override fun onDisplayChanged(displayId: Int) {
            if (displayId != android.view.Display.DEFAULT_DISPLAY || !open) return
            val bounds = (context.getSystemService(Context.WINDOW_SERVICE) as android.view.WindowManager).maximumWindowMetrics.bounds
            val now = bounds.width() to bounds.height()
            if (now != screen) restart()
        }
    }

    /** The screen turned: the capture starts again at the new size. */
    private fun restart() {
        if (!open) return
        open = false
        runCatching { display?.release() }
        runCatching { codec?.stop() }
        runCatching { codec?.release() }
        runCatching { surface?.release() }
        live.closeStream(MirrorStreams.VIDEO)
        if (!begin()) stop("could not restart after rotation")
    }

    /** Starts, and tells the desktop the shape; false when nothing could start. */
    fun start(): Boolean {
        if (!begin()) return false
        displayManager.registerDisplayListener(rotationListener, Handler(thread.looper))
        return true
    }

    private fun begin(): Boolean {
        val metrics = context.resources.displayMetrics
        val bounds = (context.getSystemService(Context.WINDOW_SERVICE) as android.view.WindowManager).maximumWindowMetrics.bounds
        screen = bounds.width() to bounds.height()
        picture = MirrorGeometry.fit(screen.first, screen.second, options.maxSize)
        val mime = if (options.codec == 1) MediaFormat.MIMETYPE_VIDEO_AVC else MediaFormat.MIMETYPE_VIDEO_HEVC
        val format = MediaFormat.createVideoFormat(mime, picture.first, picture.second).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_BIT_RATE, options.bitrateKbps * 1000)
            setInteger(MediaFormat.KEY_FRAME_RATE, options.fps.coerceIn(10, 120))
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 10)
            setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
            setInteger(MediaFormat.KEY_LATENCY, 1)
            setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR)
            setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, REPEAT_FRAME_US)
            setFloat(MediaFormat.KEY_MAX_FPS_TO_ENCODER, options.fps.coerceIn(10, 120).toFloat())
        }
        val delimiter = if (options.codec == 1) AUD_H264 else AUD_HEVC
        val encoder = runCatching { MediaCodec.createEncoderByType(mime) }.getOrNull() ?: return false
        val handler = Handler(thread.looper)
        encoder.setCallback(object : MediaCodec.Callback() {
            override fun onInputBufferAvailable(codec: MediaCodec, index: Int) {}
            override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) {}
            override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) { stop("encoder: ${e.message}") }
            override fun onOutputBufferAvailable(codec: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
                val buffer = runCatching { codec.getOutputBuffer(index) }.getOrNull()
                if (buffer != null && info.size > 0 && open) {
                    val config = info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0
                    outputs++
                    outputBytes += info.size
                    if (info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0) keyOutputs++
                    val now = android.os.SystemClock.elapsedRealtime()
                    if (now - lastReport > 5000) {
                        android.util.Log.i("ScreenMirror", "encoder: $outputs units, $keyOutputs keys, $outputBytes bytes in ${now - lastReport} ms")
                        outputs = 0; keyOutputs = 0; outputBytes = 0; lastReport = now
                    }
                    val bytes = ByteArray(info.size + if (config) 0 else delimiter.size)
                    buffer.position(info.offset)
                    buffer.get(bytes, 0, info.size)
                    if (!config) delimiter.copyInto(bytes, info.size)
                    if (!live.writeTransfer(MirrorStreams.VIDEO, bytes)) {
                        stop("stream closed")
                        return
                    }
                }
                runCatching { codec.releaseOutputBuffer(index, false) }
                if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) stop("encoder ended")
            }
        }, handler)
        return runCatching {
            encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            val input = encoder.createInputSurface()
            projection.registerCallback(object : MediaProjection.Callback() {
                override fun onStop() { stop("projection ended") }
            }, handler)
            val virtual = projection.createVirtualDisplay(
                "magnetita-mirror", picture.first, picture.second, metrics.densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, input, null, handler,
            ) ?: error("no virtual display")
            if (!live.openStream(MirrorStreams.VIDEO)) error("no stream")
            open = true
            encoder.start()
            codec = encoder
            surface = input
            display = virtual
            live.sendMirrorStarted(picture.first, picture.second, options.codec, false)
        }.onFailure {
            runCatching { encoder.release() }
        }.isSuccess
    }

    private var outputs = 0
    private var keyOutputs = 0
    private var outputBytes = 0L
    private var lastReport = 0L

    /** Asks the encoder for a key frame on its next output. */
    fun requestKeyframe() {
        android.util.Log.i("ScreenMirror", "key frame requested by the desktop")
        val encoder = codec ?: return
        runCatching {
            encoder.setParameters(android.os.Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0) })
        }
    }

    /** Ends everything once; later calls do nothing. */
    fun stop(reason: String) {
        if (!open) return
        open = false
        runCatching { displayManager.unregisterDisplayListener(rotationListener) }
        runCatching { display?.release() }
        runCatching { codec?.stop() }
        runCatching { codec?.release() }
        runCatching { surface?.release() }
        runCatching { projection.stop() }
        live.closeStream(MirrorStreams.VIDEO)
        thread.quitSafely()
        onStopped(reason)
    }

    companion object {
        /** scrcpy's value: a still screen still yields ten frames a second. */
        private const val REPEAT_FRAME_US = 100_000L

        /** Access unit delimiters, Annex B: HEVC NAL type 35, H.264 NAL type 9. */
        private val AUD_HEVC = byteArrayOf(0, 0, 0, 1, 0x46, 0x01, 0x50)
        private val AUD_H264 = byteArrayOf(0, 0, 0, 1, 0x09, 0xF0.toByte())
    }
}
