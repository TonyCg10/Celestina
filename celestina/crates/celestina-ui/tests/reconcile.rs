//! Keyed reconciliation through the public API: what survives a new view,
//! what slides, what fades, and how the result is hit and focused.

use std::time::{Duration, Instant};

use celestina_paint::{Role, TextStyle, Theme, Weight};
use celestina_ui::{view, Interaction, Key, NodeId, Tree, UiEvent, View, BTN_LEFT};
use celestina_wayland::{InputEvent, Modifiers};

const AVAILABLE: (f32, f32) = (760.0, 560.0);
const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_ESCAPE: u32 = 0xff1b;

fn body() -> TextStyle {
    TextStyle {
        size: 13.0,
        weight: Weight::Regular,
        tabular: false,
        role: Role::TextHi,
    }
}

/// 7 px per char, 16 px tall.
fn measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
    (7.0 * s.chars().count() as f32, 16.0)
}

fn row(i: u64) -> View {
    view::row(Key::indexed("row", i))
        .height(40.0)
        .fill(Role::ControlFill)
        .interactive()
        .focusable()
}

/// A 300-wide column, 20 px padding: rows at y 20, 60, 100, …
fn card(rows: impl IntoIterator<Item = View>) -> View {
    view::col(Key::named("card"))
        .width(300.0)
        .padding(20.0)
        .children(rows)
}

fn rows(order: &[u64]) -> View {
    card(order.iter().map(|&i| row(i)))
}

fn id(tree: &Tree, key: Key) -> NodeId {
    tree.node_by_key(key)
        .unwrap_or_else(|| panic!("no node for {key:?}"))
}

fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

fn applied(order: &[u64], t0: Instant) -> Tree {
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(rows(order), t0);
    tree.layout_measured(AVAILABLE, measure);
    tree
}

fn key(keysym: u32) -> InputEvent {
    InputEvent::Key {
        keysym,
        utf8: None,
        modifiers: Modifiers::default(),
        pressed: true,
        serial: 1,
    }
}

#[test]
fn apply_keeps_hover_focus_and_running_animations_on_surviving_keys() {
    let t0 = Instant::now();
    let mut tree = applied(&[0, 1, 2, 3], t0);
    let mut ia = Interaction::new();
    let row2 = id(&tree, Key::indexed("row", 2));
    // Row 2 is at y 100..140: hover, focus (Tab ×3) and press it.
    ia.pointer(
        &mut tree,
        &InputEvent::PointerMotion { x: 100.0, y: 120.0 },
        t0,
    );
    for _ in 0..3 {
        ia.key(&mut tree, &key(XKB_KEY_TAB));
    }
    let press = InputEvent::PointerPress {
        x: 100.0,
        y: 120.0,
        button: BTN_LEFT,
        serial: 7,
    };
    ia.pointer(&mut tree, &press, t0);
    let h = tree.highlight();
    assert_eq!(
        (h.hovered, h.pressed, h.focused),
        (Some(row2), Some(row2), Some(row2))
    );
    let mid = tree.scale(row2, ms(t0, 50));
    assert!(mid < 1.0 && mid > 0.96, "{mid}");

    // The rows are rebuilt in another order halfway through the recoil.
    tree.apply(rows(&[3, 2, 1, 0]), ms(t0, 50));
    tree.layout_measured(AVAILABLE, measure);
    assert_eq!(tree.node_by_key(Key::indexed("row", 2)), Some(row2));
    let h = tree.highlight();
    assert_eq!(
        (h.hovered, h.pressed, h.focused),
        (Some(row2), Some(row2), Some(row2))
    );
    // The recoil continues where it was, never restarted.
    assert_eq!(tree.scale(row2, ms(t0, 50)), mid);
    assert!(tree.scale(row2, ms(t0, 75)) < mid);
    assert!((tree.scale(row2, ms(t0, 100)) - 0.96).abs() < 1e-6);
    // Row 2 is now second (y 60..100): releasing there still clicks it.
    let release = InputEvent::PointerRelease {
        x: 100.0,
        y: 80.0,
        button: BTN_LEFT,
        serial: 8,
    };
    assert_eq!(
        ia.pointer(&mut tree, &release, ms(t0, 120)),
        vec![UiEvent::Click(Key::indexed("row", 2))]
    );
}

#[test]
fn apply_animates_a_moved_node_on_the_default_curve() {
    let t0 = Instant::now();
    let mut tree = applied(&[0, 1, 2], t0);
    let row1 = id(&tree, Key::indexed("row", 1));
    assert_eq!(tree.rect(row1).y, 60.0);
    tree.apply(rows(&[0, 2, 1]), t0);
    tree.layout_measured(AVAILABLE, measure);
    assert_eq!(tree.rect(row1).y, 100.0);
    assert_eq!(tree.offset(row1, t0), (0.0, -40.0));
    assert!(tree.is_animating(ms(t0, 100)));
    // One UI eases out: well past halfway at half time.
    assert!(tree.offset(row1, ms(t0, 100)).1 > -8.0);
    assert_eq!(tree.offset(row1, ms(t0, 200)), (0.0, 0.0));
    assert!(!tree.is_animating(ms(t0, 200)));
    let row2 = id(&tree, Key::indexed("row", 2));
    assert_eq!(tree.offset(row2, t0), (0.0, 40.0));
    // A node that did not move does not slide.
    assert_eq!(
        tree.offset(id(&tree, Key::indexed("row", 0)), t0),
        (0.0, 0.0)
    );
}

#[test]
fn apply_fades_removed_and_inserted_nodes() {
    let t0 = Instant::now();
    let mut tree = applied(&[0, 1, 2], t0);
    let row1 = id(&tree, Key::indexed("row", 1));

    // Removed: still there, fading to 0 over Exit (150 ms).
    tree.apply(rows(&[0, 2]), t0);
    tree.layout_measured(AVAILABLE, measure);
    assert_eq!(tree.node_by_key(Key::indexed("row", 1)), Some(row1));
    assert!(tree.paint_order().contains(&row1));
    assert_eq!(tree.effective_opacity(row1, t0), 1.0);
    assert!((tree.effective_opacity(row1, ms(t0, 75)) - 0.5).abs() < 1e-3);
    assert!(tree.is_animating(ms(t0, 100)));
    assert_eq!(tree.effective_opacity(row1, ms(t0, 150)), 0.0);
    // Dropped once the fade has settled.
    tree.apply(rows(&[0, 2]), ms(t0, 150));
    assert_eq!(tree.node_by_key(Key::indexed("row", 1)), None);
    assert!(!tree.paint_order().contains(&row1));

    // Inserted: from 0 to 1 over Normal (200 ms).
    let t1 = ms(t0, 300);
    tree.apply(rows(&[0, 2, 5]), t1);
    tree.layout_measured(AVAILABLE, measure);
    let row5 = id(&tree, Key::indexed("row", 5));
    assert_eq!(tree.effective_opacity(row5, t1), 0.0);
    assert!((tree.effective_opacity(row5, ms(t1, 100)) - 0.5).abs() < 1e-3);
    assert_eq!(tree.effective_opacity(row5, ms(t1, 200)), 1.0);
    assert!(!tree.is_animating(ms(t1, 200)));
}

#[test]
fn a_key_that_returns_while_leaving_is_revived_in_place() {
    let t0 = Instant::now();
    let mut tree = applied(&[0, 1, 2], t0);
    let row1 = id(&tree, Key::indexed("row", 1));
    tree.apply(rows(&[0, 2]), t0);
    tree.apply(rows(&[0, 1, 2]), ms(t0, 75));
    tree.layout_measured(AVAILABLE, measure);
    assert_eq!(tree.node_by_key(Key::indexed("row", 1)), Some(row1));
    // It fades back from where it was, not from 0 and not from 1.
    let now = tree.effective_opacity(row1, ms(t0, 75));
    assert!((now - 0.5).abs() < 1e-3, "{now}");
    assert_eq!(tree.effective_opacity(row1, ms(t0, 400)), 1.0);
    tree.apply(rows(&[0, 1, 2]), ms(t0, 400));
    assert_eq!(tree.node_by_key(Key::indexed("row", 1)), Some(row1));
}

#[test]
fn apply_updates_text_in_place_without_relayout_when_unchanged() {
    use std::cell::Cell;
    let t0 = Instant::now();
    let label = |s: &str| card([row(0).child(view::text(Key::named("label"), s, body()))]);
    let calls = Cell::new(0);
    let counting = |s: &str, st: &TextStyle, m: Option<f32>| {
        calls.set(calls.get() + 1);
        measure(s, st, m)
    };
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(label("Isla"), t0);
    tree.layout_measured(AVAILABLE, counting);
    let text = id(&tree, Key::named("label"));
    let first = calls.get();
    assert!(first > 0);

    tree.apply(label("Isla"), t0);
    tree.layout_measured(AVAILABLE, counting);
    assert_eq!(
        calls.get(),
        first,
        "the same text must not be measured again"
    );

    tree.apply(label("Espacios"), t0);
    tree.layout_measured(AVAILABLE, counting);
    assert!(calls.get() > first);
    assert_eq!(tree.node_by_key(Key::named("label")), Some(text));
    assert_eq!(tree.rect(text).w, 56.0);
}

#[test]
fn reduced_motion_steps_spatial_and_keeps_opacity_fades() {
    let t0 = Instant::now();
    let mut tree = Tree::new(true, &Theme::dark());
    tree.apply(rows(&[0, 1, 2]), t0);
    tree.layout_measured(AVAILABLE, measure);
    let row1 = id(&tree, Key::indexed("row", 1));
    tree.apply(rows(&[0, 2, 1, 3]), t0);
    tree.layout_measured(AVAILABLE, measure);
    // The move is a single step…
    assert_eq!(tree.offset(row1, t0), (0.0, 0.0));
    // …but the inserted row still fades in.
    let row3 = id(&tree, Key::indexed("row", 3));
    assert_eq!(tree.effective_opacity(row3, t0), 0.0);
    assert!(tree.is_animating(ms(t0, 100)));

    // Switching it off brings the slide back.
    tree.set_reduced_motion(false);
    tree.apply(rows(&[0, 1, 2, 3]), t0);
    tree.layout_measured(AVAILABLE, measure);
    assert_eq!(tree.offset(row1, t0), (0.0, 40.0));
}

#[test]
fn hit_test_respects_clip_and_opacity_zero() {
    let t0 = Instant::now();
    // A 100-high clipping box with a row hanging out of it (y 80..120) and
    // a fully transparent row over its top.
    let clipped = view::col(Key::named("clip"))
        .size(300.0, 100.0)
        .radius(12.0)
        .clip()
        .child(
            view::row(Key::named("hang"))
                .absolute(0.0, 80.0)
                .size(300.0, 40.0)
                .interactive(),
        )
        .child(
            view::row(Key::named("ghost"))
                .absolute(0.0, 0.0)
                .size(300.0, 40.0)
                .interactive()
                .opacity(0.0),
        );
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(clipped, t0);
    tree.layout_measured(AVAILABLE, measure);
    let mut ia = Interaction::new();
    let at = |y: f32| InputEvent::PointerMotion { x: 50.0, y };
    assert_eq!(
        ia.pointer(&mut tree, &at(90.0), t0),
        vec![UiEvent::HoverChanged(Key::named("hang"), true)]
    );
    // Inside the row's rect but outside the clip: nothing.
    assert_eq!(
        ia.pointer(&mut tree, &at(110.0), t0),
        vec![UiEvent::HoverChanged(Key::named("hang"), false)]
    );
    // The transparent row is never hit.
    assert!(ia.pointer(&mut tree, &at(20.0), t0).is_empty());
}

#[test]
fn tab_order_skips_hidden_and_unfocusable() {
    let t0 = Instant::now();
    let view = card([
        row(0),
        row(1).hidden(true),
        view::row(Key::indexed("row", 2)).height(40.0).interactive(),
        row(3),
        row(4),
    ]);
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(view, t0);
    tree.layout_measured(AVAILABLE, measure);
    // Row 4 leaves: it is no longer in the Tab order either.
    tree.apply(
        card([
            row(0),
            row(1).hidden(true),
            view::row(Key::indexed("row", 2)).height(40.0).interactive(),
            row(3),
        ]),
        t0,
    );
    tree.layout_measured(AVAILABLE, measure);
    let mut ia = Interaction::new();
    let mut seen = Vec::new();
    for _ in 0..3 {
        ia.key(&mut tree, &key(XKB_KEY_TAB));
        seen.push(tree.highlight().focused.map(|id| tree.key_of(id)));
    }
    assert_eq!(
        seen,
        vec![
            Some(Key::indexed("row", 0)),
            Some(Key::indexed("row", 3)),
            Some(Key::indexed("row", 0))
        ]
    );
}

#[test]
fn escape_bubbles_as_an_event() {
    let t0 = Instant::now();
    let mut tree = applied(&[0, 1], t0);
    let mut ia = Interaction::new();
    assert_eq!(
        ia.key(&mut tree, &key(XKB_KEY_ESCAPE)),
        vec![UiEvent::Escape]
    );
    // Focus is not consumed or moved by Escape: the surface decides.
    ia.key(&mut tree, &key(XKB_KEY_TAB));
    let focused = tree.highlight().focused;
    assert_eq!(
        ia.key(&mut tree, &key(XKB_KEY_ESCAPE)),
        vec![UiEvent::Escape]
    );
    assert_eq!(tree.highlight().focused, focused);
}
