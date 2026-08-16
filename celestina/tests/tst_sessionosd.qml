import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// What the on-screen display paints for the values it is handed. It is
// constructed offscreen here: this proves the component builds and reads
// truthfully, never how it looks on a compositor — that stays in VALIDATION.md.
TestCase {
    id: testCase

    name: "SessionOsd"

    // Repeater delegates are visual children, not QObject children, so the
    // fields are reached by walking the scene rather than by findChild.
    function fields() {
        const found = [];
        function walk(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-soft-menu-field")
                    found.push(child);
                walk(child);
            }
        }
        walk(osd.contentItem);
        return found;
    }

    function prepareBottomField() {
        // Remove the fixture card synchronously, then create the bottom-route
        // delegate with its reveal gate held closed. `resetForReuse()` is the
        // shared persistent-carrier contract and avoids racing the delegate's
        // automatic reveal fallback in an offscreen Quick Test window.
        osd.reducedMotion = true;
        osd.readings = [];
        osd.syncCards();
        osd.departingKinds = [];
        osd.entersFromBottom = true;
        osd.shellScale = 1.0;
        osd.surfaceOriginX = 0;
        osd.surfaceWidth = osd.cardWidth;
        osd.surfaceHeight = osd.cardHeight + CelestinaTheme.spaceLg;
        osd.readings = [
            {"kind": "volume", "percent": 40, "muted": false, "label": ""}
        ];
        const field = testCase.fields()[0];
        verify(field);
        field.resetForReuse();
        osd.collectGlass();
        osd.reducedMotion = false;
        return field;
    }

    Desktop.SessionOsd {
        id: osd

        kind: "volume"
        percent: 40
        muted: false
        label: ""
        reducedMotion: false
    }

    function init() {
        osd.reducedMotion = true;
        osd.readings = [];
        osd.kind = "volume";
        osd.percent = 40;
        osd.muted = false;
        osd.label = "";
        osd.anchoredFromPanel = false;
        osd.attachmentStartY = -1;
        osd.surfaceOriginX = 0;
        osd.surfaceWidth = osd.cardWidth;
        osd.surfaceHeight = osd.cardHeight;
        osd.shellScale = 1.0;
        osd.entersFromBottom = false;
        osd.readings = [
            {"kind": "volume", "percent": 40, "muted": false, "label": ""}
        ];
        osd.reducedMotion = false;
    }

    function cleanup() {
        osd.reducedMotion = true;
        osd.readings = [];
        osd.syncCards();
        osd.departingKinds = [];
        osd.visible = false;
        tryVerify(function() { return testCase.fields().length === 0; });
        osd.entersFromBottom = false;
        osd.reducedMotion = false;
    }

    function test_a_level_reads_as_whole_percent() {
        osd.kind = "volume";
        osd.percent = 40;
        osd.muted = false;
        compare(osd.headline, "Volumen");
        compare(osd.valueText, "40 %");
        verify(osd.hasLevel);
    }

    function test_a_muted_device_says_so_instead_of_its_number() {
        osd.kind = "volume";
        osd.percent = 40;
        osd.muted = true;
        compare(osd.valueText, "Silenciado");
        // The level it remembers is still there to be drawn.
        verify(osd.hasLevel);
    }

    function test_no_reading_is_not_a_level_of_zero() {
        osd.kind = "microphone";
        osd.percent = -1;
        osd.muted = false;
        verify(!osd.hasLevel);
        compare(osd.valueText, "Sin lectura");
    }

    function test_a_monitor_is_named_in_its_title() {
        osd.kind = "brightness";
        osd.percent = 55;
        osd.muted = false;
        osd.label = "DP-2";
        compare(osd.headline, "Brillo — DP-2");
        compare(osd.valueText, "55 %");
    }

    function test_what_is_spoken_carries_both_facts() {
        osd.kind = "volume";
        osd.percent = 30;
        osd.muted = false;
        osd.label = "";
        compare(osd.spokenText, "Volumen: 30 %");
    }

    function test_an_unknown_kind_is_shown_rather_than_dropped() {
        osd.kind = "night-light";
        compare(osd.headline, "night-light");
    }

    // The display is made of the shell's glass, not of its own opaque plate:
    // one contextual veil over a compositor sample, carrying one denser
    // content section. A surface that published no region is exactly the bug
    // that made it read as a solid rectangle over the desktop.
    function test_it_is_the_same_glass_as_the_bar_and_its_menus() {
        // Only a mapped surface has a region to publish: the collector skips
        // geometry nobody is looking at.
        osd.visible = true;
        const field = testCase.fields()[0];
        verify(field);
        field.reveal();
        tryVerify(function() {
            return osd.glassRegions.length === 1;
        });

        const body = findChild(field, "celestina-menu-body-tint");
        verify(body);
        compare(body.backdropMode, GlassSurface.ExternalBackdrop);
        compare(body.captureActive, false);
        compare(body.materialRole, GlassSurface.ContextualVeil);
        compare(body.elevation, 0);

        const section = findChild(field, "celestina-menu-section");
        verify(section);
        compare(section.materialRole, GlassSurface.ContentSurface);
        verify(body.materialStrength < section.materialStrength);
        osd.visible = false;
    }

    function test_bottom_entry_waits_for_reveal_before_moving_or_glass() {
        const field = testCase.prepareBottomField();
        verify(field.transform.length > 0);
        const ride = field.transform[0];
        const offscreen = osd.cardHeight + CelestinaTheme.spaceLg;

        compare(field.revealed, false);
        compare(ride.y, offscreen);
        compare(osd.glassRegions.length, 0);
        // Longer than SoftMenuField's offscreen reveal fallback: resetting the
        // reusable field must have cancelled both presentation and movement.
        wait(70);
        compare(field.revealed, false);
        compare(ride.y, offscreen);
        compare(osd.glassRegions.length, 0);

        field.revealNow();
        tryVerify(function() { return ride.y < offscreen; });
        tryVerify(function() { return osd.glassRegions.length === 1; });
    }

    function test_hidden_persistent_carrier_waits_to_spend_its_reveal() {
        osd.reducedMotion = false;
        osd.visible = false;
        osd.readings = [];
        osd.syncCards();
        osd.departingKinds = [];
        osd.entersFromBottom = true;
        osd.surfaceHeight = osd.cardHeight + CelestinaTheme.spaceLg;
        osd.readings = [
            {"kind": "volume", "percent": 40,
             "muted": false, "label": ""}
        ];
        const field = testCase.fields()[0];
        verify(field);

        // Longer than SoftMenuField's fallback. Component completion must not
        // animate a delegate that its persistent QWindow has not shown yet.
        wait(70);
        compare(field.revealed, false);
        compare(osd.glassRegions.length, 0);

        osd.visible = true;
        tryCompare(field, "revealed", true);
        tryVerify(function() { return osd.glassRegions.length === 1; });
    }

    function test_bottom_glass_follows_the_real_entry_transform() {
        const field = testCase.prepareBottomField();
        const ride = field.transform[0];
        const offscreen = osd.cardHeight + CelestinaTheme.spaceLg;
        field.revealNow();

        let sawMovingRegion = false;
        for (let step = 0; step < 24; ++step) {
            wait(8);
            if (ride.y <= 1 || ride.y >= offscreen - 1
                    || osd.glassRegions.length === 0)
                continue;
            const moving = osd.glassRegions[0].rect;
            // The animation may advance once between the deferred collector
            // and this sample. A token-sized bound still rejects either old
            // failure: a stationary landed or fully offscreen footprint.
            verify(Math.abs(moving.y - ride.y) < CelestinaTheme.spaceSm,
                   "glass y " + moving.y + " did not follow bottom ride y "
                   + ride.y + " at step " + step);
            sawMovingRegion = true;
            break;
        }
        verify(sawMovingRegion, "no transformed entry region was observed");

        tryVerify(function() { return Math.abs(ride.y) < 0.01; });
        tryVerify(function() {
            return osd.glassRegions.length === 1
                    && Math.abs(osd.glassRegions[0].rect.y) < 0.01;
        });
    }

    // The display attaches to the bar exactly as a menu does: handed the
    // membrane contract in output-local units, its card centres on the
    // control past the window's origin and the field carries the top
    // attachment. The card's stated size is the host's constant, so a drift
    // between the two is caught here rather than as a mouth beside its card.
    function test_an_anchored_display_hangs_from_the_bar_like_a_menu() {
        osd.reducedMotion = true;
        osd.shellScale = 1.0;
        osd.surfaceOriginX = 1200;
        osd.surfaceWidth = 720;
        osd.surfaceHeight = 232;
        osd.openerRect = Qt.rect(1500, 5, 60, 30);
        osd.attachmentAnchorRect = Qt.rect(1520, 11, 18, 18);
        osd.attachmentStartY = 40;
        osd.anchoredFromPanel = true;

        const field = testCase.fields()[0];
        verify(field);
        compare(field.width, osd.cardWidth);
        compare(field.height, osd.cardHeight);
        verify(field.attachedToTop);
        verify(field.topAttachmentRequested);
        // Centred on the opener: 1500 + 30 - 130 = 1400 on the output, which
        // is 200 inside this window.
        compare(field.x, 200);
        // Below the seam, so the membrane has a drop to draw.
        verify(field.y > osd.attachmentStartY);
        compare(field.surfacePosition.x, osd.surfaceOriginX + field.x);
        compare(field.surfacePosition.y, field.y);
        verify(field.edgeShapeActive);

        osd.anchoredFromPanel = false;
        osd.attachmentStartY = -1;
        osd.surfaceOriginX = 0;
        osd.surfaceWidth = osd.cardWidth;
        osd.surfaceHeight = osd.cardHeight;
        osd.reducedMotion = false;
    }

    // The paint law of the attached window, proven on pixels: while a card
    // falls — and even if a card is misplaced by any geometry race — nothing
    // may show above the seam, where the bar lives. The author's recordings
    // caught the finished card over the bar's own icons twice, through two
    // different orderings; this grabs real frames during the entry and
    // asserts the strip stays untouched, whatever future ordering appears.
    function test_nothing_shows_above_the_seam_on_the_attached_window() {
        osd.visible = true;
        osd.reducedMotion = false;
        osd.shellScale = 1.0;
        osd.surfaceOriginX = 1200;
        osd.surfaceWidth = 720;
        osd.surfaceHeight = 232;
        osd.openerRect = Qt.rect(1500, 5, 60, 30);
        osd.attachmentAnchorRect = Qt.rect(1520, 11, 18, 18);
        osd.attachmentStartY = 40;
        osd.anchoredFromPanel = true;
        osd.readings = [{"kind": "volume", "percent": 40, "muted": false,
                         "label": ""}];

        const field = testCase.fields()[0];
        verify(field);
        field.revealNow();
        // Sample frames across the whole fall, the recoil included.
        for (let step = 0; step < 8; ++step) {
            wait(30);
            const shot = grabImage(osd.contentItem);
            for (let y = 0; y < osd.attachmentStartY - 1; y += 6) {
                for (let x = 0; x < osd.surfaceWidth; x += 24) {
                    const px = shot.pixel(x, y);
                    verify((px & 0xFF000000) === 0
                           || px === shot.pixel(0, 0),
                           "painted above the seam at " + x + "," + y
                           + " step " + step);
                }
            }
        }

        osd.readings = [];
        osd.anchoredFromPanel = false;
        osd.attachmentStartY = -1;
        osd.surfaceOriginX = 0;
        osd.surfaceWidth = osd.cardWidth;
        osd.surfaceHeight = osd.cardHeight;
    }

    // The window still renders whatever file it is handed — the host is the
    // one that now keeps a single card, the latest change. What this pins is
    // the choreography around a row leaving: it recedes for one exit beat
    // instead of vanishing, and only then is it removed.
    function test_two_readings_are_two_cards_in_one_file() {
        osd.readings = [
            {"kind": "volume", "percent": 30, "muted": false, "label": ""},
            {"kind": "brightness", "percent": 70, "muted": false, "label": "DP-1"}
        ];

        // The synthesized card of whichever test ran before recedes for its
        // exit beat first; the file settles at the two real readings.
        tryVerify(function() { return testCase.fields().length === 2; });
        let cards = testCase.fields();
        cards.sort(function(a, b) { return a.y - b.y; });
        compare(cards[0].kind, "volume");
        compare(cards[1].kind, "brightness");
        compare(cards[1].y - cards[0].y, osd.stackPeek);
        verify(cards[0].z > cards[1].z);
        compare(cards[0].cardValueText, "30 %");
        compare(cards[1].cardHeadline, "Brillo — DP-1");
        // The window grew by exactly one peek for the card behind.
        compare(osd.neededHeight, osd.cardHeight + osd.stackPeek);

        // Brightness becomes the news: same two cards, reordered in place.
        osd.readings = [
            {"kind": "brightness", "percent": 75, "muted": false, "label": "DP-1"},
            {"kind": "volume", "percent": 30, "muted": false, "label": ""}
        ];
        cards = testCase.fields();
        compare(cards.length, 2);
        cards.sort(function(a, b) { return a.y - b.y; });
        compare(cards[0].kind, "brightness");
        compare(cards[0].cardValueText, "75 %");

        // Emptying does not vanish the rows: they recede — faded, shrunk —
        // for one exit beat, and the sweep then removes them. Front
        // compatibility properties never synthesize a replacement card.
        osd.readings = [];
        verify(testCase.fields().length >= 1);
        tryVerify(function() { return testCase.fields().length === 0; });
    }

    // A kind the host replaced recedes by moving away: faded and shrunk while
    // its exit beat plays, removed after it.
    function test_a_replaced_card_recedes_instead_of_vanishing() {
        osd.readings = [
            {"kind": "volume", "percent": 30, "muted": false, "label": ""}
        ];
        tryVerify(function() { return testCase.fields().length === 1; });

        osd.readings = [
            {"kind": "brightness", "percent": 70, "muted": false, "label": "DP-1"}
        ];
        // Both rows exist during the exit beat: the newcomer entering and the
        // replaced card receding toward zero opacity.
        compare(testCase.fields().length, 2);
        let leaving = null;
        const cards = testCase.fields();
        for (let index = 0; index < cards.length; ++index) {
            if (cards[index].kind === "volume")
                leaving = cards[index];
        }
        verify(leaving);
        verify(leaving.departing);
        tryVerify(function() { return leaving.opacity < 1; });
        // The sweep removes it once the beat has played.
        tryVerify(function() { return testCase.fields().length === 1; });
        compare(testCase.fields()[0].kind, "brightness");
        osd.readings = [];
        tryVerify(function() { return testCase.fields().length === 0; });
    }

    function test_bottom_glass_shrinks_with_departing_paint() {
        const leaving = testCase.prepareBottomField();
        const ride = leaving.transform[0];
        leaving.revealNow();
        tryVerify(function() { return Math.abs(ride.y) < 0.01; });
        tryVerify(function() { return osd.glassRegions.length === 1; });
        const resting = osd.glassRegions[0].rect;

        osd.readings = [];
        compare(osd.departingKinds.length, 1);
        let faded = null;
        for (let step = 0; step < 28; ++step) {
            wait(8);
            if (testCase.fields().indexOf(leaving) < 0)
                break;
            if (leaving.opacity < 0.35 && osd.glassRegions.length === 1) {
                faded = osd.glassRegions[0].rect;
                break;
            }
        }
        verify(faded !== null,
               "no glass region accompanied the nearly exhausted paint");
        verify(faded.width < resting.width - 1);
        verify(faded.height < resting.height - 1);
        verify(Math.abs((faded.x + faded.width / 2)
                        - (resting.x + resting.width / 2)) < 1);
        verify(Math.abs((faded.y + faded.height / 2)
                        - (resting.y + resting.height / 2)) < 1);

        tryVerify(function() { return testCase.fields().length === 0; });
        tryVerify(function() { return osd.glassRegions.length === 0; });
    }

    function test_only_the_presenting_twin_can_receive_a_card_file() {
        const component = Qt.createComponent("../qml/SessionOsd.qml");
        verify(component.status === Component.Ready, component.errorString());
        const first = component.createObject(null, {
            "kind": "volume", "percent": 35, "muted": false, "label": "",
            "reducedMotion": false,
            "readings": []
        });
        const second = component.createObject(null, {
            "kind": "volume", "percent": 35, "muted": false, "label": "",
            "reducedMotion": false,
            "readings": []
        });
        verify(first && second);
        compare(first.cards.length, 0);
        compare(second.cards.length, 0);

        first.readings = [
            {"kind": "volume", "percent": 35, "muted": false, "label": ""}
        ];
        compare(first.cards.length, 1);
        compare(second.cards.length, 0);

        first.readings = [];
        second.readings = [
            {"kind": "volume", "percent": 35, "muted": false, "label": ""}
        ];
        compare(first.cards.length, 0);
        compare(second.cards.length, 1);
        first.destroy();
        second.destroy();
    }

    // The three quantities the bar shows are named here by the same glyphs.
    function test_each_reading_wears_the_glyph_its_panel_control_does() {
        osd.muted = false;
        osd.kind = "volume";
        compare(osd.iconName, "media-volume");
        osd.muted = true;
        compare(osd.iconName, "media-volume-muted");
        osd.kind = "microphone";
        compare(osd.iconName, "mic-off");
        osd.muted = false;
        compare(osd.iconName, "mic");
        osd.kind = "brightness";
        compare(osd.iconName, "sun");
    }
}
