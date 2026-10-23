//! The overview's pure state: the mode, the workspaces of one output as the
//! scrolling layout arranged them, and the selected tile (design §6,
//! Overview).
//!
//! Each workspace's windows go through `celestina_shell::workspace_map`, which
//! turns them into columns left to right and tiles top to bottom with their
//! share of the room. The selection walks that grid: Left and Right change
//! the column, Up and Down the row and, past the first or last row, the
//! neighbouring workspace. Windows floating over a workspace have no place in
//! the grid, so they form one extra last column of their own. A workspace
//! with no window can still be chosen: Enter then focuses the workspace.

use std::ops::Range;

use celestina_shell::keybinds::Bind;
use celestina_shell::workspace_map::{map, Map, Window as MapWindow};

use crate::{
    copy,
    providers::{
        keybinds::{Keybinds, KeybindsUnavailable},
        niri::{NiriModel, Window},
    },
};

/// The most workspaces one page of the overview shows; the page that holds
/// the selection is the one drawn.
pub const MAX_WORKSPACES: usize = 8;

/// What the panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The workspaces and their windows.
    #[default]
    Desktop,
    /// The compositor's key bindings (T7).
    Shortcuts,
}

impl Mode {
    /// The other mode.
    #[must_use]
    pub fn other(self) -> Mode {
        match self {
            Mode::Desktop => Mode::Shortcuts,
            Mode::Shortcuts => Mode::Desktop,
        }
    }
}

/// A direction of the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// What a window tile shows besides its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: u64,
    /// The title as the client set it; empty when it has none.
    pub title: String,
    /// The `.desktop` icon name, when the model found one.
    pub icon: Option<String>,
    pub focused: bool,
}

/// One workspace of the output.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceView {
    /// niri's workspace id: the target of a focus request.
    pub id: u64,
    /// Its name, or its numbered label.
    pub label: String,
    /// The workspace shown on the output.
    pub active: bool,
    /// Its windows arranged in columns, with their shares.
    pub map: Map,
    /// What each window of the map shows.
    pub windows: Vec<WindowInfo>,
    /// The selectable grid: the columns' window ids, then one more column
    /// with the floating windows when there are any.
    pub grid: Vec<Vec<u64>>,
}

impl WorkspaceView {
    /// The window `id`'s details.
    #[must_use]
    pub fn info(&self, id: u64) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// How many windows the workspace holds, hidden ones excluded.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.grid.iter().map(Vec::len).sum()
    }
}

/// The selected tile: indices into the workspaces, their grid's columns and
/// the column's rows. A workspace with an empty grid selects itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub workspace: usize,
    pub column: usize,
    pub row: usize,
}

/// What Enter or a click on the selection asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Window(u64),
    Workspace(u64),
}

/// A scroll step of the Shortcuts mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scroll {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// One row of the Shortcuts mode, in the order the groups list them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutLine {
    /// The title of a group.
    Heading(String),
    /// One binding.
    Bind { chord: String, label: String },
    /// The one explanatory line shown when there is nothing to list.
    Note(&'static str),
}

/// What a binding's row says it does: the owner's own title when there is
/// one; for a `spawn` the Spanish "open" with the program's name, for a
/// `spawn-sh` the Spanish "run a command"; otherwise niri's raw action token
/// verbatim (`focus-column-left`), which is the config's word, not prose.
#[must_use]
pub fn bind_label(bind: &Bind) -> String {
    if let Some(title) = &bind.title {
        return title.clone();
    }
    match (bind.action.as_str(), bind.program.as_deref()) {
        ("spawn", Some(program)) => copy::shortcuts_open_program(program),
        ("spawn" | "spawn-sh", _) => copy::SHORTCUTS_RUN_COMMAND.to_owned(),
        (action, _) => action.to_owned(),
    }
}

/// The mode, the workspaces and the selection.
#[derive(Debug, Default)]
pub struct OverviewState {
    pub mode: Mode,
    pub workspaces: Vec<WorkspaceView>,
    pub selected: Selection,
    output: String,
    /// The bindings, once read; `None` until the first answer arrives.
    keybinds: Option<Keybinds>,
    /// The first Shortcuts row shown.
    scroll: usize,
    /// How many Shortcuts rows fit the body, set by the view's host.
    viewport: usize,
}

impl OverviewState {
    /// The overview of `output` over `model`, the focused window selected
    /// (else the active workspace).
    #[must_use]
    pub fn new(model: &NiriModel, output: &str) -> OverviewState {
        let mut state = OverviewState {
            output: output.to_owned(),
            ..OverviewState::default()
        };
        state.workspaces = build(model, output);
        state.selected = state.initial();
        state
    }

    /// A new compositor model: the workspaces are built again and the
    /// selection stays on the same window (or workspace) when it still
    /// exists, else on the nearest valid tile. Returns whether anything the
    /// view shows changed.
    pub fn set_model(&mut self, model: &NiriModel) -> bool {
        let before = self.target();
        let rebuilt = build(model, &self.output);
        if rebuilt == self.workspaces {
            return false;
        }
        self.workspaces = rebuilt;
        self.selected = before
            .and_then(|target| self.find(target))
            .unwrap_or_else(|| self.clamped(self.selected));
        true
    }

    /// Switches to `mode`; returns whether it changed.
    pub fn set_mode(&mut self, mode: Mode) -> bool {
        let changed = self.mode != mode;
        self.mode = mode;
        changed
    }

    /// Hands the state the bindings just read; returns whether anything the
    /// view shows changed. The scroll offset is kept inside the new rows.
    pub fn set_keybinds(&mut self, keybinds: Keybinds) -> bool {
        if self.keybinds.as_ref() == Some(&keybinds) {
            return false;
        }
        self.keybinds = Some(keybinds);
        self.scroll = self.scroll.min(self.max_scroll());
        true
    }

    /// The bindings, when they were read.
    #[must_use]
    pub fn keybinds(&self) -> Option<&Keybinds> {
        self.keybinds.as_ref()
    }

    /// Every Shortcuts row: a heading then its binds for each group, or the
    /// one line that says why there is nothing. Empty until the bindings
    /// arrive.
    #[must_use]
    pub fn shortcut_lines(&self) -> Vec<ShortcutLine> {
        match &self.keybinds {
            None => Vec::new(),
            Some(Keybinds::Unavailable(why)) => vec![ShortcutLine::Note(match why {
                KeybindsUnavailable::Missing => copy::SHORTCUTS_MISSING,
                KeybindsUnavailable::Unreadable => copy::SHORTCUTS_UNREADABLE,
                KeybindsUnavailable::TooLarge => copy::SHORTCUTS_TOO_LARGE,
                KeybindsUnavailable::Invalid => copy::SHORTCUTS_INVALID,
            })],
            Some(Keybinds::Groups(groups)) => {
                let groups: Vec<_> = groups.iter().filter(|g| !g.binds.is_empty()).collect();
                if groups.is_empty() {
                    return vec![ShortcutLine::Note(copy::SHORTCUTS_EMPTY)];
                }
                let mut lines = Vec::new();
                for group in groups {
                    lines.push(ShortcutLine::Heading(match &group.name {
                        None => copy::SHORTCUTS_NORMAL_TITLE.to_owned(),
                        Some(name) => copy::shortcuts_mode_heading(name, group.enter.as_deref()),
                    }));
                    lines.extend(group.binds.iter().map(|b| ShortcutLine::Bind {
                        chord: b.chord.clone(),
                        label: bind_label(b),
                    }));
                }
                lines
            }
        }
    }

    /// The first Shortcuts row shown.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll
    }

    /// How many rows fit; the scroll offset is pulled back inside the rows.
    /// Returns whether the offset changed.
    pub fn set_viewport(&mut self, rows: usize) -> bool {
        self.viewport = rows;
        let before = self.scroll;
        self.scroll = self.scroll.min(self.max_scroll());
        self.scroll != before
    }

    /// Scrolls the Shortcuts rows; returns whether the offset changed.
    pub fn scroll_by(&mut self, step: Scroll) -> bool {
        let max = self.max_scroll();
        let page = self.viewport.saturating_sub(1).max(1);
        let next = match step {
            Scroll::Up => self.scroll.saturating_sub(1),
            Scroll::Down => self.scroll.saturating_add(1),
            Scroll::PageUp => self.scroll.saturating_sub(page),
            Scroll::PageDown => self.scroll.saturating_add(page),
            Scroll::Home => 0,
            Scroll::End => max,
        }
        .min(max);
        let moved = next != self.scroll;
        self.scroll = next;
        moved
    }

    /// The last useful first row: the one that leaves the viewport full.
    fn max_scroll(&self) -> usize {
        self.shortcut_lines()
            .len()
            .saturating_sub(self.viewport.max(1))
    }

    /// The workspaces of the page that holds the selection.
    #[must_use]
    pub fn page(&self) -> Range<usize> {
        let start = self.selected.workspace / MAX_WORKSPACES * MAX_WORKSPACES;
        start..self.workspaces.len().min(start + MAX_WORKSPACES)
    }

    /// The selected tile's target: the window, or the workspace when it
    /// holds none.
    #[must_use]
    pub fn target(&self) -> Option<Target> {
        let ws = self.workspaces.get(self.selected.workspace)?;
        match ws
            .grid
            .get(self.selected.column)
            .and_then(|c| c.get(self.selected.row))
        {
            Some(&id) => Some(Target::Window(id)),
            None => Some(Target::Workspace(ws.id)),
        }
    }

    /// Moves the selection one tile; returns whether it moved.
    pub fn move_selection(&mut self, dir: Dir) -> bool {
        let Selection {
            workspace,
            column,
            row,
        } = self.selected;
        let Some(ws) = self.workspaces.get(workspace) else {
            return false;
        };
        let next = match dir {
            Dir::Left | Dir::Right if ws.grid.is_empty() => return false,
            Dir::Left | Dir::Right => {
                let column = if dir == Dir::Left {
                    column.saturating_sub(1)
                } else {
                    (column + 1).min(ws.grid.len() - 1)
                };
                let rows = ws.grid.get(column).map_or(0, Vec::len);
                Selection {
                    workspace,
                    column,
                    row: row.min(rows.saturating_sub(1)),
                }
            }
            Dir::Up if row > 0 => Selection {
                row: row - 1,
                ..self.selected
            },
            Dir::Down if row + 1 < ws.grid.get(column).map_or(0, Vec::len) => Selection {
                row: row + 1,
                ..self.selected
            },
            Dir::Up => {
                let Some(workspace) = workspace.checked_sub(1) else {
                    return false;
                };
                self.entering(workspace, column, true)
            }
            Dir::Down => {
                if workspace + 1 >= self.workspaces.len() {
                    return false;
                }
                self.entering(workspace + 1, column, false)
            }
        };
        let moved = next != self.selected;
        self.selected = next;
        moved
    }

    /// Selects the tile of window `id`; returns whether it exists.
    pub fn select_window(&mut self, id: u64) -> bool {
        self.select(Target::Window(id))
    }

    /// Selects workspace `id` itself; returns whether it exists.
    pub fn select_workspace(&mut self, id: u64) -> bool {
        self.select(Target::Workspace(id))
    }

    fn select(&mut self, target: Target) -> bool {
        match self.find(target) {
            Some(at) => {
                self.selected = at;
                true
            }
            None => false,
        }
    }

    /// Where `target` is, if it is in the workspaces.
    fn find(&self, target: Target) -> Option<Selection> {
        match target {
            Target::Window(id) => self.workspaces.iter().enumerate().find_map(|(w, ws)| {
                ws.grid.iter().enumerate().find_map(|(c, col)| {
                    col.iter().position(|&x| x == id).map(|r| Selection {
                        workspace: w,
                        column: c,
                        row: r,
                    })
                })
            }),
            Target::Workspace(id) => {
                self.workspaces
                    .iter()
                    .position(|ws| ws.id == id)
                    .map(|workspace| Selection {
                        workspace,
                        ..Selection::default()
                    })
            }
        }
    }

    /// The selection on entering `workspace` from above (`last` row) or
    /// below (first row), in the nearest column to `column`.
    fn entering(&self, workspace: usize, column: usize, from_below: bool) -> Selection {
        let Some(ws) = self.workspaces.get(workspace) else {
            return self.selected;
        };
        let column = column.min(ws.grid.len().saturating_sub(1));
        let rows = ws.grid.get(column).map_or(0, Vec::len);
        Selection {
            workspace,
            column,
            row: if from_below {
                rows.saturating_sub(1)
            } else {
                0
            },
        }
    }

    /// `at` pulled back inside the grid.
    fn clamped(&self, at: Selection) -> Selection {
        let workspace = at.workspace.min(self.workspaces.len().saturating_sub(1));
        let Some(ws) = self.workspaces.get(workspace) else {
            return Selection::default();
        };
        let column = at.column.min(ws.grid.len().saturating_sub(1));
        let rows = ws.grid.get(column).map_or(0, Vec::len);
        Selection {
            workspace,
            column,
            row: at.row.min(rows.saturating_sub(1)),
        }
    }

    /// The focused window, else the active workspace, else the first.
    fn initial(&self) -> Selection {
        let focused = self
            .workspaces
            .iter()
            .flat_map(|ws| ws.windows.iter())
            .find(|w| w.focused)
            .and_then(|w| self.find(Target::Window(w.id)));
        let active = || {
            self.workspaces
                .iter()
                .position(|ws| ws.active)
                .map(|workspace| {
                    self.clamped(Selection {
                        workspace,
                        ..Selection::default()
                    })
                })
        };
        focused.or_else(active).unwrap_or_default()
    }
}

/// The workspaces of `output` in niri's order, each with its windows folded
/// into the scrolling layout.
fn build(model: &NiriModel, output: &str) -> Vec<WorkspaceView> {
    model
        .workspaces
        .iter()
        .filter(|w| w.output.as_deref() == Some(output))
        .map(|w| {
            let windows: Vec<&Window> = model
                .windows
                .iter()
                .filter(|win| win.workspace_id == Some(w.id))
                .collect();
            let arranged = map(&windows
                .iter()
                .map(|win| map_window(win))
                .collect::<Vec<_>>());
            let mut grid: Vec<Vec<u64>> = arranged
                .columns
                .iter()
                .map(|c| {
                    c.tiles
                        .iter()
                        .filter_map(|t| t.window.id.parse().ok())
                        .collect()
                })
                .collect();
            if !arranged.floating.is_empty() {
                grid.push(
                    arranged
                        .floating
                        .iter()
                        .filter_map(|f| f.id.parse().ok())
                        .collect(),
                );
            }
            let kept = |id: u64| grid.iter().any(|c| c.contains(&id));
            WorkspaceView {
                id: w.id,
                label: w
                    .name
                    .clone()
                    .unwrap_or_else(|| copy::workspace_number(w.idx)),
                active: w.is_active,
                windows: windows
                    .iter()
                    .filter(|win| kept(win.id))
                    .map(|win| WindowInfo {
                        id: win.id,
                        title: win.title.clone().unwrap_or_default(),
                        icon: win.icon.clone(),
                        focused: win.is_focused,
                    })
                    .collect(),
                map: arranged,
                grid,
            }
        })
        .collect()
}

/// A model window as `workspace_map` wants it: a window without a column is
/// floating.
fn map_window(w: &Window) -> MapWindow {
    MapWindow::new(
        &w.id.to_string(),
        w.title.as_deref().unwrap_or_default(),
        w.app_id.as_deref().unwrap_or_default(),
        w.column.unwrap_or(0),
        w.row.unwrap_or(0),
    )
    .sized(f64::from(w.tile_size.0), f64::from(w.tile_size.1))
    .with_states(w.is_focused, w.column.is_none(), false)
}

/// Models the tests of this module and of the view share.
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::providers::niri::{NiriModel, Window, Workspace};

    pub fn ws(id: u64, idx: u8, active: bool) -> Workspace {
        Workspace {
            id,
            idx,
            name: None,
            output: Some("DP-1".into()),
            is_active: active,
            is_focused: active,
            columns: 0,
        }
    }

    pub fn win(id: u64, workspace: u64, at: Option<(u16, u16)>, size: (i32, i32)) -> Window {
        Window {
            id,
            title: Some(format!("w{id}")),
            app_id: Some("app".into()),
            workspace_id: Some(workspace),
            column: at.map(|a| a.0),
            row: at.map(|a| a.1),
            tile_size: size,
            ..Window::default()
        }
    }

    /// Workspace 1: columns [10, 11 over 12] and a floating 13; workspace 2
    /// empty; workspace 3: one window 30.
    pub fn model() -> NiriModel {
        NiriModel {
            workspaces: vec![ws(1, 1, true), ws(2, 2, false), ws(3, 3, false)],
            windows: vec![
                win(10, 1, Some((1, 1)), (400, 600)),
                win(11, 1, Some((2, 1)), (800, 300)),
                win(12, 1, Some((2, 2)), (800, 300)),
                win(13, 1, None, (200, 200)),
                win(30, 3, Some((1, 1)), (800, 600)),
            ],
            outputs: vec!["DP-1".into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fixtures::*, *};

    fn at(workspace: usize, column: usize, row: usize) -> Selection {
        Selection {
            workspace,
            column,
            row,
        }
    }

    #[test]
    fn the_grid_has_the_columns_then_the_floating_windows() {
        let s = OverviewState::new(&model(), "DP-1");
        assert_eq!(s.workspaces.len(), 3);
        assert_eq!(s.workspaces[0].grid, [vec![10], vec![11, 12], vec![13]]);
        assert!(s.workspaces[1].grid.is_empty());
        assert_eq!(s.workspaces[2].grid, [vec![30]]);
        assert_eq!(s.workspaces[0].window_count(), 4);
        assert_eq!(s.workspaces[0].label, copy::workspace_number(1));
    }

    #[test]
    fn another_output_has_no_workspaces_and_nothing_to_choose() {
        let mut s = OverviewState::new(&model(), "HDMI-A-1");
        assert!(s.workspaces.is_empty());
        assert_eq!(s.target(), None);
        for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
            assert!(!s.move_selection(dir));
        }
        assert_eq!(s.selected, Selection::default());
        assert!(s.page().is_empty());
        let empty = OverviewState::new(&NiriModel::default(), "DP-1");
        assert_eq!(empty.target(), None);
    }

    #[test]
    fn the_focused_window_starts_selected_else_the_active_workspace() {
        let mut m = model();
        m.windows[2].is_focused = true;
        let s = OverviewState::new(&m, "DP-1");
        assert_eq!(s.selected, at(0, 1, 1));
        assert_eq!(s.target(), Some(Target::Window(12)));
        m.windows[2].is_focused = false;
        m.workspaces[0].is_active = false;
        m.workspaces[1].is_active = true;
        let s = OverviewState::new(&m, "DP-1");
        assert_eq!(s.target(), Some(Target::Workspace(2)));
    }

    #[test]
    fn left_and_right_walk_the_columns_and_clamp_at_the_ends() {
        let mut s = OverviewState::new(&model(), "DP-1");
        assert_eq!(s.selected, at(0, 0, 0));
        assert!(!s.move_selection(Dir::Left));
        assert!(s.move_selection(Dir::Right));
        assert_eq!(s.target(), Some(Target::Window(11)));
        s.selected.row = 1;
        assert!(s.move_selection(Dir::Right));
        assert_eq!(
            s.target(),
            Some(Target::Window(13)),
            "the floating column has one row: the row is clamped"
        );
        assert!(!s.move_selection(Dir::Right));
        assert!(s.move_selection(Dir::Left));
        assert_eq!(s.selected, at(0, 1, 0));
    }

    #[test]
    fn up_and_down_walk_the_rows_then_the_neighbouring_workspaces() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.selected = at(0, 1, 0);
        assert!(s.move_selection(Dir::Down));
        assert_eq!(s.target(), Some(Target::Window(12)));
        assert!(
            s.move_selection(Dir::Down),
            "past the last row: next workspace"
        );
        assert_eq!(
            s.target(),
            Some(Target::Workspace(2)),
            "an empty workspace selects itself"
        );
        assert!(s.move_selection(Dir::Down));
        assert_eq!(s.target(), Some(Target::Window(30)));
        assert!(!s.move_selection(Dir::Down), "the last workspace stops");
        assert!(s.move_selection(Dir::Up));
        assert!(s.move_selection(Dir::Up));
        assert_eq!(
            s.target(),
            Some(Target::Window(10)),
            "entering from below lands on the last row of the nearest column"
        );
        s.selected = at(0, 1, 1);
        assert!(s.move_selection(Dir::Up));
        assert!(!s.move_selection(Dir::Up), "the first workspace stops");
    }

    #[test]
    fn left_and_right_do_nothing_on_an_empty_workspace() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.selected = at(1, 0, 0);
        assert!(!s.move_selection(Dir::Left));
        assert!(!s.move_selection(Dir::Right));
        assert_eq!(s.selected, at(1, 0, 0));
    }

    #[test]
    fn a_new_model_keeps_the_selected_window_or_clamps() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.select_window(12);
        let mut m = model();
        m.windows.remove(0); // window 10 closes: column 11/12 becomes the first
        assert!(s.set_model(&m));
        assert_eq!(s.target(), Some(Target::Window(12)));
        assert_eq!(s.selected, at(0, 0, 1));
        assert!(!s.set_model(&m), "the same model changes nothing");
        // The selected window closes: the selection is clamped, never lost.
        m.windows.retain(|w| w.id != 12 && w.id != 11);
        assert!(s.set_model(&m));
        assert!(s.target().is_some());
        // The output loses every workspace.
        assert!(s.set_model(&NiriModel::default()));
        assert_eq!(s.target(), None);
    }

    #[test]
    fn select_by_id_names_a_window_or_a_workspace_and_refuses_strangers() {
        let mut s = OverviewState::new(&model(), "DP-1");
        assert!(s.select_window(30));
        assert_eq!(s.selected, at(2, 0, 0));
        assert!(s.select_workspace(2));
        assert_eq!(s.target(), Some(Target::Workspace(2)));
        assert!(!s.select_window(999));
        assert!(!s.select_workspace(999));
        assert_eq!(s.target(), Some(Target::Workspace(2)));
    }

    #[test]
    fn the_page_is_the_block_of_workspaces_holding_the_selection() {
        let m = NiriModel {
            workspaces: (1..=11).map(|i| ws(u64::from(i), i, i == 1)).collect(),
            ..NiriModel::default()
        };
        let mut s = OverviewState::new(&m, "DP-1");
        assert_eq!(s.page(), 0..MAX_WORKSPACES);
        s.selected.workspace = 9;
        assert_eq!(s.page(), MAX_WORKSPACES..11);
    }

    #[test]
    fn modes_switch_and_say_whether_they_changed() {
        let mut s = OverviewState::new(&model(), "DP-1");
        assert_eq!(s.mode, Mode::Desktop);
        assert!(!s.set_mode(Mode::Desktop));
        assert!(s.set_mode(Mode::Shortcuts));
        assert_eq!(s.mode.other(), Mode::Desktop);
    }

    fn binds(n: usize) -> Keybinds {
        use celestina_shell::keybinds::BindGroup;
        Keybinds::Groups(vec![
            BindGroup {
                name: None,
                enter: None,
                binds: (0..n)
                    .map(|i| Bind {
                        chord: format!("Mod+{i}"),
                        title: None,
                        action: format!("do-{i}"),
                        program: None,
                    })
                    .collect(),
            },
            BindGroup {
                name: Some("size".to_owned()),
                enter: Some("Mod+R".to_owned()),
                binds: vec![Bind {
                    chord: "R".to_owned(),
                    title: None,
                    action: "switch-preset-column-width".to_owned(),
                    program: None,
                }],
            },
        ])
    }

    #[test]
    fn lines_list_a_heading_then_the_binds_of_each_group() {
        let mut s = OverviewState::new(&model(), "DP-1");
        assert!(s.shortcut_lines().is_empty(), "nothing before the answer");
        assert!(s.set_keybinds(binds(2)));
        assert!(!s.set_keybinds(binds(2)), "the same answer changes nothing");
        let lines = s.shortcut_lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(
            lines[0],
            ShortcutLine::Heading(copy::SHORTCUTS_NORMAL_TITLE.to_owned())
        );
        assert_eq!(
            lines[3],
            ShortcutLine::Heading(copy::shortcuts_mode_heading("size", Some("Mod+R")))
        );
    }

    #[test]
    fn a_row_shows_the_title_else_spanish_for_spawns_else_the_raw_token() {
        let bind = |title: Option<&str>, action: &str, program: Option<&str>| Bind {
            chord: "Mod+X".to_owned(),
            title: title.map(str::to_owned),
            action: action.to_owned(),
            program: program.map(str::to_owned),
        };
        assert_eq!(
            bind_label(&bind(Some("My own title"), "spawn", Some("kitty"))),
            "My own title"
        );
        assert_eq!(
            bind_label(&bind(None, "spawn", Some("kitty"))),
            copy::shortcuts_open_program("kitty")
        );
        assert_eq!(
            bind_label(&bind(None, "spawn", None)),
            copy::SHORTCUTS_RUN_COMMAND
        );
        assert_eq!(
            bind_label(&bind(None, "spawn-sh", None)),
            copy::SHORTCUTS_RUN_COMMAND
        );
        assert_eq!(
            bind_label(&bind(None, "focus-column-left", None)),
            "focus-column-left",
            "the raw token, never made into English prose"
        );
    }

    #[test]
    fn unavailable_and_empty_give_one_note() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.set_keybinds(Keybinds::Unavailable(KeybindsUnavailable::TooLarge));
        assert_eq!(
            s.shortcut_lines(),
            [ShortcutLine::Note(copy::SHORTCUTS_TOO_LARGE)]
        );
        s.set_keybinds(Keybinds::Groups(Vec::new()));
        assert_eq!(
            s.shortcut_lines(),
            [ShortcutLine::Note(copy::SHORTCUTS_EMPTY)]
        );
    }

    #[test]
    fn scrolling_clamps_at_both_ends() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.set_keybinds(binds(20)); // 23 lines
        s.set_viewport(10);
        assert!(!s.scroll_by(Scroll::Up), "already at the top");
        assert!(s.scroll_by(Scroll::Down));
        assert_eq!(s.scroll_offset(), 1);
        assert!(s.scroll_by(Scroll::PageDown));
        assert_eq!(s.scroll_offset(), 10, "a page keeps one row of context");
        assert!(s.scroll_by(Scroll::End));
        assert_eq!(s.scroll_offset(), 13, "the last page is full");
        assert!(!s.scroll_by(Scroll::Down));
        assert!(!s.scroll_by(Scroll::PageDown));
        assert!(s.scroll_by(Scroll::PageUp));
        assert_eq!(s.scroll_offset(), 4);
        assert!(s.scroll_by(Scroll::Home));
        assert_eq!(s.scroll_offset(), 0);
        assert!(!s.scroll_by(Scroll::Home));
    }

    #[test]
    fn new_rows_or_a_taller_viewport_pull_the_offset_back() {
        let mut s = OverviewState::new(&model(), "DP-1");
        s.set_keybinds(binds(20));
        s.set_viewport(10);
        s.scroll_by(Scroll::End);
        assert!(s.set_viewport(30), "everything fits: back to the top");
        assert_eq!(s.scroll_offset(), 0);
        s.set_viewport(10);
        s.scroll_by(Scroll::End);
        s.set_keybinds(binds(2));
        assert_eq!(s.scroll_offset(), 0);
        assert!(!s.scroll_by(Scroll::End), "five rows fit ten");
    }
}
