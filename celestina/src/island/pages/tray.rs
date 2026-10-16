//! The Tray page (design §5.5). This unit shows only why the page is
//! empty: the explanation line while another program holds the status
//! notifier watcher name, else the empty state. The items arrive with the
//! tray host (CEL-1-G).

use celestina_ui::{view, Key, View};

use super::{note_list, PageCx, PageView};
use crate::{copy, island::state::PAGE_SIZE};

/// The Tray page.
#[derive(Clone, Copy, Debug)]
pub struct Tray;

impl PageView for Tray {
    fn title(&self) -> &str {
        copy::PAGE_TRAY
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let line = cx
            .owners
            .tray
            .as_deref()
            .map_or_else(|| copy::TRAY_EMPTY.to_owned(), copy::tray_owner);
        view::col(Key::named("tray")).child(note_list(theme, Key::named("tray-rows"), &line))
    }

    fn wants(&self, _: &PageCx<'_>) -> (f32, f32) {
        PAGE_SIZE
    }
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::wants_of;
    use crate::island::pages::NameOwner;

    fn page_for(owners: &NameOwner) -> View {
        Tray.view(&PageCx {
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
        })
    }

    fn only_row_text(v: &View) -> Option<String> {
        let rows = &v.children[0];
        assert_eq!(rows.children.len(), 1, "exactly one row");
        match &rows.children[0].children[0].kind {
            ViewKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    #[test]
    fn tray_page_shows_the_owner_line_while_another_program_holds_the_name() {
        let owner = "org.kde.StatusNotifierWatcher is held by :1.42 (noctalia)";
        let owners = NameOwner {
            notifications: None,
            tray: Some(owner.to_owned()),
        };
        let line = only_row_text(&page_for(&owners));
        assert_eq!(line, Some(copy::tray_owner(owner)));
        assert!(line.is_some_and(|l| l.contains(owner)));
    }

    #[test]
    fn tray_page_without_an_owner_shows_the_empty_state() {
        assert_eq!(
            only_row_text(&page_for(&NameOwner::default())),
            Some(copy::TRAY_EMPTY.to_owned())
        );
    }

    #[test]
    fn the_page_asks_for_the_default_size() {
        assert_eq!(wants_of(&Tray, None, (760.0, 560.0)), PAGE_SIZE);
        assert_eq!(Tray.title(), copy::PAGE_TRAY);
    }
}
