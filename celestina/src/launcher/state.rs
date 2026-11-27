//! The launcher's pure state: the query, the rows it answers and the
//! selected one (design §5.5, Launcher).
//!
//! A query that starts with `/` lists the shell's switches
//! ([`super::commands::commands_for`]); any other query ranks the desktop
//! index with `celestina_shell::launcher_search` (words in any order, one
//! typo, aliases), and within equal scores the application used more comes
//! first ([`super::usage`]). An empty query, or one of only separators,
//! answers from the [`Tab`]: the recent applications, the whole index by
//! name, or one group. Every answer is a row: the view scrolls the list
//! ([`super::scrolling`]) or lays it out as a grid.
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
use super::groups::{self, Group};
use super::selector;
use crate::bubbles::Bubbles;

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

/// What an empty query lists (design §5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// The applications launched before, the most used first.
    #[default]
    Recent,
    /// Every application, by name.
    All,
    /// The applications of one group, by name.
    Group(Group),
}

/// How the rows are laid out (design §5.5): the choice `shell.json` keeps.
pub use celestina_shell::settings::LauncherView as ListView;

/// The most applications the Recent tab lists.
pub const RECENT_LIMIT: usize = 50;

/// The cells of a grid line.
pub const GRID_COLUMNS: usize = 6;

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
    /// What an empty query lists.
    pub tab: Tab,
    /// How the rows are laid out.
    pub view: ListView,
    /// The group of each prepared entry, built with `prepared`.
    entry_groups: Vec<Group>,
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
        self.entry_groups.clear();
        let mut fell_back = false;
        if matches!(self.source, Source::Apps) {
            self.sync(index);
            if let Tab::Group(group) = self.tab {
                if !self.visible_tabs().contains(&Tab::Group(group)) {
                    self.tab = Tab::All;
                    fell_back = true;
                }
            }
        }
        let rows = self.answer(index);
        self.selected = match self.source {
            Source::Apps if fell_back => 0,
            Source::Apps if self.selected < rows.len() => self.selected,
            Source::Apps => 0,
            Source::Bubbles(_) => selector::relocate(&self.rows, self.selected, &rows),
        };
        self.rows = rows;
    }

    /// Whether the search or the `/` commands answer the query, not the
    /// tab: the view dims the tab row meanwhile.
    #[must_use]
    pub fn searching(&self) -> bool {
        matches!(self.source, Source::Apps) && {
            let query = self.query.text();
            commands_for(query).is_some() || has_match_chars(query)
        }
    }

    /// The tabs to show: Recent, All, then each group with an application
    /// in the last index.
    #[must_use]
    pub fn visible_tabs(&self) -> Vec<Tab> {
        let mut tabs = vec![Tab::Recent, Tab::All];
        tabs.extend(
            Group::ALL
                .into_iter()
                .filter(|group| self.entry_groups.contains(group))
                .map(Tab::Group),
        );
        tabs
    }

    /// Where the open tab sits among [`LauncherState::visible_tabs`].
    #[must_use]
    pub fn tab_position(&self) -> usize {
        self.visible_tabs()
            .iter()
            .position(|t| *t == self.tab)
            .unwrap_or(0)
    }

    /// Shows `tab`: the query is emptied and the selection goes to the
    /// first row.
    pub fn set_tab(&mut self, tab: Tab, index: &[DesktopEntry]) {
        self.tab = tab;
        self.query.set_text("");
        self.copied = false;
        self.rows = self.answer(index);
        self.selected = 0;
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
                    // First: the row at the top is the one Enter copies.
                    rows.insert(0, Row::Calc { result });
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
        self.entry_groups = index
            .iter()
            .map(|entry| groups::group_of(&entry.categories))
            .collect();
        self.prepared = index
            .iter()
            .map(|entry| {
                let aliases = groups::aliases(&entry.categories);
                Prepared::new(entry, self.locale.as_ref(), &aliases)
            })
            .collect();
    }

    /// The rows of `query` over the prepared `index`: every answer.
    fn ranked(&self, index: &[DesktopEntry], query: &str) -> Vec<Row> {
        let score = |id: &str| self.scores.get(id).copied().unwrap_or(0);
        let hits: Vec<usize> = if has_match_chars(query) {
            search(&self.prepared, query, &score)
        } else {
            self.listed_by_tab(&score)
        };
        hits.into_iter()
            .map(|i| {
                Row::App(Box::new(AppRow {
                    entry: index[i].clone(),
                    name: self.prepared[i].display.clone(),
                    detail: self.prepared[i].detail.clone(),
                }))
            })
            .collect()
    }

    /// The prepared entries the current tab lists, in order.
    fn listed_by_tab(&self, score: &dyn Fn(&str) -> u64) -> Vec<usize> {
        let mut listed: Vec<(std::cmp::Reverse<u64>, String, &str, usize)> = self
            .prepared
            .iter()
            .enumerate()
            .filter(|&(i, p)| match self.tab {
                Tab::Recent => score(&p.id) > 0,
                Tab::All => true,
                Tab::Group(group) => self.entry_groups.get(i) == Some(&group),
            })
            .map(|(i, p)| {
                let used = if self.tab == Tab::Recent {
                    score(&p.id)
                } else {
                    0
                };
                (std::cmp::Reverse(used), fold(&p.display), p.id.as_str(), i)
            })
            .collect();
        listed.sort();
        if self.tab == Tab::Recent {
            listed.truncate(RECENT_LIMIT);
        }
        listed.into_iter().map(|(_, _, _, i)| i).collect()
    }

    /// The grid line and the column of row `i`: a calculator row is a line
    /// of one, then the rows run in lines of [`GRID_COLUMNS`].
    #[must_use]
    pub fn grid_line(&self, i: usize) -> (usize, usize) {
        let lead = self.grid_lead();
        if i < lead {
            return (0, 0);
        }
        let cell = i - lead;
        (lead + cell / GRID_COLUMNS, cell % GRID_COLUMNS)
    }

    /// 1 when the first row is the calculator's, which is a grid line of
    /// its own, else 0.
    #[must_use]
    pub fn grid_lead(&self) -> usize {
        usize::from(matches!(self.rows.first(), Some(Row::Calc { .. })))
    }

    /// Moves the selection through the grid: `dx` cells along the rows,
    /// `dy` lines down, to the same column clamped to the line, both
    /// wrapping at the ends.
    pub fn move_grid(&mut self, dx: i32, dy: i32) {
        let count = self.rows.len();
        if count == 0 {
            self.selected = 0;
            return;
        }
        let lead = self.grid_lead();
        let mut at = wrapped(self.selected.min(count - 1), dx, count);
        if dy != 0 {
            let (line, column) = self.grid_line(at);
            let lines = lead + (count - lead).div_ceil(GRID_COLUMNS);
            let line = wrapped(line, dy, lines);
            at = if line < lead {
                0
            } else {
                let start = lead + (line - lead) * GRID_COLUMNS;
                let len = (count - start).min(GRID_COLUMNS);
                start + column.min(len - 1)
            };
        }
        self.selected = at;
    }

    /// Moves the selection `delta` rows, wrapping at both ends.
    pub fn move_selection(&mut self, delta: i32) {
        let count = self.rows.len();
        if count == 0 {
            self.selected = 0;
            return;
        }
        self.selected = wrapped(self.selected, delta, count);
    }

    /// The selected row, which Enter chooses.
    #[must_use]
    pub fn chosen(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }
}

/// `at` moved `delta` places among `count`, wrapping at both ends; 0 when
/// there are none.
pub fn wrapped(at: usize, delta: i32, count: usize) -> usize {
    let Ok(count) = i64::try_from(count) else {
        return 0;
    };
    if count == 0 {
        return 0;
    }
    let at = i64::try_from(at).unwrap_or(0);
    usize::try_from((at + i64::from(delta)).rem_euclid(count)).unwrap_or(0)
}

/// Whether `query` has anything the search can match.
fn has_match_chars(query: &str) -> bool {
    query.chars().any(|c| c.is_alphanumeric() || c == '+')
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
        // Every answer is a row, past eight: the view scrolls.
        let many: Vec<_> = (0..20)
            .map(|i| entry(&format!("t{i:02}"), &format!("Tool {i:02}")))
            .collect();
        assert_eq!(typed("t", &many, &[]).rows.len(), 20);
        let mut all = typed("", &many, &[]);
        all.set_tab(Tab::All, &many);
        assert_eq!(all.rows.len(), 20);
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
        // With nothing typed, Recent lists the used ones, the most used first.
        assert_eq!(
            ids(&typed(
                "",
                &index,
                &[("b.desktop", 200), ("ab.desktop", 100)]
            )),
            ["b.desktop", "ab.desktop"]
        );
    }

    #[test]
    fn a_query_of_only_separators_lists_the_index_like_an_empty_one() {
        let index = [entry("aa", "Aa"), entry("b", "Bb")];
        let scores = [("b.desktop", 5)];
        let empty = ids(&typed("", &index, &scores));
        assert_eq!(empty, ["b.desktop"]);
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
        state.set_tab(Tab::All, &first);
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
        // The row leads the whole list.
        let many: Vec<_> = (0..20)
            .map(|i| entry(&format!("t{i:02}"), &format!("2+2 Tool {i:02}")))
            .collect();
        let full = typed("2+2", &many, &[]);
        assert_eq!(full.rows.len(), 21, "the 20 tools and the result");
        assert!(matches!(full.rows[0], Row::Calc { .. }));
    }

    #[test]
    fn the_selector_never_shows_the_calculator_row() {
        let mut state = LauncherState::selector(Bubbles::default());
        state.set_query("2+2", &[]);
        assert!(state.rows.is_empty());
    }

    fn with_cats(id: &str, name: &str, cats: &str) -> DesktopEntry {
        DesktopEntry {
            categories: cats.split(';').map(str::to_owned).collect(),
            ..entry(id, name)
        }
    }

    fn mixed() -> Vec<DesktopEntry> {
        vec![
            with_cats("zed", "Zed", "Development"),
            with_cats("fox", "Firefox", "Network;WebBrowser"),
            with_cats("chess", "Chess", "Game"),
            with_cats("code", "Code", "Development"),
        ]
    }

    fn scored(index: &[DesktopEntry], scores: &[(&str, u64)]) -> LauncherState {
        typed("", index, scores)
    }

    #[test]
    fn recent_lists_used_entries_best_first_capped_and_empty_with_no_scores() {
        let index = mixed();
        assert!(scored(&index, &[]).rows.is_empty());
        let state = scored(
            &index,
            &[("fox.desktop", 3), ("zed.desktop", 9), ("code.desktop", 3)],
        );
        assert_eq!(ids(&state), ["zed.desktop", "code.desktop", "fox.desktop"]);
        let many: Vec<_> = (0..60)
            .map(|i| entry(&format!("t{i:02}"), &format!("T {i:02}")))
            .collect();
        let scores: Vec<(String, u64)> = (0..60)
            .map(|i| (format!("t{i:02}.desktop"), 1 + i))
            .collect();
        let refs: Vec<(&str, u64)> = scores.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let state = scored(&many, &refs);
        assert_eq!(state.rows.len(), RECENT_LIMIT);
        assert_eq!(ids(&state)[0], "t59.desktop");
    }

    #[test]
    fn all_lists_by_folded_name_and_a_group_filters_by_group_of() {
        let index = mixed();
        let mut state = scored(&index, &[]);
        state.set_tab(Tab::All, &index);
        assert_eq!(
            ids(&state),
            [
                "chess.desktop",
                "code.desktop",
                "fox.desktop",
                "zed.desktop"
            ]
        );
        state.set_tab(Tab::Group(Group::Development), &index);
        assert_eq!(ids(&state), ["code.desktop", "zed.desktop"]);
        state.set_tab(Tab::Group(Group::Games), &index);
        assert_eq!(ids(&state), ["chess.desktop"]);
        // Whitespace answers from the tab too; the search ignores it.
        state.set_query("  ", &index);
        assert_eq!(ids(&state), ["chess.desktop"]);
        assert!(!state.searching());
        state.set_query("zed", &index);
        assert_eq!(ids(&state), ["zed.desktop"]);
        assert!(state.searching());
        state.set_query("/", &index);
        assert!(state.searching());
    }

    #[test]
    fn visible_tabs_hide_an_empty_group() {
        let index = mixed();
        let state = scored(&index, &[]);
        assert_eq!(
            state.visible_tabs(),
            [
                Tab::Recent,
                Tab::All,
                Tab::Group(Group::Internet),
                Tab::Group(Group::Games),
                Tab::Group(Group::Development),
            ]
        );
    }

    #[test]
    fn an_empty_index_hides_every_group() {
        let index = mixed();
        let mut state = scored(&index, &[]);
        state.update(&[]);
        assert_eq!(state.visible_tabs(), [Tab::Recent, Tab::All]);
    }

    #[test]
    fn set_tab_clears_the_query_and_selects_the_first_row() {
        let index = mixed();
        let mut state = typed("zed", &index, &[]);
        state.move_selection(0);
        state.copied = true;
        state.set_tab(Tab::All, &index);
        assert_eq!(state.query.text(), "");
        assert_eq!(
            (state.selected, state.copied, state.tab),
            (0, false, Tab::All)
        );
        assert_eq!(state.rows.len(), 4);
    }

    #[test]
    fn a_group_that_leaves_the_index_falls_back_to_all() {
        let index = mixed();
        let mut state = scored(&index, &[]);
        state.set_tab(Tab::Group(Group::Games), &index);
        state.update(&index);
        assert_eq!(state.tab, Tab::Group(Group::Games));
        let without = &index[..1];
        state.update(without);
        assert_eq!((state.tab, state.selected), (Tab::All, 0));
        assert_eq!(ids(&state), ["zed.desktop"]);
    }

    fn grid(n: usize, calc: bool) -> LauncherState {
        let index: Vec<_> = (0..n)
            .map(|i| entry(&format!("t{i:02}"), &format!("T {i:02}")))
            .collect();
        let mut state = LauncherState::new(HashMap::new(), None);
        state.update(&index);
        state.set_tab(Tab::All, &index);
        if calc {
            state.rows.insert(
                0,
                Row::Calc {
                    result: "4".to_owned(),
                },
            );
        }
        state
    }

    #[test]
    fn the_grid_moves_by_lines_of_six_and_wraps() {
        let mut state = grid(14, false); // lines of 6, 6, 2
        state.move_grid(1, 0);
        assert_eq!(state.selected, 1);
        state.move_grid(0, 1);
        assert_eq!(state.selected, 7);
        state.move_grid(0, 1);
        assert_eq!(state.selected, 13, "column 1 of a line of two");
        state.move_grid(0, 1);
        assert_eq!(state.selected, 1, "the last line wraps to the first");
        state.move_grid(0, -1);
        assert_eq!(state.selected, 13, "the first line wraps to the last");
        state.move_grid(1, 0);
        assert_eq!(state.selected, 0, "right from the last wraps");
        state.move_grid(-1, 0);
        assert_eq!(state.selected, 13);
        state.move_grid(0, -1);
        assert_eq!(state.selected, 7);
        // Clamp to a short last line.
        state.selected = 5;
        state.move_grid(0, 2);
        assert_eq!(state.selected, 13);
        assert_eq!(state.grid_line(13), (2, 1));
    }

    #[test]
    fn the_calculator_row_is_a_line_of_one() {
        let mut state = grid(8, true);
        assert_eq!(state.selected, 0);
        state.move_grid(0, 1);
        assert_eq!(
            state.selected, 1,
            "down from the calculator is the first cell"
        );
        state.move_grid(0, 1);
        assert_eq!(state.selected, 7);
        state.move_grid(0, -1);
        assert_eq!(state.selected, 1);
        state.selected = 4;
        state.move_grid(0, -1);
        assert_eq!(
            state.selected, 0,
            "up from the first cell line is the calculator"
        );
        state.move_grid(0, -1);
        assert_eq!(state.selected, 7, "the calculator wraps to the last line");
        state.move_grid(1, 0);
        assert_eq!(state.selected, 8);
        state.move_grid(1, 0);
        assert_eq!(state.selected, 0);
        assert_eq!(
            (state.grid_line(0), state.grid_line(1), state.grid_line(7)),
            ((0, 0), (1, 0), (2, 0))
        );
        let mut empty = grid(0, false);
        empty.move_grid(1, 1);
        assert_eq!(empty.selected, 0);
    }
}
