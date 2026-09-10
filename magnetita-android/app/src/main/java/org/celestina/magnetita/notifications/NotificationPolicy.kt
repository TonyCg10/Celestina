package org.celestina.magnetita.notifications

/**
 * Which of the phone's notifications are worth the desktop. Pure, so the
 * JVM tests pin it: not our own, not the ongoing ones (a download, a
 * foreground service), not group summaries, which repeat what their
 * children say, and a player's now-playing one only when the person asks
 * for it; that one is never dismissed from the desktop, because Android
 * stops the playback when its notification goes.
 */
object NotificationPolicy {
    const val OWN_PACKAGE = "org.celestina.magnetita"

    fun mirrors(
        packageName: String,
        ongoing: Boolean,
        groupSummary: Boolean,
        hasContent: Boolean,
        media: Boolean = false,
        mediaWanted: Boolean = false,
    ): Boolean =
        packageName != OWN_PACKAGE && !groupSummary && hasContent &&
            (if (media) mediaWanted else !ongoing)

    /** Whether the desktop's dismissal may reach the phone's notification. */
    fun dismissable(media: Boolean): Boolean = !media
}
