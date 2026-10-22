//! The launcher's pure state: the query, the rows it answers and the
//! selected one (design §5.5, Launcher).
//!
//! A query that starts with `/` lists the shell's switches
//! ([`super::commands::commands_for`]); any other query ranks the desktop
//! index with `celestina_shell::launcher` (the name exactly, its prefix, a
//! word's start, then anything that mentions the letters in order), and
//! within one of those tiers an application launched recently comes first,
//! the most recent first; the rest keep the ranking's name order. At most
//! [`MAX_ROWS`] rows are shown.
//!
//! The `Mod+K` selector is the same state over the bubbles [`Source`]: its
//! rows are Melibea's minimised windows ([`super::selector`]), and no query
//! is a command there.

use celestina_core::desktop_entry::DesktopEntry;
use celestina_shell::launcher::rank;
use celestina_ui::Field;

use super::commands::{commands_for, Switch};
use super::selector;
use crate::bubbles::Bubbles;

/// The most rows the launcher shows.
pub const MAX_ROWS: usize = 8;

/// One row of the launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// An application, boxed: the entry is large beside a switch.
    App(Box<DesktopEntry>),
    Switch(Switch),
    /// A minimised window of the selector: Melibea's id, its title and
    /// application id (bounded, empty when it has none) and its icon name.
    Bubble {
        window_id: u64,
        title: String,
        app_id: String,
        icon: Option<String>,
    },
}

/// What the overlay's rows answer from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// The launcher: the desktop index and the `/` switches.
    #[default]
    Apps,
    /// The selector: the minimised windows as last seen.
    Bubbles(Bubbles),
}

/// The query, the rows it answers and the selected row.
#[derive(Debug, Default)]
pub struct LauncherState {
    /// What was typed; the launcher copies its field's text here.
    pub query: Field,
    /// The selected row's index in `rows` (0 when there are none).
    pub selected: usize,
    pub rows: Vec<Row>,
    /// The desktop-file ids launched last, most recent first.
    pub recent: Vec<String>,
    /// Where the rows come from.
    pub source: Source,
}

impl LauncherState {
    /// An empty query over `recent` launches (most recent first).
    #[must_use]
    pub fn new(recent: Vec<String>) -> LauncherState {
        LauncherState {
            recent,
            ..LauncherState::default()
        }
    }

    /// The selector over `bubbles`, its query empty and its first row (the
    /// most recently minimised window) selected.
    #[must_use]
    pub fn selector(bubbles: Bubbles) -> LauncherState {
        let mut state = LauncherState {
            source: Source::Bubbles(bubbles),
            ..LauncherState::default()
        };
        state.update(&[]);
        state
    }

    /// Whether this is the selector's state.
    #[must_use]
    pub fn is_selector(&self) -> bool {
        matches!(self.source, Source::Bubbles(_))
    }

    /// The selector's windows changed to `bubbles`: the query is answered
    /// again and the selection follows its window ([`selector::relocate`]).
    /// Returns whether anything changed: the windows changed, so the rows
    /// or the note may have (a note changes with no row, absent to empty).
    /// The launcher's own state ignores it and returns `false`.
    pub fn set_bubbles(&mut self, bubbles: &Bubbles) -> bool {
        let Source::Bubbles(current) = &mut self.source else {
            return false;
        };
        if current == bubbles {
            return false;
        }
        current.clone_from(bubbles);
        self.update(&[]);
        true
    }

    /// Replaces the query with `text` and answers it from `index`, the
    /// selection back on the first row. Returns whether the text changed.
    pub fn set_query(&mut self, text: &str, index: &[DesktopEntry]) -> bool {
        if self.query.text() == text {
            return false;
        }
        self.query.set_text(text);
        self.rows = self.answer(index);
        self.selected = 0;
        true
    }

    /// Answers the current query from `index` again (a new index arrived,
    /// or the query changed); the selection stays where it was when it
    /// still names a row. The selector answers from its own windows and
    /// keeps the selected window ([`selector::relocate`]).
    pub fn update(&mut self, index: &[DesktopEntry]) {
        let rows = self.answer(index);
        self.selected = match self.source {
            Source::Apps if self.selected < rows.len() => self.selected,
            Source::Apps => 0,
            Source::Bubbles(_) => selector::relocate(&self.rows, self.selected, &rows),
        };
        self.rows = rows;
    }

    /// The rows the current query names.
    fn answer(&self, index: &[DesktopEntry]) -> Vec<Row> {
        let query = self.query.text();
        if let Source::Bubbles(bubbles) = &self.source {
            return selector::rows(bubbles, query);
        }
        match commands_for(query) {
            Some(switches) => switches.into_iter().map(Row::Switch).collect(),
            None => ranked(index, query.trim(), &self.recent)
                .into_iter()
                .map(|entry| Row::App(Box::new(entry.clone())))
                .collect(),
        }
    }

    /// Moves the selection `delta` rows, wrapping at both ends.
    pub fn move_selection(&mut self, delta: i32) {
        let count = self.rows.len();
        if count == 0 {
            self.selected = 0;
            return;
        }
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let at = i64::try_from(self.selected).unwrap_or(0);
        let next = (at + i64::from(delta)).rem_euclid(count);
        self.selected = usize::try_from(next).unwrap_or(0);
    }

    /// The selected row, which Enter chooses.
    #[must_use]
    pub fn chosen(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }
}

/// The entries of `index` that answer `query`, best first, recent launches
/// first within each tier, at most [`MAX_ROWS`].
fn ranked<'a>(index: &'a [DesktopEntry], query: &str, recent: &[String]) -> Vec<&'a DesktopEntry> {
    let mut hits = rank(index, query);
    let recency = |entry: &DesktopEntry| {
        recent
            .iter()
            .position(|id| *id == entry.id)
            .unwrap_or(usize::MAX)
    };
    // Stable: equal tiers and equal recency keep the ranking's name order.
    hits.sort_by_key(|hit| (std::cmp::Reverse(hit.quality), recency(hit.entry)));
    hits.into_iter()
        .take(MAX_ROWS)
        .map(|hit| hit.entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use celestina_shell::launcher::rank_bounded;

    use super::*;

    fn entry(id: &str, name: &str) -> DesktopEntry {
        DesktopEntry {
            id: format!("{id}.desktop"),
            name: name.to_owned(),
            exec: id.to_owned(),
            is_application: true,
            ..DesktopEntry::default()
        }
    }

    fn ids(state: &LauncherState) -> Vec<String> {
        state
            .rows
            .iter()
            .map(|row| match row {
                Row::App(e) => e.id.clone(),
                Row::Switch(s) => s.command().to_owned(),
                Row::Bubble { window_id, .. } => window_id.to_string(),
            })
            .collect()
    }

    fn typed(text: &str, index: &[DesktopEntry], recent: &[&str]) -> LauncherState {
        let mut state = LauncherState::new(recent.iter().map(|s| (*s).to_owned()).collect());
        state.update(index);
        state.set_query(text, index);
        state
    }

    #[test]
    fn ranking_orders_prefix_then_word_start_then_substring() {
        let index = [
            entry("loose", "Text Remarks"),  // t..e..r..m: letters in order
            entry("word", "GNOME Terminal"), // a word starts with it
            entry("prefix", "Terminology"),  // the name starts with it
            entry("other", "Calculator"),    // no answer
            entry("exact", "Term"),          // the whole name
        ];
        let want = [
            "exact.desktop",
            "prefix.desktop",
            "word.desktop",
            "loose.desktop",
        ];
        let bounded = rank_bounded(&index, "term", MAX_ROWS);
        assert_eq!(bounded.ids, want);
        assert!(!bounded.truncated);
        // The launcher shows the salvaged ranking as it is.
        assert_eq!(ids(&typed("term", &index, &[])), want);
        // And never more than eight rows.
        let many: Vec<_> = (0..20)
            .map(|i| entry(&format!("t{i:02}"), &format!("Tool {i:02}")))
            .collect();
        assert_eq!(rank_bounded(&many, "t", MAX_ROWS).ids.len(), MAX_ROWS);
        assert_eq!(typed("t", &many, &[]).rows.len(), MAX_ROWS);
    }

    #[test]
    fn recent_use_lifts_an_entry() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("b", "Bab")];
        assert_eq!(
            ids(&typed("a", &index, &[])),
            ["aa.desktop", "ab.desktop", "b.desktop"]
        );
        // Within the prefix tier, the recent one comes first...
        assert_eq!(
            ids(&typed("a", &index, &["ab.desktop"])),
            ["ab.desktop", "aa.desktop", "b.desktop"]
        );
        // ...but recency never lifts a loose answer over a prefix one.
        assert_eq!(
            ids(&typed("a", &index, &["b.desktop"])),
            ["aa.desktop", "ab.desktop", "b.desktop"]
        );
        // With nothing typed, the recent launches lead, most recent first.
        assert_eq!(
            ids(&typed("", &index, &["b.desktop", "ab.desktop"])),
            ["b.desktop", "ab.desktop", "aa.desktop"]
        );
    }

    #[test]
    fn slash_unknown_lists_nothing_and_chooses_nothing() {
        let index = [entry("zed", "Zed"), entry("z", "zzz")];
        let state = typed("/zzz", &index, &[]);
        assert!(state.rows.is_empty(), "no switch and no application");
        assert_eq!(state.chosen(), None);
        let all = typed("/", &index, &[]);
        assert_eq!(
            all.rows,
            Switch::ALL.map(Row::Switch).to_vec(),
            "a slash lists the switches, never applications"
        );
        assert_eq!(all.chosen(), Some(&Row::Switch(Switch::Notificaciones)));
    }

    #[test]
    fn arrows_wrap_within_the_rows() {
        let index = [entry("a", "Aa"), entry("b", "Ab"), entry("c", "Ac")];
        let mut state = typed("a", &index, &[]);
        assert_eq!(state.selected, 0);
        state.move_selection(-1);
        assert_eq!(state.selected, 2, "up from the first is the last");
        state.move_selection(1);
        assert_eq!(state.selected, 0, "down from the last is the first");
        state.move_selection(1);
        state.move_selection(1);
        assert_eq!(state.chosen(), Some(&Row::App(Box::new(index[2].clone()))));
        state.move_selection(7);
        assert_eq!(state.selected, 0);
        // Typing starts again from the first row; no rows, no selection.
        state.set_query("ab", &index);
        assert_eq!(state.selected, 0);
        state.set_query("zz", &index);
        state.move_selection(1);
        assert_eq!((state.selected, state.chosen()), (0, None));
    }

    #[test]
    fn a_new_index_keeps_a_selection_that_still_names_a_row() {
        let index = [entry("a", "Aa"), entry("b", "Ab")];
        let mut state = typed("a", &index, &[]);
        state.move_selection(1);
        state.update(&index);
        assert_eq!(state.selected, 1);
        state.update(&index[..1]);
        assert_eq!(state.selected, 0);
    }

    fn window(id: u64, title: &str) -> celestina_shell::melibea::Window {
        celestina_shell::melibea::Window {
            id,
            app_id: Some("app".to_owned()),
            title: Some(title.to_owned()),
            icon_name: None,
        }
    }

    fn live(ids: &[u64]) -> Bubbles {
        Bubbles {
            windows: ids
                .iter()
                .map(|&id| window(id, &format!("w{id}")))
                .collect(),
            link: crate::bubbles::Link::Live,
        }
    }

    #[test]
    fn the_selector_starts_on_the_most_recent_window_and_wraps() {
        let mut state = LauncherState::selector(live(&[4, 2, 9]));
        assert!(state.is_selector());
        assert_eq!(ids(&state), ["4", "2", "9"]);
        assert_eq!(state.selected, 0, "Mod+K, Enter restores the newest");
        state.move_selection(-1);
        assert_eq!(ids(&state)[state.selected], "9");
        state.move_selection(1);
        assert_eq!(state.selected, 0);
        // A slash is no command in the selector: it is a title like any.
        let mut slashed = LauncherState::selector(Bubbles {
            windows: vec![window(1, "/tmp"), window(2, "other")],
            link: crate::bubbles::Link::Live,
        });
        slashed.set_query("/", &[]);
        assert_eq!(ids(&slashed), ["1"]);
        // The desktop index answers nothing in the selector.
        let index = [entry("a", "w4")];
        slashed.set_query("", &index);
        assert_eq!(ids(&slashed), ["1", "2"]);
    }

    #[test]
    fn typing_filters_the_windows_and_selects_the_first_answer() {
        let mut state = LauncherState::selector(live(&[1, 2, 3]));
        state.move_selection(2);
        assert!(state.set_query("w2", &[]));
        assert_eq!((ids(&state), state.selected), (vec!["2".to_owned()], 0));
        assert!(state.set_query("zz", &[]));
        assert!(state.rows.is_empty());
        state.move_selection(1);
        assert_eq!((state.selected, state.chosen()), (0, None));
    }

    #[test]
    fn a_new_projection_keeps_the_selected_window_or_the_row_in_its_place() {
        let mut state = LauncherState::selector(live(&[1, 2, 3]));
        state.move_selection(1);
        // A window minimised meanwhile goes first; the selection stays on 2.
        assert!(state.set_bubbles(&live(&[7, 1, 2, 3])));
        assert_eq!(ids(&state)[state.selected], "2");
        assert!(!state.set_bubbles(&live(&[7, 1, 2, 3])), "the same windows");
        // The selected window left: the one now in its place.
        assert!(state.set_bubbles(&live(&[7, 1, 3])));
        assert_eq!(ids(&state)[state.selected], "3");
        // The last one left: the new last.
        assert!(state.set_bubbles(&live(&[7, 1])));
        assert_eq!(ids(&state)[state.selected], "1");
        // All gone, then Melibea lost: no rows, no selection.
        assert!(state.set_bubbles(&live(&[])));
        assert_eq!((state.rows.len(), state.selected), (0, 0));
        assert!(state.set_bubbles(&Bubbles::default()));
        assert!(state.rows.is_empty());
        // The launcher's own state takes no windows.
        let mut apps = LauncherState::new(Vec::new());
        assert!(!apps.set_bubbles(&live(&[1])));
        assert!(!apps.is_selector() && apps.rows.is_empty());
    }
}
