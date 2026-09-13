package org.celestina.magnetita.mirror

import android.content.Context
import android.provider.Settings

/**
 * "Turn the screen off" as far as an application may: the brightness to
 * its minimum, in manual mode, for as long as the mirror streams, and the
 * previous brightness and mode back afterwards. A normal application
 * cannot switch the panel off while the phone stays unlocked; scrcpy does
 * it with a hidden API `adb` grants.
 */
class ScreenDimmer(private val context: Context) {
    private var previousBrightness = -1
    private var previousMode = -1

    fun dim(): Boolean {
        if (!Settings.System.canWrite(context)) {
            android.util.Log.i(TAG, "no settings permission: the screen stays lit")
            return false
        }
        val resolver = context.contentResolver
        previousBrightness = Settings.System.getInt(resolver, Settings.System.SCREEN_BRIGHTNESS, -1)
        previousMode = Settings.System.getInt(resolver, Settings.System.SCREEN_BRIGHTNESS_MODE, -1)
        return runCatching {
            Settings.System.putInt(resolver, Settings.System.SCREEN_BRIGHTNESS_MODE, Settings.System.SCREEN_BRIGHTNESS_MODE_MANUAL)
            Settings.System.putInt(resolver, Settings.System.SCREEN_BRIGHTNESS, 0)
        }.isSuccess
    }

    fun restore() {
        val resolver = context.contentResolver
        runCatching {
            if (previousBrightness >= 0) Settings.System.putInt(resolver, Settings.System.SCREEN_BRIGHTNESS, previousBrightness)
            if (previousMode >= 0) Settings.System.putInt(resolver, Settings.System.SCREEN_BRIGHTNESS_MODE, previousMode)
        }
    }

    companion object {
        private const val TAG = "ScreenDimmer"
    }
}
