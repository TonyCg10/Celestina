//! The tray's types and the bounds that keep a hostile peer from making them
//! large: item identity, icons, menus, text, counts, scroll steps and the
//! one-update-per-window coalescing of the item list.

use celestina_shell::coalesce::Coalescer;

/// The most items the tray keeps; further registrations are ignored.
pub const MAX_ITEMS: usize = 64;
/// The longest title or icon name, in characters.
pub const MAX_TEXT_CHARS: usize = 128;
/// The longest menu label, in characters.
pub const MAX_LABEL_CHARS: usize = 128;
/// The deepest menu nesting kept: the root level is 1.
pub const MAX_MENU_DEPTH: usize = 4;
/// The most entries kept on one menu level.
pub const MAX_MENU_ENTRIES: usize = 64;
/// The most entries kept in one whole menu tree, over all levels.
pub const MAX_MENU_TOTAL: usize = 512;
/// The largest pixmap side kept, in pixels.
pub const MAX_PIXMAP_SIDE: u32 = 256;
/// The longest bus name or object path, in bytes (the bus's own limit).
pub const MAX_ID_BYTES: usize = 255;
/// Pixels of scroll that make one step.
pub const SCROLL_PX_PER_STEP: i32 = 15;
/// The icon an item shows when its own cannot be used.
pub const FALLBACK_ICON: &str = "app-window";
/// The object path an item registered by bare name is served at.
pub const DEFAULT_ITEM_PATH: &str = "/StatusNotifierItem";

/// Why a registration string does not name an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ItemIdError {
    /// The bus name part is empty, too long or has control characters.
    #[error("the item's bus name is empty, too long or malformed")]
    BusName,
    /// The object path part is not a valid, bounded D-Bus object path.
    #[error("the item's object path is not a valid object path")]
    Path,
}

/// A status notifier item: the bus name that serves it and its object path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ItemId {
    /// The unique or well-known bus name.
    pub bus_name: String,
    /// The object path, always starting with `/`.
    pub path: String,
}

impl ItemId {
    /// Reads a registration string as `RegisterStatusNotifierItem` receives it:
    /// `name`, `name/path` or `:1.23/org/ayatana/...`. A bare name is served at
    /// [`DEFAULT_ITEM_PATH`].
    ///
    /// # Errors
    /// [`ItemIdError`] when either part is empty where it must not be, longer
    /// than [`MAX_ID_BYTES`], or malformed.
    pub fn parse(registration: &str) -> Result<Self, ItemIdError> {
        let (name, path) = match registration.find('/') {
            Some(at) => (&registration[..at], &registration[at..]),
            None => (registration, DEFAULT_ITEM_PATH),
        };
        if name.is_empty()
            || name.len() > MAX_ID_BYTES
            || name.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(ItemIdError::BusName);
        }
        if !valid_object_path(path) {
            return Err(ItemIdError::Path);
        }
        Ok(Self {
            bus_name: name.to_owned(),
            path: path.to_owned(),
        })
    }
}

/// Whether `path` is a D-Bus object path of at most [`MAX_ID_BYTES`].
fn valid_object_path(path: &str) -> bool {
    if path.len() > MAX_ID_BYTES || !path.starts_with('/') {
        return false;
    }
    if path == "/" {
        return true;
    }
    path[1..]
        .split('/')
        .all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// An item's `Status` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// Nothing to show; the item may hide.
    Passive,
    /// The normal state.
    #[default]
    Active,
    /// The item wants the person's attention.
    NeedsAttention,
}

/// A bounded ARGB32 (network byte order) pixmap an item published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    /// Width in pixels, 1..=[`MAX_PIXMAP_SIDE`].
    pub w: u32,
    /// Height in pixels, 1..=[`MAX_PIXMAP_SIDE`].
    pub h: u32,
    /// `w * h * 4` bytes.
    pub argb: Vec<u8>,
}

/// One status notifier item the Tray page lists. It carries both ways the
/// item describes its icon, so the page can draw the theme icon when the
/// theme has it and fall back to the pixmap when it does not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayItem {
    /// Who serves it.
    pub id: ItemId,
    /// Its title, at most [`MAX_TEXT_CHARS`] characters.
    pub title: String,
    /// Its status.
    pub status: Status,
    /// Its `IconName`, bounded; empty when it has none.
    pub icon_name: String,
    /// The best of its `IconPixmap`s for the page's size, when it has one.
    pub pixmap: Option<Pixmap>,
    /// Whether it serves a `dbusmenu`.
    pub has_menu: bool,
}

/// Who holds a bus name the shell wanted: the unique name of the connection
/// and, when `/proc` names it, the process's `comm`. The Tray page's copy
/// composes the line a person reads from these.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameHolder {
    /// The unique name of the holder's connection.
    pub unique: String,
    /// The holder's process name, when known.
    pub comm: Option<String>,
}

/// One level of a menu: the layout node `id` and its entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuTree {
    /// The `dbusmenu` id of the node this level hangs from.
    pub id: i32,
    /// Its entries in order.
    pub entries: Vec<MenuEntry>,
}

/// One entry of a menu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuEntry {
    /// The `dbusmenu` id events are sent with.
    pub id: i32,
    /// The label without mnemonics, at most [`MAX_LABEL_CHARS`] characters.
    pub label: String,
    /// Whether the entry can be activated.
    pub enabled: bool,
    /// `Some(checked)` for a check or radio entry.
    pub toggle: Option<bool>,
    /// The entry's submenu, when it has one.
    pub submenu: Option<MenuTree>,
    /// Whether the entry is a separator line.
    pub separator: bool,
}

/// A scroll axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Up and down.
    Vertical,
    /// Left and right.
    Horizontal,
}

/// What the tray host publishes to the event loop.
#[derive(Clone, Debug, PartialEq)]
pub enum TrayEvent {
    /// Who holds the watcher name, or `None` when the shell holds it or
    /// nobody does.
    Owner(Option<NameHolder>),
    /// The whole item list, after a change.
    Items(Vec<TrayItem>),
    /// An item's menu, after it was asked for.
    Menu(ItemId, MenuTree),
}

/// What the event loop asks of the tray host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    /// A primary activation.
    Activate(ItemId),
    /// A secondary activation (middle click).
    SecondaryActivate(ItemId),
    /// A scroll of whole steps on an axis.
    Scroll(ItemId, i32, Orientation),
    /// A click on a menu entry.
    MenuEvent(ItemId, i32),
    /// Fetch the item's menu.
    OpenMenu(ItemId),
    /// A submenu entry of the item's menu is about to be shown; the menu
    /// is published again when the item says its layout changed.
    AboutToShow(ItemId, i32),
    /// The item's menu closed: its updates are not followed any more.
    CloseMenu(ItemId),
}

/// Whether `c` is a Unicode format character (general category `Cf`): the
/// bidi controls (`U+202E` reverses what follows), zero-width joiners and
/// spaces, soft hyphens, the byte order mark, interlinear annotations and the
/// tag characters. The ranges are those of Unicode 15; a peer's text must
/// not steer how the page lays out what comes after it.
fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

/// Whether `c` is dropped from text a peer sent: a control or a format
/// character.
fn is_dropped(c: char) -> bool {
    c.is_control() || is_format(c)
}

/// `text` without control and format characters and cut to `max`
/// characters.
#[must_use]
pub fn bound_text(text: &str, max: usize) -> String {
    text.chars().filter(|c| !is_dropped(*c)).take(max).collect()
}

/// A menu label for display: the mnemonic marker `_` removed (`__` is a
/// literal underscore), control and format characters dropped, cut to the
/// limit.
fn clean_label(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                chars.next();
                out.push('_');
            }
            continue;
        }
        if !is_dropped(c) {
            out.push(c);
        }
    }
    out.chars().take(MAX_LABEL_CHARS).collect()
}

/// Whether a pixmap of `w` x `h` with `len` bytes is usable.
fn pixmap_ok(w: i32, h: i32, len: usize) -> Option<(u32, u32)> {
    let w = u32::try_from(w).ok()?;
    let h = u32::try_from(h).ok()?;
    if w == 0 || h == 0 || w > MAX_PIXMAP_SIDE || h > MAX_PIXMAP_SIDE {
        return None;
    }
    let want = usize::try_from(w)
        .ok()?
        .checked_mul(usize::try_from(h).ok()?)?
        .checked_mul(4)?;
    (len == want).then_some((w, h))
}

/// The best usable pixmap of `candidates` (`width`, `height`, ARGB bytes) for
/// an icon drawn `want` pixels square: the smallest one at least that large,
/// else the largest. Oversized and inconsistent ones are rejected, so a peer
/// cannot make the shell hold a huge image.
#[must_use]
pub fn choose_pixmap(candidates: &[(i32, i32, Vec<u8>)], want: u32) -> Option<Pixmap> {
    let mut best: Option<(u32, u32, &Vec<u8>)> = None;
    for (w, h, bytes) in candidates {
        let Some((w, h)) = pixmap_ok(*w, *h, bytes.len()) else {
            continue;
        };
        let side = w.max(h);
        let better = match best {
            None => true,
            Some((bw, bh, _)) => {
                let best_side = bw.max(bh);
                if best_side >= want {
                    side >= want && side < best_side
                } else {
                    side > best_side
                }
            }
        };
        if better {
            best = Some((w, h, bytes));
        }
    }
    best.map(|(w, h, bytes)| Pixmap {
        w,
        h,
        argb: bytes.clone(),
    })
}

/// `raw` with every bound applied: depth at most [`MAX_MENU_DEPTH`], at most
/// [`MAX_MENU_ENTRIES`] per level and [`MAX_MENU_TOTAL`] in all, labels clean
/// and short.
#[must_use]
pub fn bound_menu(raw: MenuTree) -> MenuTree {
    let mut budget = MAX_MENU_TOTAL;
    bound_level(raw, 1, &mut budget)
}

fn bound_level(raw: MenuTree, depth: usize, budget: &mut usize) -> MenuTree {
    let mut entries = Vec::new();
    for entry in raw.entries {
        if entries.len() >= MAX_MENU_ENTRIES || *budget == 0 {
            break;
        }
        *budget -= 1;
        let submenu = match entry.submenu {
            Some(sub) if depth < MAX_MENU_DEPTH => Some(bound_level(sub, depth + 1, budget)),
            _ => None,
        };
        entries.push(MenuEntry {
            label: clean_label(&entry.label),
            submenu,
            ..entry
        });
    }
    MenuTree {
        id: raw.id,
        entries,
    }
}

/// `items` with at most [`MAX_ITEMS`] kept and each one's text bounded.
#[must_use]
pub fn bound_items(items: Vec<TrayItem>) -> Vec<TrayItem> {
    items
        .into_iter()
        .take(MAX_ITEMS)
        .map(|item| TrayItem {
            title: bound_text(&item.title, MAX_TEXT_CHARS),
            icon_name: bound_text(&item.icon_name, MAX_TEXT_CHARS),
            ..item
        })
        .collect()
}

/// Accumulates scroll `delta_px` into whole steps (positive or negative), one
/// per [`SCROLL_PX_PER_STEP`] pixels, keeping the rest in `remainder` for the
/// next call.
pub fn scroll_steps(delta_px: i32, remainder: &mut i32) -> i32 {
    let total = remainder.saturating_add(delta_px);
    *remainder = total % SCROLL_PX_PER_STEP;
    total / SCROLL_PX_PER_STEP
}

/// Holds the latest item list and releases it at most once per coalescing
/// window; the last list is never dropped, only delayed.
#[derive(Debug, Default)]
pub struct ItemsCoalescer {
    coalescer: Coalescer,
    latest: Option<Vec<TrayItem>>,
}

impl ItemsCoalescer {
    /// Records `items` as the newest list.
    pub fn push(&mut self, items: Vec<TrayItem>) {
        self.latest = Some(items);
        self.coalescer.mark();
    }

    /// The list to publish at `now_ms`, if one is owed and the window allows.
    pub fn take_due(&mut self, now_ms: u64) -> Option<Vec<TrayItem>> {
        if self.latest.is_none() || !self.coalescer.due(now_ms) {
            return None;
        }
        self.coalescer.emitted(now_ms);
        self.latest.take()
    }

    /// How long a worker may sleep before it must look again; `None` when
    /// nothing is owed.
    #[must_use]
    pub fn wait_ms(&self, now_ms: u64) -> Option<u64> {
        self.latest.as_ref()?;
        self.coalescer.wait_ms(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(n: usize) -> TrayItem {
        TrayItem {
            title: format!("item {n}"),
            ..TrayItem::default()
        }
    }

    fn entry(label: &str, submenu: Option<MenuTree>) -> MenuEntry {
        MenuEntry {
            id: 1,
            label: label.to_owned(),
            enabled: true,
            submenu,
            ..MenuEntry::default()
        }
    }

    #[test]
    fn items_are_coalesced_to_one_update_per_100ms() {
        let mut c = ItemsCoalescer::default();
        c.push(vec![item(0)]);
        assert_eq!(c.take_due(0), Some(vec![item(0)]));
        c.push(vec![item(1)]);
        c.push(vec![item(2)]);
        c.push(vec![item(3)]);
        assert_eq!(c.take_due(50), None);
        assert_eq!(c.wait_ms(50), Some(50));
        assert_eq!(c.take_due(100), Some(vec![item(3)]));
        assert_eq!(c.take_due(1000), None);
        assert_eq!(c.wait_ms(1000), None);
    }

    #[test]
    fn pixmap_bounds_reject_more_than_256_square() {
        let huge = (4096, 4096, vec![0u8; 16]);
        assert_eq!(choose_pixmap(std::slice::from_ref(&huge), 22), None);
        let edge = (256, 256, vec![0u8; 256 * 256 * 4]);
        assert!(matches!(
            choose_pixmap(&[edge], 22),
            Some(Pixmap { w: 256, h: 256, .. })
        ));
        let over = (257, 1, vec![0u8; 257 * 4]);
        assert_eq!(choose_pixmap(&[over], 22), None);
    }

    #[test]
    fn pixmap_with_inconsistent_length_or_size_is_rejected() {
        assert_eq!(choose_pixmap(&[(2, 2, vec![0; 15])], 22), None);
        assert_eq!(choose_pixmap(&[(0, 2, vec![])], 22), None);
        assert_eq!(choose_pixmap(&[(-1, 2, vec![0; 8])], 22), None);
    }

    #[test]
    fn the_best_pixmap_is_the_smallest_that_covers_the_size_else_the_largest() {
        let px = |s: i32| (s, s, vec![0u8; (s * s * 4) as usize]);
        let side = |pixmap: Option<Pixmap>| pixmap.map_or(0, |p| p.w);
        let all = [px(16), px(48), px(32), px(64)];
        assert_eq!(side(choose_pixmap(&all, 24)), 32);
        assert_eq!(side(choose_pixmap(&all, 100)), 64);
        assert_eq!(side(choose_pixmap(&all, 8)), 16);
    }

    #[test]
    fn an_items_icon_name_is_bounded_with_its_title() {
        let long = "x".repeat(500);
        let bounded = bound_items(vec![TrayItem {
            icon_name: long,
            ..TrayItem::default()
        }]);
        assert_eq!(bounded[0].icon_name.chars().count(), MAX_TEXT_CHARS);
        assert_eq!(bounded[0].pixmap, None);
    }

    #[test]
    fn format_characters_are_dropped_from_titles_and_labels() {
        // A bidi override, a zero-width space, a soft hyphen, the byte order
        // mark and a tag character go; letters and marks stay.
        let hostile = "a\u{202E}b\u{200B}c\u{00AD}d\u{FEFF}e\u{E0041}f\u{0301}";
        assert_eq!(bound_text(hostile, 100), "abcdef\u{0301}");
        let tree = MenuTree {
            id: 0,
            entries: vec![entry("_O\u{202E}pen\u{2066}", None)],
        };
        assert_eq!(bound_menu(tree).entries[0].label, "Open");
    }

    #[test]
    fn menu_tree_depth_and_width_are_bounded() {
        // Seven levels deep, one entry per level.
        let mut tree = MenuTree {
            id: 7,
            entries: vec![entry("leaf", None)],
        };
        for _ in 0..6 {
            tree = MenuTree {
                id: 0,
                entries: vec![entry("n", Some(tree))],
            };
        }
        let bounded = bound_menu(tree);
        let mut depth = 1;
        let mut level = &bounded;
        while let Some(sub) = level.entries.first().and_then(|e| e.submenu.as_ref()) {
            depth += 1;
            level = sub;
        }
        assert_eq!(depth, MAX_MENU_DEPTH);

        let wide = MenuTree {
            id: 0,
            entries: (0..200).map(|_| entry("e", None)).collect(),
        };
        assert_eq!(bound_menu(wide).entries.len(), MAX_MENU_ENTRIES);

        let label = MenuTree {
            id: 0,
            entries: vec![entry(&"a".repeat(1000), None)],
        };
        assert_eq!(
            bound_menu(label).entries[0].label.chars().count(),
            MAX_LABEL_CHARS
        );
    }

    #[test]
    fn menu_total_entries_are_bounded_across_levels() {
        let level = |n: usize, sub: Option<MenuTree>| MenuTree {
            id: 0,
            entries: (0..n)
                .map(|i| entry("e", if i == 0 { sub.clone() } else { None }))
                .collect(),
        };
        let mut tree = level(64, None);
        for _ in 0..3 {
            tree = level(64, Some(tree));
        }
        let b = bound_menu(tree);
        fn count(t: &MenuTree) -> usize {
            t.entries
                .iter()
                .map(|e| 1 + e.submenu.as_ref().map_or(0, count))
                .sum()
        }
        assert!(count(&b) <= MAX_MENU_TOTAL);
    }

    #[test]
    fn labels_lose_mnemonics_and_control_characters() {
        let tree = MenuTree {
            id: 0,
            entries: vec![
                entry("_Open", None),
                entry("Save __as", None),
                entry("bad\u{7}\n\u{1b}[31mtext", None),
            ],
        };
        let labels: Vec<_> = bound_menu(tree)
            .entries
            .into_iter()
            .map(|e| e.label)
            .collect();
        assert_eq!(labels, ["Open", "Save _as", "bad[31mtext"]);
    }

    #[test]
    fn at_most_64_items_with_bounded_titles_are_kept() {
        let mut items: Vec<_> = (0..100).map(item).collect();
        items[0].title = format!("{}\u{7}", "t".repeat(400));
        let bounded = bound_items(items);
        assert_eq!(bounded.len(), MAX_ITEMS);
        assert_eq!(bounded[0].title.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn item_ids_parse_from_registration_strings() {
        assert_eq!(
            ItemId::parse("org.kde.foo"),
            Ok(ItemId {
                bus_name: "org.kde.foo".into(),
                path: DEFAULT_ITEM_PATH.into()
            })
        );
        assert_eq!(
            ItemId::parse(":1.23/org/ayatana/NotificationItem/x"),
            Ok(ItemId {
                bus_name: ":1.23".into(),
                path: "/org/ayatana/NotificationItem/x".into()
            })
        );
        assert_eq!(
            ItemId::parse("name/StatusNotifierItem").map(|i| i.path),
            Ok("/StatusNotifierItem".to_owned())
        );
    }

    #[test]
    fn malformed_or_oversized_item_ids_are_rejected() {
        assert_eq!(ItemId::parse(""), Err(ItemIdError::BusName));
        assert_eq!(ItemId::parse("/path"), Err(ItemIdError::BusName));
        assert_eq!(ItemId::parse("a b/x"), Err(ItemIdError::BusName));
        assert_eq!(ItemId::parse(&"n".repeat(256)), Err(ItemIdError::BusName));
        assert_eq!(ItemId::parse("n/a//b"), Err(ItemIdError::Path));
        assert_eq!(ItemId::parse("n/a/"), Err(ItemIdError::Path));
        assert_eq!(ItemId::parse("n/a-b"), Err(ItemIdError::Path));
        assert_eq!(
            ItemId::parse(&format!("n/{}", "p".repeat(300))),
            Err(ItemIdError::Path)
        );
    }

    #[test]
    fn scroll_accumulates_15_px_per_step_and_keeps_the_remainder() {
        let mut rem = 0;
        assert_eq!(scroll_steps(10, &mut rem), 0);
        assert_eq!(rem, 10);
        assert_eq!(scroll_steps(10, &mut rem), 1);
        assert_eq!(rem, 5);
        assert_eq!(scroll_steps(-50, &mut rem), -3);
        assert_eq!(rem, 0);
        assert_eq!(scroll_steps(-20, &mut rem), -1);
        assert_eq!(rem, -5);
        assert!(scroll_steps(i32::MAX, &mut rem) > 0);
    }
}
