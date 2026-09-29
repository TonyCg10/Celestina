package org.celestina.magnetita.storage

import org.junit.Assert.assertEquals
import org.junit.Test

class DocumentPathsTest {
    @Test
    fun wire_paths_become_document_ids_under_the_tree() {
        assertEquals("primary:", DocumentPaths.childId("primary:", ""))
        assertEquals("primary:DCIM/a.jpg", DocumentPaths.childId("primary:", "DCIM/a.jpg"))
        assertEquals("primary:Download/x", DocumentPaths.childId("primary:Download", "x"))
    }

    @Test
    fun paths_split_into_parent_and_name() {
        assertEquals("" to "a", DocumentPaths.split("a"))
        assertEquals("a/b" to "c", DocumentPaths.split("a/b/c"))
    }
}
