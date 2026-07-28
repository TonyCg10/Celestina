import QtQuick

// ─── GlassCard ────────────────────────────────────────────────────────────────
// A frosted-glass modal card: the GlassSurface look (blur + border) used by the
// context menus, applied to dialog cards. The consumer sets `backdropSource`
// (usually the tab's content root) and puts the dialog content inside. The
// capture re-arms on show, resize and move (event-driven; the per-frame
// self-tracking was reverted for CPU cost — see a8c0084). A modal is L3: it
// uses a scrim behind, never a drop shadow, so it keeps the default
// elevation 0.
// ──────────────────────────────────────────────────────────────────────────────
GlassSurface {
    id: glassCard

    cornerRadius: CelestinaTheme.radiusMd
    density: GlassSurface.Strong
    captureEnabled: visible
    // A modal can be scrolled under, so track the backdrop live rather than
    // freezing a snapshot that visibly desyncs when the content moves.
    liveCapture: true

    onVisibleChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    onWidthChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    onHeightChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    // Anchors keep re-centring a size-clamped card while the window resizes,
    // so position changes must re-arm the capture too or the blur goes stale.
    onXChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
    onYChanged: if (visible) Qt.callLater(glassCard.refreshBackdrop)
}
