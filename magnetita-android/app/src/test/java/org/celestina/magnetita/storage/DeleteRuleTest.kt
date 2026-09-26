package org.celestina.magnetita.storage

import org.celestina.magnetita.link.StorageEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The wire's delete removes a file or an empty directory, never a tree (audit finding AND-4). */
class DeleteRuleTest {
    private val dir = StorageEntry("DCIM", dir = true, size = 0, mtimeMs = 0)
    private val file = StorageEntry("a.jpg", dir = false, size = 12, mtimeMs = 0)

    @Test
    fun a_non_empty_directory_is_refused_with_enotempty() {
        assertEquals(DeleteRule.NOT_EMPTY, DeleteRule.refusal(dir) { 3 })
        assertEquals("not empty", DeleteRule.NOT_EMPTY)
    }

    @Test
    fun an_empty_directory_and_a_file_are_deleted() {
        assertNull(DeleteRule.refusal(dir) { 0 })
        var listed = false
        assertNull(DeleteRule.refusal(file) { listed = true; 5 })
        // A file's delete never lists anything.
        assertEquals(false, listed)
    }

    @Test
    fun a_directory_that_cannot_be_listed_is_kept() {
        assertEquals(DeleteRule.UNREADABLE, DeleteRule.refusal(dir) { null })
    }

    @Test
    fun a_missing_target_is_not_found() {
        assertEquals(DeleteRule.NOT_FOUND, DeleteRule.refusal(null) { 0 })
    }
}
