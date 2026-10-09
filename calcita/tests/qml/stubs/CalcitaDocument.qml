// language-contract: allow-non-english (the Spanish notices the window shows, compared verbatim)
import QtQuick

// Stands in for the Rust `CalcitaDocument` under qmltestrunner: the same
// properties, signals and invokables, with the page grammar, the zoom
// ladder and the hit order of `calcita-core` written out again in JavaScript.
QtObject {
    id: reader

    property string documentKey: ""
    readonly property string documentName: documentKey.substring(documentKey.lastIndexOf("/") + 1)
    readonly property url documentUrl: documentKey.length > 0 ? "file://" + documentKey : ""
    property int page: 0
    property int pageCount: 0
    property string zoomMode: "fitWidth"
    property real zoomFactor: 1
    property bool loaded: false

    readonly property var ladder: [0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0]

    signal pageRequested(int page)
    signal notice(string kind, string text)

    function parsePage(text) {
        const word = text.trim().toLowerCase()
        if (word === "inicio")
            return 1
        if (word === "fin")
            return pageCount
        if (/^[+-][0-9]+$/.test(word))
            return page + parseInt(word, 10)
        if (/^[0-9]+$/.test(word))
            return parseInt(word, 10)
        return -1
    }

    function goTo(text) {
        const target = parsePage(text)
        if (target < 1 || target > pageCount) {
            notice("error", "Esa página no existe en este documento.")
            return false
        }
        page = target
        pageRequested(target)
        return true
    }

    function setFree(factor) {
        zoomMode = "free"
        zoomFactor = factor
    }

    function zoomIn() {
        const next = ladder.find(rung => rung > zoomFactor + 0.005)
        setFree(next !== undefined ? next : ladder[ladder.length - 1])
    }

    function zoomOut() {
        const below = ladder.filter(rung => rung < zoomFactor - 0.005)
        setFree(below.length > 0 ? below[below.length - 1] : ladder[0])
    }

    function fitWidth() { zoomMode = "fitWidth" }
    function fitPage() { zoomMode = "fitPage" }
    function setZoom(factor) { setFree(Math.min(4, Math.max(0.5, factor))) }

    function restoreZoom(word) {
        if (word === "width")
            fitWidth()
        else if (word === "page")
            fitPage()
        else if (word.startsWith("free:"))
            setZoom(parseFloat(word.substring(5)))
    }

    function zoomWord() {
        if (zoomMode === "fitWidth")
            return "width"
        if (zoomMode === "fitPage")
            return "page"
        return "free:" + zoomFactor
    }

    function reportLoaded(count) {
        loaded = count >= 0
        pageCount = Math.max(0, count)
        page = pageCount > 0 ? 1 : 0
    }

    function reportPage(value) {
        if (value >= 1 && value <= pageCount)
            page = value
    }

    function searchQuery(text) {
        return text.trim()
    }

    function nextHit(current, count, forward) {
        if (count < 1)
            return -1
        if (current < 0 || current >= count)
            return forward ? 0 : count - 1
        if (forward)
            return current === count - 1 ? 0 : current + 1
        return current === 0 ? count - 1 : current - 1
    }

    function reportScale(factor) {
        if (factor > 0)
            zoomFactor = factor
    }
}
