import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// SIMPLE-1 (2026-08-22): the author reset the surface system. The fall, the
// membrane silhouette, the entry rides and the compositor glass this suite
// used to pin are gone: every surface is one solid dark card, and its one
// animation is a fade in on reveal and a fade out on retirement. What is
// left to defend is exactly that contract — settled geometry from the first
// frame, the presentation gate holding paint at zero, one fade each way,
// and no glass ever published.
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

            // The frost lives in the content cards now; a field carrying one
            // section is the anatomy every real surface has.
            Desktop.MenuSection {
                ink: testInk
            }
        }
    }

    function fieldWith(reducedMotion) {
        return fieldComponent.createObject(
            testCase, {"reducedMotion": reducedMotion});
    }

    function visualOf(field) {
        return findChild(field, "celestina-soft-menu-content");
    }

    // Every route resolves settled immediately: no fall, no ride, no
    // silhouette — the attachment inputs place the card and nothing more.
    function test_every_route_opens_at_the_settled_geometry() {
        const field = testCase.fieldWith(false);
        verify(field);
        verify(!field.fallsIntoPlace);
        verify(!field.edgeShapeActive);
        compare(field.attachmentProgress, 1);
        compare(field.entryOffsetY, 0);
        field.destroy();
    }

    // The presentation gate is unchanged: nothing paints before the reveal,
    // whatever the route knows or does not know yet.
    function test_no_route_paints_before_the_presentation_gate() {
        const field = testCase.fieldWith(false);
        compare(field.presentationOpacity, 0);
        compare(testCase.visualOf(field).opacity, 0);

        field.revealNow();
        compare(field.presentationOpacity, 1);
        tryVerify(function() {
            return testCase.visualOf(field).opacity === 1;
        });
        field.destroy();
    }

    // The one entry: a fade through a real intermediate frame.
    function test_the_reveal_is_one_fade_in() {
        const field = testCase.fieldWith(false);
        const visual = testCase.visualOf(field);
        field.revealNow();

        let sawMidFade = false;
        for (let step = 0; step < 24; ++step) {
            wait(8);
            if (visual.opacity > 0.05 && visual.opacity < 0.95) {
                sawMidFade = true;
                break;
            }
        }
        verify(sawMidFade, "no intermediate fade-in frame was observed");
        tryVerify(function() { return visual.opacity === 1; });
        field.destroy();
    }

    // The one exit: the same fade, back down, with no shrink.
    function test_the_retirement_is_one_fade_out() {
        const field = testCase.fieldWith(false);
        const visual = testCase.visualOf(field);
        field.revealNow();
        tryVerify(function() { return visual.opacity === 1; });

        field.retire();
        verify(field.retiring);
        compare(visual.scale, 1);
        let sawMidFade = false;
        for (let step = 0; step < 24; ++step) {
            wait(8);
            if (visual.opacity > 0.05 && visual.opacity < 0.95) {
                sawMidFade = true;
                break;
            }
        }
        verify(sawMidFade, "no intermediate fade-out frame was observed");
        tryVerify(function() { return visual.opacity === 0; });
        compare(visual.scale, 1);
        field.destroy();
    }

    // Reduced motion resolves both edges instantly.
    function test_reduced_motion_resolves_instantly() {
        const field = testCase.fieldWith(true);
        compare(testCase.visualOf(field).opacity, 0);
        field.revealNow();
        compare(testCase.visualOf(field).opacity, 1);
        field.retire();
        compare(testCase.visualOf(field).opacity, 0);
        field.destroy();
    }

    // The frost is published only at rest: one settled rectangle once the
    // fade has fully landed, nothing while it is still moving, and nothing
    // from the first frame of a retirement on.
    function test_the_frost_is_published_only_at_rest() {
        const field = testCase.fieldWith(false);
        field.revealNow();
        // Mid-fade: no region yet.
        field.collectGlass();
        compare(field.glassRegions.length, 0);

        tryVerify(function() {
            return testCase.visualOf(field).opacity === 1;
        });
        tryVerify(function() { return field.glassRegions.length === 1; });
        compare(field.glassRects.length, 1);

        field.retire();
        compare(field.glassRegions.length, 0);
        compare(field.glassRects.length, 0);
        field.destroy();
    }

    // A retiring field refuses its reveal — the terminal edge stays
    // irreversible for ordinary menus.
    function test_a_retiring_field_refuses_its_reveal() {
        const field = testCase.fieldWith(false);
        field.retire();
        field.reveal();
        compare(field.revealed, false);
        compare(field.presentationOpacity, 0);
        field.destroy();
    }

    // The persistent-carrier contract: revive clears the terminal edge and
    // the next reveal earns a fresh fade.
    function test_revive_gives_a_resumed_carrier_a_fresh_fade() {
        const field = testCase.fieldWith(false);
        field.revealNow();
        tryVerify(function() {
            return testCase.visualOf(field).opacity === 1;
        });
        field.retire();
        tryVerify(function() {
            return testCase.visualOf(field).opacity === 0;
        });

        field.reviveForReuse();
        compare(field.retiring, false);
        compare(field.revealed, false);
        compare(field.presentationOpacity, 0);

        field.revealNow();
        compare(field.presentationOpacity, 1);
        tryVerify(function() {
            return testCase.visualOf(field).opacity === 1;
        });
        field.destroy();
    }
}
