package org.celestina.magnetita.storage

/**
 * Wire paths onto document ids of the tree the person shared. The external
 * storage provider's ids are `<volume>:<relative path>`, so a child of the
 * tree's document is its id with the wire path appended. Pure, JVM-tested.
 */
object DocumentPaths {
    /**
     * The document id of `path` under the tree's document `treeId`. The core
     * checked every path at the wire (plain `/`-separated components), so it
     * is not checked again here.
     */
    fun childId(treeId: String, path: String): String {
        if (path.isEmpty()) return treeId
        return if (treeId.endsWith(":") || treeId.endsWith("/")) treeId + path else "$treeId/$path"
    }

    /** The parent path and the name of a non-root path. */
    fun split(path: String): Pair<String, String> {
        val slash = path.lastIndexOf('/')
        return if (slash < 0) "" to path else path.substring(0, slash) to path.substring(slash + 1)
    }
}
