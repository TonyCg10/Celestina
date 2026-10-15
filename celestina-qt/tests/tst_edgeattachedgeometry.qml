import CelestinaStyle
import QtTest
import "../qml/EdgeAttachedGeometry.js" as EdgeAttachedGeometry

TestCase {
    name: "EdgeAttachedGeometry"

    function near(actual, expected) {
        verify(Math.abs(actual - expected) < 0.001,
               "expected " + expected + ", got " + actual);
    }

    function metric(width, ratio, minimum, maximum) {
        return EdgeAttachedGeometry.proportionalMetric(
            width, ratio, minimum, maximum);
    }

    function hasPoint(points, x, y) {
        for (let index = 0; index < points.length; ++index) {
            if (Math.abs(points[index].x - x) < 0.001
                && Math.abs(points[index].y - y) < 0.001) {
                return true;
            }
        }
        return false;
    }

    function membrane(anchorX, anchorWidth, bodyX, bodyWidth,
                      connectorHeight, bodyHeight) {
        return EdgeAttachedGeometry.topAttachedMembrane(
            760, connectorHeight + bodyHeight,
            bodyX, connectorHeight, bodyWidth, bodyHeight,
            anchorX, anchorWidth, CelestinaTheme.radiusMd);
    }

    function test_real_menu_widths_keep_proportional_vertical_travel() {
        const rows = [
            {"width": 328, "gap": 20},
            {"width": 360, "gap": 22},
            {"width": 424, "gap": 25},
            {"width": 460, "gap": 28},
            {"width": 530, "gap": 32},
            {"width": 620, "gap": 36}
        ];

        for (let index = 0; index < rows.length; ++index) {
            const row = rows[index];
            compare(Math.round(metric(
                        row.width,
                        CelestinaTheme.compEdgeAttachmentGapRatio,
                        CelestinaTheme.compEdgeAttachmentGapMin,
                        CelestinaTheme.compEdgeAttachmentGapMax)), row.gap);
        }
    }

    function test_vertical_travel_clamps_below_and_above_real_menu_widths() {
        compare(metric(200, CelestinaTheme.compEdgeAttachmentGapRatio,
                       CelestinaTheme.compEdgeAttachmentGapMin,
                       CelestinaTheme.compEdgeAttachmentGapMax), 20);

        compare(metric(1000, CelestinaTheme.compEdgeAttachmentGapRatio,
                       CelestinaTheme.compEdgeAttachmentGapMin,
                       CelestinaTheme.compEdgeAttachmentGapMax), 36);
    }

    function test_droplet_mouth_is_narrow_and_the_landing_is_body_wide() {
        // The bar seam is one narrow mouth around the clicked 18 px glyph, not
        // a body-wide edge. The swell lands tangent on the body's top edge,
        // whose remaining span keeps its ordinary rounded corners.
        const shape = membrane(371, 18, 70, 620, 36, 420);

        verify(shape.path.length > 0);
        verify(shape.edgePath.length > 0);
        compare(shape.bodyLeft, 70);
        compare(shape.bodyRight, 690);

        // The polygon starts with the two open mouth points on the seam.
        compare(shape.polygon[0].y, 0);
        compare(shape.polygon[1].y, 0);
        near(shape.polygon[0].x, shape.mouthLeft);
        near(shape.polygon[1].x, shape.mouthRight);
        const mouthWidth = shape.mouthRight - shape.mouthLeft;
        verify(mouthWidth < 620 * 0.15);
        verify(mouthWidth >= 18);

        // No other polygon sample touches the seam row: the top edge is only
        // the mouth, so the panel rows beside it stay uncovered. The final
        // sample may close the outline back at the mouth's left point.
        for (let index = 2; index < shape.polygon.length; ++index) {
            if (shape.polygon[index].y > 0.0005)
                continue;
            near(shape.polygon[index].x, shape.mouthLeft);
            compare(index, shape.polygon.length - 1);
        }

        // The landing reaches the flat body-top span on both sides and the
        // body's rounded corners begin outside it.
        verify(hasPoint(shape.polygon, shape.joinRight, 36));
        verify(hasPoint(shape.polygon, shape.joinLeft, 36));
        verify(shape.joinLeft >= 70 + 1);
        verify(shape.joinRight <= 690 - 1);
        verify(hasPoint(shape.polygon, 690, 36 + CelestinaTheme.radiusMd));
        verify(hasPoint(shape.polygon, 70, 36 + CelestinaTheme.radiusMd));

        near(shape.waistCenter, 380);
        // The painted edge opens across the mouth rather than closing it with
        // a cap. Do not couple this semantic check to exact control-point
        // serialization.
        verify(shape.edgePath.indexOf("Z") === -1);
    }

    function test_neck_tracks_the_icon_center_and_clamps_inside_the_body() {
        const centred = membrane(225, 18, 70, 328, 20, 200);
        near(centred.waistCenter, 234);

        const beyondLeft = membrane(30, 18, 70, 328, 20, 200);
        verify(beyondLeft.mouthLeft
               >= beyondLeft.bodyLeft + CelestinaTheme.radiusMd - 0.001);
        verify(beyondLeft.waistCenter > beyondLeft.bodyLeft);

        const beyondRight = membrane(600, 18, 210, 328, 20, 200);
        verify(beyondRight.mouthRight
               <= beyondRight.bodyRight - CelestinaTheme.radiusMd + 0.001);
        verify(beyondRight.waistCenter < beyondRight.bodyRight);
    }

    function test_real_icon_to_menu_pairs_tighten_proportionally() {
        const rows = [
            {"width": 328, "gap": 20},
            {"width": 360, "gap": 22},
            {"width": 424, "gap": 25},
            {"width": 460, "gap": 28},
            {"width": 530, "gap": 32},
            {"width": 620, "gap": 36}
        ];
        let previous = null;
        for (let index = 0; index < rows.length; ++index) {
            const row = rows[index];
            const bodyX = 70;
            const anchorX = bodyX + row.width / 2 - 9;
            const shape = membrane(anchorX, 18, bodyX, row.width,
                                   row.gap, 420);
            if (previous !== null)
                verify(shape.tension > previous.tension);
            previous = {"tension": shape.tension};
        }

        const aligned = membrane(371, 18, 170, 420, 28, 300);
        const displaced = membrane(471, 18, 170, 420, 28, 300);
        verify(displaced.tension > aligned.tension);
        verify(displaced.waistWidth <= aligned.waistWidth);
    }

    function test_neck_is_finite_icon_proportional_and_inside_the_body() {
        const rows = [
            {"shape": membrane(185, 18, 30, 328, 20, 200), "gap": 20},
            {"shape": membrane(371, 18, 70, 620, 36, 420), "gap": 36},
            {"shape": membrane(362, 18, 210, 328, 20, 260), "gap": 20}
        ];
        for (let index = 0; index < rows.length; ++index) {
            const row = rows[index].shape;
            verify(Number.isFinite(row.tension));
            verify(Number.isFinite(row.waistWidth));
            const bodyWidth = row.bodyRight - row.bodyLeft;
            // An icon-scaled droplet neck: wide enough to read as liquid,
            // never a body-proportional band and never an icon-thin thread.
            verify(row.waistWidth >= 22);
            verify(row.waistWidth <= 48);
            verify(row.waistWidth <= bodyWidth / 3);
            verify(row.mouthLeft >= row.bodyLeft);
            verify(row.mouthRight <= row.bodyRight);
            // The neck sits just below the bar, leaving the longer swelling
            // lobe to carry most of the travel toward the body.
            verify(row.waistY > 0);
            verify(row.waistY < rows[index].gap / 2);
        }
    }

    function test_side_attached_droplet_grows_from_the_parent_edge() {
        // A child body left of its parent: the seam is this frame's right
        // edge and the droplet grows leftward from the invoking row's icon.
        const fromRight = EdgeAttachedGeometry.sideAttachedMembrane(
            380, 700, 0, 20, 356, 640, 180, 24,
            CelestinaTheme.radiusMd, true);
        verify(fromRight.path.length > 0);
        verify(fromRight.edgePath.length > 0);
        compare(fromRight.polygon[0].x, 380);
        compare(fromRight.polygon[1].x, 380);
        near((fromRight.polygon[0].y + fromRight.polygon[1].y) / 2, 192);
        near(fromRight.waistCenter, 192);
        for (let index = 0; index < fromRight.polygon.length; ++index)
            verify(fromRight.polygon[index].x <= 380.001);
        // The landing reaches the body's right edge tangentially and the
        // body keeps its rounded corners: the near edge sits at x=356.
        verify(hasPoint(fromRight.polygon, 356, fromRight.joinLeft));
        verify(hasPoint(fromRight.polygon, 356, fromRight.joinRight));

        // A child body right of its parent mirrors the seam onto x=0.
        const fromLeft = EdgeAttachedGeometry.sideAttachedMembrane(
            380, 700, 24, 20, 356, 640, 180, 24,
            CelestinaTheme.radiusMd, false);
        verify(fromLeft.path.length > 0);
        compare(fromLeft.polygon[0].x, 0);
        compare(fromLeft.polygon[1].x, 0);
        near(fromLeft.waistCenter, 192);
        for (let index = 0; index < fromLeft.polygon.length; ++index)
            verify(fromLeft.polygon[index].x >= -0.001);
    }

    function progressed(progress) {
        return EdgeAttachedGeometry.topAttachedMembrane(
            760, 456, 70, 36, 620, 420, 371, 18,
            CelestinaTheme.radiusMd, progress);
    }

    function test_a_settled_progress_is_the_unanimated_geometry() {
        // The fall may never change where a surface ends up: progress 1 and
        // an omitted progress must produce the same bytes.
        const settled = membrane(371, 18, 70, 620, 36, 420);
        const explicit = progressed(1);
        compare(explicit.path, settled.path);
        compare(explicit.edgePath, settled.edgePath);
        compare(explicit.polygon.length, settled.polygon.length);
        near(explicit.waistWidth, settled.waistWidth);
        verify(explicit.flightTension < 0.001);
    }

    function test_the_body_falls_whole_from_the_seam() {
        const settled = progressed(1);
        let previousY = -1;
        for (let step = 0; step <= 10; ++step) {
            const shape = progressed(step / 10);
            // The mouth is settled geometry at every frame: the seam contact
            // never scales or moves with the fall.
            near(shape.mouthLeft, settled.mouthLeft);
            near(shape.mouthRight, settled.mouthRight);
            near(shape.waistCenter, settled.waistCenter);
            // The body keeps its complete size and only falls away from the
            // seam: no frame grows, scales or reflows the card.
            near(shape.openLeft, settled.bodyLeft);
            near(shape.openRight, settled.bodyRight);
            near(shape.openDepth, settled.openDepth);
            verify(shape.openRect.y > previousY);
            previousY = shape.openRect.y;
        }
        // It hangs at the seam when born and rests at the settled travel.
        const born = progressed(0);
        verify(born.openRect.y <= 1.001);
        near(settled.openRect.y, 36);
    }

    function test_the_falling_neck_is_under_tension_but_never_detaches() {
        const settled = progressed(1);
        let thinnest = settled.waistWidth;
        for (let step = 0; step <= 20; ++step) {
            const shape = progressed(step / 20);
            // Flight tension may only thin the neck, never widen it past its
            // resting width, and never below the floor that keeps the drop
            // in one piece.
            verify(shape.waistWidth <= settled.waistWidth + 0.001);
            verify(shape.waistWidth >= shape.waistFloor - 0.001);
            verify(shape.waistWidth > 0);
            // The outline stays one closed drop: the neck never reaches the
            // mouth edges, so no frame can pinch it apart.
            verify(shape.waistCenter - shape.waistWidth / 2 > shape.mouthLeft);
            verify(shape.waistCenter + shape.waistWidth / 2 < shape.mouthRight);
            verify(shape.path.length > 0);
            thinnest = Math.min(thinnest, shape.waistWidth);
        }
        // Mid-fall is visibly thinner than rest: the drop reads as stretched.
        verify(thinnest < settled.waistWidth);
        near(progressed(0.5).flightTension, 1);
        compare(progressed(0).flightTension, 0);
    }

    function test_the_body_rect_is_what_the_outline_encloses() {
        // The carried content rides inside the drop, so the rectangle the
        // shell translates it to must be the momentary body and nothing else.
        const settled = progressed(1);
        compare(settled.openRect.x, 70);
        compare(settled.openRect.y, 36);
        compare(settled.openRect.width, 620);
        compare(settled.openRect.height, 420);

        for (let step = 0; step <= 10; ++step) {
            const shape = progressed(step / 10);
            near(shape.openRect.x, shape.openLeft);
            near(shape.openRect.width, shape.openRight - shape.openLeft);
            near(shape.openRect.height, shape.openDepth);
            // The full-sized body never crosses its own seam.
            verify(shape.openRect.y > 0);
            // And never changes size while it falls.
            near(shape.openRect.width, settled.openRect.width);
            near(shape.openRect.height, settled.openRect.height);
        }
    }

    function test_the_fall_bounces_past_rest_and_is_drawn_back() {
        const settled = progressed(1);
        // Past 1 the body has been carried below its resting place by its own
        // weight. It keeps its full size — only its distance from the seam
        // gives.
        const carried = progressed(1.05);
        verify(carried.recoil > 0);
        verify(carried.recoilTravel > 0);
        verify(carried.openRect.y > settled.openRect.y);
        near(carried.openRect.width, settled.openRect.width);
        near(carried.openRect.height, settled.openRect.height);
        // A bouncing membrane is taut, so the neck is thinner than at rest.
        verify(carried.waistWidth < settled.waistWidth);
        verify(carried.waistWidth >= carried.waistFloor - 0.001);
        // The mouth still does not move, so the drop stays welded to the bar
        // while it springs.
        near(carried.mouthLeft, settled.mouthLeft);
        near(carried.mouthRight, settled.mouthRight);

        // The bounce is bounded twice: an arbitrarily large overshoot can
        // neither swing the surface nor read as a jump on a short connector.
        const extreme = progressed(3);
        verify(extreme.recoilTravel <= 9.001);
        verify(extreme.openRect.y <= settled.openRect.y + 9.001);
        verify(extreme.openRect.y <= settled.openRect.y + 36 * 0.3 + 0.001);
    }

    function test_a_side_attached_child_falls_with_the_same_progress() {
        const born = EdgeAttachedGeometry.sideAttachedMembrane(
            380, 700, 0, 20, 356, 640, 180, 24,
            CelestinaTheme.radiusMd, true, 0);
        const settled = EdgeAttachedGeometry.sideAttachedMembrane(
            380, 700, 0, 20, 356, 640, 180, 24,
            CelestinaTheme.radiusMd, true, 1);
        verify(born.path.length > 0);
        near(born.mouthLeft, settled.mouthLeft);
        near(born.mouthRight, settled.mouthRight);
        // The full-sized child hangs at the parent's edge and falls away
        // from it sideways; its size never changes.
        near(born.openRect.width, settled.openRect.width);
        near(born.openRect.height, settled.openRect.height);
        verify(born.openRect.x > settled.openRect.x);
        verify(born.waistWidth >= born.waistFloor - 0.001);
        // The seam stays on the parent's edge for the whole fall.
        compare(born.polygon[0].x, 380);
        compare(born.polygon[1].x, 380);
    }

    function test_droplet_tangents_cling_to_the_bar_and_land_flat() {
        const shape = membrane(371, 18, 70, 620, 36, 420);
        // Meniscus: the first curve command leaves the mouth with a
        // horizontal tangent (its first control point stays on the seam).
        const firstCurve = shape.path.split("C")[1].trim().split(" ");
        near(parseFloat(firstCurve[1]), 0);
        // Landing: the swell's last control point lies on the body top edge,
        // so the membrane meets the body tangentially instead of at a corner.
        const secondCurve = shape.path.split("C")[2].trim().split(" ");
        near(parseFloat(secondCurve[3]), 36);
        near(parseFloat(secondCurve[5]), 36);
    }

}
