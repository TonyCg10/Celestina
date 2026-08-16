import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// PANEL-1-J gave the attached surface its morphing fall; PANEL-1-S's follow-up
// replaced the top route with a rigid fall from beyond the screen edge, while
// the side push keeps the morph. What matters here is still the contract, not
// the curve: the settled geometry is the destination, the membrane is settled
// from the first frame on the top route, the whole assembly rides one offset,
// the compositor is always told the resting place, and reduced motion never
// animates.
TestCase {
    id: testCase

    name: "AttachmentFall"
    when: windowShown
    visible: true
    width: 700
    height: 500

    Desktop.BackdropInk {
        id: testInk
    }

    Component {
        id: fieldComponent

        Desktop.SoftMenuField {
            ink: testInk
            width: 420
            height: 300
            attachedToTop: true
            openerRect: Qt.rect(300, 5, 30, 30)
            attachmentAnchorRect: Qt.rect(306, 11, 18, 18)
            attachmentStartY: 40
            surfacePosition: Qt.point(120, 72)
        }
    }

    function fieldWith(reducedMotion) {
        return fieldComponent.createObject(
            testCase, {"reducedMotion": reducedMotion});
    }

    function bodyOf(field) {
        return findChild(field, "celestina-soft-menu-body-window");
    }

    function near(actual, expected) {
        verify(Math.abs(actual - expected) < 0.001,
               "expected " + expected + ", got " + actual);
    }

    function test_reduced_motion_opens_at_the_settled_geometry() {
        const field = testCase.fieldWith(true);
        verify(field);
        verify(field.edgeShapeActive);
        verify(!field.fallsIntoPlace);
        // No frame of an animation exists for this route: the surface is
        // already where it belongs before anything is revealed.
        compare(field.attachmentProgress, 1);
        compare(field.attachmentContentOpacity, 1);
        compare(testCase.bodyOf(field).opacity, 1);

        field.reveal();
        compare(field.attachmentProgress, 1);
        field.destroy();
    }

    function test_a_falling_surface_starts_beyond_the_screen_and_lands_settled() {
        const field = testCase.fieldWith(false);
        verify(field);
        verify(field.edgeShapeActive);
        verify(field.fallsIntoPlace);

        // Born entirely above the screen: full size, no membrane yet — there
        // is no gap below the seam for one to grow in, so the silhouette is
        // the bare card at its ridden position.
        compare(field.attachmentProgress, 0);
        verify(field.entryBodyY < 0);
        near(field.entryOffsetY, -(field.surfacePosition.y + field.height));
        const born = field.edgeSilhouette;
        verify(born.path.length > 0);
        near(born.openRect.width, field.width);
        near(born.openRect.height, field.height);
        verify(born.openRect.y < 0);
        verify(born.mouthLeft === undefined || born.tension === 0);
        // And it arrives at full opacity — off screen, then entering — so
        // there is no fade to pop.
        compare(field.attachmentContentOpacity, 1);

        // Mid-descent the gap below the seam is open and the drop has grown
        // into it: a real membrane, at the ridden travel, formed by the
        // descent rather than carried by it.
        field.attachmentProgress =
            1 - (field.surfacePosition.y - field.attachmentStartY) / 2
                / (field.surfacePosition.y + field.height);
        verify(field.entryBodyY > 0.5);
        const forming = field.edgeSilhouette;
        verify(forming.tension > 0);
        near(forming.openRect.y, field.entryBodyY);

        field.reveal();
        tryCompare(field, "attachmentProgress", 1);
        compare(field.entryOffsetY, 0);
        const settled = field.edgeSilhouette;
        verify(settled.mouthLeft !== undefined);
        near(settled.openRect.y, -field.attachmentStartY
                                 + field.surfacePosition.y);
        field.destroy();
    }

    // The recoil is a fixed short dip past the resting place, not a fraction
    // of the flight: a tall menu bounces exactly as far as a short card.
    function test_the_recoil_is_a_fixed_dip_not_a_fraction_of_the_travel() {
        const field = testCase.fieldWith(false);
        verify(field);
        field.attachmentProgress = 1.05;
        near(field.entryOffsetY, CelestinaTheme.spaceLg);
        field.attachmentProgress = 1;
        compare(field.entryOffsetY, 0);
        field.destroy();
    }

    function test_the_carried_content_rides_with_the_descending_card() {
        const field = testCase.fieldWith(false);
        verify(field);
        const window = testCase.bodyOf(field);
        verify(window);
        // The rows ride at the card's ridden position — the same openRect
        // the glass draws the body at — with their settled layout intact:
        // nothing is stretched, reflowed or clipped by the motion, and the
        // seam clip is the enclosing item's job, not this window's.
        const content = findChild(field, "celestina-soft-menu-body");
        verify(content);
        compare(content.width, field.width);
        compare(content.height, field.height);
        verify(!window.clip);
        verify(window.y < 0);
        near(window.y, field.entryOffsetY);
        compare(window.width, field.width);
        compare(window.height, field.height);

        field.reveal();
        tryCompare(field, "attachmentProgress", 1);
        compare(field.entryOffsetY, 0);
        compare(window.y, 0);
        verify(!window.clip);
        field.destroy();
    }

    function test_the_blur_follows_every_frame_and_never_climbs_the_seam() {
        const field = testCase.fieldWith(false);
        verify(field);
        // The glass publishes nothing before the reveal begins — armed
        // earlier it is a milky slab leading the paint. This case is about
        // the frames of the fall, so the fall is started the way a presented
        // frame starts it.
        field.revealed = true;
        const seamMapped = field.mapToItem(null, 0, 0).y
                           + field.attachmentStartY - field.surfacePosition.y;

        // Emergence: the card is still leaving the bar and its region is the
        // visible part alone, clamped at the seam — blur under the card on
        // every frame, never a frame asking to blur the bar's own rows.
        field.attachmentProgress = 0.4;
        field.collectGlass();
        compare(field.glassRegions.length, 1);
        let polygon = field.glassRegions[0].polygon;
        verify(polygon.length >= 4);
        for (let index = 0; index < polygon.length; ++index)
            verify(polygon[index].y >= seamMapped - 0.001);

        // Formation: the growing drop publishes its own momentary outline.
        field.attachmentProgress =
            1 - (field.surfacePosition.y - field.attachmentStartY) / 2
                / (field.surfacePosition.y + field.height);
        field.collectGlass();
        compare(field.glassRegions.length, 1);
        polygon = field.glassRegions[0].polygon;
        verify(polygon.length > 4);
        for (let index = 0; index < polygon.length; ++index)
            verify(polygon[index].y >= seamMapped - 0.001);

        field.attachmentProgress = 1;
        tryVerify(function() { return field.glassRegions.length === 1; });
        verify(field.glassRegions[0].polygon.length > 3);
        field.destroy();
    }

    function test_a_settled_surface_never_falls_again() {
        const field = testCase.fieldWith(false);
        verify(field);
        field.reveal();
        tryCompare(field, "attachmentProgress", 1);
        // A route that reveals twice must not replay the opening under the
        // user, and a lease refresh must not either.
        field.reveal();
        compare(field.attachmentProgress, 1);
        field.beginDropFall();
        compare(field.attachmentProgress, 1);
        field.destroy();
    }

    // The host turns the side attachment on after the popup has already
    // revealed — synchronously, but later in the same call. The sideways push
    // must still run once, and only once: forced-settled is not fallen.
    function test_an_attachment_arriving_after_reveal_still_pushes_once() {
        const field = fieldComponent.createObject(
            testCase, {"reducedMotion": false, "attachedToTop": false});
        verify(field);
        field.reveal();
        // The reveal waits for the window's next presented frame; the
        // offscreen fallback resolves it within a beat, and this case is
        // about what happens on a window that is already open.
        tryCompare(field, "revealed", true);
        compare(field.attachmentProgress, 1);
        verify(!field.hasFallen);

        // What the tray-child host writes onto an already-open window.
        field.attachedToSide = true;
        field.attachmentSideRight = true;
        field.sideAttachmentGap = 24;
        verify(field.edgeShapeActive);
        // The push started from the seam rather than staying parked settled.
        verify(field.hasFallen);
        verify(field.attachmentProgress < 1);

        // And it is one push, not one per lease refresh.
        field.reveal();
        field.beginDropFall();
        tryCompare(field, "attachmentProgress", 1);
        field.beginDropFall();
        compare(field.attachmentProgress, 1);
        field.destroy();
    }

    // Behind the bar, not over it: while the assembly is entering, the field
    // clips everything above the seam, and at rest the clip is off so the
    // settled surface pays nothing for it.
    function test_nothing_is_painted_above_the_seam_while_entering() {
        const field = testCase.fieldWith(false);
        verify(field);
        const window = findChild(field, "celestina-soft-menu-content").parent;
        verify(window);
        // Mid-entry: clipping, and the clip region starts at the seam.
        field.attachmentProgress = 0.4;
        verify(field.entryOffsetY < 0);
        verify(window.clip);
        near(window.y, field.attachmentStartY - field.surfacePosition.y);

        field.attachmentProgress = 1;
        compare(field.entryOffsetY, 0);
        verify(!window.clip);
        field.destroy();
    }

    function test_a_floating_surface_has_no_fall_at_all() {
        const field = fieldComponent.createObject(
            testCase, {"reducedMotion": false, "attachedToTop": false});
        verify(field);
        verify(!field.edgeShapeActive);
        verify(!field.fallsIntoPlace);
        compare(field.attachmentProgress, 1);
        compare(field.attachmentContentOpacity, 1);
        field.destroy();
    }
}
