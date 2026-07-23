import QtQuick

// ─── GlassCard ────────────────────────────────────────────────────────────────
// A frosted-glass modal card: the GlassSurface look (blur + border) used by the
// context menus, applied to dialog cards. The consumer sets `backdropSource`
// (usually the tab's content root) and puts the dialog content inside; the card
// captures/refreshes its backdrop whenever it appears or is resized.
// ──────────────────────────────────────────────────────────────────────────────
GlassSurface {
    id: glassCard

    cornerRadius: CelestinaTheme.radiusMd
    captureEnabled: visible

    onVisibleChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    onWidthChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    onHeightChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
}
