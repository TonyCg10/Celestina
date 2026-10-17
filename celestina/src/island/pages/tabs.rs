//! The row of page tabs under the page's content (design §5.5): one pill per
//! page, the open one selected. A click switches pages through
//! [`crate::island::state::IslandState::switch`].

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::JustifyContent;

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

/// The label's style: the caption size, because six equal tabs on a page
/// 420 px wide are 58 px each and "Actividad" at the body size measures 60 px
/// with the real font. A label wider than its tab (the larger text scales) is
/// clipped by the tab.
fn label_style(theme: &Theme, ink: Role) -> celestina_paint::TextStyle {
    text_style(theme.type_scale.caption, Weight::Semibold, false, ink)
}

/// One tab per page of the piece table, in its order, in a row `width`
/// wide, equal in width: the open page's is an accent pill, the others lift
/// the control fill.
#[must_use]
pub fn tabs(theme: &Theme, open: PageId, width: f32) -> View {
    let pages = PageId::all();
    let count = pages.len().max(1) as f32;
    let tab_w = ((width - theme.spacing.sm * (count - 1.0)) / count).max(0.0);
    centred(view::row(Key::named("tabs")))
        .size(width, TABS_H)
        .gap(theme.spacing.sm)
        .children(pages.into_iter().map(|page| {
            let (fill, ink) = if page == open {
                (Role::Accent, Role::AccentInk)
            } else {
                (Role::ControlFill, Role::TextLo)
            };
            let mut tab = centred(view::row(tab_key(page)))
                .size(tab_w, TABS_H)
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
            tab
        }))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::TextStyle;
    use celestina_ui::{Tree, ViewKind};

    use super::*;

    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
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
        for open in PageId::all() {
            let row = tabs(&theme, open, 388.0);
            assert_eq!(row.children.len(), PageId::all().len());
            let keys: Vec<Key> = row.children.iter().map(|t| t.key).collect();
            let want: Vec<Key> = PageId::all().into_iter().map(tab_key).collect();
            assert_eq!(keys, want, "one tab per page, in order");
            for (tab, page) in row.children.iter().zip(PageId::all()) {
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

    #[test]
    fn tabs_are_equal_pills_across_the_row_and_name_their_pages() {
        let theme = Theme::dark();
        let mut tree = Tree::new(false, &theme);
        tree.apply(tabs(&theme, PageId::TRAY, 388.0), Instant::now());
        tree.layout_measured((600.0, 100.0), fake_measure);
        let rects: Vec<_> = PageId::all()
            .into_iter()
            .filter_map(|p| tree.node_by_key(tab_key(p)))
            .map(|id| tree.rect(id))
            .collect();
        assert_eq!(rects.len(), PageId::all().len());
        // Layout rounds to whole px, so the tabs agree to within one.
        assert!(rects
            .iter()
            .all(|r| (r.w - rects[0].w).abs() <= 1.0 && r.h == TABS_H));
        for pair in rects.windows(2) {
            assert!(pair[0].x + pair[0].w <= pair[1].x);
        }
        let last = rects.last().expect("tabs");
        assert!((last.x + last.w - 388.0).abs() <= 1.0);
        assert_eq!(
            page_of(tab_key(PageId::NOTIFICATIONS)),
            Some(PageId::NOTIFICATIONS)
        );
        assert_eq!(page_of(Key::named("tab")), None);
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
        ];
        for (page, key) in want {
            assert_eq!(tab_key(page), key, "{page:?}");
            assert_eq!(page_of(key), Some(page), "{page:?}");
        }
        for foreign in [
            Key::indexed("tab", 6),
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
            for open in PageId::all() {
                let mut tree = Tree::new(false, &theme);
                tree.apply(tabs(&theme, open, width), Instant::now());
                tree.layout_measured((600.0, 100.0), measure_real);
                for page in PageId::all() {
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
        let want: Vec<(Key, String)> = PageId::all()
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
                "Portap."
            ]
        );
    }

    #[test]
    fn a_tab_clips_its_label() {
        let row = tabs(&Theme::dark(), PageId::ACTIVITY, 388.0);
        assert!(row.children.iter().all(|tab| tab.clip));
    }
}
