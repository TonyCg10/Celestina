//! The row of page tabs under the page's content (design §5.5): one pill per
//! page, the open one selected. A click switches pages through
//! [`crate::island::state::IslandState::switch`].

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::JustifyContent;

use crate::{
    command::Page,
    island::view::{centred, page_title, text_style},
};

/// The tab row's height.
pub const TABS_H: f32 = 36.0;

/// The tab of `page`.
#[must_use]
pub fn tab_key(page: Page) -> Key {
    Key::indexed("tab", index(page))
}

/// The page whose tab is `key`, if it is one.
#[must_use]
pub fn page_of(key: Key) -> Option<Page> {
    Page::ALL.into_iter().find(|page| tab_key(*page) == key)
}

fn index(page: Page) -> u64 {
    Page::ALL
        .iter()
        .position(|p| *p == page)
        .map_or(0, |i| i as u64)
}

/// The three tabs in a row `width` wide, equal in width: the open page's is
/// an accent pill, the others lift the control fill.
#[must_use]
pub fn tabs(theme: &Theme, open: Page, width: f32) -> View {
    let count = Page::ALL.len() as f32;
    let tab_w = ((width - theme.spacing.sm * (count - 1.0)) / count).max(0.0);
    centred(view::row(Key::named("tabs")))
        .size(width, TABS_H)
        .gap(theme.spacing.sm)
        .children(Page::ALL.into_iter().map(|page| {
            let (fill, ink) = if page == open {
                (Role::Accent, Role::AccentInk)
            } else {
                (Role::ControlFill, Role::TextLo)
            };
            let mut tab = centred(view::row(tab_key(page)))
                .size(tab_w, TABS_H)
                .radius(TABS_H / 2.0)
                .fill(fill)
                .interactive()
                .focusable()
                .child(view::text(
                    Key::indexed("tab-label", index(page)),
                    page_title(page),
                    text_style(theme.type_scale.body, Weight::Semibold, false, ink),
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
    fn tabs_show_three_pages_with_the_open_one_selected() {
        let theme = Theme::dark();
        for open in Page::ALL {
            let row = tabs(&theme, open, 388.0);
            assert_eq!(row.children.len(), 3);
            let keys: Vec<Key> = row.children.iter().map(|t| t.key).collect();
            let want: Vec<Key> = Page::ALL.into_iter().map(tab_key).collect();
            assert_eq!(keys, want, "one tab per page, in order");
            for (tab, page) in row.children.iter().zip(Page::ALL) {
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
        tree.apply(tabs(&theme, Page::Tray, 388.0), Instant::now());
        tree.layout_measured((600.0, 100.0), fake_measure);
        let rects: Vec<_> = Page::ALL
            .into_iter()
            .filter_map(|p| tree.node_by_key(tab_key(p)))
            .map(|id| tree.rect(id))
            .collect();
        assert_eq!(rects.len(), 3);
        assert!(rects.iter().all(|r| r.w == rects[0].w && r.h == TABS_H));
        assert!(rects[0].x + rects[0].w <= rects[1].x);
        assert!(rects[1].x + rects[1].w <= rects[2].x);
        assert_eq!(rects[2].x + rects[2].w, 388.0);
        assert_eq!(
            page_of(tab_key(Page::Notifications)),
            Some(Page::Notifications)
        );
        assert_eq!(page_of(Key::named("tab")), None);
    }
}
