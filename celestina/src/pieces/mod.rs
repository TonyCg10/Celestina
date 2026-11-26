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
//!    outside its piece names (a verb, a launcher switch). Every node key
//!    its widget draws starts with the piece's id or is a name only that
//!    piece uses, because the bar hands a node's input to the first piece
//!    that drew its key; the table-wide key test
//!    (`no_two_pieces_of_the_table_draw_the_same_node_key`) enforces it, and
//!    fails for a piece its fixture (`tests_support::Everything`) draws
//!    nothing for.
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

pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod bubbles;
pub mod clipboard;
pub mod clock;
pub mod conn_glyph;
mod level;
pub mod media;
pub mod network;
pub mod notifications;
pub mod phone;
pub mod power_profile;
pub mod session;
pub mod sysmon;
pub mod tray;
pub mod volume;
pub mod weather;
pub mod workspaces;

use celestina_paint::Theme;
use celestina_shell::audio::Direction;
use celestina_shell::connectivity::Action;
use celestina_shell::glance::Today;
use celestina_ui::{Key, View};

use celestina_shell::phone::PhoneState;

use crate::{
    bubbles::Bubbles,
    connectivity::state::ConnectivityState,
    island::pages::{
        clipboard::ClipboardPanel, media::MediaState, HistoryRow, NameOwner, PageView, TrayRow,
    },
    media::MediaCommand,
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
    /// The Media page.
    pub const MEDIA: PageId = PageId("media");
    /// The Notifications page.
    pub const NOTIFICATIONS: PageId = PageId("notifications");
    /// The Phone page (the Phone piece's).
    pub const PHONE: PageId = PageId("phone");
    /// The Tray page.
    pub const TRAY: PageId = PageId("tray");
    /// The System page (the System monitor piece's): the meters and their
    /// history.
    pub const SYSMON: PageId = PageId("sysmon");
    /// The Clock page (the Clock and Weather pieces'): the month calendar and
    /// the weather reading.
    pub const CLOCK: PageId = PageId("clock");
    /// The Network page (the network piece's).
    pub const NETWORK: PageId = PageId("network");
    /// The Bluetooth page (the Bluetooth piece's).
    pub const BLUETOOTH: PageId = PageId("bluetooth");
    /// The Power page (the battery piece's, and the power-profile piece's
    /// click opens it too).
    pub const POWER: PageId = PageId("power");

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
    /// The network, Bluetooth and power state (CEL-2-F), read-only.
    pub connectivity: &'a ConnectivityState,
    /// The audio and brightness readings the Volume and Brightness pieces
    /// show; empty while the `osd` duty is off.
    pub levels: Levels,
    /// The phone as Magnetita lists it (CEL-2-J); `Absent` while the duty
    /// is off.
    pub phone: &'a PhoneState,
    /// The minimised windows Melibea keeps (CEL-2-J); empty while the duty
    /// is off.
    pub bubbles: &'a Bubbles,
    /// The players and the one shown (CEL-2-D).
    pub media: &'a MediaState,
}

/// One audio device's reading: whole percent and whether it is muted. Muted
/// is not zero; the level is the one the device remembers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Level {
    pub percent: u8,
    pub muted: bool,
}

/// The default audio devices' readings. `None` is no reading (no device, or
/// the worker has not read it), never 0 %.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioLevels {
    /// The default output (speakers, headphones).
    pub output: Option<Level>,
    /// The default input (microphone).
    pub input: Option<Level>,
}

/// What the Volume and Brightness pieces read: the audio devices, and the
/// brightness of the monitor of the bar's output (`None` while unknown or
/// when that output has no monitor this shell can read).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levels {
    pub audio: AudioLevels,
    pub brightness: Option<u8>,
}

/// The wheel steps of one gesture a piece forwards at most: a flick is not a
/// jump from one end of the range to the other.
pub const MAX_WHEEL_STEPS: u8 = 4;

/// The menus a capsule can grow into (CEL-D-002, CEL-D-003, CEL-D-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind {
    Volume,
    Brightness,
    /// The session menu (CEL-I-006).
    Session,
    /// An Island page, with its overflow button: what a bar piece that asked
    /// for a page opens.
    Page(PageId),
    /// The workspaces list (CEL-D-005).
    Workspaces,
}

/// What the Volume piece asks for. The default output is the one it drives;
/// the microphone is in its sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioAction {
    /// Move the level `steps` steps (at most [`MAX_WHEEL_STEPS`]).
    Step(Direction, u8),
    /// Flip the output's mute.
    ToggleMute,
}

/// What the Brightness piece asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrightnessAction {
    /// Move the level `steps` steps (at most [`MAX_WHEEL_STEPS`]).
    Step(Direction, u8),
}

/// The step action a wheel gesture of `steps` whole steps (the Wayland sign:
/// positive is down) asks for, or `None` for no step. Shared by the two
/// level pieces.
#[must_use]
pub fn wheel(steps: i32) -> Option<(Direction, u8)> {
    if steps == 0 {
        return None;
    }
    let direction = if steps > 0 {
        Direction::Down
    } else {
        Direction::Up
    };
    let count = u8::try_from(steps.unsigned_abs()).unwrap_or(MAX_WHEEL_STEPS);
    Some((direction, count.min(MAX_WHEEL_STEPS)))
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
    /// Grow the piece's capsule into this sheet (or switch to it, or close
    /// it when it is the open one's piece).
    Sheet(SheetKind),
    /// Focus the niri workspace with this id.
    FocusWorkspace(u64),
    /// Something the tray host does for an item.
    Tray(TrayCommand),
    /// The item's menu, anchored to the node the input landed on.
    TrayMenu(ItemId),
    /// The item shows its own menu: the Island sends `ContextMenu` with the
    /// pointer's output coordinates.
    TrayContext(ItemId),
    /// Something the Volume piece asks of the audio worker.
    Audio(AudioAction),
    /// Something the Brightness piece asks of the brightness workers.
    Brightness(BrightnessAction),
    /// Something the notification server does.
    Notification(NotificationsCommand),
    /// A request to the connectivity workers (CEL-2-F): the Island forwards
    /// it, and the workers check it again before they act.
    Connectivity(Action),
    /// Bring the minimised window with this id back; the Island anchors the
    /// transition to the node the input landed on.
    RestoreWindow(u64),
    /// Open the selector of minimised windows.
    OpenSelector,
    /// Something the media worker does to the shown player.
    Media(MediaCommand),
}

/// What a piece shows of itself when its sheet opens: the icon and the text
/// that become the sheet's subtitle (CEL-D-008). The text is the very string
/// the piece's bar view shows (one function per piece builds both); a piece
/// with none leaves it `None` and the subtitle shows the menu's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub icon: &'static str,
    pub text: Option<String>,
}

impl Face {
    /// The face of the page `page`: its tab icon, with `text`.
    #[must_use]
    pub fn of_page(page: PageId, text: Option<String>) -> Option<Face> {
        let entry = page_entry(page)?;
        Some(Face {
            icon: entry.view.icon(),
            text,
        })
    }
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
    /// The icon and text its sheet's subtitle shows, for a piece that opens
    /// a sheet; `None` for one that does not. The default is the page's icon
    /// and no text (the subtitle then shows the page's title); a piece with
    /// text, or with no page of its own, says so.
    fn face(&self, _cx: &BarCx<'_>) -> Option<Face> {
        self.page().map(|entry| Face {
            icon: entry.view.icon(),
            text: None,
        })
    }
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
    &network::Network,
    &bluetooth::Bluetooth,
    &battery::Battery,
    &power_profile::PowerProfile,
    &volume::Volume,
    &brightness::Brightness,
    &phone::Phone,
    &bubbles::Bubbles,
    &media::Media,
    &session::Session,
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
                "clipboard",
                "network",
                "bluetooth",
                "battery",
                "power-profile",
                "volume",
                "brightness",
                "phone",
                "bubbles",
                "media",
                "session"
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
                PageId::CLIPBOARD,
                PageId::NETWORK,
                PageId::BLUETOOTH,
                PageId::POWER,
                PageId::PHONE,
                PageId::MEDIA
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
            "network",
            "bluetooth",
            "power",
            "phone",
            "media",
        ] {
            assert_eq!(page_by_word(word).map(PageId::word), Some(word));
        }
        for foreign in [
            "",
            "Activity",
            "island",
            "workspaces",
            "tray ",
            "menu",
            "bubbles",
        ] {
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
            PageId::NETWORK,
            PageId::BLUETOOTH,
            PageId::POWER,
            PageId::PHONE,
            PageId::MEDIA,
        ] {
            assert!(page_entry(id).is_some(), "{id:?}");
        }
    }

    #[test]
    fn a_wheel_gesture_is_a_bounded_step_with_the_wayland_sign() {
        assert_eq!(wheel(0), None);
        assert_eq!(wheel(1), Some((Direction::Down, 1)));
        assert_eq!(wheel(-1), Some((Direction::Up, 1)));
        assert_eq!(wheel(3), Some((Direction::Down, 3)));
        assert_eq!(wheel(-50), Some((Direction::Up, MAX_WHEEL_STEPS)));
        assert_eq!(wheel(i32::MIN), Some((Direction::Up, MAX_WHEEL_STEPS)));
        assert_eq!(wheel(i32::MAX), Some((Direction::Down, MAX_WHEEL_STEPS)));
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
            connectivity: &ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
            media: &crate::island::pages::media::NO_MEDIA,
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

    #[test]
    fn the_battery_owns_the_power_page_and_the_profile_piece_has_none() {
        let battery = piece("battery").expect("battery");
        let profile = piece("power-profile").expect("power-profile");
        assert_eq!(battery.page().map(|e| e.id), Some(PageId::POWER));
        assert!(profile.page().is_none());
        assert_eq!(pages().filter(|e| e.id == PageId::POWER).count(), 1);
    }

    #[test]
    fn every_connectivity_piece_is_absent_until_its_service_answers() {
        let state = ConnectivityState::default();
        tests_support::with_state(&state, |cx| {
            for id in ["network", "bluetooth", "battery", "power-profile"] {
                let piece = piece(id).expect(id);
                assert!(piece.bar(cx).is_none(), "{id}");
            }
        });
    }

    #[test]
    fn a_ready_state_draws_every_piece_with_unique_keys_across_pieces() {
        let state = crate::connectivity::state::sample::ready();
        tests_support::with_state(&state, |cx| {
            let mut keys = HashSet::new();
            for id in ["network", "bluetooth", "battery", "power-profile"] {
                let view = piece(id).expect(id).bar(cx).expect(id);
                let mut stack = vec![&view];
                while let Some(node) = stack.pop() {
                    assert!(keys.insert(node.key), "{id}: duplicate key");
                    stack.extend(node.children.iter());
                }
            }
        });
    }
}

#[cfg(test)]
mod key_tests {
    use std::collections::HashMap;

    use super::*;

    /// What the survey calls the bar when a piece draws one of its keys; no
    /// piece id can spell it.
    const THE_BAR: &str = "(the bar)";

    /// Draws every piece of the table under `cx`: the ids of the pieces that
    /// drew nothing, and the pairs of owners of every key drawn twice (two
    /// pieces, or the bar itself and a piece).
    fn survey(cx: &BarCx<'_>) -> (Vec<&'static str>, Vec<(&'static str, &'static str)>) {
        let mut owners: HashMap<Key, &'static str> = crate::island::bar_tree::own_keys()
            .into_iter()
            .map(|key| (key, THE_BAR))
            .collect();
        let mut absent = Vec::new();
        let mut clashes = Vec::new();
        for piece in PIECES {
            let id = piece.id().0;
            let Some(view) = piece.bar(cx) else {
                absent.push(id);
                continue;
            };
            let mut stack = vec![&view];
            while let Some(node) = stack.pop() {
                match owners.get(&node.key) {
                    Some(first) if *first != id => clashes.push((*first, id)),
                    Some(_) => {}
                    None => {
                        owners.insert(node.key, id);
                    }
                }
                stack.extend(node.children.iter());
            }
        }
        (absent, clashes)
    }

    /// The nodes of `view` and everything under it.
    fn nodes(view: &View, out: &mut Vec<Key>) {
        out.push(view.key);
        for child in &view.children {
            nodes(child, out);
        }
    }

    /// Whether a left click on some node of `piece`'s widget grows a sheet
    /// (a page's or a menu's): the real set the faces are for.
    fn opens_a_sheet(piece: &dyn Piece, cx: &BarCx<'_>) -> bool {
        let Some(view) = piece.bar(cx) else {
            return false;
        };
        let mut keys = Vec::new();
        nodes(&view, &mut keys);
        keys.into_iter().any(|key| {
            matches!(
                piece.activate(&PieceInput {
                    key,
                    gesture: Gesture::Primary,
                    cx,
                }),
                PieceAction::OpenPage(_) | PieceAction::Sheet(_)
            )
        })
    }

    #[test]
    fn every_piece_that_opens_a_sheet_has_a_face() {
        let everything = tests_support::Everything::new();
        everything.with(|cx| {
            let mut with_face = Vec::new();
            for p in PIECES {
                let opens = opens_a_sheet(*p, cx);
                let face = p.face(cx);
                assert_eq!(
                    face.is_some(),
                    opens,
                    "{:?}: a face exactly when its click opens a sheet",
                    p.id()
                );
                if let Some(face) = face {
                    assert!(!face.icon.is_empty(), "{:?}", p.id());
                    with_face.push(p.id().as_str());
                }
            }
            // The set is what the bar's clicks open today: the tray and the
            // bubbles open none.
            assert_eq!(
                with_face,
                [
                    "workspaces",
                    "notifications",
                    "sysmon",
                    "clock",
                    "weather",
                    "clipboard",
                    "network",
                    "bluetooth",
                    "battery",
                    "power-profile",
                    "volume",
                    "brightness",
                    "phone",
                    "media",
                    "session"
                ]
            );
        });
    }

    #[test]
    fn a_face_text_is_the_string_the_bar_shows() {
        let everything = tests_support::Everything::new();
        everything.with(|cx| {
            for p in PIECES {
                let Some(Face {
                    text: Some(text), ..
                }) = p.face(cx)
                else {
                    continue;
                };
                let view = p.bar(cx).expect("a piece with text is drawn");
                let mut shown = Vec::new();
                texts(&view, &mut shown);
                assert!(
                    shown.contains(&text),
                    "{:?}: {text:?} is not among {shown:?}",
                    p.id()
                );
            }
        });
    }

    fn texts(view: &View, out: &mut Vec<String>) {
        if let celestina_ui::ViewKind::Text { text, .. } = &view.kind {
            out.push(text.clone());
        }
        for child in &view.children {
            texts(child, out);
        }
    }

    /// The bar hands a node's input to the piece that owns its key, the
    /// first to draw it (`bar::view::own`), and finds its own capsules by
    /// key, so no two pieces may draw the same key and no piece one of the
    /// bar's. The Volume and Brightness pieces share one widget and once
    /// shared its two inner keys (found live on 2026-11-23: a warning on
    /// every rebuild of the bar).
    ///
    /// Every piece of the table is drawn: one that draws nothing under
    /// `tests_support::Everything` fails here, so a new piece brings its
    /// state to that fixture and is compared from its first commit.
    #[test]
    fn no_two_pieces_of_the_table_draw_the_same_node_key() {
        let everything = tests_support::Everything::new();
        let (absent, clashes) = everything.with(survey);
        assert!(
            absent.is_empty(),
            "pieces the fixture draws nothing for: {absent:?}"
        );
        assert!(clashes.is_empty(), "sharing a node key: {clashes:?}");
    }
}

/// Test support for the pieces' tests: a context over a given state, the
/// fixtures more than one test module builds, and a state that draws every
/// piece.
#[cfg(test)]
pub(crate) mod tests_support {
    use celestina_shell::{
        connectivity::{ChargeState, Service},
        melibea::Window,
        phone::Phone as Device,
    };

    use super::*;
    use crate::{
        bubbles::Link,
        connectivity::{power::PowerEvent, state::sample},
        media::PlayerView,
        providers::niri::Workspace,
    };

    /// The output every context here is built for.
    const OUTPUT: &str = "DP-1";

    /// Runs `f` with a bar context that reads `state`.
    pub(crate) fn with_state<R>(state: &ConnectivityState, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            theme: &theme,
            output: OUTPUT,
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: state,
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
            media: &crate::island::pages::media::NO_MEDIA,
        })
    }

    /// A workspace on `output`, active and focused or neither.
    pub(crate) fn workspace(
        id: u64,
        idx: u8,
        name: Option<&str>,
        output: &str,
        active: bool,
    ) -> Workspace {
        Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some(output.to_owned()),
            is_active: active,
            is_focused: active,
            columns: 1,
        }
    }

    /// A system monitor reading with no history and no temperature.
    pub(crate) fn sys_reading(cpu: u8, ram: u8) -> SysReading {
        SysReading {
            cpu,
            ram,
            ram_used_kib: 0,
            ram_total_kib: 0,
            temp_c: None,
            cpu_history: Vec::new(),
            ram_history: Vec::new(),
        }
    }

    /// A weather reading for a fixed place.
    pub(crate) fn weather_reading(celsius: i16, code: u8, daylight: bool) -> WeatherReading {
        WeatherReading {
            celsius,
            code,
            daylight,
            label: "Madrid".to_owned(),
            taken_unix: 0,
        }
    }

    /// A connected phone with no player.
    pub(crate) fn phone_device(battery: Option<u8>, charging: bool) -> PhoneState {
        PhoneState::Present(Device {
            id: "dev".to_owned(),
            name: "Pixel".to_owned(),
            battery,
            charging,
            media: None,
        })
    }

    /// A minimised window as Melibea lists it.
    pub(crate) fn bubble_window(
        id: u64,
        title: Option<&str>,
        app_id: Option<&str>,
        icon: Option<&str>,
    ) -> Window {
        Window {
            id,
            app_id: app_id.map(str::to_owned),
            title: title.map(str::to_owned),
            icon_name: icon.map(str::to_owned),
        }
    }

    /// `n` minimised windows without an icon, ids from 1, over `link`.
    pub(crate) fn bubbles_state(n: u64, link: Link) -> Bubbles {
        Bubbles {
            windows: (1..=n)
                .map(|id| bubble_window(id, Some(&format!("w{id}")), Some("app"), None))
                .collect(),
            link,
        }
    }

    /// One player on the bus.
    pub(crate) fn media_player(playing: bool, artist: &str, title: &str) -> PlayerView {
        PlayerView {
            bus_name: "org.mpris.MediaPlayer2.vlc".to_owned(),
            identity: "vlc".to_owned(),
            playing,
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: String::new(),
            art_url: String::new(),
            length_ms: 100_000,
            position_ms: 5_000,
            pinned: false,
        }
    }

    /// The media state of a duty that is on and shows the first of `players`.
    pub(crate) fn media_state(players: Vec<PlayerView>) -> MediaState {
        let active = players.first().map(|p| p.bus_name.clone());
        MediaState {
            players,
            active,
            cover: None,
            duty_on: true,
            unavailable: false,
        }
    }

    /// A shell state under which every piece of [`PIECES`] has something to
    /// show, with the nodes a piece draws only sometimes where one state can
    /// hold them (the unread mark, the paused mark, the charging bolt, the
    /// Wi-Fi bars, the overflow bubble, a bubble with an icon and one
    /// without). A new piece adds its state here: the table-wide key test
    /// fails for a piece this draws nothing for.
    pub(crate) struct Everything {
        connectivity: ConnectivityState,
        model: NiriModel,
        history: Vec<HistoryRow>,
        items: Vec<TrayRow>,
        sysmon: SysReading,
        weather: WeatherReading,
        clipboard: ClipboardPanel,
        phone: PhoneState,
        bubbles: Bubbles,
        media: MediaState,
    }

    impl Everything {
        pub(crate) fn new() -> Everything {
            let mut connectivity = sample::ready();
            connectivity.apply_power(&PowerEvent::Battery(
                sample::battery(ChargeState::Charging, 64.0)
                    .map_or(Service::Unavailable, Service::Ready),
            ));
            let mut bubbles = bubbles_state(bubbles::MAX_VISIBLE as u64 + 1, Link::Live);
            bubbles.windows[0].icon_name = Some("app".to_owned());
            Everything {
                connectivity,
                model: NiriModel {
                    workspaces: vec![workspace(1, 1, None, OUTPUT, true)],
                    ..NiriModel::default()
                },
                history: vec![HistoryRow::default()],
                items: vec![TrayRow::default()],
                sysmon: sys_reading(23, 41),
                weather: weather_reading(12, 3, true),
                clipboard: ClipboardPanel {
                    on: true,
                    paused: true,
                    ..ClipboardPanel::default()
                },
                phone: phone_device(Some(80), false),
                bubbles,
                media: media_state(vec![media_player(true, "Artist", "Title")]),
            }
        }

        /// Runs `f` with a bar context that reads this state.
        pub(crate) fn with<R>(&self, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
            with_state(&self.connectivity, |base| {
                f(&BarCx {
                    model: Some(&self.model),
                    history: &self.history,
                    items: &self.items,
                    sysmon: Some(&self.sysmon),
                    weather: Some(&self.weather),
                    today: Today::from_unix(1_792_281_480, 0),
                    clipboard: &self.clipboard,
                    levels: Levels {
                        audio: AudioLevels {
                            output: Some(Level {
                                percent: 40,
                                muted: false,
                            }),
                            input: None,
                        },
                        brightness: Some(70),
                    },
                    phone: &self.phone,
                    bubbles: &self.bubbles,
                    media: &self.media,
                    ..*base
                })
            })
        }
    }
}
