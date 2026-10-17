//! The piece contract (parity spec §6.3): a **piece** is one thing the bar
//! can hold, and the static [`PIECES`] table is every piece the shell has.
//! The table is the one source of the Island's tabs, of their order and of
//! the words of `island open <page>`.
//!
//! # Adding a piece (CEL-2-C onwards)
//!
//! 1. Write `pieces/<name>.rs` with a unit struct that implements [`Piece`].
//!    Its [`PieceId`] is the key `shell.json`'s `bar` zones list it by
//!    (`[a-z0-9_.-]`, at most 64 bytes, never `island`, which marks the
//!    Island's place); its [`Piece::name`] is product copy from `copy.rs`.
//!    A piece with a page returns its `PageView` (`island/pages/`) from
//!    [`Piece::page`]; a [`PageId`] constant exists only for a page that code
//!    outside its piece names (a verb, a launcher switch).
//! 2. If the piece needs shared state the bar does not carry yet, add one
//!    typed, read-only slice to [`BarCx`] (and fill it where the Island
//!    builds the context). A piece reads only its context: never another
//!    piece, never the Island.
//! 3. If it asks for something new, add one variant to [`PieceAction`] and
//!    its arm where the Island turns piece actions into `IslandAction`s.
//! 4. Append it to [`PIECES`]. Its place there is its tab's place, if it has
//!    a page; a piece's place in the bar is the layout's, not the table's.
//!
//! # What a piece is not allowed to do
//!
//! A piece is a pure description. It never touches a surface, a bus, a file
//! or a process, keeps no state (it is a shared `static`) and knows no
//! pixels: it builds a [`View`] and answers an input with a [`PieceAction`].
//! The Island lays the view out, paints it, routes the input and anchors any
//! popup to the node the input landed on. Input on a node goes only to the
//! piece whose view drew that node (the first, should two pieces draw the
//! same key, which is logged), so a piece never sees another's keys. A
//! widget is keyed and marked focusable, and every size and colour comes
//! from [`Theme`].
//!
//! # What the bar does not do yet
//!
//! The bar is operated by the pointer only. At rest its surface takes no
//! keyboard, and with a page open the keys go to the page; nothing moves the
//! focus onto a bar node, and the toolkit has no channel to assistive
//! technology, so [`Piece::name`] and the pieces' labels are not exposed
//! yet. Until the keyboard path exists (a later unit of CEL-2-M, or group 2),
//! every action a piece offers must also be reachable from a page or a verb,
//! as the three pieces' actions are (their pages and their rows).

pub mod clipboard;
pub mod clock;
pub mod notifications;
pub mod sysmon;
pub mod tray;
pub mod weather;
pub mod workspaces;

use celestina_paint::Theme;
use celestina_shell::glance::Today;
use celestina_ui::{Key, View};

use crate::{
    island::pages::{clipboard::ClipboardPanel, HistoryRow, NameOwner, PageView, TrayRow},
    providers::{
        niri::NiriModel, notifications::NotificationsCommand, sysmon::SysReading,
        weather::WeatherReading,
    },
    tray::{ItemId, TrayCommand},
};

/// A piece's stable name: its key in the layout of `shell.json`. Only the
/// pieces' own modules make one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PieceId(&'static str);

impl PieceId {
    /// The name as the layout spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// An Island page, by the word `island open <word>` names it with. Only the
/// pieces' own modules make one, so every `PageId` names a page of the
/// table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageId(&'static str);

impl PageId {
    /// The Activity page (the Workspaces piece's): workspaces and windows.
    pub const ACTIVITY: PageId = PageId("activity");
    /// The Clipboard page.
    pub const CLIPBOARD: PageId = PageId("clipboard");
    /// The Notifications page.
    pub const NOTIFICATIONS: PageId = PageId("notifications");
    /// The Tray page.
    pub const TRAY: PageId = PageId("tray");
    /// The System page (the System monitor piece's): the meters and their
    /// history.
    pub const SYSMON: PageId = PageId("sysmon");
    /// The Clock page (the Clock and Weather pieces'): the month calendar and
    /// the weather reading.
    pub const CLOCK: PageId = PageId("clock");

    /// Every page, in tab order: the table's order.
    #[must_use]
    pub fn all() -> Vec<PageId> {
        pages().map(|entry| entry.id).collect()
    }

    /// The page's word in a verb.
    #[must_use]
    pub fn word(self) -> &'static str {
        self.0
    }
}

/// The page a piece opens: its id and the view the Island draws for it.
#[derive(Clone, Copy)]
pub struct PageEntry {
    pub id: PageId,
    pub view: &'static dyn PageView,
}

/// Everything a piece may read while it builds its widget: the sibling of
/// the pages' `PageCx`, with the same slices of the shell's state.
#[derive(Clone, Copy, Debug)]
pub struct BarCx<'a> {
    /// Every colour, size and radius a piece draws with.
    pub theme: &'a Theme,
    /// The output whose bar shows the piece.
    pub output: &'a str,
    /// The last niri model, if the compositor has sent one.
    pub model: Option<&'a NiriModel>,
    /// Who holds the bus names the pieces depend on.
    pub owners: &'a NameOwner,
    /// The do-not-disturb setting in force.
    pub do_not_disturb: bool,
    /// The notification history, newest first.
    pub history: &'a [HistoryRow],
    /// The tray's items, with their icons resolved.
    pub items: &'a [TrayRow],
    /// The item `Id`s the bar's Tray piece leaves out (`bar.tray_hidden`);
    /// the Tray page still lists them.
    pub tray_hidden: &'a [String],
    /// The system monitor's reading as the bar shows it (held back to the
    /// bar's cadence, never the page's newest); `None` while there is none.
    pub sysmon: Option<&'a SysReading>,
    /// The weather's reading; `None` while there is no fresh one.
    pub weather: Option<&'a WeatherReading>,
    /// Today's local date, as of the last minute the Island read.
    pub today: Option<Today>,
    /// The clipboard history as the bar shows it (CEL-2-G): whether the duty
    /// is on and whether it is paused.
    pub clipboard: &'a ClipboardPanel,
}

/// What the Island saw on a node of a piece's widget.
#[derive(Clone, Copy, Debug)]
pub struct PieceInput<'a> {
    /// The node, a key of the view [`Piece::bar`] returned.
    pub key: Key,
    pub gesture: Gesture,
    /// The context the widget was built from.
    pub cx: &'a BarCx<'a>,
}

/// One input on a node, already told apart by the Island.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// A left click. (Enter or Space once the bar takes the keyboard.)
    Primary,
    /// A middle click.
    Middle,
    /// A right click. (The Menu key or Shift+F10 once the bar takes the
    /// keyboard.)
    Context,
    /// Whole wheel steps on one axis, with the Wayland sign (positive is
    /// down and right). The Island keeps what is not yet a step, per node.
    Scroll(Axis, i32),
}

/// A wheel axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// What a piece asks the Island for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PieceAction {
    /// Nothing.
    None,
    /// Grow into (or switch to) the page.
    OpenPage(PageId),
    /// Focus the niri workspace with this id.
    FocusWorkspace(u64),
    /// Something the tray host does for an item.
    Tray(TrayCommand),
    /// The item's menu, anchored to the node the input landed on.
    TrayMenu(ItemId),
    /// Something the notification server does.
    Notification(NotificationsCommand),
}

/// One thing the bar can hold.
pub trait Piece: Sync {
    /// The piece's layout key.
    fn id(&self) -> PieceId;
    /// The accessible name of its widget, from `copy.rs`.
    fn name(&self) -> &'static str;
    /// Its widget for `cx`, or `None` when it has nothing to show (a zone
    /// whose pieces are all absent draws no capsule).
    fn bar(&self, cx: &BarCx<'_>) -> Option<View>;
    /// The Island page it opens, if any: that page is one of the tabs.
    fn page(&self) -> Option<PageEntry>;
    /// What `input` on its widget asks for.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction;
}

/// Every piece. The order of those with a page is the tab order.
pub static PIECES: &[&dyn Piece] = &[
    &workspaces::Workspaces,
    &notifications::Notifications,
    &tray::Tray,
    &sysmon::Sysmon,
    &clock::Clock,
    &weather::Weather,
    &clipboard::Clipboard,
];

/// The piece the layout names `id`, if the table has it.
#[must_use]
pub fn piece(id: &str) -> Option<&'static dyn Piece> {
    PIECES.iter().copied().find(|p| p.id().as_str() == id)
}

/// The pieces' pages, in tab order.
pub fn pages() -> impl Iterator<Item = PageEntry> {
    PIECES.iter().filter_map(|p| p.page())
}

/// The page `island open <word>` names, if the table has it.
#[must_use]
pub fn page_by_word(word: &str) -> Option<PageId> {
    pages().map(|entry| entry.id).find(|id| id.word() == word)
}

/// The table's entry of page `id`.
#[must_use]
pub fn page_entry(id: PageId) -> Option<PageEntry> {
    pages().find(|entry| entry.id == id)
}

/// Page `id`'s place in the tab row, left to right.
#[must_use]
pub fn page_index(id: PageId) -> Option<usize> {
    pages().position(|entry| entry.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::providers::niri::Workspace;

    #[test]
    fn the_table_yields_the_pieces_and_the_tab_order() {
        let ids: Vec<&str> = PIECES.iter().map(|p| p.id().as_str()).collect();
        assert_eq!(
            ids,
            [
                "workspaces",
                "notifications",
                "tray",
                "sysmon",
                "clock",
                "weather",
                "clipboard"
            ]
        );
        assert_eq!(
            PageId::all(),
            [
                PageId::ACTIVITY,
                PageId::NOTIFICATIONS,
                PageId::TRAY,
                PageId::SYSMON,
                PageId::CLOCK,
                PageId::CLIPBOARD
            ]
        );
        for (i, id) in PageId::all().into_iter().enumerate() {
            assert_eq!(page_index(id), Some(i), "{id:?}");
            assert_eq!(page_entry(id).map(|e| e.id), Some(id));
        }
    }

    #[test]
    fn ids_and_words_are_unique_and_fit_the_layout_grammar() {
        let ids: HashSet<&str> = PIECES.iter().map(|p| p.id().as_str()).collect();
        assert_eq!(ids.len(), PIECES.len(), "one piece per id");
        let words: HashSet<&str> = pages().map(|e| e.id.word()).collect();
        assert_eq!(words.len(), pages().count(), "one page per word");
        let fits = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
        };
        for id in &ids {
            assert!(fits(id) && *id != "island", "{id}");
        }
        for word in &words {
            assert!(fits(word), "{word}");
        }
    }

    #[test]
    fn lookups_find_only_what_the_table_holds() {
        for p in PIECES {
            let found = piece(p.id().as_str()).map(|f| f.id());
            assert_eq!(found, Some(p.id()));
        }
        for word in [
            "activity",
            "notifications",
            "tray",
            "sysmon",
            "clock",
            "clipboard",
        ] {
            assert_eq!(page_by_word(word).map(PageId::word), Some(word));
        }
        for foreign in ["", "Activity", "island", "workspaces", "tray ", "menu"] {
            assert_eq!(page_by_word(foreign), None, "{foreign:?}");
        }
        for foreign in ["", "Tray", "island", "activity", "Clock", "sysmon "] {
            assert!(piece(foreign).is_none(), "{foreign:?}");
        }
    }

    #[test]
    fn every_named_page_is_a_page_of_the_table() {
        for id in [
            PageId::ACTIVITY,
            PageId::NOTIFICATIONS,
            PageId::TRAY,
            PageId::SYSMON,
            PageId::CLOCK,
            PageId::CLIPBOARD,
        ] {
            assert!(page_entry(id).is_some(), "{id:?}");
        }
    }

    #[test]
    fn every_piece_has_an_accessible_name() {
        for p in PIECES {
            assert!(!p.name().is_empty(), "{:?}", p.id());
        }
    }

    // ---- Characterisation (CEL-2-K T0): the three CEL-2-A pieces as they
    // are before the table grows; they must pass unchanged.

    fn keys_of(v: &View) -> Vec<Key> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n.key);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn bar_of(
        id: &str,
        model: Option<&NiriModel>,
        history: &[HistoryRow],
        items: &[TrayRow],
    ) -> Option<Vec<Key>> {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        let cx = BarCx {
            theme: &theme,
            output: "DP-1",
            model,
            owners: &owners,
            do_not_disturb: false,
            history,
            items,
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
        };
        piece(id)
            .expect("in the table")
            .bar(&cx)
            .map(|v| keys_of(&v))
    }

    #[test]
    fn the_three_pieces_keep_their_names_pages_and_bar_keys() {
        // The table grew in CEL-2-K: the first three entries are asserted as
        // they were.
        let names: Vec<&str> = PIECES.iter().take(3).map(|p| p.name()).collect();
        assert_eq!(names, ["Espacios de trabajo", "Notificaciones", "Bandeja"]);
        let pages: Vec<Option<&str>> = PIECES
            .iter()
            .take(3)
            .map(|p| p.page().map(|e| e.id.word()))
            .collect();
        assert_eq!(
            pages,
            [Some("activity"), Some("notifications"), Some("tray")]
        );

        // Nothing to show: workspaces and the tray are absent, the bell stays.
        assert_eq!(bar_of("workspaces", None, &[], &[]), None);
        assert_eq!(bar_of("tray", None, &[], &[]), None);
        assert_eq!(
            bar_of("notifications", None, &[], &[]),
            Some(vec![Key::named("bell"), Key::named("bell-icon")])
        );

        // Something to show.
        let ws = |id, active| Workspace {
            id,
            idx: 1,
            name: None,
            output: Some("DP-1".to_owned()),
            is_active: active,
            is_focused: active,
            columns: 1,
        };
        let model = NiriModel {
            workspaces: vec![ws(5, false), ws(6, true)],
            ..NiriModel::default()
        };
        let keys = bar_of("workspaces", Some(&model), &[], &[]).expect("dots");
        assert_eq!(keys.len(), 5, "container, two dots, two marks");
        assert_eq!(keys[0], Key::named("workspaces"));
        assert_eq!(keys[2], Key::indexed("dot-mark", 5));
        assert_eq!(keys[4], Key::indexed("dot-mark", 6));

        let unread = HistoryRow::default();
        assert!(!unread.read);
        assert_eq!(
            bar_of("notifications", None, &[unread], &[]),
            Some(vec![
                Key::named("bell"),
                Key::named("bell-icon"),
                Key::named("bell-unread")
            ])
        );

        let item = TrayRow {
            id: ItemId {
                bus_name: ":1.1".to_owned(),
                path: "/StatusNotifierItem".to_owned(),
            },
            ..TrayRow::default()
        };
        let keys = bar_of("tray", None, &[], &[item]).expect("icons");
        assert!(keys.len() >= 2, "a container and one icon: {keys:?}");
    }
}
