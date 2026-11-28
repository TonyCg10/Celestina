//! The rows of a tray item's menu (design §5.6): the shell's own entry, the
//! row of a level (an app's entry, a separator line or the shell's entry), the
//! views of those rows and the keys they carry. [`super::pane`] puts them in
//! a scrolling pane and the capsule menu windows hold the pane; this module
//! draws no window, takes no input and talks to no host.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use unicode_segmentation::UnicodeSegmentation;

use super::MenuTree;
use crate::copy;
use crate::island::view::{centred, text_style};

/// The one entry the shell adds to the end of a tray app's menu; it is not
/// the app's and the app never hears of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellEntry {
    /// Hide the app from the bar (it is in the bar now).
    Hide,
    /// Show the app in the bar again (it is hidden now).
    Show,
}

impl ShellEntry {
    /// The entry's label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            ShellEntry::Hide => copy::TRAY_HIDE,
            ShellEntry::Show => copy::TRAY_SHOW,
        }
    }

    /// The Lucide glyph in the check column.
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            ShellEntry::Hide => "eye-off",
            ShellEntry::Show => "eye",
        }
    }
}

/// One row of a level: an entry of the app's menu, a separator line or the
/// shell's own entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuRow<'a> {
    /// An entry of the app's menu.
    Entry(&'a super::MenuEntry),
    /// A separator line, the app's or the one before the shell's entry.
    Separator,
    /// The shell's entry.
    Shell(ShellEntry),
}

/// The row at `index` of `level` followed, when `shell` is `Some`, by a
/// separator (unless the level is empty or already ends in one) and the
/// shell's entry.
pub(crate) fn row_at(
    level: &MenuTree,
    shell: Option<ShellEntry>,
    index: usize,
) -> Option<MenuRow<'_>> {
    let own = level.entries.len();
    if let Some(entry) = level.entries.get(index) {
        return Some(if entry.separator {
            MenuRow::Separator
        } else {
            MenuRow::Entry(entry)
        });
    }
    let line = own > 0 && level.entries.last().is_some_and(|e| !e.separator);
    let entry = shell?;
    if line && index == own {
        Some(MenuRow::Separator)
    } else if index == own + usize::from(line) {
        Some(MenuRow::Shell(entry))
    } else {
        None
    }
}

/// Every row of `level`, the shell's entry included.
pub(crate) fn rows(
    level: &MenuTree,
    shell: Option<ShellEntry>,
) -> impl Iterator<Item = MenuRow<'_>> {
    (0..).map_while(move |index| row_at(level, shell, index))
}

/// How many rows `level` has, the shell's entry included.
pub(crate) fn row_count(level: &MenuTree, shell: Option<ShellEntry>) -> usize {
    rows(level, shell).count()
}

/// The level of `tree` at `path`: the submenu of the entry with each id in
/// turn, from the root. `None` when the path leads nowhere.
#[must_use]
pub fn level_at<'a>(tree: &'a MenuTree, path: &[i32]) -> Option<&'a MenuTree> {
    let mut level = tree;
    for id in path {
        level = level
            .entries
            .iter()
            .find(|e| e.id == *id)?
            .submenu
            .as_ref()?;
    }
    Some(level)
}

/// A menu level's width, logical px.
pub const MENU_W: f32 = 300.0;
/// The longest label shown, in grapheme clusters, the ellipsis included.
pub const MAX_LABEL_GRAPHEMES: usize = 36;
/// The check mark's and the chevron's size.
pub(crate) const MARK: f32 = 16.0;

pub(crate) const XKB_KEY_TAB: u32 = 0xff09;
pub(crate) const XKB_KEY_LEFT: u32 = 0xff51;
pub(crate) const XKB_KEY_UP: u32 = 0xff52;
pub(crate) const XKB_KEY_RIGHT: u32 = 0xff53;
pub(crate) const XKB_KEY_DOWN: u32 = 0xff54;
pub(crate) const XKB_KEY_ESCAPE: u32 = 0xff1b;

/// The row of the entry at `index` of the level (separators included).
#[must_use]
pub fn entry_key(index: usize) -> Key {
    Key::indexed("tray-menu-entry", index as u64)
}

/// The height of a separator row.
pub(crate) fn separator_h(theme: &Theme) -> f32 {
    2.0 * theme.spacing.sm + 1.0
}

/// `label` cut to [`MAX_LABEL_GRAPHEMES`] grapheme clusters with an ellipsis.
pub(crate) fn short(label: &str) -> String {
    let kept: Vec<&str> = label
        .graphemes(true)
        .take(MAX_LABEL_GRAPHEMES + 1)
        .collect();
    if kept.len() <= MAX_LABEL_GRAPHEMES {
        return label.to_owned();
    }
    let mut cut = kept[..MAX_LABEL_GRAPHEMES - 1].concat();
    cut.push('\u{2026}');
    cut
}

pub(crate) fn separator(theme: &Theme, index: usize) -> View {
    centred(view::row(entry_key(index)))
        .height(separator_h(theme))
        .padding_xy(theme.spacing.md, 0.0)
        .child(
            view::row(Key::indexed("tray-menu-line", index as u64))
                .height(1.0)
                .fill(Role::Divider)
                .grow(),
        )
}

/// The row of an entry showing `label`: the entry's label, cut ([`short`])
/// and, in a narrow card, cut again to the room the row leaves it.
pub(crate) fn entry_row_labelled(
    theme: &Theme,
    index: usize,
    entry: &super::MenuEntry,
    label: &str,
) -> View {
    let role = if entry.enabled {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let at = index as u64;
    let mark = if entry.toggle == Some(true) {
        view::icon(Key::indexed("tray-menu-check", at), "check", MARK).tint(role)
    } else {
        view::row(Key::indexed("tray-menu-check", at)).size(MARK, MARK)
    };
    let mut row = centred(view::row(entry_key(index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(mark)
        .child(
            view::text(
                Key::indexed("tray-menu-label", at),
                label,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .grow(),
        );
    if entry.submenu.is_some() {
        row = row.child(
            view::icon(Key::indexed("tray-menu-more", at), "chevron-right", MARK).tint(role),
        );
    }
    if entry.enabled {
        row = row.interactive().focusable();
    }
    row
}

/// The shell's own entry: its glyph in the check column, then its label.
pub(crate) fn shell_row(theme: &Theme, index: usize, entry: ShellEntry) -> View {
    let at = index as u64;
    centred(view::row(entry_key(index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(
            view::icon(Key::indexed("tray-menu-check", at), entry.glyph(), MARK).tint(Role::TextHi),
        )
        .child(
            view::text(
                Key::indexed("tray-menu-label", at),
                entry.label(),
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
            )
            .grow(),
        )
        .interactive()
        .focusable()
}

/// The index of the row whose key is `key` among the `count` rows of a level.
#[must_use]
pub fn index_of(count: usize, key: Key) -> Option<usize> {
    (0..count).find(|&i| entry_key(i) == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::MenuEntry;

    fn entry(id: i32, label: &str) -> MenuEntry {
        MenuEntry {
            id,
            label: label.to_owned(),
            enabled: true,
            ..MenuEntry::default()
        }
    }

    /// An entry, a separator and an entry with a submenu of one entry.
    fn level() -> MenuTree {
        MenuTree {
            id: 0,
            entries: vec![
                entry(1, "Open"),
                MenuEntry {
                    separator: true,
                    enabled: true,
                    ..MenuEntry::default()
                },
                MenuEntry {
                    submenu: Some(MenuTree {
                        id: 5,
                        entries: vec![entry(51, "Inner")],
                    }),
                    ..entry(5, "More")
                },
            ],
        }
    }

    #[test]
    fn a_path_of_entry_ids_finds_the_level() {
        let tree = level();
        assert_eq!(level_at(&tree, &[]), Some(&tree));
        let inner = level_at(&tree, &[5]).expect("the submenu");
        assert_eq!(inner.id, 5);
        assert_eq!(level_at(&tree, &[5, 51]), None, "a leaf has no level");
        assert_eq!(level_at(&tree, &[9]), None);
    }

    #[test]
    fn index_of_finds_only_this_levels_rows() {
        let level = level();
        assert_eq!(index_of(level.entries.len(), entry_key(2)), Some(2));
        assert_eq!(index_of(level.entries.len(), entry_key(3)), None);
        assert_eq!(index_of(level.entries.len(), Key::named("other")), None);
    }

    #[test]
    fn the_shell_entry_ends_the_root_after_one_separator() {
        let tree = level();
        let kinds: Vec<&str> = rows(&tree, Some(ShellEntry::Hide))
            .map(|row| match row {
                MenuRow::Entry(_) => "entry",
                MenuRow::Separator => "line",
                MenuRow::Shell(_) => "shell",
            })
            .collect();
        assert_eq!(kinds, ["entry", "line", "entry", "line", "shell"]);
        assert_eq!(row_count(&tree, Some(ShellEntry::Show)), 5);
        assert_eq!(row_count(&tree, None), 3);
        // An empty level holds the shell's entry alone; a level that ends in
        // a separator of its own adds no second one.
        let empty = MenuTree::default();
        assert_eq!(row_count(&empty, Some(ShellEntry::Hide)), 1);
        assert_eq!(row_count(&empty, None), 0);
        let ends_in_line = MenuTree {
            id: 0,
            entries: vec![
                entry(1, "Open"),
                MenuEntry {
                    separator: true,
                    ..MenuEntry::default()
                },
            ],
        };
        assert_eq!(row_count(&ends_in_line, Some(ShellEntry::Hide)), 3);
    }

    #[test]
    fn labels_are_cut_at_36_graphemes_with_an_ellipsis() {
        assert_eq!(short("Open"), "Open");
        let exact = "x".repeat(MAX_LABEL_GRAPHEMES);
        assert_eq!(short(&exact), exact);
        let cut = short(&"x".repeat(MAX_LABEL_GRAPHEMES + 1));
        assert_eq!(cut.chars().count(), MAX_LABEL_GRAPHEMES);
        assert!(cut.ends_with('\u{2026}'));
    }
}
