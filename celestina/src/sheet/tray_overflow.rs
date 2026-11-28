//! The tray overflow window's list (CEL-I-012): one row per app the bar
//! hides, drawn as the Island's Tray page draws its rows (a 24 px icon, the
//! title and, when the app wants a look, `copy::TRAY_NEEDS_ATTENTION` in the
//! accent), 280 px wide with 8 px of padding. The Island builds it from the
//! tray's rows. There is no subtitle: the window grows out of the chevron.
//! Under an app's row the list can hold that app's menu as a card
//! ([`CardView`], CEL-D-016): a 264 px box the card's pane is painted in.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{fit_middle, view, Key, View};

use super::Measure;
use crate::island::{
    pages::tray::{
        display_title, item_row_looking, status_word, title_weight, RowLook, TrayRow, ICON_SIZE,
    },
    view::text_style,
};
use crate::tray::card::CardView;

/// The list's width, logical px.
pub const LIST_W: f32 = 280.0;
/// A card's width: the list's less its padding.
pub const CARD_W: f32 = LIST_W - 2.0 * PAD;
/// The padding around the rows.
pub const PAD: f32 = 8.0;
/// The gap between rows.
const ROW_GAP: f32 = 2.0;

/// The list's root.
#[must_use]
pub fn list_key() -> Key {
    Key::named("tray-overflow")
}

/// The list's size for `rows` apps, logical px: the rows, the gap between
/// them and the padding.
#[must_use]
pub fn list_size(theme: &Theme, rows: usize) -> (f32, f32) {
    let n = rows as f32;
    let gaps = rows.saturating_sub(1) as f32;
    (LIST_W, 2.0 * PAD + n * theme.row_height + gaps * ROW_GAP)
}

/// The card's box.
#[must_use]
pub fn card_key() -> Key {
    Key::named("tray-card")
}

/// The card's place in the list: the box and the space under it.
fn card_slot_key() -> Key {
    Key::named("tray-card-slot")
}

/// The space under the card's box: with the list's gap it is 4 px between
/// the box and the next row, and 4 px (the gap and this) to the padding when
/// the card is the last child.
fn card_pad(last: bool) -> f32 {
    if last {
        4.0
    } else {
        2.0
    }
}

/// [`list_size`] for `rows` apps with `card` (its box height and whether
/// its row is the last) under one of them.
#[must_use]
pub fn list_size_with(theme: &Theme, rows: usize, card: Option<(f32, bool)>) -> (f32, f32) {
    let (w, h) = list_size(theme, rows);
    match card {
        Some((box_h, last)) if box_h > 1.0 => (w, h + ROW_GAP + box_h + card_pad(last)),
        _ => (w, h),
    }
}

/// The list: one row for each of `rows` (an app's place in the tray's list,
/// which is what its row's key carries, and the app) showing the matching
/// entry of `titles`, in the order given.
#[must_use]
pub fn overflow_view(
    theme: &Theme,
    rows: &[(usize, TrayRow)],
    titles: &[String],
    card: Option<CardView>,
) -> View {
    let last = rows.last().map(|(i, _)| *i);
    let shown = card.filter(CardView::shown);
    let mut children: Vec<View> = Vec::with_capacity(rows.len() + 1);
    for ((index, row), title) in rows.iter().zip(titles) {
        let mine = card.filter(|c| c.index == *index);
        let look = RowLook {
            held: mine.is_some_and(|c| c.held),
            open: mine.is_some_and(|c| c.answered),
        };
        children.push(item_row_looking(theme, *index, row, title, look));
        if shown.is_some_and(|c| c.index == *index) {
            children.push(card_slot(theme, shown, last == Some(*index)));
        }
    }
    if shown.is_none_or(|c| !rows.iter().any(|(i, _)| *i == c.index)) {
        // Always in the tree, out of the layout: a card that goes leaves at
        // once, with no fade of its box.
        children.push(card_slot(theme, None, false));
    }
    view::col(list_key())
        .width(LIST_W)
        .padding(PAD)
        .gap(ROW_GAP)
        .children(children)
}

/// The card's place under its row: the box and the space under it; out of
/// the layout while there is no box.
fn card_slot(theme: &Theme, card: Option<CardView>, last: bool) -> View {
    let (box_h, pad) = card.map_or((0.0, 0.0), |c| (c.box_h, card_pad(last)));
    view::col(card_slot_key())
        .size(CARD_W, box_h + pad)
        .padding_sides(0.0, 0.0, pad)
        .hidden(card.is_none())
        .child(card_box(theme, box_h, Some(CARD_W)))
}

/// The card's box: `box_h` tall and `width` wide (as wide as its place when
/// `None`), in the card's fill.
fn card_box(theme: &Theme, box_h: f32, width: Option<f32>) -> View {
    let sized = view::col(card_key());
    match width {
        Some(w) => sized.size(w, box_h),
        None => sized.height(box_h),
    }
    .fill(Role::ControlFill)
    .radius(theme.radius_md)
}

/// The card's place under a row of the Tray page, whose rows have no gap
/// between them: 2 px over the box and 4 under it, as wide as the rows. With
/// no `box_h` it is out of the layout.
pub(crate) fn page_card_slot(theme: &Theme, box_h: Option<f32>) -> View {
    let (h, over, under) = box_h.map_or((0.0, 0.0, 0.0), |h| (h, 2.0, 4.0));
    view::col(card_slot_key())
        .height(h + over + under)
        .padding_sides(over, 0.0, under)
        .hidden(box_h.is_none())
        .child(card_box(theme, h, None))
}

/// The titles of `rows` cut to the room each row leaves them (the row of the
/// app `open` shows its title in semibold): the row's
/// content width less the icon, the gaps and the measured status word. A
/// title that fits stays whole; a longer one keeps its start and end around
/// an ellipsis.
#[must_use]
pub fn fit_titles(
    theme: &Theme,
    rows: &[(usize, TrayRow)],
    open: Option<usize>,
    measure: &mut Measure<'_>,
) -> Vec<String> {
    let gap = theme.spacing.md;
    let inner = LIST_W - 2.0 * PAD - 2.0 * gap;
    rows.iter()
        .map(|(index, row)| {
            let status = status_word(row).map_or(0.0, |(word, _)| {
                let style = text_style(
                    theme.type_scale.caption,
                    Weight::Regular,
                    false,
                    Role::TextLo,
                );
                gap + measure(word, &style, None).0
            });
            let room = (inner - ICON_SIZE - gap - status).max(0.0);
            let weight = if open == Some(*index) {
                Weight::Semibold
            } else {
                title_weight(row)
            };
            let style = text_style(theme.type_scale.body, weight, false, Role::TextHi);
            fit_middle(display_title(row), room, |t| measure(t, &style, None).0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::TextStyle;
    use celestina_ui::{Tree, ELLIPSIS};

    use super::*;
    use crate::island::pages::tray::row_key;

    /// 7 px per char, 16 px tall.
    fn measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn laid_out(theme: &Theme, rows: &[(usize, TrayRow)]) -> (Tree, Vec<String>) {
        let titles = fit_titles(theme, rows, None, &mut measure);
        let mut tree = Tree::new(false, theme);
        tree.apply(overflow_view(theme, rows, &titles, None), Instant::now());
        tree.layout_measured((LIST_W, 560.0), measure);
        (tree, titles)
    }

    #[test]
    fn rows_are_40_tall_in_a_280_wide_list_with_8_of_padding() {
        let theme = Theme::dark();
        let rows = [(3, TrayRow::default()), (7, TrayRow::default())];
        let (tree, _) = laid_out(&theme, &rows);
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let root = rect(list_key());
        assert_eq!((root.w, root.h), list_size(&theme, 2));
        assert_eq!(list_size(&theme, 2), (280.0, 98.0));
        let first = rect(row_key(3));
        assert_eq!(
            (first.x, first.y, first.w, first.h),
            (8.0, 8.0, 264.0, 40.0)
        );
        assert_eq!(rect(row_key(7)).y, 50.0);
        assert_eq!(list_size(&theme, 0), (280.0, 16.0));
        assert!(tree.node_by_key(row_key(3)).is_some());
    }

    #[test]
    fn a_long_title_ends_in_an_ellipsis_and_the_attention_word_stays_inside_the_row() {
        let theme = Theme::dark();
        let long = "A very long application title that cannot fit";
        let rows = [(
            0,
            TrayRow {
                title: long.to_owned(),
                status: crate::tray::Status::NeedsAttention,
                ..TrayRow::default()
            },
        )];
        let (tree, titles) = laid_out(&theme, &rows);
        assert!(titles[0].contains(ELLIPSIS), "{}", titles[0]);
        assert!(titles[0].chars().count() < long.chars().count());
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let line = rect(row_key(0));
        let title = rect(Key::indexed("tray-title", 0));
        let status = rect(Key::indexed("tray-status", 0));
        let right = line.x + line.w - 12.0;
        assert!(
            status.x + status.w <= right + 0.01,
            "{status:?} in {line:?}"
        );
        assert!(
            title.x + title.w <= status.x + 0.01,
            "{title:?} before {status:?}"
        );
        // The cut title's measured width is what the row leaves.
        let word = copy_status_w(&theme);
        let room = LIST_W - 16.0 - 24.0 - ICON_SIZE - 12.0 - 12.0 - word;
        assert!(7.0 * titles[0].chars().count() as f32 <= room + 0.01);
    }

    fn copy_status_w(_: &Theme) -> f32 {
        7.0 * crate::copy::TRAY_NEEDS_ATTENTION.chars().count() as f32
    }

    #[test]
    fn a_short_title_stays_whole() {
        let theme = Theme::dark();
        let row = |title: &str, status| {
            (
                0,
                TrayRow {
                    title: title.to_owned(),
                    status,
                    ..TrayRow::default()
                },
            )
        };
        use crate::tray::Status;
        // 7 px a char: the room is 204 px without a status word.
        let whole = [
            row("Missing icon", Status::Active),
            row("Steam", Status::Active),
        ];
        assert_eq!(
            fit_titles(&theme, &whole, None, &mut measure),
            ["Missing icon", "Steam"]
        );
        // A title that fits beside the word stays whole too (about 4 chars).
        let beside = [row("Bt", Status::NeedsAttention)];
        assert_eq!(fit_titles(&theme, &beside, None, &mut measure), ["Bt"]);
        // An untitled item shows the copy's word.
        let untitled = [row("", Status::Active)];
        assert_eq!(
            fit_titles(&theme, &untitled, None, &mut measure),
            [crate::copy::TRAY_ITEM_UNTITLED]
        );
    }

    #[test]
    fn a_card_slot_is_in_the_list_only_while_it_has_a_box() {
        let theme = Theme::dark();
        let rows = [(3, TrayRow::default()), (7, TrayRow::default())];
        let titles = fit_titles(&theme, &rows, None, &mut measure);
        let card = |box_h| CardView {
            index: 3,
            held: true,
            answered: true,
            box_h,
            rows: usize::MAX,
        };
        let laid = |card: Option<CardView>| {
            let mut tree = Tree::new(false, &theme);
            tree.apply(overflow_view(&theme, &rows, &titles, card), Instant::now());
            let size = list_size_with(&theme, 2, card.map(|c| (c.box_h, false)));
            tree.layout_measured(size, measure);
            tree
        };
        let rect = |tree: &Tree, k| tree.rect(tree.node_by_key(k).expect("node"));
        // Under row 3: 2 px below it, the box 264 wide, then 2 px and the gap.
        let tree = laid(Some(card(120.0)));
        let (row, boxed, next) = (
            rect(&tree, row_key(3)),
            rect(&tree, card_key()),
            rect(&tree, row_key(7)),
        );
        assert_eq!(
            (boxed.x, boxed.w, boxed.h, boxed.y),
            (8.0, 264.0, 120.0, row.y + 40.0 + 2.0)
        );
        assert_eq!(next.y, boxed.y + boxed.h + 4.0, "4 px under the box");
        assert_eq!(
            rect(&tree, list_key()).h,
            list_size_with(&theme, 2, Some((120.0, false))).1
        );
        // A box of 1 px or less takes no room.
        for tree in [laid(Some(card(1.0))), laid(None)] {
            assert_eq!(rect(&tree, row_key(7)).y, 50.0);
        }
        // The last row's card has 4 px to the padding.
        let mut tree = Tree::new(false, &theme);
        let last = CardView {
            index: 7,
            ..card(100.0)
        };
        tree.apply(
            overflow_view(&theme, &rows, &titles, Some(last)),
            Instant::now(),
        );
        let size = list_size_with(&theme, 2, Some((100.0, true)));
        tree.layout_measured(size, measure);
        let root = rect(&tree, list_key());
        let boxed = rect(&tree, card_key());
        assert_eq!(root.h, boxed.y + boxed.h + 4.0 + PAD);
    }

    #[test]
    fn a_wide_label_in_a_card_ends_inside_its_row() {
        use crate::tray::{menu::entry_key, pane::MenuPane, MenuEntry, MenuTree};
        let theme = Theme::dark();
        let t0 = Instant::now();
        let wide = |c: char| c.to_string().repeat(36);
        let entries = [('W', false), ('\u{4e2d}', true)]
            .iter()
            .enumerate()
            .map(|(i, (c, sub))| MenuEntry {
                id: i as i32 + 1,
                label: wide(*c),
                enabled: true,
                submenu: sub.then(|| MenuTree {
                    id: 9,
                    entries: Vec::new(),
                }),
                ..MenuEntry::default()
            })
            .collect();
        let mut pane = MenuPane::new(
            &theme,
            MenuTree { id: 0, entries },
            None,
            crate::tray::pane::PaneFrame::card(CARD_W),
            400.0,
            false,
            t0,
        );
        assert!(pane.fit_labels(&mut measure, t0), "the labels were cut");
        assert!(!pane.fit_labels(&mut measure, t0), "and only once");
        pane.layout(&mut |tree, size| tree.layout_measured(size, measure));
        let tree = pane.tree();
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let gap = theme.spacing.md;
        for (i, chevron) in [(0usize, false), (1, true)] {
            let row = rect(entry_key(i));
            let label = rect(Key::indexed("tray-menu-label", i as u64));
            let end = if chevron {
                rect(Key::indexed("tray-menu-more", i as u64)).x - gap
            } else {
                row.x + row.w - gap
            };
            // The text, as the pane shows it, fits the room: not only its box.
            let shown = pane.shown_label(i).expect("a label");
            let text_w = 7.0 * shown.chars().count() as f32;
            assert!(text_w <= label.w + 0.01, "{shown}: {text_w} in {label:?}");
            assert!(label.x + text_w <= end + 0.01, "{shown} ends before {end}");
            assert!(shown.contains(ELLIPSIS), "{shown}");
        }
    }
}
