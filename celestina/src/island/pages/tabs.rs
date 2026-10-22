//! The row of page tabs under the page's content (design §5.5): one pill per
//! page, the open one selected. A click switches pages through
//! [`crate::island::state::IslandState::switch`].
//!
//! Each pill is as wide as its label plus an equal share of the room left
//! (CEL-2-F): the labels of every page in the default 388 px body do not fit
//! equal pills, but their own widths do at the caption size. When they do
//! not fit (the larger text scales), every pill shrinks with its label
//! clipped inside it, so no pill ever covers another or leaves the row.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::{JustifyContent, LengthPercentageAuto};

use crate::{
    copy,
    island::view::{centred, page_title, text_style},
    pieces::{self, PageId},
};

/// The tab row's height.
pub const TABS_H: f32 = 36.0;

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

/// What the tab says: the page's title, or its short form where the title
/// does not fit the pill. The toolkit has no ellipsis, so a long title is
/// shortened here, in the copy, and the page keeps its full title.
fn tab_label(page: PageId) -> &'static str {
    if page == PageId::NOTIFICATIONS {
        copy::TAB_NOTIFICATIONS
    } else if page == PageId::CLIPBOARD {
        copy::TAB_CLIPBOARD
    } else {
        page_title(page)
    }
}

fn index(page: PageId) -> u64 {
    pieces::page_index(page).map_or(0, |i| i as u64)
}

/// The label's style: the caption size, because "Actividad" at the body size
/// measures 60 px with the real font and every page's tab shares one row of
/// a page 420 px wide. A label wider than its tab (the larger text scales) is
/// clipped by the tab.
fn label_style(theme: &Theme, ink: Role) -> celestina_paint::TextStyle {
    text_style(theme.type_scale.caption, Weight::Semibold, false, ink)
}

/// The pages that have a tab: every page of the piece table except the Phone
/// page, which never has one (CEL-2-J): the row does not fit a tenth tab, and
/// the page opens from the phone piece and from `island open phone`. With it
/// open no tab is selected.
#[must_use]
pub fn shown_pages() -> Vec<PageId> {
    PageId::all()
        .into_iter()
        .filter(|page| *page != PageId::PHONE)
        .collect()
}

/// One tab per shown page ([`shown_pages`]), in the table's order, in a row
/// `width` wide: each as wide as its title plus an equal share of what is
/// left; the open page's is an accent pill, the others lift the control fill.
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
                .child(view::text(
                    Key::indexed("tab-label", index(page)),
                    tab_label(page),
                    label_style(theme, ink),
                ));
            tab.style.justify_content = JustifyContent::CENTER;
            // The title is the pill's basis; past the row's room the pill
            // may shrink below it (the title is clipped) instead of
            // overflowing the row.
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

    /// The tabs' rects and their labels' widths in a row `width` wide,
    /// with `measure`.
    fn tab_rects(
        width: f32,
        measure: fn(&str, &TextStyle, Option<f32>) -> (f32, f32),
    ) -> Vec<(celestina_paint::Rect, f32)> {
        let theme = Theme::dark();
        let mut tree = Tree::new(false, &theme);
        tree.apply(tabs(&theme, PageId::TRAY, width), Instant::now());
        tree.layout_measured((600.0, 100.0), measure);
        shown_pages()
            .into_iter()
            .filter_map(|p| {
                let tab = tree.node_by_key(tab_key(p))?;
                let label = tree.node_by_key(Key::indexed("tab-label", index(p)))?;
                Some((tree.rect(tab), tree.rect(label).w))
            })
            .collect()
    }

    fn assert_in_one_row(rects: &[(celestina_paint::Rect, f32)], width: f32) {
        assert_eq!(rects.len(), shown_pages().len());
        assert!(rects.iter().all(|(r, _)| r.h == TABS_H && r.w >= 0.0));
        for pair in rects.windows(2) {
            assert!(pair[0].0.x + pair[0].0.w <= pair[1].0.x, "{pair:?}");
        }
        let (first, last) = (rects[0].0, rects[rects.len() - 1].0);
        assert!(first.x >= -0.01);
        assert!(last.x + last.w <= width + 0.01, "{last:?} leaves the row");
    }

    #[test]
    fn every_tab_fits_the_default_body_each_wider_than_its_title() {
        // The default page's body (420 - 2 x 16) with every page's label.
        let rects = tab_rects(388.0, fake_measure);
        assert_in_one_row(&rects, 388.0);
        for (rect, label) in &rects {
            assert!(rect.w >= *label, "{rect:?} is narrower than its title");
        }
        // The row is filled: the room left is shared, not left at the end.
        let last = rects[rects.len() - 1].0;
        assert!((last.x + last.w - 388.0).abs() < 0.01);
        // Shared equally: every pill is its title plus the same extra (to
        // the pixel the layout rounds to).
        let extra: Vec<f32> = rects.iter().map(|(r, l)| r.w - l).collect();
        assert!(
            extra.iter().all(|e| (e - extra[0]).abs() <= 1.0),
            "{extra:?}"
        );
    }

    #[test]
    fn titles_too_wide_for_the_row_shrink_the_pills_inside_it() {
        // Nearly three times the width per character, as a much larger text
        // would be.
        fn wide(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
            (14.0 * s.chars().count() as f32, 16.0)
        }
        let rects = tab_rects(388.0, wide);
        assert_in_one_row(&rects, 388.0);
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
    fn the_phone_page_never_has_a_tab_and_opening_it_selects_none() {
        let without_phone: Vec<PageId> = PageId::all()
            .into_iter()
            .filter(|page| *page != PageId::PHONE)
            .collect();
        assert_eq!(shown_pages(), without_phone);
        let theme = Theme::dark();
        let row = tabs(&theme, PageId::PHONE, 388.0);
        let keys: Vec<Key> = row.children.iter().map(|t| t.key).collect();
        let want: Vec<Key> = without_phone.iter().copied().map(tab_key).collect();
        assert_eq!(keys, want);
        assert!(!keys.contains(&tab_key(PageId::PHONE)));
        // With the Phone page open every tab is the unselected fill.
        assert!(row
            .children
            .iter()
            .all(|tab| fill_of(tab) == Some(Role::ControlFill)));
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
        ];
        for (page, key) in want {
            assert_eq!(tab_key(page), key, "{page:?}");
            assert_eq!(page_of(key), Some(page), "{page:?}");
        }
        for foreign in [
            Key::indexed("tab", 10),
            Key::indexed("tab-label", 0),
            Key::named("tab"),
            Key::named("tabs"),
            Key::STALE,
        ] {
            assert_eq!(page_of(foreign), None, "{foreign:?}");
        }
    }

    /// The labels, measured with the real font (Inter Variable from the
    /// checkout) and laid out in the row the page draws, fit their pill at the
    /// compact and the default text scales; at the larger scales the pill
    /// clips instead of letting a label touch its neighbour (VALIDATION: the
    /// larger scales are judged by eye).
    #[test]
    fn no_tab_label_is_wider_than_its_pill() {
        use crate::island::{pages::measure_real, state::PAGE_SIZE};

        for factor in [0.9_f64, 1.0] {
            let theme = Theme::dark().scaled(factor);
            // The row the page draws: the page's width less its padding.
            let width = PAGE_SIZE.0 - 2.0 * theme.spacing.lg;
            for open in shown_pages() {
                let mut tree = Tree::new(false, &theme);
                tree.apply(tabs(&theme, open, width), Instant::now());
                tree.layout_measured((600.0, 100.0), measure_real);
                for page in shown_pages() {
                    let pill = tree
                        .node_by_key(tab_key(page))
                        .map(|id| tree.rect(id))
                        .expect("a pill");
                    let label = tree
                        .node_by_key(Key::indexed("tab-label", index(page)))
                        .map(|id| tree.rect(id))
                        .expect("a label");
                    let want =
                        measure_real(tab_label(page), &label_style(&theme, Role::TextLo), None).0;
                    assert!(
                        want <= pill.w,
                        "{page:?} is {want} px in a {} px pill at {factor}",
                        pill.w
                    );
                    assert!(label.x >= pill.x - 0.5 && label.x + label.w <= pill.x + pill.w + 0.5);
                }
            }
        }
    }

    #[test]
    fn tab_labels_are_the_page_titles_in_tab_order() {
        let row = tabs(&Theme::dark(), PageId::ACTIVITY, 388.0);
        let labels: Vec<(Key, String)> = row
            .children
            .iter()
            .filter_map(|tab| match &tab.children.first()?.kind {
                ViewKind::Text { text, .. } => Some((tab.children[0].key, text.clone())),
                _ => None,
            })
            .collect();
        let want: Vec<(Key, String)> = shown_pages()
            .into_iter()
            .enumerate()
            .map(|(i, page)| {
                (
                    Key::indexed("tab-label", i as u64),
                    tab_label(page).to_owned(),
                )
            })
            .collect();
        assert_eq!(labels, want);
        let names: Vec<&str> = labels.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            names,
            [
                "Actividad",
                "Notific.",
                "Bandeja",
                "Sistema",
                "Reloj",
                "Portap.",
                copy::PAGE_NETWORK,
                copy::PAGE_BLUETOOTH,
                copy::PAGE_POWER
            ]
        );
    }

    #[test]
    fn a_tab_clips_its_label() {
        let row = tabs(&Theme::dark(), PageId::ACTIVITY, 388.0);
        assert!(row.children.iter().all(|tab| tab.clip));
    }
}
