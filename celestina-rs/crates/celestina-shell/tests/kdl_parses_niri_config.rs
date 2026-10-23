//! Proves the `kdl` crate reads niri's config syntax: binding modes with
//! `exit-on-action=true`, `repeat=false`, `hotkey-overlay-title`, comments and
//! a `spawn` with several string arguments.

use kdl::KdlDocument;

const FIXTURE: &str = include_str!("fixtures/niri-binds.kdl");

#[test]
fn niri_binds_fixture_parses() {
    let doc: KdlDocument = FIXTURE.parse().expect("the fixture is niri-style KDL");

    let modes: Vec<_> = doc
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "binding-mode")
        .collect();
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0].get(0).and_then(|v| v.as_string()), Some("monitor"));
    assert_eq!(
        modes[0].get("exit-on-action").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(modes[1].get("exit-on-action").is_none());

    let binds = doc
        .get("binds")
        .and_then(|n| n.children())
        .expect("a binds block");
    assert_eq!(
        binds.nodes().len(),
        6,
        "the commented-out bind is not a node"
    );

    let terminal = binds.get("Mod+T").expect("Mod+T");
    assert_eq!(
        terminal
            .get("hotkey-overlay-title")
            .and_then(|v| v.as_string()),
        Some("Open a terminal")
    );
    let spawn = terminal
        .children()
        .and_then(|c| c.get("spawn"))
        .expect("spawn action");
    assert_eq!(spawn.entries().len(), 4);

    let blackout = binds.get("Mod+1").expect("Mod+1");
    assert_eq!(
        blackout.get("repeat").and_then(|v| v.as_bool()),
        Some(false)
    );
}
