//! The Notifications page (design §5.5). This unit shows only why the page
//! is empty: the explanation line while another program holds the
//! notifications name, else the empty state. The do-not-disturb switch is
//! drawn but disabled until the notification server (CEL-1-F) exists.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, switch, text_style},
    },
};

/// The Notifications page.
#[derive(Clone, Copy, Debug)]
pub struct Notifications;

/// The key of the do-not-disturb switch.
#[must_use]
pub fn switch_key() -> Key {
    Key::named("dnd-switch")
}

impl PageView for Notifications {
    fn title(&self) -> &str {
        copy::PAGE_NOTIFICATIONS
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let line = cx.owners.notifications.as_deref().map_or_else(
            || copy::NOTIFICATIONS_EMPTY.to_owned(),
            copy::notifications_owner,
        );
        view::col(Key::named("notifications"))
            .gap(theme.spacing.sm)
            .child(disabled_switch(theme, cx.do_not_disturb))
            .child(note_list(theme, Key::named("notifications-rows"), &line))
    }

    fn wants(&self, _: &PageCx<'_>) -> (f32, f32) {
        PAGE_SIZE
    }
}

/// The do-not-disturb switch in the state `on` in force, out of the focus
/// order and hit testing, in the faint roles: the overflow menu changes the
/// setting; the page's own switch stays disabled until the server that
/// would honour it exists (CEL-1-F), and while another program holds the
/// notifications name.
fn disabled_switch(theme: &Theme, on: bool) -> View {
    let track = switch(Key::named("dnd-track"), on, false);
    centred(view::row(switch_key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .child(
            view::text(
                Key::named("dnd-label"),
                copy::DO_NOT_DISTURB,
                text_style(
                    theme.type_scale.body,
                    Weight::Regular,
                    false,
                    Role::TextFaint,
                ),
            )
            .grow(),
        )
        .child(track)
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::wants_of;
    use crate::island::pages::NameOwner;

    fn page_for(owners: &NameOwner) -> View {
        Notifications.view(&PageCx {
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

    fn text_of(v: &View) -> Option<&str> {
        match &v.kind {
            ViewKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    #[test]
    fn notifications_page_shows_the_owner_line_while_noctalia_holds_the_name() {
        let owner = "org.freedesktop.Notifications is held by :1.42 (noctalia)";
        let owners = NameOwner {
            notifications: Some(owner.to_owned()),
            tray: None,
        };
        let v = page_for(&owners);
        let rows = &v.children[1];
        assert_eq!(rows.key, Key::named("notifications-rows"));
        assert_eq!(rows.children.len(), 1, "exactly one row");
        let line = text_of(&rows.children[0].children[0]);
        assert_eq!(line, Some(copy::notifications_owner(owner).as_str()));
        assert!(line.is_some_and(|l| l.contains(owner)));
        // The switch is there, off the focus order and the hit test.
        let sw = &v.children[0];
        assert_eq!(sw.key, switch_key());
        assert!(!sw.interactive && !sw.focusable, "disabled");
        assert_eq!(text_of(&sw.children[0]), Some(copy::DO_NOT_DISTURB));
    }

    fn track_of(page: &View) -> &View {
        &page.children[0].children[1]
    }

    #[test]
    fn the_switch_shows_the_do_not_disturb_setting_in_force() {
        let theme = celestina_paint::Theme::dark();
        let held = NameOwner {
            notifications: Some(":1.42 (noctalia)".to_owned()),
            tray: None,
        };
        for owners in [NameOwner::default(), held] {
            for on in [false, true] {
                let v = Notifications.view(&PageCx {
                    theme: &theme,
                    max_size: (760.0, 560.0),
                    model: None,
                    output: "DP-1",
                    owners: &owners,
                    do_not_disturb: on,
                    history: &[],
                    items: &[],
                });
                assert_eq!(
                    *track_of(&v),
                    switch(Key::named("dnd-track"), on, false),
                    "on {on}, owners {owners:?}"
                );
                assert!(!v.children[0].interactive && !v.children[0].focusable);
            }
        }
    }

    #[test]
    fn notifications_page_without_an_owner_shows_the_empty_state() {
        let v = page_for(&NameOwner::default());
        let rows = &v.children[1];
        assert_eq!(rows.children.len(), 1);
        assert_eq!(
            text_of(&rows.children[0].children[0]),
            Some(copy::NOTIFICATIONS_EMPTY)
        );
        assert!(!v.children[0].interactive && !v.children[0].focusable);
    }

    #[test]
    fn the_page_asks_for_the_default_size() {
        assert_eq!(wants_of(&Notifications, None, (760.0, 560.0)), PAGE_SIZE);
        assert_eq!(Notifications.title(), copy::PAGE_NOTIFICATIONS);
    }
}
