//! The Island's capsule is lent to the notices surface while it shows
//! (CEL-D-011): the panel paints no capsule glass or tree, leaves the rect
//! out of its regions and takes no pointer input over it, while the bar's
//! neighbours keep the width.

use super::*;

fn rest_shape(island: &Island) -> Shape {
    (island.rest_rect(), island.theme.radius_lg)
}

#[test]
fn the_rest_rect_is_the_capsule_centred_on_the_surface_at_y_8() {
    let (island, _) = bar_island(1920.0);
    let rest = island.rest_rect();
    assert_eq!((rest.y, rest.h), (8.0, 40.0));
    assert!(rest.w > 0.0);
    assert_eq!(rest.x, (1920.0 - rest.w) / 2.0);
}

#[test]
fn the_clock_face_is_the_panels_capsule() {
    let (island, t) = bar_island(1920.0);
    let mut tree = Tree::new(false, &island.theme);
    tree.apply(island.capsule_view(), t);
    tree.layout_measured(island.size, fake_measure);
    let body = tree
        .node_by_key(view::capsule_key())
        .map(|id| tree.rect(id));
    assert_eq!(
        body.map(|r| (r.x, r.w)),
        Some((island.rest_rect().x, island.rest_rect().w)),
        "the surface paints the width the panel's capsule has"
    );
}

#[test]
fn a_lent_capsule_leaves_paint_shapes_and_input() {
    let (mut island, t) = bar_island(1920.0);
    let shape = rest_shape(&island);
    assert!(island.shapes(t).contains(&shape), "premise: shown at rest");
    assert!(island.paints_capsule(t));

    island.lend_capsule(true);
    assert!(island.capsule_lent());
    assert!(!island.paints_capsule(t));
    assert!(!island.shapes(t).contains(&shape), "no blur or input shape");

    // A click on it reaches nothing: the page stays closed.
    let (x, y) = centre(island.rest_rect());
    let mut actions = feed(&mut island, press(x, y, BTN_LEFT), t);
    actions.extend(feed(&mut island, release(x, y, BTN_LEFT), t));
    assert!(actions.is_empty());
    assert!(!island.state.is_open());

    // Taken back, it paints and clicks again.
    island.lend_capsule(false);
    assert!(island.paints_capsule(t));
    assert!(island.shapes(t).contains(&shape));
    feed(&mut island, press(x, y, BTN_LEFT), t);
    feed(&mut island, release(x, y, BTN_LEFT), t);
    assert!(island.state.is_open(), "the capsule opens the page again");
}

#[test]
fn the_bars_neighbours_keep_the_width_while_the_capsule_is_lent() {
    let (mut island, t) = bar_island(1920.0);
    let before = island.state.rect(t, 1920.0).w;
    island.lend_capsule(true);
    lay_fake(&mut island, t);
    assert_eq!(island.state.rect(t, 1920.0).w, before);
    assert_eq!(island.rest_rect().w, before);
}
