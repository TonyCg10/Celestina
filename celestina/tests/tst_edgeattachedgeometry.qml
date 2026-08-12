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
