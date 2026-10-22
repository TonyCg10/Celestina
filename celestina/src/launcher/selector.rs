//! The launcher's bubbles source: the `Mod+K` selector of the windows
//! Melibea keeps minimised (CEL-2-J, plan decision D3). The overlay, its
//! field and its rows are the launcher's; only what the rows answer changes.
//!
//! A query filters the windows with the launcher's own matcher
//! (`celestina_shell::launcher::score`): the title is what a person types
//! at, the application id only ever answers loosely, so a title outranks an
//! id. Within one tier the windows keep Melibea's order, the most recently
//! minimised first. At most [`MAX_ROWS`] rows are shown. With no windows, or
//! with Melibea not live, the card shows one note instead of rows.

use celestina_core::desktop_entry::DesktopEntry;
use celestina_shell::launcher::score;

use super::state::{Row, MAX_ROWS};
use crate::{
    bubbles::{Bubbles, Link},
    copy,
};

/// The most characters of a title or an application id a row keeps (and
/// the matcher reads): the protocol already bounds them, and this keeps a
/// hostile one from costing a fold per keystroke.
pub const MAX_ROW_CHARS: usize = 512;

fn bounded(text: Option<&str>) -> String {
    text.unwrap_or("").chars().take(MAX_ROW_CHARS).collect()
}

/// The rows `bubbles` answer `query` with: none unless the link is live.
#[must_use]
pub fn rows(bubbles: &Bubbles, query: &str) -> Vec<Row> {
    if bubbles.link != Link::Live {
        return Vec::new();
    }
    let query = query.trim();
    let mut hits: Vec<_> = bubbles
        .windows
        .iter()
        .filter_map(|window| {
            let title = bounded(window.title.as_deref());
            let app_id = bounded(window.app_id.as_deref());
            // The matcher's shape: the name is the title, the id is one
            // of the texts that answer only loosely.
            let probe = DesktopEntry {
                name: title,
                generic_name: app_id,
                ..DesktopEntry::default()
            };
            let quality = score(&probe, query)?;
            Some((
                quality,
                Row::Bubble {
                    window_id: window.id,
                    title: probe.name,
                    app_id: probe.generic_name,
                    icon: window.icon_name.clone().filter(|name| !name.is_empty()),
                },
            ))
        })
        .collect();
    // Stable: equal tiers keep Melibea's order.
    hits.sort_by_key(|(quality, _)| std::cmp::Reverse(*quality));
    hits.into_iter()
        .take(MAX_ROWS)
        .map(|(_, row)| row)
        .collect()
}

/// The one line the card shows instead of rows when `bubbles` has nothing
/// to offer: no minimised windows, or Melibea not reachable. `None` when
/// there are windows (a query that answers none shows the launcher's own
/// "no results" line).
#[must_use]
pub fn note(bubbles: &Bubbles) -> Option<&'static str> {
    match bubbles.link {
        Link::Live if bubbles.windows.is_empty() => Some(copy::SELECTOR_NO_WINDOWS),
        Link::Live => None,
        Link::Absent | Link::Refused | Link::Unavailable => Some(copy::SELECTOR_UNAVAILABLE),
    }
}

/// Where the selection goes when the rows change from `before` to `after`
/// while `selected` was chosen: the same window when it is still there,
/// else the row that took its place (the last one when it was at the end),
/// so the selection never jumps back to the top on its own.
#[must_use]
pub fn relocate(before: &[Row], selected: usize, after: &[Row]) -> usize {
    if after.is_empty() {
        return 0;
    }
    let kept = match before.get(selected) {
        Some(Row::Bubble { window_id, .. }) => after
            .iter()
            .position(|row| matches!(row, Row::Bubble { window_id: id, .. } if id == window_id)),
        _ => None,
    };
    kept.unwrap_or_else(|| selected.min(after.len() - 1))
}

#[cfg(test)]
mod tests {
    use celestina_shell::melibea::Window;

    use super::*;

    fn window(id: u64, title: &str, app_id: &str) -> Window {
        Window {
            id,
            app_id: Some(app_id.to_owned()),
            title: Some(title.to_owned()),
            icon_name: None,
        }
    }

    fn live(windows: Vec<Window>) -> Bubbles {
        Bubbles {
            windows,
            link: Link::Live,
        }
    }

    fn ids(rows: &[Row]) -> Vec<u64> {
        rows.iter()
            .map(|row| match row {
                Row::Bubble { window_id, .. } => *window_id,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn an_empty_query_lists_the_windows_in_melibeas_order() {
        let b = live(vec![
            window(9, "Zeta", "z"),
            window(3, "Alpha", "a"),
            window(5, "Mid", "m"),
        ]);
        assert_eq!(ids(&rows(&b, "")), [9, 3, 5]);
        assert_eq!(ids(&rows(&b, "   ")), [9, 3, 5]);
    }

    #[test]
    fn the_title_answers_before_the_application_id() {
        let b = live(vec![
            window(1, "Notes", "org.example.Term"),
            window(2, "Terminal", "kitty"),
            window(3, "Readme", "other"),
        ]);
        // "term": the second window by its title's prefix, then the first
        // only by its id (loose); the third not at all.
        assert_eq!(ids(&rows(&b, "term")), [2, 1]);
        // Case and accents fold as in the launcher.
        assert_eq!(ids(&rows(&b, "T\u{c9}RM")), [2, 1]);
        assert!(rows(&b, "zzz").is_empty());
    }

    #[test]
    fn equal_tiers_keep_melibeas_order_and_the_rows_are_capped() {
        let many: Vec<_> = (0..20)
            .map(|i| window(100 - i, &format!("Doc {i}"), "app"))
            .collect();
        let got = ids(&rows(&live(many), "doc"));
        assert_eq!(got.len(), MAX_ROWS);
        assert_eq!(
            got,
            (0..MAX_ROWS as u64).map(|i| 100 - i).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_link_that_is_not_live_has_no_rows_and_a_note() {
        for link in [Link::Absent, Link::Refused, Link::Unavailable] {
            let b = Bubbles {
                windows: vec![window(1, "a", "b")],
                link,
            };
            assert!(rows(&b, "").is_empty(), "{link:?}");
            assert_eq!(note(&b), Some(copy::SELECTOR_UNAVAILABLE), "{link:?}");
        }
        assert_eq!(note(&live(Vec::new())), Some(copy::SELECTOR_NO_WINDOWS));
        assert_eq!(note(&live(vec![window(1, "a", "b")])), None);
    }

    #[test]
    fn a_row_carries_the_window_its_icon_and_bounded_texts() {
        let mut w = window(7, &"\u{1f600}".repeat(10_000), &"x".repeat(10_000));
        w.icon_name = Some("kitty".to_owned());
        let mut untitled = Window {
            id: 8,
            app_id: None,
            title: None,
            icon_name: Some(String::new()),
        };
        let got = rows(&live(vec![w, untitled.clone()]), "");
        let Row::Bubble {
            window_id,
            title,
            app_id,
            icon,
        } = &got[0]
        else {
            panic!("{got:?}");
        };
        assert_eq!(*window_id, 7);
        assert_eq!(title.chars().count(), MAX_ROW_CHARS);
        assert_eq!(app_id.chars().count(), MAX_ROW_CHARS);
        assert_eq!(icon.as_deref(), Some("kitty"));
        assert_eq!(
            got[1],
            Row::Bubble {
                window_id: 8,
                title: String::new(),
                app_id: String::new(),
                icon: None,
            },
            "an empty icon name is no icon"
        );
        // A long query still matches a long title by its prefix, and answers
        // nothing elsewhere; neither fails.
        untitled.title = Some("a".repeat(10_000));
        assert_eq!(ids(&rows(&live(vec![untitled]), &"a".repeat(300))), [8]);
        let query = "a".repeat(10_000);
        assert!(rows(&live(vec![window(1, "b", "c")]), &query).is_empty());
    }

    fn bubble(id: u64) -> Row {
        Row::Bubble {
            window_id: id,
            title: format!("w{id}"),
            app_id: String::new(),
            icon: None,
        }
    }

    #[test]
    fn the_selection_follows_its_window_or_takes_the_row_in_its_place() {
        let before = [bubble(1), bubble(2), bubble(3)];
        // The selected window moved: the selection follows it.
        assert_eq!(relocate(&before, 1, &[bubble(4), bubble(1), bubble(2)]), 2);
        // The selected window left: the row now in its place.
        assert_eq!(relocate(&before, 1, &[bubble(1), bubble(3)]), 1);
        // The last one left: the new last one.
        assert_eq!(relocate(&before, 2, &[bubble(1), bubble(2)]), 1);
        // Nothing left: no selection.
        assert_eq!(relocate(&before, 2, &[]), 0);
        // No rows before (a note was shown): the first row.
        assert_eq!(relocate(&[], 0, &[bubble(5)]), 0);
    }
}
