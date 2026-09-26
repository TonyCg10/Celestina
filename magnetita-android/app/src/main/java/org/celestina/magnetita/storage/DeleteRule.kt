package org.celestina.magnetita.storage

import org.celestina.magnetita.link.StorageEntry

/**
 * The wire's delete removes a file or an empty directory
 * (`magnetita/docs/protocol.md`, storage kind 12). The document provider
 * deletes a directory with everything under it, so the phone looks first and
 * answers [NOT_EMPTY], the error the desktop maps to `ENOTEMPTY`. Both shared
 * roots answer the same way. Pure, so the JVM tests pin it.
 */
object DeleteRule {
    const val NOT_EMPTY = "not empty"
    const val NOT_FOUND = "not found"
    const val UNREADABLE = "cannot list"

    /**
     * Null when `target` may be deleted, else the error to answer.
     * `children` counts a directory's entries, null when it cannot; it is
     * asked only for a directory.
     */
    fun refusal(target: StorageEntry?, children: () -> Int?): String? {
        if (target == null) return NOT_FOUND
        if (!target.dir) return null
        val count = children() ?: return UNREADABLE
        return if (count > 0) NOT_EMPTY else null
    }
}
