package org.celestina.magnetita.settings

import android.content.Context

/** The person's switches, one preference file; read wherever they gate. */
class Preferences(context: Context) {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    /** Mirror a player's now-playing notification to the desktop. Off: it repeats the media card. */
    var mediaNotifications: Boolean
        get() = prefs.getBoolean(KEY_MEDIA_NOTIFICATIONS, false)
        set(value) = prefs.edit().putBoolean(KEY_MEDIA_NOTIFICATIONS, value).apply()

    companion object {
        private const val KEY_MEDIA_NOTIFICATIONS = "media_notifications"
    }
}
