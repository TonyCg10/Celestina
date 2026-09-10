package org.celestina.magnetita.phone

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The call state machine and the vCard the book writes. */
class PhoneTest {
    @Test
    fun ringingThenOffHookIsAnsweredRingingThenIdleIsMissedOffHookThenIdleIsEnded() {
        assertEquals(CallStateMachine.RINGING, CallStateMachine.transition(null, "RINGING"))
        assertEquals(CallStateMachine.ANSWERED, CallStateMachine.transition("RINGING", "OFFHOOK"))
        assertEquals(CallStateMachine.MISSED, CallStateMachine.transition("RINGING", "IDLE"))
        assertEquals(CallStateMachine.ENDED, CallStateMachine.transition("OFFHOOK", "IDLE"))
        assertNull("an outgoing call is not reported", CallStateMachine.transition("IDLE", "OFFHOOK"))
        assertNull(CallStateMachine.transition("IDLE", "IDLE"))
    }

    @Test
    fun theVcardCarriesTheNameAndTheNumbersOnly() {
        val card = PhoneBook.vcard("Ana, la de casa", listOf("+34 600 111 222", "912 345 678"))
        assertEquals("BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\\, la de casa\r\nTEL:+34600111222\r\nTEL:912345678\r\nEND:VCARD\r\n", card)
    }
}
