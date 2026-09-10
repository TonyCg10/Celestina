package org.celestina.magnetita.storage

import org.celestina.magnetita.link.DesktopSignal
import org.celestina.magnetita.link.LinkEvent
import org.celestina.magnetita.link.StorageRequest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentPathsTest {
    @Test
    fun wire_paths_become_document_ids_under_the_tree() {
        assertEquals("primary:", DocumentPaths.childId("primary:", ""))
        assertEquals("primary:DCIM/a.jpg", DocumentPaths.childId("primary:", "DCIM/a.jpg"))
        assertEquals("primary:Download/x", DocumentPaths.childId("primary:Download", "x"))
        assertNull(DocumentPaths.childId("primary:", "../x"))
        assertNull(DocumentPaths.childId("primary:", "a//b"))
    }

    @Test
    fun paths_split_into_parent_and_name() {
        assertEquals("" to "a", DocumentPaths.split("a"))
        assertEquals("a/b" to "c", DocumentPaths.split("a/b/c"))
        assertTrue(DocumentPaths.valid(""))
        assertFalse(DocumentPaths.valid("."))
    }

    @Test
    fun a_storage_envelope_becomes_a_signal() {
        val request = StorageRequest(StorageRequest.READ, 4, "a", "", 10, 20, ByteArray(0), false)
        assertEquals(DesktopSignal.Storage(request), DesktopSignal.of(LinkEvent(13, 6, "", storage = request)))
    }
}
