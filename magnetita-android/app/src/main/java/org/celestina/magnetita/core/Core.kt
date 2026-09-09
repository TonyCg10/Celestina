package org.celestina.magnetita.core

import android.content.Context
import uniffi.magnetita_mobile.Identity
import uniffi.magnetita_mobile.MobilePhone
import uniffi.magnetita_mobile.PinnedDesktop

/**
 * The Kotlin face of the Rust core: one place that opens it, so the
 * identity lives under the app's own files directory and is opened once.
 * Every protocol rule is on the other side of these calls.
 */
object Core {
    @Volatile private var phone: MobilePhone? = null

    /** Opens (or returns) the phone; blocking, never from the UI thread. */
    fun phone(context: Context): MobilePhone =
        phone ?: synchronized(this) {
            phone ?: MobilePhone.open(context.filesDir.resolve("magnetita").absolutePath, deviceName()).also { phone = it }
        }

    fun identity(context: Context): Identity = phone(context).identity()

    fun pinned(context: Context): List<PinnedDesktop> = phone(context).pinned()

    fun forget(context: Context, deviceId: String) = phone(context).forget(deviceId)

    private fun deviceName(): String = android.os.Build.MODEL.ifBlank { "Android" }
}
