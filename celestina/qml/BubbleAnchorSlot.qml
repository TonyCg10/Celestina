// M7 — where a minimize travels to, and where a restore comes from.
//
// The slot is permanent on purpose. A minimize needs a destination *before* its bubble
// exists, so the first one cannot wait for the group to appear, and reserving the space
// once means adding bubbles never shifts the front one. It draws nothing, takes no input,
// and is hidden from assistive technology: it is a coordinate, not a control.
//
// The rectangle is output-local logical geometry because that is the only part Celestina
// owns. Niri keeps output topology, transforms, scale, and clipping, and degrades to its
// ordinary motion whenever what it receives is unusable.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    // The monitor this panel speaks for. One helper serves every panel, so an anchor that
    // did not name its output could not be told apart from another screen's.
    required property string outputName

    readonly property int slotSize: 22

    objectName: "celestina-bubble-anchor-slot"
    width: root.slotSize
    height: root.slotSize

    // A coordinate is not a control: nothing to read out, nothing to press.
    enabled: false
    Accessible.ignored: true

    // Output-local logical coordinates. `mapToGlobal` answers in Qt's virtual desktop
    // space, which spans every monitor, so the screen's own origin comes back off before
    // the rectangle leaves this shell.
    function outputLocalRect() {
        const topLeft = root.mapToGlobal(0, 0);
        const bottomRight = root.mapToGlobal(root.width, root.height);
        return Qt.rect(Math.min(topLeft.x, bottomRight.x) - Screen.virtualX,
                       Math.min(topLeft.y, bottomRight.y) - Screen.virtualY,
                       Math.abs(bottomRight.x - topLeft.x),
                       Math.abs(bottomRight.y - topLeft.y));
    }

    // The options a Melibea v2 minimize or restore carries. Built at action time rather
    // than stored, so a rectangle can never be stale: if the panel moved, the next action
    // simply describes where it is now.
    //
    // `reducedMotion` wins over the anchor. Someone who asked for less movement is asking
    // for no travel at all, not for travel to a different place.
    function transitionOptions(reducedMotion) {
        if (reducedMotion)
            return {"transition": "disabled"};
        const rect = root.outputLocalRect();
        if (root.outputName.length === 0 || rect.width <= 0 || rect.height <= 0)
            return {};
        return {
            "transition": "anchored",
            "anchor_output": root.outputName,
            "anchor_x": rect.x,
            "anchor_y": rect.y,
            "anchor_width": rect.width,
            "anchor_height": rect.height
        };
    }
}
