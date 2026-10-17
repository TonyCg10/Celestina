//! The Notifications piece: a bell with a mark while something is unread,
//! and the Notifications page. A click opens the page.

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{pages::notifications::Notifications as Page, view::centred},
};

/// The bell's icon size, logical px.
const BELL: f32 = 18.0;
/// The unread mark's diameter, logical px.
const MARK: f32 = 7.0;
/// The space around the bell that still counts as the bell.
const HIT_PAD: f32 = 5.0;

/// The Notifications piece.
#[derive(Clone, Copy, Debug)]
pub struct Notifications;

fn bell_key() -> Key {
    Key::named("bell")
}

impl Notifications {
    /// The accessible name of the widget built from `cx`: the piece's name
    /// with its state, unread and do-not-disturb.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> &'static str {
        copy::bar_notifications_state(unread(cx), cx.do_not_disturb)
    }
}

fn unread(cx: &BarCx<'_>) -> bool {
    cx.history.iter().any(|row| !row.read)
}

impl Piece for Notifications {
    fn id(&self) -> PieceId {
        PieceId("notifications")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_NOTIFICATIONS
    }

    /// The bell (crossed out while do-not-disturb is on), with the accent
    /// mark at its corner while a notification is unread. Always present.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let name = if cx.do_not_disturb {
            "bell-off"
        } else {
            "bell"
        };
        let icon = view::icon(Key::named("bell-icon"), name, BELL).tint(Role::TextHi);
        let mark = unread(cx).then(|| {
            view::row(Key::named("bell-unread"))
                .fill(Role::Accent)
                .radius(MARK / 2.0)
                .size(MARK, MARK)
                .absolute(HIT_PAD + BELL - MARK, HIT_PAD / 2.0)
        });
        Some(
            centred(view::row(bell_key()))
                .padding(HIT_PAD)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .child(icon)
                .children(mark),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::NOTIFICATIONS,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == bell_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::NOTIFICATIONS),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        island::pages::{HistoryRow, NameOwner},
        pieces::Axis,
    };

    fn row(read: bool) -> HistoryRow {
        HistoryRow {
            read,
            ..HistoryRow::default()
        }
    }

    fn with<R>(history: &[HistoryRow], dnd: bool, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: dnd,
            history,
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
        })
    }

    fn keys(v: &View) -> Vec<Key> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n.key);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    #[test]
    fn the_bell_has_a_mark_only_while_something_is_unread() {
        let bell = Key::named("bell");
        let icon = Key::named("bell-icon");
        let mark = Key::named("bell-unread");
        for (history, expected) in [
            (vec![], vec![bell, icon]),
            (vec![row(true), row(true)], vec![bell, icon]),
            (vec![row(true), row(false)], vec![bell, icon, mark]),
        ] {
            let v = with(&history, false, |cx| Notifications.bar(cx)).expect("bell");
            assert_eq!(keys(&v), expected, "{history:?}");
            assert!(v.interactive && v.focusable);
        }
    }

    #[test]
    fn do_not_disturb_crosses_the_bell_out() {
        let icon_name = |dnd| {
            let v = with(&[], dnd, |cx| Notifications.bar(cx)).expect("bell");
            match &v.children[0].kind {
                ViewKind::Icon {
                    key: celestina_paint::IconKey::Semantic(name),
                    ..
                } => *name,
                other => panic!("not an icon: {other:?}"),
            }
        };
        assert_eq!(icon_name(false), "bell");
        assert_eq!(icon_name(true), "bell-off");
    }

    fn act(key: Key, gesture: Gesture) -> PieceAction {
        with(&[], false, |cx| {
            Notifications.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn a_primary_input_opens_the_page_and_nothing_else_does_anything() {
        assert_eq!(
            act(bell_key(), Gesture::Primary),
            PieceAction::OpenPage(PageId::NOTIFICATIONS)
        );
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Vertical, 2),
            Gesture::Scroll(Axis::Horizontal, -2),
        ] {
            assert_eq!(act(bell_key(), gesture), PieceAction::None);
        }
        assert_eq!(
            act(Key::named("other"), Gesture::Primary),
            PieceAction::None
        );
    }

    #[test]
    fn the_label_carries_the_unread_and_do_not_disturb_state() {
        let labels: Vec<&str> = [(false, false), (true, false), (false, true), (true, true)]
            .into_iter()
            .map(|(unread, dnd)| {
                let history = if unread {
                    vec![row(false)]
                } else {
                    vec![row(true)]
                };
                with(&history, dnd, Notifications::label)
            })
            .collect();
        let distinct: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(distinct.len(), 4, "{labels:?}");
        assert!(labels
            .iter()
            .all(|l| l.starts_with(copy::PAGE_NOTIFICATIONS)));
    }

    #[test]
    fn the_piece_opens_the_notifications_page() {
        assert_eq!(
            Notifications.page().map(|e| e.id),
            Some(PageId::NOTIFICATIONS)
        );
    }
}
