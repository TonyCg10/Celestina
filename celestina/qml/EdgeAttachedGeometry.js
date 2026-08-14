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

// How many chords a curve is cut into for the compositor polygon.
//
// The material paints the exact curve; the region is a set of pixels, so the
// boundary between them is the one place the two can visibly disagree. A fixed
// twelve samples held that disagreement under a pixel while the compositor's
// blur was slight and its edge nearly invisible. Once the blur became a colour
// summary (2026-08-14) the edge turned into a hard contrast line and every
// chord showed as a tooth — the author's own word for it.
//
// The count now follows the curve's own size instead of a constant, because
// the error a chord makes is proportional to how far it spans: the control
// polygon's length is a cheap upper bound on that, and roughly one sample per
// unit keeps the chord under a pixel at every per-output factor this shell
// draws at. The bounds keep a hairline corner cheap and a long body edge
// honest; `appendPoint` drops anything that lands on its predecessor, so
// oversampling a short curve costs nothing downstream.
function curveSegments(controlPoints) {
    let span = 0;
    for (let index = 1; index < controlPoints.length; ++index) {
        const dx = controlPoints[index].x - controlPoints[index - 1].x;
        const dy = controlPoints[index].y - controlPoints[index - 1].y;
        span += Math.sqrt(dx * dx + dy * dy);
    }
    return clamp(Math.ceil(span), 12, 96);
}

// A rounded rectangle as a compositor polygon, with every point held at or
// below `floorY`. The corners are sampled at the same density every other
// curve here is, so the region's boundary tracks the painted one.
function roundedRectPolygon(left, top, right, bottom, requestedRadius, floorY) {
    const radius = Math.max(0, Math.min(requestedRadius,
                                        (right - left) / 2,
                                        (bottom - top) / 2));
    const corners = [
        // Centre, then the angle the arc sweeps through, clockwise from the
        // top-left corner so the polygon keeps one winding.
        {"cx": left + radius, "cy": top + radius, "from": Math.PI},
        {"cx": right - radius, "cy": top + radius, "from": -Math.PI / 2},
        {"cx": right - radius, "cy": bottom - radius, "from": 0},
        {"cx": left + radius, "cy": bottom - radius, "from": Math.PI / 2}
    ];
    const points = [];
    const segments = clamp(Math.ceil(radius), 4, 24);
    for (let corner = 0; corner < corners.length; ++corner) {
        const at = corners[corner];
        for (let step = 0; step <= segments; ++step) {
            const angle = at.from + (Math.PI / 2) * (step / segments);
            appendPoint(points, point(
                at.cx + radius * Math.cos(angle),
                Math.max(floorY, at.cy + radius * Math.sin(angle))));
        }
    }
    return points;
}

function appendCubic(points, start, controlOne, controlTwo, end) {
    const segments = curveSegments([start, controlOne, controlTwo, end]);
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
    const segments = curveSegments([start, control, end]);
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

// A panel capsule held against the screen's top edge by an elastic skin: it
// is widest where the edge grips it and draws in as it descends, and that
// drawing-in is one long sweep down the whole side rather than a lip at the
// top. The curve leaves the edge along it and arrives tangent to the bottom's
// own round, so there is no straight flank between them and the capsule reads
// as stretched rather than built.
//
// Two earlier shapes were rejected. Squaring it off at the edge read as a box
// pushed against the screen. Pinching it *narrower* at the edge — the drop
// hanging off a ceiling — was correct physics for a falling drop and wrong for
// this: a reading is held by the bar, not dripping from it. Between those, the
// same outward curve confined to a shallow lip read as a bucket, which is what
// the sweep below fixes: the flare is not smaller, it is spread over the
// entire height.
//
// `width` is the complete painted span, flare included; the capsule's body is
// that span inset by `flare` on each side, so the reading behind it stays on
// the same axis.
function topWeldedCapsule(width, height, requestedRadius, requestedFlare) {
    if (width <= 0 || height <= 0)
        return {"path": "", "edgePath": ""};

    const flare = clamp(requestedFlare === undefined ? 0 : requestedFlare,
                        0, width / 3);
    const bodyLeft = flare;
    const bodyRight = width - flare;
    const bodyWidth = bodyRight - bodyLeft;
    if (bodyWidth <= 0)
        return {"path": "", "edgePath": ""};

    const radius = Math.max(0, Math.min(requestedRadius, bodyWidth / 2,
                                        height / 2));
    // The skin stretches over everything above the bottom's round. Confining
    // it to a short lip is what made the same flare look like a rigid rim.
    const meniscus = Math.max(0, height - radius);
    const bottom = pathNumber(height);
    const shoulder = pathNumber(height - radius);

    const body = [
        // Right side: horizontal where the edge grips it, vertical where it
        // meets the bottom's round, one continuous stretch between.
        "C " + pathNumber(width - flare * 0.55) + " 0 "
             + pathNumber(bodyRight) + " " + pathNumber(meniscus * 0.42) + " "
             + pathNumber(bodyRight) + " " + shoulder,
        "Q " + pathNumber(bodyRight) + " " + bottom + " "
             + pathNumber(bodyRight - radius) + " " + bottom,
        "L " + pathNumber(bodyLeft + radius) + " " + bottom,
        "Q " + pathNumber(bodyLeft) + " " + bottom + " "
             + pathNumber(bodyLeft) + " " + shoulder,
        // Left side, back up to the edge.
        "C " + pathNumber(bodyLeft) + " " + pathNumber(meniscus * 0.42) + " "
             + pathNumber(flare * 0.55) + " 0 0 0"
    ];
    const path = ["M 0 0", "L " + pathNumber(width) + " 0"]
        .concat(body, ["Z"]).join(" ");
    // Open at the grip: the stroke runs from one side around to the other and
    // never draws a line across the screen edge between them.
    const edgePath = ["M " + pathNumber(width) + " 0"].concat(body).join(" ");
    return {"path": path, "edgePath": edgePath};
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

// A progress value above 1 is the bounce: the falling body has been carried
// past its resting place by its own weight and the membrane pulls it gently
// back. Splitting it out here keeps every consumer from having to know that
// the caller's easing overshoots.
function membraneOpening(progress) {
    return clamp(progress, 0, 1);
}

function membraneRecoil(progress) {
    return Math.max(0, progress - 1);
}

// How the falling drop deviates from its settled shape. Flight tension peaks
// halfway through the fall, and again while the body is bouncing, so the
// neck is thinnest while the body is moving and relaxes only once it has
// stopped. It is a shape term, not a duration: the caller owns the easing.
function membraneFlightTension(progress) {
    return Math.max(
        Math.sin(Math.PI * membraneOpening(progress)),
        clamp(membraneRecoil(progress) / 0.05, 0, 1)
    );
}

// The neck may thin under flight tension but must never pinch off — this is a
// drop under tension that stays attached, not one that separates. The floor
// is proportional to the settled neck so a wide menu keeps a proportionally
// visible thread, with an absolute minimum for the narrowest real anchor.
function membraneNeckFloor(settledNeckWidth) {
    return Math.min(settledNeckWidth, Math.max(12, settledNeckWidth * 0.55));
}

// Build the droplet outline in seam space and map it onto the frame. Seam
// space measures `u` away from the attachment seam (0 at the seam, `travel`
// at the body's near edge, `travel + bodyDepth` at its far edge) and `v`
// laterally along the seam. The same construction therefore hangs a drop
// from the bar (u grows downward) or sideways out of a parent menu's edge
// (u grows toward the child body), and the very same curve samples the
// compositor polygon in every orientation.
//
// `progress` drops that body from the seam. At 1 the result is exactly the
// settled geometry, so the animation can never change where a surface ends
// up. Below 1 the complete, full-sized body hangs closer to the seam and the
// neck is correspondingly shorter; above 1 it has been carried slightly past
// its resting place and bounces gently back. The mouth is never scaled or
// moved: it stays welded to the seam at every frame.
function membraneOutline(travel, bodyLo, bodyHi, bodyDepth,
                         anchorLo, anchorHi, requestedRadius, mapPoint,
                         requestedProgress) {
    const bodyWidth = bodyHi - bodyLo;
    if (travel <= 0 || bodyWidth <= 0 || bodyDepth <= 0
        || anchorHi - anchorLo <= 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};
    }

    const progress = requestedProgress === undefined ? 1 : requestedProgress;
    const opening = membraneOpening(progress);
    const recoil = membraneRecoil(progress);
    const settledRadius = Math.max(0, Math.min(requestedRadius,
                                               bodyWidth / 2, bodyDepth));
    const anchorCenter = (anchorLo + anchorHi) / 2;
    const bodyCenter = (bodyLo + bodyHi) / 2;
    const centerOffset = bodyCenter - anchorCenter;
    const tension = membraneTension(anchorHi - anchorLo, bodyWidth,
                                    travel, centerOffset);
    let settledNeckWidth = dropletNeckWidth(
        anchorHi - anchorLo, bodyWidth, travel, centerOffset);
    settledNeckWidth = Math.min(settledNeckWidth, bodyWidth / 3);
    // The meniscus makes the mouth cling to the seam: the outline leaves it
    // along the seam and only then folds into the neck.
    const flare = Math.min(clamp(travel * 0.5, 8, 18), settledRadius + 6,
                           (bodyWidth - settledNeckWidth) / 2 - settledRadius > 0
                               ? (bodyWidth - settledNeckWidth) / 2 - settledRadius
                               : 2);
    // Follow the clicked icon exactly whenever the body span permits it. At a
    // clamped edge, keep the complete mouth inside the body's flat near-edge
    // span instead of crossing a rounded corner. The mouth is settled
    // geometry: progress opens the body around it and never moves it.
    const minCenter = bodyLo + settledRadius + settledNeckWidth / 2 + flare;
    const maxCenter = bodyHi - settledRadius - settledNeckWidth / 2 - flare;
    const neckCenter = minCenter <= maxCenter
        ? clamp(anchorCenter, minCenter, maxCenter)
        : bodyCenter;
    const mouthLo = neckCenter - settledNeckWidth / 2 - flare;
    const mouthHi = neckCenter + settledNeckWidth / 2 + flare;

    // The falling drop. Its body keeps its complete settled span and depth at
    // every frame — the author rejected every variant that grew or scaled the
    // card — and the only thing progress moves is its distance from the seam.
    // The card is born hanging at the bar, falls away from it as the neck
    // stretches, is carried slightly past its resting place by its own
    // weight, and springs gently back. The neck thins under flight tension
    // while the body is moving and relaxes once it has stopped, measured
    // against the settled neck so no frame can pinch it off.
    const flight = membraneFlightTension(progress);
    const neckWidth = Math.max(
        membraneNeckFloor(settledNeckWidth),
        settledNeckWidth * lerp(1, 0.72, flight)
    );
    const openLo = bodyLo;
    const openHi = bodyHi;
    const openWidth = bodyWidth;
    // Past 1 the body has been carried below its resting place; the bounce is
    // bounded twice — in pixels, so a tall surface cannot swing, and against
    // the travel, so it stays proportionate on the shortest connectors.
    const bounce = Math.min(clamp(recoil * 260, 0, 9), travel * 0.3);
    const openTravel = Math.max(1, travel * opening + bounce);
    const openDepth = bodyDepth;
    const radius = Math.max(0, Math.min(requestedRadius,
                                        openWidth / 2, openDepth));
    const bodyFar = openTravel + openDepth;
    // The narrowest point sits just past the seam so the longer, swelling
    // lobe carries most of the travel — a drop, not a symmetric pinch.
    const neckU = openTravel * 0.34;
    const neckLo = neckCenter - neckWidth / 2;
    const neckHi = neckCenter + neckWidth / 2;
    // The swell lands tangent on the body's near edge before its rounded
    // corners begin; a wider body or longer travel spreads the landing. The
    // bounds are ordered explicitly so a body still narrower than its own
    // landing run cannot invert them mid-fall.
    const run = clamp(openWidth * 0.18 + openTravel * 1.5, 48, 140);
    const joinHi = clamp(neckHi + run, neckHi,
                         Math.max(neckHi, openHi - radius));
    const joinLo = clamp(neckLo - run,
                         Math.min(neckLo, openLo + radius), neckLo);
    const drop = openTravel - neckU;

    const at = function(u, v) { return mapPoint(u, v); };
    // The momentary body as a frame-space rectangle. Mapping the two corners
    // through the same projection keeps this correct in every orientation, so
    // the carried content can ride inside the drop without any consumer
    // knowing which edge the surface is attached to.
    const nearCorner = at(openTravel, openLo);
    const farCorner = at(bodyFar, openHi);
    const openRect = {
        "x": Math.min(nearCorner.x, farCorner.x),
        "y": Math.min(nearCorner.y, farCorner.y),
        "width": Math.abs(farCorner.x - nearCorner.x),
        "height": Math.abs(farCorner.y - nearCorner.y)
    };
    const mouthStart = at(0, mouthLo);
    const mouthEnd = at(0, mouthHi);
    const hiNeck = at(neckU, neckHi);
    const loNeck = at(neckU, neckLo);
    const hiJoin = at(openTravel, joinHi);
    const loJoin = at(openTravel, joinLo);

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
         "two": at(openTravel, joinHi - (joinHi - neckHi) * 0.45),
         "to": hiJoin},
        {"kind": "line", "to": at(openTravel, openHi - radius)},
        {"kind": "quad", "one": at(openTravel, openHi),
         "to": at(openTravel + radius, openHi)},
        {"kind": "line", "to": at(bodyFar - radius, openHi)},
        {"kind": "quad", "one": at(bodyFar, openHi),
         "to": at(bodyFar, openHi - radius)},
        {"kind": "line", "to": at(bodyFar, openLo + radius)},
        {"kind": "quad", "one": at(bodyFar, openLo),
         "to": at(bodyFar - radius, openLo)},
        {"kind": "line", "to": at(openTravel + radius, openLo)},
        {"kind": "quad", "one": at(openTravel, openLo),
         "to": at(openTravel, openLo + radius)},
        {"kind": "line", "to": loJoin},
        // Low side, body back to the seam.
        {"kind": "cubic",
         "one": at(openTravel, joinLo + (neckLo - joinLo) * 0.45),
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
        // The settled span, so a contract can prove where the drop is going
        // while it is still opening out of its mouth.
        "bodyLeft": bodyLo,
        "bodyRight": bodyHi,
        // The momentary span and the terms that produced it.
        "progress": progress,
        "opening": opening,
        "recoil": recoil,
        "recoilTravel": bounce,
        "openRect": openRect,
        "flightTension": flight,
        "settledWaistWidth": settledNeckWidth,
        "waistFloor": membraneNeckFloor(settledNeckWidth),
        "openLeft": openLo,
        "openRight": openHi,
        "openTravel": openTravel,
        "openDepth": openDepth
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
// The card alone, mid-emergence: a plain rounded rectangle at its ridden
// position inside the pane, for the stretch of the entry in which the card
// is still leaving the bar and there is no gap for a membrane to grow in.
// The caller's seam clip hides whatever is still behind the bar.
function emergingBodyPath(bodyX, bodyTop, bodyWidth, bodyHeight,
                          requestedRadius) {
    const radius = Math.max(0, Math.min(requestedRadius,
                                        bodyWidth / 2, bodyHeight / 2));
    const left = bodyX;
    const right = bodyX + bodyWidth;
    const top = bodyTop;
    const bottom = bodyTop + bodyHeight;
    const arc = " A " + pathNumber(radius) + " " + pathNumber(radius)
              + " 0 0 1 ";
    const path = "M " + pathPoint(point(left + radius, top))
        + " L " + pathPoint(point(right - radius, top))
        + arc + pathPoint(point(right, top + radius))
        + " L " + pathPoint(point(right, bottom - radius))
        + arc + pathPoint(point(right - radius, bottom))
        + " L " + pathPoint(point(left + radius, bottom))
        + arc + pathPoint(point(left, bottom - radius))
        + " L " + pathPoint(point(left, top + radius))
        + arc + pathPoint(point(left + radius, top))
        + " Z";
    // The region follows the painted corners, not the bounding box. A
    // four-point rectangle asked the compositor to blur the square corners
    // this path never paints, and once the blur became a colour summary that
    // overhang read as a square block behind every rounded corner.
    //
    // The region polygon never rises above the pane's top — the seam — so a
    // card still leaving the bar asks the compositor to blur only the part of
    // it that is already out, never the bar's own rows above. Clamping every
    // sampled y rather than the rectangle's edge is what keeps the corners
    // round in the ordinary case and flattens only what the bar covers.
    const clampedTop = Math.max(0, top);
    const polygon = bottom > clampedTop
        ? roundedRectPolygon(left, top, right, bottom, radius, 0)
        : [];
    return {"path": path, "edgePath": path, "polygon": polygon,
            "tension": 0, "waistWidth": 0, "waistY": 0, "waistCenter": 0,
            "openRect": {"x": bodyX, "y": bodyTop,
                         "width": bodyWidth, "height": bodyHeight}};
}

function topAttachedMembrane(frameWidth, frameHeight,
                             bodyX, bodyY, bodyWidth, bodyHeight,
                             anchorX, anchorWidth, requestedRadius,
                             progress) {
    if (frameWidth <= 0 || frameHeight <= 0
        || bodyWidth <= 0 || bodyHeight <= 0
        || anchorWidth <= 0 || bodyY < 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};
    }

    const anchorLeft = clamp(anchorX, 0, frameWidth);
    const anchorRight = clamp(anchorX + anchorWidth, anchorLeft, frameWidth);
    if (anchorRight - anchorLeft <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};

    const shape = membraneOutline(
        Math.max(1, bodyY), bodyX, bodyX + bodyWidth, bodyHeight,
        anchorLeft, anchorRight, requestedRadius,
        function(u, v) { return point(v, u); }, progress);
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
                              seamAtRight, progress) {
    if (frameWidth <= 0 || frameHeight <= 0
        || bodyWidth <= 0 || bodyHeight <= 0 || anchorHeight <= 0) {
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};
    }

    const anchorTop = clamp(anchorY, 0, frameHeight);
    const anchorBottom = clamp(anchorY + anchorHeight, anchorTop, frameHeight);
    if (anchorBottom - anchorTop <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};

    const seamX = seamAtRight ? frameWidth : 0;
    const travel = seamAtRight ? frameWidth - (bodyX + bodyWidth) : bodyX;
    if (travel <= 0)
        return {"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistY": 0,
                "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}};

    const shape = membraneOutline(
        travel, bodyY, bodyY + bodyHeight, bodyWidth,
        anchorTop, anchorBottom, requestedRadius,
        seamAtRight
            ? function(u, v) { return point(seamX - u, v); }
            : function(u, v) { return point(seamX + u, v); },
        progress);
    shape.anchorTop = anchorTop;
    shape.anchorBottom = anchorBottom;
    return shape;
}

// An item's own bounds in the coordinates of the window that carries it.
//
// Mapping the origin and then publishing the item's own width and height is
// wrong the moment anything between the item and the window is scaled: the
// origin comes back in real pixels and the size does not. That mismatch is not
// hypothetical — it shipped, and on a 1.15-scaled output it left a third of
// the bar's width and the last pixels of its height outside the region the
// compositor was asked to blur. Two mapped corners cannot disagree with each
// other.
function mapRect(item) {
    if (!item)
        return Qt.rect(0, 0, 0, 0);

    const near = item.mapToItem(null, 0, 0);
    const far = item.mapToItem(null, item.width, item.height);
    return Qt.rect(Math.min(near.x, far.x), Math.min(near.y, far.y),
                   Math.abs(far.x - near.x), Math.abs(far.y - near.y));
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
