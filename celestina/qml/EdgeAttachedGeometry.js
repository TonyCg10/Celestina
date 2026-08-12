.pragma library

// One geometry source for both pixels and compositor blur. The SVG path paints
// the canonical glass material; the sampled polygon is mapped to window
// coordinates by the collector and becomes the finite KWindowEffects region.
// Keeping both products here prevents a visually curved pane from quietly
// requesting a rectangular blur behind its transparent corners.

function clamp(value, minimum, maximum) {
    return Math.max(minimum, Math.min(value, maximum));
}

function lerp(from, to, progress) {
    return from + (to - from) * progress;
}

// Scale one connector dimension from the carried menu's stable width. Keeping
// this arithmetic beside the path generator gives placement and paint the same
// bounded rule without coupling either one to dynamic menu height.
function proportionalMetric(size, ratio, minimum, maximum) {
    const lower = Math.min(minimum, maximum);
    const upper = Math.max(minimum, maximum);
    return clamp(Math.max(0, size) * Math.max(0, ratio), lower, upper);
}

function point(x, y) {
    return {"x": x, "y": y};
}

function pathNumber(value) {
    return String(Math.round(value * 1000) / 1000);
}

function pathPoint(value) {
    return pathNumber(value.x) + " " + pathNumber(value.y);
}

function appendPoint(points, value) {
    if (points.length > 0) {
        const previous = points[points.length - 1];
        if (Math.abs(previous.x - value.x) < 0.001
            && Math.abs(previous.y - value.y) < 0.001) {
            return;
        }
    }
    points.push(Qt.point(value.x, value.y));
}

function appendCubic(points, start, controlOne, controlTwo, end) {
    // The compositor accepts a polygon while the material paints the exact
    // cubic. Twelve samples keep the finite blur edge within a sub-pixel visual
    // tolerance for the real 20..36 px connector travel.
    const segments = 12;
    for (let step = 1; step <= segments; ++step) {
        const t = step / segments;
        const inverse = 1 - t;
        appendPoint(points, point(
            inverse * inverse * inverse * start.x
                + 3 * inverse * inverse * t * controlOne.x
                + 3 * inverse * t * t * controlTwo.x
                + t * t * t * end.x,
            inverse * inverse * inverse * start.y
                + 3 * inverse * inverse * t * controlOne.y
                + 3 * inverse * t * t * controlTwo.y
                + t * t * t * end.y));
    }
}

function appendQuadratic(points, start, control, end) {
    const segments = 5;
    for (let step = 1; step <= segments; ++step) {
        const t = step / segments;
        const inverse = 1 - t;
        appendPoint(points, point(
            inverse * inverse * start.x
                + 2 * inverse * t * control.x
                + t * t * end.x,
            inverse * inverse * start.y
                + 2 * inverse * t * control.y
                + t * t * end.y));
    }
}

// Paint an edge-to-edge top bar without closing its lower edge with a stroke.
// The fill remains a complete rectangle, while the open edge path leaves any
// contextual connector free to continue from the bar's lower boundary.
function openBottomRectangle(width, height) {
    if (width <= 0 || height <= 0)
        return {"path": "", "edgePath": ""};

    const path = [
        "M 0 0",
        "L " + pathNumber(width) + " 0",
        "L " + pathNumber(width) + " " + pathNumber(height),
        "L 0 " + pathNumber(height),
        "Z"
    ];
    const edgePath = [
        "M 0 " + pathNumber(height),
        "L 0 0",
        "L " + pathNumber(width) + " 0",
        "L " + pathNumber(width) + " " + pathNumber(height)
    ];
    return {"path": path.join(" "), "edgePath": edgePath.join(" ")};
}

// A membrane narrows as its travel, body-to-icon scale change and horizontal
// displacement increase. The calculation is deliberately shell-local: all
// four inputs are real placement geometry, not reusable material anatomy. Its
// result remains finite at zero and monotonic for each tension input while the
// remaining inputs stay fixed.
function membraneTension(anchorWidth, bodyWidth, connectorHeight, centerOffset) {
    const waistScale = Math.max(1, Math.min(anchorWidth, bodyWidth));
    const bodyScale = Math.max(1, Math.max(anchorWidth, bodyWidth));
    // An icon is intentionally much narrower than every real contextual body.
    // Normalising travel by that 18 px waist reference would saturate and
    // collapse every menu to the same waist. The geometric mean keeps travel
    // sensitive at both reference scales, while logarithmic spread preserves a
    // useful distinction between the 328-620 px bodies without dominating it.
    const spanScale = Math.sqrt(waistScale * bodyScale);
    const stretch = clamp(Math.max(0, connectorHeight) / spanScale, 0, 1);
    const spread = clamp(Math.log(bodyScale / waistScale) / Math.log(48), 0, 1);
    const displacement = clamp(Math.abs(centerOffset) / bodyScale, 0, 1);
    return clamp(stretch * 0.42 + spread * 0.43
                 + displacement * 0.35, 0, 1);
}

// The droplet neck is icon-proportional, never body-proportional. Higher
// tension (more travel, larger scale spread or displacement) pulls it a
// little thinner, exactly like a heavier hanging drop.
function dropletNeckWidth(anchorWidth, bodyWidth, connectorHeight,
                          centerOffset) {
    const tension = membraneTension(anchorWidth, bodyWidth,
                                    connectorHeight, centerOffset);
    return clamp(Math.max(1, anchorWidth) * lerp(2.3, 1.7, tension), 22, 48);
}

// Build the droplet outline in seam space and map it onto the frame. Seam
// space measures `u` away from the attachment seam (0 at the seam, `travel`
// at the body's near edge, `travel + bodyDepth` at its far edge) and `v`
// laterally along the seam. The same construction therefore hangs a drop
// from the bar (u grows downward) or sideways out of a parent menu's edge
// (u grows toward the child body), and the very same curve samples the
// compositor polygon in every orientation.
function membraneOutline(travel, bodyLo, bodyHi, bodyDepth,
                         anchorLo, anchorHi, requestedRadius, mapPoint) {
    const bodyWidth = bodyHi - bodyLo;
    if (travel <= 0 || bodyWidth <= 0 || bodyDepth <= 0
        || anchorHi - anchorLo <= 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};
    }

    const radius = Math.max(0, Math.min(requestedRadius,
                                        bodyWidth / 2, bodyDepth));
    const bodyFar = travel + bodyDepth;
    const anchorCenter = (anchorLo + anchorHi) / 2;
    const bodyCenter = (bodyLo + bodyHi) / 2;
    const centerOffset = bodyCenter - anchorCenter;
    const tension = membraneTension(anchorHi - anchorLo, bodyWidth,
                                    travel, centerOffset);
    let neckWidth = dropletNeckWidth(
        anchorHi - anchorLo, bodyWidth, travel, centerOffset);
    neckWidth = Math.min(neckWidth, bodyWidth / 3);
    // The meniscus makes the mouth cling to the seam: the outline leaves it
    // along the seam and only then folds into the neck.
    const flare = Math.min(clamp(travel * 0.5, 8, 18), radius + 6,
                           (bodyWidth - neckWidth) / 2 - radius > 0
                               ? (bodyWidth - neckWidth) / 2 - radius
                               : 2);
    // The narrowest point sits just past the seam so the longer, swelling
    // lobe carries most of the travel — a drop, not a symmetric pinch.
    const neckU = travel * 0.34;
    // Follow the clicked icon exactly whenever the body span permits it. At a
    // clamped edge, keep the complete mouth inside the body's flat near-edge
    // span instead of crossing a rounded corner.
    const minCenter = bodyLo + radius + neckWidth / 2 + flare;
    const maxCenter = bodyHi - radius - neckWidth / 2 - flare;
    const neckCenter = minCenter <= maxCenter
        ? clamp(anchorCenter, minCenter, maxCenter)
        : bodyCenter;
    const neckLo = neckCenter - neckWidth / 2;
    const neckHi = neckCenter + neckWidth / 2;
    const mouthLo = neckLo - flare;
    const mouthHi = neckHi + flare;
    // The swell lands tangent on the body's near edge before its rounded
    // corners begin; a wider body or longer travel spreads the landing.
    const run = clamp(bodyWidth * 0.18 + travel * 1.5, 48, 140);
    const joinHi = clamp(neckHi + run, neckHi + 4, bodyHi - radius);
    const joinLo = clamp(neckLo - run, bodyLo + radius, neckLo - 4);
    const drop = travel - neckU;

    const at = function(u, v) { return mapPoint(u, v); };
    const mouthStart = at(0, mouthLo);
    const mouthEnd = at(0, mouthHi);
    const hiNeck = at(neckU, neckHi);
    const loNeck = at(neckU, neckLo);
    const hiJoin = at(travel, joinHi);
    const loJoin = at(travel, joinLo);

    const segments = [
        {"kind": "line", "to": mouthEnd},
        // High side, seam to body: tangent along the seam (meniscus), then
        // perpendicular through the neck, then tangent along the landing.
        {"kind": "cubic",
         "one": at(0, mouthHi - flare * 0.6),
         "two": at(neckU * 0.45, neckHi),
         "to": hiNeck},
        {"kind": "cubic",
         "one": at(neckU + drop * 0.6, neckHi),
         "two": at(travel, joinHi - (joinHi - neckHi) * 0.45),
         "to": hiJoin},
        {"kind": "line", "to": at(travel, bodyHi - radius)},
        {"kind": "quad", "one": at(travel, bodyHi),
         "to": at(travel + radius, bodyHi)},
        {"kind": "line", "to": at(bodyFar - radius, bodyHi)},
        {"kind": "quad", "one": at(bodyFar, bodyHi),
         "to": at(bodyFar, bodyHi - radius)},
        {"kind": "line", "to": at(bodyFar, bodyLo + radius)},
        {"kind": "quad", "one": at(bodyFar, bodyLo),
         "to": at(bodyFar - radius, bodyLo)},
        {"kind": "line", "to": at(travel + radius, bodyLo)},
        {"kind": "quad", "one": at(travel, bodyLo),
         "to": at(travel, bodyLo + radius)},
        {"kind": "line", "to": loJoin},
        // Low side, body back to the seam.
        {"kind": "cubic",
         "one": at(travel, joinLo + (neckLo - joinLo) * 0.45),
         "two": at(neckU + drop * 0.6, neckLo),
         "to": loNeck},
        {"kind": "cubic",
         "one": at(neckU * 0.45, neckLo),
         "two": at(0, mouthLo + flare * 0.6),
         "to": mouthStart}
    ];

    const commands = ["M " + pathPoint(mouthStart)];
    const polygon = [];
    appendPoint(polygon, mouthStart);
    let previous = mouthStart;
    for (let index = 0; index < segments.length; ++index) {
        const segment = segments[index];
        if (segment.kind === "line") {
            commands.push("L " + pathPoint(segment.to));
            appendPoint(polygon, segment.to);
        } else if (segment.kind === "quad") {
            commands.push("Q " + pathPoint(segment.one) + " "
                          + pathPoint(segment.to));
            appendQuadratic(polygon, previous, segment.one, segment.to);
        } else {
            commands.push("C " + pathPoint(segment.one) + " "
                          + pathPoint(segment.two) + " "
                          + pathPoint(segment.to));
            appendCubic(polygon, previous, segment.one, segment.two,
                        segment.to);
        }
        previous = segment.to;
    }
    commands.push("Z");

    // The same outer boundary without the seam segment or close command is
    // used by the glass strokes. Leaving that one edge open removes the cap
    // at the seam, so the material reads as continuing into its attachment.
    const edgeCommands = commands.slice(2, commands.length - 1);
    edgeCommands[0] = "M " + pathPoint(mouthEnd) + " " + edgeCommands[0];

    return {
        "path": commands.join(" "),
        "edgePath": edgeCommands.join(" "),
        "polygon": polygon,
        "tension": tension,
        "waistWidth": neckWidth,
        "waistY": neckU,
        "waistCenter": neckCenter,
        "mouthLeft": mouthLo,
        "mouthRight": mouthHi,
        "joinLeft": joinLo,
        "joinRight": joinHi,
        "bodyLeft": bodyLo,
        "bodyRight": bodyHi
    };
}

// Build one contextual body hanging from the panel through a droplet
// membrane. The seam at the bar is a narrow mouth centred on the exact
// clicked icon; a meniscus leaves the bar with a horizontal tangent on both
// sides, the outline narrows to the neck just below the bar and then swells
// concavely until it lands tangent on the body's top edge. The body keeps its
// ordinary rounded top corners outside that swell, so the whole read is one
// drop falling out of the bar rather than an hourglass pinched between two
// body-wide edges.
function topAttachedMembrane(frameWidth, frameHeight,
                             bodyX, bodyY, bodyWidth, bodyHeight,
                             anchorX, anchorWidth, requestedRadius) {
    if (frameWidth <= 0 || frameHeight <= 0
        || bodyWidth <= 0 || bodyHeight <= 0
        || anchorWidth <= 0 || bodyY < 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};
    }

    const anchorLeft = clamp(anchorX, 0, frameWidth);
    const anchorRight = clamp(anchorX + anchorWidth, anchorLeft, frameWidth);
    if (anchorRight - anchorLeft <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};

    const shape = membraneOutline(
        Math.max(1, bodyY), bodyX, bodyX + bodyWidth, bodyHeight,
        anchorLeft, anchorRight, requestedRadius,
        function(u, v) { return point(v, u); });
    shape.anchorLeft = anchorLeft;
    shape.anchorRight = anchorRight;
    return shape;
}

// The same drop, grown sideways out of a parent surface's vertical edge into
// an adjacent child body. `seamAtRight` places the seam on this frame's right
// edge (a child sitting left of its parent); otherwise the seam is at x=0.
// The lateral axis is vertical: the mouth follows the invoking row's icon in
// y, and `waistCenter`/`mouthLeft`/`mouthRight`/`joinLeft`/`joinRight` are y
// coordinates in this orientation.
function sideAttachedMembrane(frameWidth, frameHeight,
                              bodyX, bodyY, bodyWidth, bodyHeight,
                              anchorY, anchorHeight, requestedRadius,
                              seamAtRight) {
    if (frameWidth <= 0 || frameHeight <= 0
        || bodyWidth <= 0 || bodyHeight <= 0 || anchorHeight <= 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};
    }

    const anchorTop = clamp(anchorY, 0, frameHeight);
    const anchorBottom = clamp(anchorY + anchorHeight, anchorTop, frameHeight);
    if (anchorBottom - anchorTop <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};

    const seamX = seamAtRight ? frameWidth : 0;
    const travel = seamAtRight ? frameWidth - (bodyX + bodyWidth) : bodyX;
    if (travel <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0};

    const shape = membraneOutline(
        travel, bodyY, bodyY + bodyHeight, bodyWidth,
        anchorTop, anchorBottom, requestedRadius,
        seamAtRight
            ? function(u, v) { return point(seamX - u, v); }
            : function(u, v) { return point(seamX + u, v); });
    shape.anchorTop = anchorTop;
    shape.anchorBottom = anchorBottom;
    return shape;
}

function mapPolygon(item, polygon) {
    const mapped = [];
    if (!item || !polygon)
        return mapped;

    for (let index = 0; index < polygon.length; ++index) {
        const at = item.mapToItem(null, polygon[index].x, polygon[index].y);
        mapped.push(Qt.point(at.x, at.y));
    }
    return mapped;
}
