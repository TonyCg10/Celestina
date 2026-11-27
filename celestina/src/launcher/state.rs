//! The launcher's pure state: the query, the rows it answers and the
//! selected one (design §5.5, Launcher).
//!
//! A query that starts with `/` lists the shell's switches
//! ([`super::commands::commands_for`]); any other query ranks the desktop
//! index with `celestina_shell::launcher_search` (words in any order, one
//! typo, aliases), and within equal scores the application used more comes
//! first ([`super::usage`]). An empty query, or one of only separators,
//! lists the whole index by use, then by name. At most [`MAX_ROWS`] rows are
//! shown.
//!
//! The `Mod+K` selector is the same state over the bubbles [`Source`]: its
//! rows are Melibea's minimised windows ([`super::selector`]), and no query
//! is a command there.

use std::collections::HashMap;

use celestina_core::desktop_entry::{DesktopEntry, Locale};
use celestina_shell::calc;
use celestina_shell::launcher::fold;
use celestina_shell::launcher_search::{search, Prepared};
use celestina_ui::Field;

use super::commands::{commands_for, Switch};
use super::{groups, selector};
use crate::bubbles::Bubbles;

/// The most rows the launcher shows.
pub const MAX_ROWS: usize = 8;

/// An application row: the entry and the texts the row shows (the localized
/// name and generic name when the locale has them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRow {
    pub entry: DesktopEntry,
    pub name: String,
    pub detail: String,
}

/// One row of the launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// An application, boxed: the entry is large beside a switch.
    App(Box<AppRow>),
    Switch(Switch),
    /// The calculator: the query is an expression and `result` its value as
    /// shown (a comma for the decimal point). Always the first row.
    Calc {
        result: String,
    },
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
    /// The use score of each launched desktop-file id.
    scores: HashMap<String, u64>,
    /// The locale the names are shown in.
    locale: Option<Locale>,
    /// The index, folded once; rebuilt when the index changes.
    prepared: Vec<Prepared>,
    /// Where the rows come from.
    pub source: Source,
    /// The calculator row's result was just copied: the row says so.
    pub copied: bool,
}

impl LauncherState {
    /// An empty query over the use `scores` and the `locale` of the names.
    #[must_use]
    pub fn new(scores: HashMap<String, u64>, locale: Option<Locale>) -> LauncherState {
        LauncherState {
            scores,
            locale,
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
        self.copied = false;
        true
    }

    /// Answers the current query from `index` again (a new index arrived,
    /// or the query changed); the selection stays where it was when it
    /// still names a row. The selector answers from its own windows and
    /// keeps the selected window ([`selector::relocate`]).
    pub fn update(&mut self, index: &[DesktopEntry]) {
        // A new index may keep every id and change what an entry says (a
        // rename, a translation added), so it is prepared again whole.
        self.prepared.clear();
        let rows = self.answer(index);
        self.selected = match self.source {
            Source::Apps if self.selected < rows.len() => self.selected,
            Source::Apps => 0,
            Source::Bubbles(_) => selector::relocate(&self.rows, self.selected, &rows),
        };
        self.rows = rows;
    }

    /// The rows the current query names.
    fn answer(&mut self, index: &[DesktopEntry]) -> Vec<Row> {
        let query = self.query.text().to_owned();
        let query = query.as_str();
        if let Source::Bubbles(bubbles) = &self.source {
            return selector::rows(bubbles, query);
        }
        match commands_for(query) {
            Some(switches) => switches.into_iter().map(Row::Switch).collect(),
            None => {
                self.sync(index);
                let mut rows = self.ranked(index, query);
                if let Some(result) = calc::evaluate(query) {
                    // First, so the bound on the rows still holds.
                    rows.insert(0, Row::Calc { result });
                    rows.truncate(MAX_ROWS);
                }
                rows
            }
        }
    }

    /// Prepares `index` again when its length or ids differ from the last
    /// build.
    fn sync(&mut self, index: &[DesktopEntry]) {
        let same = self.prepared.len() == index.len()
            && self.prepared.iter().zip(index).all(|(p, e)| p.id == e.id);
        if same {
            return;
        }
        self.prepared = index
            .iter()
            .map(|entry| {
                let aliases = groups::aliases(&entry.categories);
                Prepared::new(entry, self.locale.as_ref(), &aliases)
            })
            .collect();
    }

    /// The rows of `query` over the prepared `index`, at most [`MAX_ROWS`].
    fn ranked(&self, index: &[DesktopEntry], query: &str) -> Vec<Row> {
        let score = |id: &str| self.scores.get(id).copied().unwrap_or(0);
        let hits: Vec<usize> = if query.chars().any(|c| c.is_alphanumeric() || c == '+') {
            search(&self.prepared, query, &score)
        } else {
            // Nothing to match: every entry, the most used first.
            let mut all: Vec<(std::cmp::Reverse<u64>, String, usize)> = self
                .prepared
                .iter()
                .enumerate()
                .map(|(i, p)| (std::cmp::Reverse(score(&p.id)), fold(&p.display), i))
                .collect();
            all.sort();
            all.into_iter().map(|(_, _, i)| i).collect()
        };
        hits.into_iter()
            .take(MAX_ROWS)
            .map(|i| {
                Row::App(Box::new(AppRow {
                    entry: index[i].clone(),
                    name: self.prepared[i].display.clone(),
                    detail: self.prepared[i].detail.clone(),
                }))
            })
            .collect()
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

#[cfg(test)]
mod tests {
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
                Row::App(a) => a.entry.id.clone(),
                Row::Switch(s) => s.command().to_owned(),
                Row::Calc { result } => format!("={result}"),
                Row::Bubble { window_id, .. } => window_id.to_string(),
            })
            .collect()
    }

    fn typed(text: &str, index: &[DesktopEntry], scores: &[(&str, u64)]) -> LauncherState {
        let mut state = LauncherState::new(
            scores
                .iter()
                .map(|(id, n)| ((*id).to_owned(), *n))
                .collect(),
            None,
        );
        state.update(index);
        state.set_query(text, index);
        state
    }

    fn app_row(entry: &DesktopEntry) -> Row {
        Row::App(Box::new(AppRow {
            entry: entry.clone(),
            name: entry.name.clone(),
            detail: entry.generic_name.clone(),
        }))
    }

    #[test]
    fn ranking_orders_exact_prefix_word_then_subsequence() {
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
        assert_eq!(ids(&typed("term", &index, &[])), want);
        // And never more than eight rows.
        let many: Vec<_> = (0..20)
            .map(|i| entry(&format!("t{i:02}"), &format!("Tool {i:02}")))
            .collect();
        assert_eq!(typed("t", &many, &[]).rows.len(), MAX_ROWS);
        assert_eq!(typed("", &many, &[]).rows.len(), MAX_ROWS);
    }

    #[test]
    fn use_score_lifts_an_entry_within_equal_scores_only() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("b", "Bab")];
        assert_eq!(
            ids(&typed("a", &index, &[])),
            ["aa.desktop", "ab.desktop", "b.desktop"]
        );
        // Between two prefix hits, the one used more comes first...
        assert_eq!(
            ids(&typed("a", &index, &[("ab.desktop", 100)])),
            ["ab.desktop", "aa.desktop", "b.desktop"]
        );
        // ...but use never lifts a weaker match over a stronger one.
        assert_eq!(
            ids(&typed("a", &index, &[("b.desktop", 900)])),
            ["aa.desktop", "ab.desktop", "b.desktop"]
        );
        // With nothing typed, the most used lead, then the name.
        assert_eq!(
            ids(&typed(
                "",
                &index,
                &[("b.desktop", 200), ("ab.desktop", 100)]
            )),
            ["b.desktop", "ab.desktop", "aa.desktop"]
        );
    }

    #[test]
    fn a_query_of_only_separators_lists_the_index_like_an_empty_one() {
        let index = [entry("aa", "Aa"), entry("b", "Bb")];
        let scores = [("b.desktop", 5)];
        let empty = ids(&typed("", &index, &scores));
        assert_eq!(empty, ["b.desktop", "aa.desktop"]);
        assert_eq!(ids(&typed("  ", &index, &scores)), empty);
        assert_eq!(ids(&typed(" - . ", &index, &scores)), empty);
    }

    #[test]
    fn a_localized_name_is_shown_and_matched() {
        let mut named = entry("files", "Files");
        named.name_l10n = vec![("es".to_owned(), "Archivos".to_owned())];
        let index = [named];
        let locale = Locale::parse("es_ES.UTF-8");
        let mut state = LauncherState::new(HashMap::new(), locale);
        state.set_query("archi", &index);
        let Some(Row::App(row)) = state.rows.first() else {
            panic!("the localized name answers: {:?}", state.rows);
        };
        assert_eq!(
            (row.name.as_str(), row.entry.name.as_str()),
            ("Archivos", "Files")
        );
    }

    #[test]
    fn a_new_index_with_other_ids_is_prepared_again() {
        let first = [entry("aa", "Aa")];
        let second = [entry("bb", "Bb")];
        let mut state = typed("", &first, &[]);
        assert_eq!(ids(&state), ["aa.desktop"]);
        state.update(&second);
        assert_eq!(ids(&state), ["bb.desktop"]);
    }

    #[test]
    fn a_new_index_with_the_same_ids_and_new_names_is_prepared_again() {
        let first = [entry("aa", "Aa")];
        let mut renamed = entry("aa", "Aa");
        renamed.name_l10n = vec![("es".to_owned(), "Zz".to_owned())];
        let mut state = LauncherState::new(HashMap::new(), Locale::parse("es"));
        state.update(&first);
        state.set_query("zz", &first);
        assert!(state.rows.is_empty());
        state.update(std::slice::from_ref(&renamed));
        assert_eq!(ids(&state), ["aa.desktop"], "the new name answers");
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
        assert_eq!(state.chosen(), Some(&app_row(&index[2])));
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
        let mut apps = LauncherState::new(HashMap::new(), None);
        assert!(!apps.set_bubbles(&live(&[1])));
        assert!(!apps.is_selector() && apps.rows.is_empty());
    }

    #[test]
    fn an_expression_puts_the_calculator_row_first_and_selects_it() {
        let index = [entry("aa", "Aa")];
        let state = typed("2*(3+4)^2", &index, &[]);
        assert_eq!(
            state.rows.first(),
            Some(&Row::Calc {
                result: "98".to_owned()
            })
        );
        assert_eq!(state.selected, 0);
        // A half-typed expression has no row; the applications answer.
        assert!(typed("2*", &index, &[])
            .rows
            .iter()
            .all(|r| !matches!(r, Row::Calc { .. })));
        // The switches' command list never shows it.
        assert_eq!(
            typed("/2+2", &index, &[]).rows,
            Vec::<Row>::new(),
            "no switch is named 2+2"
        );
        // The bound holds with the row in.
        let many: Vec<_> = (0..20)
            .map(|i| entry(&format!("t{i:02}"), &format!("2 Tool {i:02}")))
            .collect();
        let full = typed("2+2", &many, &[]);
        assert!(full.rows.len() <= MAX_ROWS);
        assert!(matches!(full.rows[0], Row::Calc { .. }));
    }

    #[test]
    fn the_selector_never_shows_the_calculator_row() {
        let mut state = LauncherState::selector(Bubbles::default());
        state.set_query("2+2", &[]);
        assert!(state.rows.is_empty());
    }
}
