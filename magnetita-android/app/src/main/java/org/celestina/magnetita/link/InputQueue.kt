package org.celestina.magnetita.link

/**
 * The trackpad's and keyboard's way to the held session (AND-7): one bounded
 * queue drained in order by one worker. Pointer motion that waits merges
 * into the move before it, so a drag sends at most one move per drain and a
 * slow link never replays stale motion; everything else keeps its order.
 * Past [capacity] waiting operations a new one is refused, so a stalled link
 * holds a bounded amount; the desktop releases what the phone held down when
 * the session ends. Pure, so the JVM tests pin it.
 */
class InputQueue(private val capacity: Int = CAPACITY) {
    /** One waiting operation. */
    sealed interface Op {
        data class Move(val dx: Int, val dy: Int) : Op

        /** A button, a scroll, a key or text, sent as it was queued. */
        class Other(val send: (LiveSession) -> Unit) : Op
    }

    private val waiting = ArrayDeque<Op>()

    /** Queues motion, merged into a move still waiting at the tail; false when full. */
    @Synchronized
    fun move(dx: Int, dy: Int): Boolean {
        val last = waiting.lastOrNull()
        if (last is Op.Move) {
            waiting[waiting.lastIndex] = Op.Move(merge(last.dx, dx), merge(last.dy, dy))
            return true
        }
        return add(Op.Move(dx, dy))
    }

    /** Queues one discrete operation; false when full. */
    @Synchronized
    fun offer(send: (LiveSession) -> Unit): Boolean = add(Op.Other(send))

    /** The oldest waiting operation, or null. */
    @Synchronized
    fun poll(): Op? = waiting.removeFirstOrNull()

    @Synchronized
    fun size(): Int = waiting.size

    private fun add(op: Op): Boolean {
        if (waiting.size >= capacity) return false
        waiting.addLast(op)
        return true
    }

    /** Motion adds up, within what one move on the wire may carry. */
    private fun merge(a: Int, b: Int): Int = (a.toLong() + b).coerceIn(Short.MIN_VALUE.toLong(), Short.MAX_VALUE.toLong()).toInt()

    companion object {
        const val CAPACITY = 256
    }
}
