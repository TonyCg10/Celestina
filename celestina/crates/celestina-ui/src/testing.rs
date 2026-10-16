//! Fixtures the unit tests share: a fixed-advance text measure and a padded
//! card of three interactive rows.
#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use celestina_paint::{Role, TextStyle, Theme, Weight};

use crate::{
    curve::Motion,
    key::Key,
    tree::{NodeId, Tree},
    view::{self, View},
};

pub(crate) fn body() -> TextStyle {
    TextStyle {
        size: 13.0,
        weight: Weight::Regular,
        tabular: false,
        role: Role::TextHi,
    }
}

/// 7 px per char, 16 px tall.
pub(crate) fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
    (7.0 * s.chars().count() as f32, 16.0)
}

pub(crate) const AVAILABLE: (f32, f32) = (760.0, 560.0);

pub(crate) fn row_key(i: u64) -> Key {
    Key::indexed("row", i)
}

/// A 40-high interactive, focusable row.
pub(crate) fn row_view(i: u64) -> View {
    view::row(row_key(i)).height(40.0).interactive().focusable()
}

/// A 300-wide column with 20 px padding around `rows`.
pub(crate) fn card(rows: impl IntoIterator<Item = View>) -> View {
    view::col(Key::named("card"))
        .width(300.0)
        .padding(20.0)
        .children(rows)
}

/// The card with rows 0, 1 and 2, applied and laid out: rows at y 20..60,
/// 60..100 and 100..140, x 20..280.
pub(crate) fn card_with_rows() -> (Tree, NodeId, [NodeId; 3]) {
    let mut tree = Tree::new(false, &Theme::dark());
    tree.apply(card((0..3).map(row_view)), Instant::now());
    tree.layout_measured(AVAILABLE, fake_measure);
    let card = id(&tree, Key::named("card"));
    let rows = [0, 1, 2].map(|i| id(&tree, row_key(i)));
    (tree, card, rows)
}

pub(crate) fn id(tree: &Tree, key: Key) -> NodeId {
    match tree.node_by_key(key) {
        Some(id) => id,
        None => panic!("no node for {key:?}"),
    }
}

/// `m` on the dark theme's ladder.
pub(crate) fn dur(m: Motion) -> Duration {
    m.duration(&Theme::dark().motion)
}
