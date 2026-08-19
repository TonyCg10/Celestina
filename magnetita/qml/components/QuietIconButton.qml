import QtQuick
import QtQuick.Controls
import org.celestina.magnetita 1.0

// The suite's icon button, without the hover tooltip.
//
// Magnetita's actions are a small, fixed set of glyphs the author sees every
// day; a label that floats over the window a second after the pointer lands is
// noise once they are learnt, and it covers the very row being acted on.
//
// `helpText` is deliberately still set by every caller: `CelestinaIconButton`
// reads it as `Accessible.name`, so it remains the name a screen reader hears.
// Removing the tooltip is a visual decision and must not quietly turn every
// action into an anonymous button.
CelestinaIconButton {
    ToolTip.visible: false
}
