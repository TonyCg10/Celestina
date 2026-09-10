package org.celestina.magnetita.notifications

/**
 * Which of the phone's notifications are worth the desktop. Pure, so the
 * JVM tests pin it: not our own, not the ongoing ones (a music player, a
 * download, a foreground service), not group summaries, which repeat what
 * their children say.
 */
object NotificationPolicy {
    const val OWN_PACKAGE = "org.celestina.magnetita"

    fun mirrors(packageName: String, ongoing: Boolean, groupSummary: Boolean, hasContent: Boolean): Boolean =
        packageName != OWN_PACKAGE && !ongoing && !groupSummary && hasContent
}
