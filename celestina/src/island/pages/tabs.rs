//! The row of page tabs under the page's content (design §5.5): one pill per
//! page, the open one selected. A click switches pages through
//! [`crate::island::state::IslandState::switch`].
//!
//! Each pill shows its page's icon (CEL-D-001; the page's title stays in the
//! title row above), and the pills share the row equally: nine icons fit
//! the narrowest 388 px body at any text scale, so no pill ever covers
//! another or leaves the row.

use celestina_paint::{Role, Theme};
use celestina_ui::{view, Key, View};
use taffy::{JustifyContent, LengthPercentageAuto};

use crate::{
    island::view::{centred, page_icon},
    pieces::{self, PageId},
};

/// The tab row's height.
pub const TABS_H: f32 = 36.0;

/// The icon in each tab, logical px.
pub const TAB_ICON: f32 = 18.0;

/// The tab of `page`.
#[must_use]
pub fn tab_key(page: PageId) -> Key {
    Key::indexed("tab", index(page))
}

/// The page whose tab is `key`, if it is one.
#[must_use]
pub fn page_of(key: Key) -> Option<PageId> {
    pieces::pages()
        .map(|entry| entry.id)
        .find(|page| tab_key(*page) == key)
}

/// The page's place in the table, which keys its tab and its icon.
fn index(page: PageId) -> u64 {
    pieces::page_index(page).map_or(0, |i| i as u64)
}

/// The pages that never have a tab: the Phone and Media pages open from their
/// own pieces.
const TABLESS: [PageId; 2] = [PageId::PHONE, PageId::MEDIA];

/// The pages that have a tab: every page of the piece table except the
/// [`TABLESS`] ones. With one of those open no tab is selected.
#[must_use]
pub fn shown_pages() -> Vec<PageId> {
    PageId::all()
        .into_iter()
        .filter(|page| !TABLESS.contains(page))
        .collect()
}

/// One tab per shown page ([`shown_pages`]), in the table's order, in a row
/// `width` wide, all of equal width with the page's icon in each; the open
/// page's is an accent pill with its icon in accent ink, the others lift the
/// control fill.
#[must_use]
pub fn tabs(theme: &Theme, open: PageId, width: f32) -> View {
    let pages = shown_pages();
    centred(view::row(Key::named("tabs")))
        .size(width, TABS_H)
        .gap(theme.spacing.xs)
        .children(pages.into_iter().map(|page| {
            let (fill, ink) = if page == open {
                (Role::Accent, Role::AccentInk)
            } else {
                (Role::ControlFill, Role::TextLo)
            };
            let mut tab = centred(view::row(tab_key(page)))
                .height(TABS_H)
                .grow()
                .radius(TABS_H / 2.0)
                .fill(fill)
                .clip()
                .interactive()
                .focusable()
                .child(
                    view::icon(
                        Key::indexed("tab-icon", index(page)),
                        page_icon(page),
                        TAB_ICON,
                    )
                    .tint(ink),
                );
            tab.style.justify_content = JustifyContent::CENTER;
            // Pills share the row and may shrink below their content rather
            // than overflow it.
            tab.style.min_size.width = LengthPercentageAuto::length(0.0);
            tab
        }))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::TextStyle;
    use celestina_ui::{Tree, ViewKind};

    use super::*;

    /// A label at the caption size: about 5 px a character with the real
    /// font.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (5.0 * s.chars().count() as f32, 14.0)
    }

    fn fill_of(v: &View) -> Option<Role> {
        match v.kind {
            ViewKind::Box { fill, .. } => fill,
            _ => None,
        }
    }

    #[test]
    fn tabs_show_every_page_with_the_open_one_selected() {
        let theme = Theme::dark();
        for open in shown_pages() {
            let row = tabs(&theme, open, 388.0);
            assert_eq!(row.children.len(), shown_pages().len());
            let keys: Vec<Key> = row.children.iter().map(|t| t.key).collect();
            let want: Vec<Key> = shown_pages().into_iter().map(tab_key).collect();
            assert_eq!(keys, want, "one tab per page, in order");
            for (tab, page) in row.children.iter().zip(shown_pages()) {
                let want = if page == open {
                    Role::Accent
                } else {
                    Role::ControlFill
                };
                assert_eq!(fill_of(tab), Some(want), "{page:?} while {open:?} is open");
                assert!(tab.interactive && tab.focusable);
            }
        }
    }

    /// The tabs' rects in a row `width` wide.
    fn tab_rects(width: f32) -> Vec<celestina_paint::Rect> {
        let theme = Theme::dark();
        let mut tree = Tree::new(false, &theme);
        tree.apply(tabs(&theme, PageId::TRAY, width), Instant::now());
        tree.layout_measured((600.0, 100.0), fake_measure);
        shown_pages()
            .into_iter()
            .filter_map(|p| tree.node_by_key(tab_key(p)).map(|id| tree.rect(id)))
            .collect()
    }

    fn icon_of(v: &View) -> Option<(&'static str, f32, Option<Role>)> {
        match v.kind {
            ViewKind::Icon {
                key: celestina_paint::IconKey::Semantic(name),
                size,
                tint,
            } => Some((name, size, tint)),
            _ => None,
        }
    }

    #[test]
    fn tabs_draw_the_page_icon_not_its_title() {
        let theme = Theme::dark();
        let row = tabs(&theme, PageId::CLOCK, 388.0);
        for (tab, page) in row.children.iter().zip(shown_pages()) {
            assert_eq!(tab.children.len(), 1, "{page:?}: one child");
            let (name, size, _) = icon_of(&tab.children[0]).expect("an icon");
            assert_eq!(name, page_icon(page));
            assert_eq!(size, TAB_ICON);
            assert!(!matches!(tab.children[0].kind, ViewKind::Text { .. }));
        }
    }

    #[test]
    fn the_open_tab_tints_its_icon_accent_ink() {
        let theme = Theme::dark();
        let row = tabs(&theme, PageId::CLOCK, 388.0);
        for (tab, page) in row.children.iter().zip(shown_pages()) {
            let (_, _, tint) = icon_of(&tab.children[0]).expect("an icon");
            let want = if page == PageId::CLOCK {
                Role::AccentInk
            } else {
                Role::TextLo
            };
            assert_eq!(tint, Some(want), "{page:?}");
        }
    }

    #[test]
    fn icon_tabs_share_the_row_equally_and_fit_388() {
        let rects = tab_rects(388.0);
        assert_eq!(rects.len(), shown_pages().len());
        assert!(rects.iter().all(|r| r.h == TABS_H));
        for pair in rects.windows(2) {
            assert!(pair[0].x + pair[0].w <= pair[1].x, "{pair:?}");
        }
        let (first, last) = (rects[0], rects[rects.len() - 1]);
        assert!(first.x >= -0.01);
        assert!((last.x + last.w - 388.0).abs() < 0.01, "{last:?}");
        assert!(
            rects.iter().all(|r| (r.w - rects[0].w).abs() <= 1.0),
            "{rects:?}"
        );
        // Each pill is wider than its icon.
        assert!(rects.iter().all(|r| r.w > TAB_ICON));
    }

    #[test]
    fn tabs_name_their_pages() {
        assert_eq!(
            page_of(tab_key(PageId::NOTIFICATIONS)),
            Some(PageId::NOTIFICATIONS)
        );
        assert_eq!(page_of(Key::named("tab")), None);
    }

    #[test]
    fn the_phone_and_media_pages_never_have_a_tab_and_opening_them_selects_none() {
        let without_phone: Vec<PageId> = PageId::all()
            .into_iter()
            .filter(|page| *page != PageId::PHONE && *page != PageId::MEDIA)
            .collect();
        assert_eq!(shown_pages(), without_phone);
        let theme = Theme::dark();
        for open in [PageId::PHONE, PageId::MEDIA] {
            let row = tabs(&theme, open, 388.0);
            let keys: Vec<Key> = row.children.iter().map(|t| t.key).collect();
            let want: Vec<Key> = without_phone.iter().copied().map(tab_key).collect();
            assert_eq!(keys, want);
            assert!(!keys.contains(&tab_key(PageId::PHONE)));
            assert!(!keys.contains(&tab_key(PageId::MEDIA)));
            // With the Phone or Media page open every tab is the unselected
            // fill.
            assert!(row
                .children
                .iter()
                .all(|tab| fill_of(tab) == Some(Role::ControlFill)));
        }
        let row = tabs(&theme, PageId::PHONE, 388.0);
        // The row still lays out and is filled to its width.
        let last_page = *without_phone.last().expect("pages");
        let mut tree = Tree::new(false, &theme);
        tree.apply(row, Instant::now());
        tree.layout_measured((600.0, 100.0), fake_measure);
        let last = tree
            .node_by_key(tab_key(last_page))
            .map(|id| tree.rect(id))
            .expect("the last tab");
        assert!((last.x + last.w - 388.0).abs() < 0.01);
    }

    // ---- Characterisation (CEL-2-A, spec 6.7): pinned before `Page` becomes a
    // table identifier; they must pass unchanged.

    #[test]
    fn every_page_has_its_indexed_tab_key_and_reads_back() {
        let want = [
            (PageId::ACTIVITY, Key::indexed("tab", 0)),
            (PageId::NOTIFICATIONS, Key::indexed("tab", 1)),
            (PageId::TRAY, Key::indexed("tab", 2)),
            (PageId::SYSMON, Key::indexed("tab", 3)),
            (PageId::CLOCK, Key::indexed("tab", 4)),
            (PageId::CLIPBOARD, Key::indexed("tab", 5)),
            (PageId::NETWORK, Key::indexed("tab", 6)),
            (PageId::BLUETOOTH, Key::indexed("tab", 7)),
            (PageId::POWER, Key::indexed("tab", 8)),
            (PageId::PHONE, Key::indexed("tab", 9)),
            (PageId::MEDIA, Key::indexed("tab", 10)),
        ];
        for (page, key) in want {
            assert_eq!(tab_key(page), key, "{page:?}");
            assert_eq!(page_of(key), Some(page), "{page:?}");
        }
        for foreign in [
            Key::indexed("tab", 11),
            Key::indexed("tab-icon", 0),
            Key::named("tab"),
            Key::named("tabs"),
            Key::STALE,
        ] {
            assert_eq!(page_of(foreign), None, "{foreign:?}");
        }
    }

    #[test]
    fn a_tab_clips_its_icon() {
        let row = tabs(&Theme::dark(), PageId::ACTIVITY, 388.0);
        assert!(row.children.iter().all(|tab| tab.clip));
    }
}
