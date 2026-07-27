import QtQuick

// ─── GlassCard ────────────────────────────────────────────────────────────────
// A frosted-glass modal card: the GlassSurface look (blur + border) used by the
// context menus, applied to dialog cards. The consumer sets `backdropSource`
// (usually the tab's content root) and puts the dialog content inside. The card
// is a live surface, so glass v2 tracks its backdrop by itself — no hand-wired
// refresh. A modal is L3: it uses a scrim behind, never a drop shadow, so it
// keeps the default elevation 0.
// ──────────────────────────────────────────────────────────────────────────────
GlassSurface {
    id: glassCard

    cornerRadius: CelestinaTheme.radiusMd
    captureEnabled: visible
    // A modal can be scrolled under, so track the backdrop live rather than
    // freezing a snapshot that visibly desyncs when the content moves.
    liveCapture: true
}
