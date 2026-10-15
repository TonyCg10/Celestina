import QtQuick
import QtTest
import "../qml" as Desktop

// Request lifecycle — sending, waiting, confirming, failing, generation loss —
// is not tested here any more. It cannot be: activating a row destroys this
// window, so a case that keeps the object alive and hand-delivers a result is
// testing something the product never does. It lives on the durable ledger and
// on the real menu lifecycle instead:
// `requestledger_test.cpp` and `indicatormenu_test.cpp`.

// What the network menu shows, and what it refuses to show.
//
// Constructed offscreen: this proves the reading and request rules, never the
// appearance. Whether a compositor delivers a click outside the card is
// `indicatormenu_test.cpp`'s question and, beyond it, the author's.
TestCase {
    id: testCase

    name: "NetworkMenu"

    QtObject {
        id: fakeSource

        property bool available: true
        property var providers: ({})
        property var requests: null
        property int revision: 0
        property var sent: []
        property int nextId: 1

        signal changed()
        signal commandResult(int requestId, string state, string reason)

        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({
                "provider": provider, "verb": verb, "options": options
            });
            return fakeSource.nextId++;
        }
    }

    Desktop.NetworkMenu {
        id: menu

        outputName: "test-output"
        providerSource: fakeSource
        reducedMotion: true
    }

    // The session's real shape: a saved profile that is attached, and one that
    // is not. Identity is the UUID; the name is only a label.
    readonly property var saved: [
        {
            "id": "9f1c-1", "name": "Tonys 1", "active": true,
            "availability": "in-range", "signal": 77, "ssid": "Tonys 1"
        },
        {
            "id": "9f1c-2", "name": "Tonys 5G", "active": false,
            "availability": "unknown"
        }
    ]

    function entriesOfKind(kind) {
        const found = [];
        for (let index = 0; index < menu.entries.length; ++index) {
            if (menu.entries[index].kind === kind)
                found.push(menu.entries[index]);
        }
        return found;
    }

    function test_a_reading_tick_moves_state_without_rebuilding_rows() {
        // The aggregate publishes on every tick, so a menu whose row list is
        // rebuilt from readings tore down and recreated every row about once
        // a second. That is what left a contextual card measured against a
        // menu mid-rebuild and permanently clipped. Identity is the proof.
        const before = [];
        for (let index = 0; index < menu.menu.count; ++index)
            before.push(menu.menu.itemAt(index));
        verify(before.length > 0);
        const signature = menu.entrySignature;

        // The same saved networks, with the live link and signal moved.
        fakeSource.publish({
            "network": {
                "kind": "wifi",
                "connection": "Tonys 5G",
                "networksState": "fresh",
                "networks": [
                    {
                        "id": "9f1c-1", "name": "Tonys 1", "active": false,
                        "availability": "in-range", "signal": 41,
                        "ssid": "Tonys 1"
                    },
                    {
                        "id": "9f1c-2", "name": "Tonys 5G", "active": true,
                        "availability": "in-range", "signal": 90,
                        "ssid": "Tonys 5G"
                    }
                ]
            }
        });

        compare(menu.entrySignature, signature);
        compare(menu.menu.count, before.length);
        for (let index = 0; index < before.length; ++index) {
            verify(menu.menu.itemAt(index) === before[index],
                   "row " + index + " was recreated by a reading tick");
        }
        // And the moved state really reached those same rows.
        compare(menu.profileById("9f1c-2").active, true);
        compare(menu.linkLine, qsTr("Conectado por Wi-Fi: Tonys 5G"));
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.nextId = 1;
        fakeSource.available = true;
        fakeSource.publish({
            "network": {
                "kind": "wifi",
                "connection": "Tonys 1",
                "networksState": "fresh",
                "networks": testCase.saved
            }
        });
    }

    function test_the_inventory_has_deliberate_vertical_rhythm() {
        compare(menu.itemSpacing, 8);
        compare(menu.headerBodyGap, 12);
        compare(menu.rowVerticalInset, 4);

        const header = menu.menu.itemAt(0);
        const firstBodyRow = menu.menu.itemAt(1);
        verify(header);
        verify(firstBodyRow);
        compare(header.headerTrailingGap, 12);
        compare(header.visualHeight, menu.headerRowHeight);
        compare(header.implicitHeight - header.visualHeight,
                menu.headerBodyGap);
        compare(firstBodyRow.verticalInset, 4);
        compare(firstBodyRow.implicitHeight - firstBodyRow.visualHeight,
                menu.itemSpacing);
    }

    function test_the_link_is_named_the_way_the_provider_confirmed_it() {
        compare(menu.linkPresent, true);
        compare(menu.linkLine, qsTr("Conectado por Wi-Fi: %1").arg("Tonys 1"));

        fakeSource.publish({
            "network": {
                "kind": "ethernet", "connection": "Cable 1",
                "networksState": "fresh", "networks": []
            }
        });
        compare(menu.linkLine, qsTr("Conectado por cable: %1").arg("Cable 1"));
    }

    // A session with no default route still has saved networks, and that is
    // exactly when this menu is wanted.
    function test_a_session_with_no_link_still_lists_what_it_could_join() {
        fakeSource.publish({
            "network": {"networksState": "fresh", "networks": testCase.saved}
        });

        compare(menu.linkPresent, false);
        compare(menu.linkLine, qsTr("Sin conexión"));
        compare(testCase.entriesOfKind("profile").length, 2);
    }

    // Each availability word means something different about whether the list
    // below can be trusted, so each gets its own sentence.
    function test_every_list_state_has_its_own_spanish_sentence() {
        const said = {};
        const states = ["fresh", "held", "unavailable", "pending"];
        for (let index = 0; index < states.length; ++index) {
            fakeSource.publish({
                "network": {
                    "kind": "wifi", "connection": "Tonys 1",
                    "networksState": states[index], "networks": testCase.saved
                }
            });
            verify(menu.listLine.length > 0);
            // No two states may read the same, or the distinction is decorative.
            verify(said[menu.listLine] === undefined, menu.listLine);
            said[menu.listLine] = true;
        }

        compare(menu.listState, "pending");
        fakeSource.publish({
            "network": {"networksState": "unavailable"}
        });
        compare(menu.listLine, qsTr("No se puede consultar: falta NetworkManager"));

        // A provider that has published nothing at all is its own case.
        fakeSource.publish({});
        compare(menu.listState, "");
        compare(menu.listLine, qsTr("Sin lectura de redes todavía"));
        compare(menu.linkLine, qsTr("Sin información de red"));
    }

    // An empty confirmed list is a fact, and reads differently from having no
    // reading at all.
    function test_a_confirmed_empty_list_says_there_are_none() {
        fakeSource.publish({
            "network": {"kind": "wifi", "connection": "Tonys 1",
                        "networksState": "fresh", "networks": []}
        });

        compare(menu.listLine, qsTr("No hay redes guardadas"));
        compare(testCase.entriesOfKind("profile").length, 0);
        // The refresh entry is always offered, so an empty menu is still useful.
        compare(testCase.entriesOfKind("refresh").length, 1);
    }

    function test_a_held_list_says_it_is_an_earlier_reading() {
        fakeSource.publish({
            "network": {"kind": "wifi", "connection": "Tonys 1",
                        "networksState": "held", "networks": testCase.saved}
        });

        compare(menu.listLine, qsTr("Redes guardadas (lectura anterior)"));
        // Held rows are real rows and stay actionable.
        compare(testCase.entriesOfKind("profile").length, 2);
    }








}
