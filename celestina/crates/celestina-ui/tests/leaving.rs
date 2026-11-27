//! A node that leaves keeps fading where it was drawn: the slide of the
//! ancestors that move because it left does not carry it along.

use std::time::{Duration, Instant};

use celestina_paint::{Role, TextStyle, Theme};
use celestina_ui::{view, Key, Tree, View};

fn measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
    (7.0 * s.chars().count() as f32, 16.0)
}

fn cell(name: &'static str) -> View {
    view::row(Key::named(name))
        .size(20.0, 20.0)
        .fill(Role::ControlFill)
}

/// A group `shift` px from the left holding `cells`.
fn scene(shift: f32, cells: &[&'static str]) -> View {
    view::col(Key::named("root"))
        .width(300.0)
        .padding_xy(shift, 0.0)
        .child(view::row(Key::named("group")).children(cells.iter().map(|c| cell(c))))
}

#[test]
fn a_leaving_node_is_not_carried_by_the_slide_of_its_ancestors() {
    let t0 = Instant::now();
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(scene(0.0, &["a", "b"]), t0);
    tree.layout_measured((300.0, 100.0), measure);
    let id = |tree: &Tree, k| tree.node_by_key(Key::named(k)).expect("node");
    let before = tree.rect(id(&tree, "b"));
    // The group moves 34 px right and `b` leaves.
    tree.apply(scene(34.0, &["a"]), t0);
    tree.layout_measured((300.0, 100.0), measure);
    let mid = t0 + Duration::from_millis(60);
    let ghost = tree.painted_rect(id(&tree, "b"), mid);
    assert_eq!(ghost, before, "it stays where it was drawn");
}
