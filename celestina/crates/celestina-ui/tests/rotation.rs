//! A node's rotation: a view says an angle, a change turns on the open
//! spring (the theme's slow step with the overshoot easing), reduced motion
//! makes it one step, and a node at rest asks for no frame.

use std::time::{Duration, Instant};

use celestina_paint::{TextStyle, Theme};
use celestina_ui::{view, Key, Tree, View};

fn measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
    (7.0 * s.chars().count() as f32, 16.0)
}

fn glyph(angle: f32) -> View {
    view::col(Key::named("root"))
        .child(view::icon(Key::named("glyph"), "chevron-up", 18.0).rotate(angle))
}

fn tree_at(angle: f32, reduced: bool, t0: Instant) -> Tree {
    let mut tree = Tree::new(reduced, &Theme::dark());
    tree.apply(glyph(angle), t0);
    tree.layout_measured((100.0, 100.0), measure);
    tree
}

fn node(tree: &Tree) -> celestina_ui::NodeId {
    tree.node_by_key(Key::named("glyph")).expect("glyph node")
}

#[test]
fn a_view_without_a_rotation_is_upright_and_still() {
    let t0 = Instant::now();
    let tree = tree_at(0.0, false, t0);
    assert_eq!(tree.rotation(node(&tree), t0), 0.0);
    assert!(!tree.is_animating(t0));
}

#[test]
fn a_new_angle_turns_on_the_slow_step_and_overshoots() {
    let t0 = Instant::now();
    let mut tree = tree_at(0.0, false, t0);
    tree.apply(glyph(180.0), t0);
    tree.layout_measured((100.0, 100.0), measure);
    let id = node(&tree);
    assert_eq!(tree.rotation(id, t0), 0.0, "it starts where it was");
    assert!(tree.is_animating(t0));
    let slow = Duration::from_millis(350);
    let mid = tree.rotation(id, t0 + slow / 2);
    assert!(mid > 0.0 && mid < 180.0 + 30.0, "{mid}");
    // OutBack goes past the target before it settles.
    let past = (1..35)
        .map(|i| tree.rotation(id, t0 + slow * i / 35))
        .fold(0.0_f32, f32::max);
    assert!(past > 180.0, "the overshoot: {past}");
    assert_eq!(tree.rotation(id, t0 + slow), 180.0);
    assert!(!tree.is_animating(t0 + slow), "at rest: no frames");
}

#[test]
fn reduced_motion_flips_with_no_turn() {
    let t0 = Instant::now();
    let mut tree = tree_at(0.0, true, t0);
    tree.apply(glyph(180.0), t0);
    tree.layout_measured((100.0, 100.0), measure);
    assert_eq!(tree.rotation(node(&tree), t0), 180.0);
    assert!(!tree.is_animating(t0));
}

#[test]
fn a_turn_back_continues_from_where_it_is() {
    let t0 = Instant::now();
    let mut tree = tree_at(0.0, false, t0);
    tree.apply(glyph(180.0), t0);
    tree.layout_measured((100.0, 100.0), measure);
    let t1 = t0 + Duration::from_millis(100);
    let here = tree.rotation(node(&tree), t1);
    tree.apply(glyph(0.0), t1);
    tree.layout_measured((100.0, 100.0), measure);
    assert_eq!(tree.rotation(node(&tree), t1), here);
}
