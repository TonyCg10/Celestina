package org.celestina.magnetita.notifications

import android.app.Notification
import android.app.RemoteInput
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.BitmapDrawable
import android.graphics.drawable.Drawable
import android.os.Bundle
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.Outbound
import org.celestina.magnetita.link.PhoneNotification
import java.io.ByteArrayOutputStream

/**
 * The phone's notifications for the desktop, and the desktop's presses
 * back. Android grants this listener only when the person enables it in
 * the notification access settings; until then it is never bound, and
 * the device screen says so.
 */
class PhoneNotifications : NotificationListenerService() {
    private val live = HashMap<String, StatusBarNotification>()
    private val iconsSent = HashSet<String>()

    override fun onListenerConnected() {
        instance = this
        activeNotifications?.forEach { post(it) }
    }

    override fun onListenerDisconnected() {
        if (instance === this) instance = null
        live.clear()
        iconsSent.clear()
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) = post(sbn)

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        if (live.remove(sbn.key) != null) LinkService.send(this, Outbound.NotificationGone(sbn.key))
    }

    private fun post(sbn: StatusBarNotification) {
        val n = sbn.notification
        val extras = n.extras
        val title = extras.text(Notification.EXTRA_TITLE)
        val body = extras.text(Notification.EXTRA_BIG_TEXT).ifBlank { extras.text(Notification.EXTRA_TEXT) }
        val groupSummary = n.flags and Notification.FLAG_GROUP_SUMMARY != 0
        if (!NotificationPolicy.mirrors(sbn.packageName, sbn.isOngoing, groupSummary, title.isNotBlank() || body.isNotBlank())) return
        live[sbn.key] = sbn
        val actions = n.actions.orEmpty()
        val icon = if (iconsSent.add(sbn.packageName)) appIcon(sbn.packageName) else null
        LinkService.send(
            this,
            Outbound.Notification(
                PhoneNotification(
                    key = sbn.key,
                    appName = appLabel(sbn.packageName),
                    title = title,
                    body = body,
                    timestampMs = sbn.postTime,
                    replyable = actions.any { it.remoteInputs?.isNotEmpty() == true },
                    actions = actions.map { it.title?.toString() ?: "" },
                    icon = icon,
                ),
            ),
        )
    }

    /** The desktop dismissed it: dismiss it here too. */
    fun dismiss(key: String) {
        live.remove(key)
        runCatching { cancelNotification(key) }
    }

    /** The desktop pressed button `index`. */
    fun press(key: String, index: Int) {
        val action = live[key]?.notification?.actions?.getOrNull(index) ?: return
        runCatching { action.actionIntent.send() }
    }

    /** The desktop answered inline. */
    fun reply(key: String, text: String) {
        val action = live[key]?.notification?.actions?.firstOrNull { it.remoteInputs?.isNotEmpty() == true } ?: return
        val inputs = action.remoteInputs ?: return
        val intent = Intent()
        val results = Bundle().apply { inputs.forEach { putCharSequence(it.resultKey, text) } }
        RemoteInput.addResultsToIntent(inputs, intent, results)
        runCatching { action.actionIntent.send(this, 0, intent) }
    }

    private fun Bundle.text(key: String): String = getCharSequence(key)?.toString().orEmpty()

    private fun appLabel(packageName: String): String = runCatching {
        packageManager.getApplicationLabel(packageManager.getApplicationInfo(packageName, 0)).toString()
    }.getOrDefault(packageName)

    /** The app's icon as a small PNG, within the wire's 64 KiB. */
    private fun appIcon(packageName: String): ByteArray? = runCatching {
        val drawable: Drawable = packageManager.getApplicationIcon(packageName)
        val bitmap = (drawable as? BitmapDrawable)?.bitmap ?: Bitmap.createBitmap(96, 96, Bitmap.Config.ARGB_8888).also {
            val canvas = Canvas(it)
            drawable.setBounds(0, 0, 96, 96)
            drawable.draw(canvas)
        }
        val scaled = if (bitmap.width > 96) Bitmap.createScaledBitmap(bitmap, 96, 96, true) else bitmap
        ByteArrayOutputStream().use { out ->
            scaled.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }.takeIf { it.size <= 64 * 1024 }
    }.getOrNull()

    companion object {
        /** The bound listener, if the person enabled notification access. */
        @Volatile var instance: PhoneNotifications? = null
            private set

        fun enabled(context: android.content.Context): Boolean =
            android.provider.Settings.Secure.getString(context.contentResolver, "enabled_notification_listeners")
                ?.split(':')?.any { it.startsWith(NotificationPolicy.OWN_PACKAGE + "/") } == true
    }
}
