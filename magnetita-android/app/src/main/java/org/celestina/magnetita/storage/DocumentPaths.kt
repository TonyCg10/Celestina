package org.celestina.magnetita.storage

/**
 * Wire paths onto document ids of the tree the person shared. The external
 * storage provider's ids are `<volume>:<relative path>`, so a child of the
 * tree's document is its id with the wire path appended. Pure, JVM-tested.
 */
object DocumentPaths {
    /** The document id of `path` under the tree's document `treeId`; null for a path the wire forbids. */
    fun childId(treeId: String, path: String): String? {
        if (!valid(path)) return null
        if (path.isEmpty()) return treeId
        return if (treeId.endsWith(":") || treeId.endsWith("/")) treeId + path else "$treeId/$path"
    }

    /** The wire's rule: `/`-separated plain components, the root empty. */
    fun valid(path: String): Boolean {
        if (path.isEmpty()) return true
        if (path.length > 4096 || '\u0000' in path) return false
        return path.split('/').all { it.isNotEmpty() && it != "." && it != ".." }
    }

    /** The parent path and the name of a non-root path. */
    fun split(path: String): Pair<String, String> {
        val slash = path.lastIndexOf('/')
        return if (slash < 0) "" to path else path.substring(0, slash) to path.substring(slash + 1)
    }
}
