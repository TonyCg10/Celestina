//! The tray overflow window's list (CEL-I-012): one row per app the bar
//! hides, drawn as the Island's Tray page draws its rows (a 24 px icon, the
//! title and, when the app wants a look, `copy::TRAY_NEEDS_ATTENTION` in the
//! accent), 240 px wide with 8 px of padding. The Island builds it from the
//! tray's rows. There is no subtitle: the window grows out of the chevron.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{fit_middle, view, Key, View};

use super::Measure;
use crate::island::{
    pages::tray::{display_title, item_row, status_word, title_weight, TrayRow, ICON_SIZE},
    view::text_style,
};

/// The list's width, logical px.
pub const LIST_W: f32 = 240.0;
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

/// The list: one row for each of `rows` (an app's place in the tray's list,
/// which is what its row's key carries, and the app) showing the matching
/// entry of `titles`, in the order given.
#[must_use]
pub fn overflow_view(theme: &Theme, rows: &[(usize, TrayRow)], titles: &[String]) -> View {
    view::col(list_key())
        .width(LIST_W)
        .padding(PAD)
        .gap(ROW_GAP)
        .children(
            rows.iter()
                .zip(titles)
                .map(|((index, row), title)| item_row(theme, *index, row, title)),
        )
}

/// The titles of `rows` cut to the room each row leaves them: the row's
/// content width less the icon, the gaps and the measured status word. A
/// title that fits stays whole; a longer one keeps its start and end around
/// an ellipsis.
#[must_use]
pub fn fit_titles(
    theme: &Theme,
    rows: &[(usize, TrayRow)],
    measure: &mut Measure<'_>,
) -> Vec<String> {
    let gap = theme.spacing.md;
    let inner = LIST_W - 2.0 * PAD - 2.0 * gap;
    rows.iter()
        .map(|(_, row)| {
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
            let style = text_style(
                theme.type_scale.body,
                title_weight(row),
                false,
                Role::TextHi,
            );
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
        let titles = fit_titles(theme, rows, &mut measure);
        let mut tree = Tree::new(false, theme);
        tree.apply(overflow_view(theme, rows, &titles), Instant::now());
        tree.layout_measured((LIST_W, 560.0), measure);
        (tree, titles)
    }

    #[test]
    fn rows_are_40_tall_in_a_240_wide_list_with_8_of_padding() {
        let theme = Theme::dark();
        let rows = [(3, TrayRow::default()), (7, TrayRow::default())];
        let (tree, _) = laid_out(&theme, &rows);
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let root = rect(list_key());
        assert_eq!((root.w, root.h), list_size(&theme, 2));
        assert_eq!(list_size(&theme, 2), (240.0, 98.0));
        let first = rect(row_key(3));
        assert_eq!(
            (first.x, first.y, first.w, first.h),
            (8.0, 8.0, 224.0, 40.0)
        );
        assert_eq!(rect(row_key(7)).y, 50.0);
        assert_eq!(list_size(&theme, 0), (240.0, 16.0));
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
        // 7 px a char: the room is 164 px without a status word.
        let whole = [
            row("Missing icon", Status::Active),
            row("Steam", Status::Active),
        ];
        assert_eq!(
            fit_titles(&theme, &whole, &mut measure),
            ["Missing icon", "Steam"]
        );
        // A title that fits beside the word stays whole too (33 px: 4 chars).
        let beside = [row("Bt", Status::NeedsAttention)];
        assert_eq!(fit_titles(&theme, &beside, &mut measure), ["Bt"]);
        // An untitled item shows the copy's word.
        let untitled = [row("", Status::Active)];
        assert_eq!(
            fit_titles(&theme, &untitled, &mut measure),
            [crate::copy::TRAY_ITEM_UNTITLED]
        );
    }
}
