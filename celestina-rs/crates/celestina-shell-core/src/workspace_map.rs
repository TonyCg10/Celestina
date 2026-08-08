//! What a workspace holds, arranged the way the person arranged it.
//!
//! A capsule that says *five workspaces, one urgent* is honest and useless: it
//! cannot say whether the thing you are looking for is behind it. This module
//! turns the window list the compositor publishes into the shape it really has —
//! columns left to right, rows down inside a column, each at its true relative
//! size — so a surface can draw the arrangement instead of listing names.
//!
//! There are no pixels anywhere near this. Wayland gives a client no access to
//! another client's buffers, so a preview is not merely unimplemented, it is
//! unavailable. What is available is geometry, and geometry has one advantage a
//! thumbnail does not: it cannot go stale into something believable. A map drawn
//! from the last frame is either right or visibly empty.
//!
//! Sizes arrive as the compositor's own measures and leave as **shares**: each
//! column's fraction of the map's width, each tile's fraction of its column's
//! height. A surface multiplies a share by whatever room it has and is done. It
//! never sees a pixel count it might be tempted to use as one, and it never has
//! to decide what an impossible size means — that is answered here, once.

use crate::bounded;

/// How many columns a map draws. A workspace scrolled wider than this is past
/// the point where another sliver of a column tells anybody anything, and the
/// bound keeps a hostile or broken frame finite.
pub const MAX_COLUMNS: usize = 12;
/// How many windows are folded at all, whatever the caller sends. The adapter
/// bounds its own frame; this module does not take that on trust.
pub const MAX_WINDOWS: usize = 64;
/// A window title, in characters.
pub const MAX_TITLE_CHARS: usize = 512;
/// An application id, in characters.
pub const MAX_APP_ID_CHARS: usize = 128;
/// A window id, in characters. Twenty digits is the widest `u64` there is.
pub const MAX_ID_CHARS: usize = 20;

/// One window, as the compositor described it.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// The compositor's own id for this window, as a decimal string. Carried so
    /// a surface can ask to focus *this* window rather than the workspace it is
    /// on. A `u64` whose range the compositor refuses to constrain, so it
    /// travels as text for the same reason a workspace id does.
    pub id: String,
    /// Whatever the client set, which may be anything at all.
    pub title: String,
    /// The application's own id: the nearest thing to a description, and the
    /// key an icon is looked up by.
    pub app_id: String,
    /// Its place in the scrolling layout, as the compositor counts it. Zero
    /// means it has none — a floating window is not in that layout — and such a
    /// window is kept apart rather than given a position it does not have.
    pub column: u16,
    pub row: u16,
    /// The tile's measures. Used only as a ratio against its siblings; a value
    /// that is not a positive, finite size is treated as unknown.
    pub width: f64,
    pub height: f64,
    pub focused: bool,
    pub floating: bool,
    pub urgent: bool,
}

impl Window {
    /// A window with both strings bounded, as everything crossing this boundary
    /// must be.
    #[must_use]
    pub fn new(id: &str, title: &str, app_id: &str, column: u16, row: u16) -> Self {
        Self {
            id: bounded(id, MAX_ID_CHARS),
            title: bounded(title, MAX_TITLE_CHARS),
            app_id: bounded(app_id, MAX_APP_ID_CHARS),
            column,
            row,
            width: 0.0,
            height: 0.0,
            focused: false,
            floating: false,
            urgent: false,
        }
    }

    /// The same window with its measures attached.
    #[must_use]
    pub fn sized(mut self, width: f64, height: f64) -> Self {
        self.width = usable(width);
        self.height = usable(height);
        self
    }

    /// The same window with its three states attached.
    #[must_use]
    pub fn with_states(mut self, focused: bool, floating: bool, urgent: bool) -> Self {
        self.focused = focused;
        self.floating = floating;
        self.urgent = urgent;
        self
    }

    /// Whether this window has a place in the scrolling layout at all.
    #[must_use]
    pub fn is_placed(&self) -> bool {
        self.column > 0 && !self.floating
    }
}

/// One window inside a column, with the fraction of that column it occupies.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub window: Window,
    /// This tile's share of its column's height, between 0 and 1. The shares of
    /// a column always sum to 1.
    pub height_share: f64,
}

/// One column of the layout, with the fraction of the map it occupies.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// This column's share of the map's width, between 0 and 1. The shares of a
    /// map always sum to 1.
    pub width_share: f64,
    /// Its tiles, top to bottom.
    pub tiles: Vec<Tile>,
}

/// A workspace's arrangement, ready to draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    /// The scrolling layout, left to right.
    pub columns: Vec<Column>,
    /// Windows with no place in that layout, in the order they arrived. They
    /// are kept apart because a floating window sits *over* the arrangement
    /// rather than in it, and folding it into a column would be the map
    /// claiming a structure the session does not have.
    pub floating: Vec<Window>,
    /// How many windows were dropped to stay within [`MAX_WINDOWS`] and
    /// [`MAX_COLUMNS`]. A surface that hides some of a workspace must be able to
    /// say so; silently showing four of nine is the map lying about the thing it
    /// exists to answer.
    pub hidden: usize,
}

impl Map {
    /// Whether the workspace holds nothing at all.
    ///
    /// Distinct from a map that was truncated to nothing: `hidden` says whether
    /// emptiness is the truth or the bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty() && self.floating.is_empty()
    }

    /// How many windows the map actually carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns
            .iter()
            .map(|column| column.tiles.len())
            .sum::<usize>()
            + self.floating.len()
    }
}

/// Folds a workspace's windows into the arrangement they are in.
///
/// The order of the input is not trusted. The adapter already publishes these
/// sorted, and doing it again here costs nothing and means a second caller — a
/// test, a later consumer — cannot get a different map from the same windows.
///
/// Shares are computed rather than passed through, and they are always usable:
/// a column whose windows all report an unknown width divides the map evenly,
/// and so does a column whose heights are all unknown. There is no input that
/// produces a `NaN`, an infinity or a negative share, which matters because
/// those reach a layout as a surface that silently fails to draw.
#[must_use]
pub fn map(windows: &[Window]) -> Map {
    let mut hidden = windows.len().saturating_sub(MAX_WINDOWS);
    let considered = &windows[..windows.len().min(MAX_WINDOWS)];

    let mut floating: Vec<Window> = considered
        .iter()
        .filter(|window| !window.is_placed())
        .cloned()
        .collect();

    // Group by column, keeping the columns in the order the compositor numbers
    // them rather than the order they happened to arrive in.
    let mut keys: Vec<u16> = considered
        .iter()
        .filter(|window| window.is_placed())
        .map(|window| window.column)
        .collect();
    keys.sort_unstable();
    keys.dedup();

    if keys.len() > MAX_COLUMNS {
        // The dropped columns are the rightmost: a scrolling layout is read from
        // the left, and the leftmost columns are the ones a person is oriented
        // by.
        for key in &keys[MAX_COLUMNS..] {
            hidden += considered
                .iter()
                .filter(|window| window.is_placed() && window.column == *key)
                .count();
        }
        keys.truncate(MAX_COLUMNS);
    }

    let mut columns: Vec<Column> = keys
        .iter()
        .map(|key| {
            let mut tiles: Vec<Window> = considered
                .iter()
                .filter(|window| window.is_placed() && window.column == *key)
                .cloned()
                .collect();
            tiles.sort_by_key(|window| window.row);

            let total: f64 = tiles.iter().map(|window| window.height).sum();
            let count = tiles.len();
            let tiles = tiles
                .into_iter()
                .map(|window| {
                    let height_share = share(window.height, total, count);
                    Tile {
                        window,
                        height_share,
                    }
                })
                .collect();

            Column {
                // Filled in below: a column's share depends on its siblings.
                width_share: 0.0,
                tiles,
            }
        })
        .collect();

    // A column has one width in this layout, so its measure is its widest tile
    // rather than a sum. A column of unknown widths contributes nothing to the
    // total and falls back to an even share along with everything else.
    let widths: Vec<f64> = columns
        .iter()
        .map(|column| {
            column
                .tiles
                .iter()
                .map(|tile| tile.window.width)
                .fold(0.0_f64, f64::max)
        })
        .collect();
    let total: f64 = widths.iter().sum();
    let count = columns.len();
    for (column, width) in columns.iter_mut().zip(widths) {
        column.width_share = share(width, total, count);
    }

    floating.truncate(MAX_WINDOWS);

    Map {
        columns,
        floating,
        hidden,
    }
}

/// One part's share of a whole, or an even split when the whole is unknowable.
///
/// The even split is the honest answer rather than a fallback: if no window in a
/// column reported a usable height, the map does not know their proportions, and
/// drawing them equal says exactly that. Inventing a difference would be worse
/// than admitting there is none.
fn share(part: f64, total: f64, count: usize) -> f64 {
    if count == 0 {
        return 0.0;
    }
    // `count` is a small collection length; the conversion is exact for every
    // value this can hold.
    let even = 1.0 / f64::from(u32::try_from(count).unwrap_or(u32::MAX));
    if total > 0.0 && part > 0.0 {
        part / total
    } else {
        even
    }
}

/// A measure the map can divide by, or zero for one it cannot.
fn usable(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shares are computed, so they are compared with a tolerance rather than
    /// for equality.
    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    fn placed(title: &str, column: u16, row: u16, width: f64, height: f64) -> Window {
        Window::new("1", title, "app", column, row).sized(width, height)
    }

    #[test]
    fn an_empty_workspace_maps_to_an_empty_map() {
        let result = map(&[]);

        assert!(result.is_empty());
        assert_eq!(result.hidden, 0);
    }

    #[test]
    fn windows_are_grouped_into_the_columns_they_are_in() {
        let result = map(&[
            placed("right", 2, 1, 800.0, 600.0),
            placed("left lower", 1, 2, 800.0, 300.0),
            placed("left upper", 1, 1, 800.0, 300.0),
        ]);

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].tiles.len(), 2);
        // Rows are ordered inside a column whatever order they arrived in.
        assert_eq!(result.columns[0].tiles[0].window.title, "left upper");
        assert_eq!(result.columns[0].tiles[1].window.title, "left lower");
        assert_eq!(result.columns[1].tiles[0].window.title, "right");
    }

    #[test]
    fn column_widths_are_shares_of_the_whole() {
        // The author's real layout when this was written: two half columns and
        // one full one.
        let result = map(&[
            placed("a", 1, 1, 942.0, 998.0),
            placed("b", 2, 1, 942.0, 998.0),
            placed("c", 3, 1, 1896.0, 998.0),
        ]);

        let shares: Vec<f64> = result
            .columns
            .iter()
            .map(|column| column.width_share)
            .collect();
        assert!(close(shares[0], 942.0 / 3780.0));
        assert!(close(shares[2], 1896.0 / 3780.0));
        assert!(close(shares.iter().sum::<f64>(), 1.0));
    }

    #[test]
    fn tile_heights_are_shares_of_their_column() {
        let result = map(&[
            placed("tall", 1, 1, 800.0, 700.0),
            placed("short", 1, 2, 800.0, 300.0),
        ]);

        let tiles = &result.columns[0].tiles;
        assert!(close(tiles[0].height_share, 0.7));
        assert!(close(tiles[1].height_share, 0.3));
    }

    #[test]
    fn a_column_of_unknown_heights_divides_itself_evenly() {
        let result = map(&[
            placed("one", 1, 1, 800.0, 0.0),
            placed("two", 1, 2, 800.0, 0.0),
            placed("three", 1, 3, 800.0, 0.0),
        ]);

        // The map does not know their proportions, and equal tiles say so.
        for tile in &result.columns[0].tiles {
            assert!(close(tile.height_share, 1.0 / 3.0));
        }
    }

    #[test]
    fn unknown_widths_divide_the_map_evenly() {
        let result = map(&[
            placed("one", 1, 1, 0.0, 600.0),
            placed("two", 2, 1, 0.0, 600.0),
        ]);

        assert!(close(result.columns[0].width_share, 0.5));
        assert!(close(result.columns[1].width_share, 0.5));
    }

    #[test]
    fn an_impossible_measure_never_reaches_a_share() {
        let result = map(&[
            placed("nan", 1, 1, f64::NAN, f64::NAN),
            placed("infinite", 2, 1, f64::INFINITY, 600.0),
            placed("negative", 3, 1, -5.0, 600.0),
        ]);

        // None of these can be divided by, so every column falls back to even
        // and nothing downstream receives a value a layout cannot use.
        for column in &result.columns {
            assert!(column.width_share.is_finite());
            assert!(column.width_share > 0.0);
            for tile in &column.tiles {
                assert!(tile.height_share.is_finite());
                assert!(tile.height_share > 0.0);
            }
        }
    }

    #[test]
    fn a_floating_window_is_kept_out_of_the_layout() {
        let result = map(&[
            placed("tiled", 1, 1, 800.0, 600.0),
            Window::new("2", "floating", "app", 0, 0)
                .sized(400.0, 300.0)
                .with_states(false, true, false),
        ]);

        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].tiles.len(), 1);
        assert_eq!(result.floating.len(), 1);
        assert_eq!(result.floating[0].title, "floating");
    }

    #[test]
    fn a_window_that_claims_a_column_while_floating_is_still_floating() {
        let result = map(&[Window::new("3", "liar", "app", 3, 1)
            .sized(400.0, 300.0)
            .with_states(false, true, false)]);

        assert!(result.columns.is_empty());
        assert_eq!(result.floating.len(), 1);
    }

    #[test]
    fn states_survive_the_fold() {
        let result = map(&[Window::new("4", "urgent", "app", 1, 1)
            .sized(800.0, 600.0)
            .with_states(true, false, true)]);

        let window = &result.columns[0].tiles[0].window;
        assert!(window.focused);
        assert!(window.urgent);
    }

    #[test]
    fn a_map_past_the_column_bound_says_how_much_it_is_hiding() {
        let windows: Vec<Window> = (1..=MAX_COLUMNS + 3)
            .map(|column| {
                placed(
                    "window",
                    u16::try_from(column).expect("small test index"),
                    1,
                    800.0,
                    600.0,
                )
            })
            .collect();

        let result = map(&windows);

        assert_eq!(result.columns.len(), MAX_COLUMNS);
        // Not silently four of nine: the surface can say what it is not showing.
        assert_eq!(result.hidden, 3);
    }

    #[test]
    fn a_map_past_the_window_bound_says_how_much_it_is_hiding() {
        let windows: Vec<Window> = (0..MAX_WINDOWS + 5)
            .map(|index| {
                placed(
                    "window",
                    1,
                    u16::try_from(index).expect("small test index"),
                    800.0,
                    600.0,
                )
            })
            .collect();

        let result = map(&windows);

        assert_eq!(result.len(), MAX_WINDOWS);
        assert_eq!(result.hidden, 5);
    }

    #[test]
    fn a_hostile_title_is_bounded_before_it_is_folded() {
        let title = "T".repeat(MAX_TITLE_CHARS + 400);
        let result = map(&[placed(&title, 1, 1, 800.0, 600.0)]);

        assert_eq!(
            result.columns[0].tiles[0].window.title.chars().count(),
            MAX_TITLE_CHARS
        );
    }

    #[test]
    fn the_same_windows_in_any_order_produce_the_same_map() {
        let forward = [
            placed("a", 1, 1, 900.0, 500.0),
            placed("b", 1, 2, 900.0, 500.0),
            placed("c", 2, 1, 700.0, 1000.0),
        ];
        let mut reversed = forward.clone();
        reversed.reverse();

        // The adapter already sorts, so this is about a second caller not being
        // able to get a different arrangement from the same session.
        assert_eq!(map(&forward), map(&reversed));
    }
}
