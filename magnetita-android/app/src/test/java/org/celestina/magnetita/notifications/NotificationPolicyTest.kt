package org.celestina.magnetita.notifications

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Which notifications cross to the desktop. */
class NotificationPolicyTest {
    @Test
    fun onlyOtherAppsContentfulNonOngoingNonSummaryNotificationsCross() {
        assertTrue(NotificationPolicy.mirrors("com.whatsapp", ongoing = false, groupSummary = false, hasContent = true))
        assertFalse(NotificationPolicy.mirrors(NotificationPolicy.OWN_PACKAGE, ongoing = false, groupSummary = false, hasContent = true))
        assertFalse(NotificationPolicy.mirrors("com.spotify", ongoing = true, groupSummary = false, hasContent = true))
        assertFalse(NotificationPolicy.mirrors("com.whatsapp", ongoing = false, groupSummary = true, hasContent = true))
        assertFalse(NotificationPolicy.mirrors("com.whatsapp", ongoing = false, groupSummary = false, hasContent = false))
    }

    @Test
    fun aPlayersNotificationCrossesOnlyWhenAskedAndIsNeverDismissedFromTheDesktop() {
        assertFalse(NotificationPolicy.mirrors("com.spotify", ongoing = false, groupSummary = false, hasContent = true, media = true))
        assertTrue(NotificationPolicy.mirrors("com.spotify", ongoing = true, groupSummary = false, hasContent = true, media = true, mediaWanted = true))
        assertFalse(NotificationPolicy.dismissable(media = true))
        assertTrue(NotificationPolicy.dismissable(media = false))
    }
}
