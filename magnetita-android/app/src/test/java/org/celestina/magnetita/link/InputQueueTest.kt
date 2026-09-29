package org.celestina.magnetita.link

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** AND-7: waiting motion merges, order holds, and the queue is bounded. */
class InputQueueTest {
    @Test
    fun waitingMovesMergeAndDiscreteInputKeepsItsPlace() {
        val queue = InputQueue()
        val click: (LiveSession) -> Unit = {}
        queue.move(1, 2)
        queue.move(3, 4)
        queue.offer(click)
        queue.move(5, 5)
        queue.move(-1, 0)
        assertEquals(InputQueue.Op.Move(4, 6), queue.poll())
        assertTrue(queue.poll() is InputQueue.Op.Other)
        assertEquals(InputQueue.Op.Move(4, 5), queue.poll())
        assertNull(queue.poll())
    }

    @Test
    fun mergedMotionStaysWithinOneWireMove() {
        val queue = InputQueue()
        repeat(4) { queue.move(20_000, -20_000) }
        assertEquals(InputQueue.Op.Move(Short.MAX_VALUE.toInt(), Short.MIN_VALUE.toInt()), queue.poll())
    }

    @Test
    fun aFullQueueRefusesAndMotionStillMergesIntoItsTail() {
        val queue = InputQueue(capacity = 2)
        assertTrue(queue.offer {})
        assertTrue(queue.move(1, 1))
        assertFalse(queue.offer {})
        assertTrue("motion merges into the waiting move", queue.move(1, 1))
        assertEquals(2, queue.size())
    }
}
