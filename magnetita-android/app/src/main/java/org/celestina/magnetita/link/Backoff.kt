package org.celestina.magnetita.link

/**
 * The reconnection schedule, the same numbers the desktop's link uses: a
 * quarter second doubling to a minute, reset by a session that was
 * actually established.
 */
class Backoff {
    private var attempt = 0

    fun nextDelayMs(): Long {
        val delay = (FIRST_MS shl attempt.coerceAtMost(8)).coerceAtMost(LONGEST_MS)
        attempt += 1
        return delay
    }

    fun reset() {
        attempt = 0
    }

    companion object {
        const val FIRST_MS = 250L
        const val LONGEST_MS = 60_000L
    }
}
