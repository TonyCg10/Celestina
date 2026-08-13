import QtQuick
import QtTest
import CelestinaStyle

// The icon catalogue answers names it does not own. A consumer supplies them,
// and through ~/.config/siderita/icons.conf they ultimately come from a file a
// person can edit by hand, so the lookup has to treat every name as input
// rather than as a key that is certainly a key.
TestCase {
    id: testCase

    name: "IconCatalog"

    // Names that exist on every JavaScript object but on no icon. On a plain
    // object literal these resolve up the prototype chain to inherited
    // functions, so the table answers a lookup for something that is not in it.
    readonly property var prototypeNames: [
        "toString", "constructor", "valueOf", "hasOwnProperty",
        "__proto__", "isPrototypeOf", "propertyIsEnumerable"
    ]

    function test_a_prototype_name_is_not_a_catalogue_entry() {
        for (const name of prototypeNames) {
            compare(CelestinaIcons.available[name], undefined,
                    name + ": the availability table answered for it")
            compare(CelestinaIcons.aliases[name], undefined,
                    name + ": the alias table answered for it")
        }
    }

    // The fallback chain still runs for such a name: an unknown name is a file,
    // which is the visible fallback the contract promises, and never an empty
    // source or a thrown lookup.
    function test_a_prototype_name_falls_back_like_any_unknown_name() {
        for (const name of prototypeNames) {
            compare(CelestinaIcons.resolve(name, ""), "file",
                    name + ": resolved to something else")
            compare(CelestinaIcons.resolve("", name), "file",
                    name + ": resolved to something else as a fallback")
            verify(CelestinaIcons.source(name, "").endsWith("file.svg"))
        }
    }

    // Guarding the tables must not cost the catalogue its ordinary answers.
    function test_the_catalogue_still_resolves_what_it_owns() {
        compare(CelestinaIcons.resolve("folder", ""), "folder")
        compare(CelestinaIcons.resolve("user-home", ""), "go-home")
        compare(CelestinaIcons.resolve("no-such-icon", "search"), "search")
        compare(CelestinaIcons.resolve("folder-nowhere", ""), "folder")
        compare(CelestinaIcons.resolve("", ""), "")
        verify(CelestinaIcons.available["search"] === true)
        // `plus` and `minus` are named together on purpose: they are one
        // stepper, and a catalogue that carries only the raising half leaves
        // every consumer of it unable to lower anything.
        for (const name of ["wifi", "bluetooth", "cpu", "memory-stick", "mic", "mic-off",
                            "bell", "bell-off", "power", "sun", "gauge", "leaf", "zap",
                            "toolbox", "system-tray", "pin", "eye", "eye-off",
                            "plus", "minus"]) {
            compare(CelestinaIcons.resolve(name, ""), name)
            verify(CelestinaIcons.source(name, "").endsWith(name + ".svg"))
        }
        // The semantic toolbox slot must remain independent from the folder
        // family: its vendored asset is Lucide's literal tool case.
        compare(CelestinaIcons.resolve("toolbox", "folder"), "toolbox")
        compare(CelestinaIcons.resolve("system-tray", "view-grid"),
                "system-tray")
    }

    // Both tables are still enumerable objects: the guard is on the prototype,
    // not on the shape consumers and tests read.
    function test_both_tables_are_still_readable_tables() {
        let names = 0
        for (const name in CelestinaIcons.available)
            names += 1
        verify(names > 50, "the availability table lost its entries")

        let aliases = 0
        for (const name in CelestinaIcons.aliases)
            aliases += 1
        verify(aliases > 40, "the alias table lost its entries")
    }
}
