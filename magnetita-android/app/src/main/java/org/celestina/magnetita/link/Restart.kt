package org.celestina.magnetita.link

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * The link comes back on its own: after a boot, and after this application
 * is updated (which stops every service it ran). Both broadcasts are the
 * ones Android lets a foreground service start from, so the person never
 * has to open the application for the desktop to find the phone again.
 */
class Restart : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED -> LinkService.start(context)
        }
    }
}
