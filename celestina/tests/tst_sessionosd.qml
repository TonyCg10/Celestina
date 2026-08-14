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

    Desktop.SessionOsd {
        id: osd

        kind: "volume"
        percent: 40
        muted: false
        label: ""
        reducedMotion: false
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
        // for one exit beat, and the sweep then removes them, leaving the
        // synthesized single card of the compatibility route.
        osd.readings = [];
        verify(testCase.fields().length >= 1);
        tryVerify(function() { return testCase.fields().length === 1; });
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
        tryVerify(function() { return testCase.fields().length === 1; });
    }

    function test_probe_creation_like_host() {
        const component = Qt.createComponent("../qml/SessionOsd.qml");
        verify(component.status === Component.Ready, component.errorString());
        const win = component.createObject(null, {
            "kind": "volume", "percent": 35, "muted": false, "label": "",
            "reducedMotion": false,
            "readings": [{"kind": "volume", "percent": 35, "muted": false, "label": ""}],
            "anchoredFromPanel": true,
            "openerRect": Qt.rect(1500, 5, 60, 30),
            "attachmentAnchorRect": Qt.rect(1520, 11, 18, 18),
            "attachmentStartY": 40,
            "surfaceOriginX": 1200,
            "surfaceWidth": 720,
            "surfaceHeight": 260,
            "shellScale": 1.0
        });
        verify(win);
        win.visible = true;
        wait(600);
        const found = [];
        function walk(item) {
            for (let i = 0; i < item.children.length; ++i) {
                const c = item.children[i];
                if (c.objectName === "celestina-soft-menu-field") found.push(c);
                walk(c);
            }
        }
        walk(win.contentItem);
        console.warn("fields:", found.length);
        for (let i = 0; i < found.length; ++i) {
            const f = found[i];
            const content = findChild(f, "celestina-soft-menu-content");
            console.warn(i, "revealed", f.revealed, "presented", f.surfacePresented,
                         "queued", f.fallQueued, "progress", f.attachmentProgress,
                         "opacity", content ? content.opacity : "?");
        }
        win.destroy();
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
