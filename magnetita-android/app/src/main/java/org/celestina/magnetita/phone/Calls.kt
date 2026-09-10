package org.celestina.magnetita.phone

import android.Manifest
import android.content.BroadcastReceiver
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import android.content.Context
import android.content.Intent
import android.media.AudioManager
import android.telecom.TelecomManager
import android.telephony.TelephonyManager
import org.celestina.magnetita.link.LinkService
import org.celestina.magnetita.link.Outbound

/**
 * What the phone is doing with a call, from the phone-state broadcast, and
 * the three things the desktop may do about it. The state machine is pure
 * so the JVM tests pin it: ringing then off-hook is an answered call,
 * ringing then idle a missed one, off-hook then idle an ended one.
 */
object CallStateMachine {
    const val RINGING = 0
    const val ANSWERED = 1
    const val MISSED = 2
    const val ENDED = 3

    // The broadcast's words, spelled here so the JVM tests need no framework.
    private const val STATE_RINGING = "RINGING"
    private const val STATE_OFFHOOK = "OFFHOOK"
    private const val STATE_IDLE = "IDLE"

    /** The event to send for a transition, or null when nothing changed. */
    fun transition(previous: String?, next: String): Int? = when (next) {
        STATE_RINGING -> RINGING
        STATE_OFFHOOK -> if (previous == STATE_RINGING) ANSWERED else null
        STATE_IDLE -> when (previous) {
            STATE_RINGING -> MISSED
            STATE_OFFHOOK -> ENDED
            else -> null
        }
        else -> null
    }
}

class Calls(private val context: Context) {
    private var mutedVolume: Int? = null

    /** The desktop's action: 0 mute, 1 answer, 2 hang up. */
    fun act(action: Int) {
        val telecom = context.getSystemService(TelecomManager::class.java) ?: return
        when (action) {
            0 -> {
                val audio = context.getSystemService(AudioManager::class.java) ?: return
                if (mutedVolume == null) mutedVolume = audio.getStreamVolume(AudioManager.STREAM_RING)
                runCatching { audio.setStreamVolume(AudioManager.STREAM_RING, 0, 0) }
            }
            1 -> if (ContextCompat.checkSelfPermission(context, Manifest.permission.ANSWER_PHONE_CALLS) == PackageManager.PERMISSION_GRANTED) {
                @Suppress("DEPRECATION")
                runCatching { telecom.acceptRingingCall() }
            }
            2 -> if (ContextCompat.checkSelfPermission(context, Manifest.permission.ANSWER_PHONE_CALLS) == PackageManager.PERMISSION_GRANTED) {
                runCatching { telecom.endCall() }
            }
        }
    }

    /** A call ended: the ringer volume comes back if the desktop muted it. */
    fun restoreRinger() {
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        mutedVolume?.let { runCatching { audio.setStreamVolume(AudioManager.STREAM_RING, it, 0) } }
        mutedVolume = null
    }

    /** The phone-state broadcast, delivered with the number when the call log grant exists. */
    class Receiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action != TelephonyManager.ACTION_PHONE_STATE_CHANGED) return
            val next = intent.getStringExtra(TelephonyManager.EXTRA_STATE) ?: return
            @Suppress("DEPRECATION")
            val number = intent.getStringExtra(TelephonyManager.EXTRA_INCOMING_NUMBER)
            if (number != null) lastNumber = number
            val event = CallStateMachine.transition(previous, next)
            previous = next
            if (event != null) {
                LinkService.send(context, Outbound.Call(event, lastNumber.orEmpty(), null, System.currentTimeMillis()))
                if (event == CallStateMachine.ENDED || event == CallStateMachine.MISSED) LinkService.callEnded(context)
            }
        }

        companion object {
            @Volatile private var previous: String? = null
            @Volatile private var lastNumber: String? = null
        }
    }
}
