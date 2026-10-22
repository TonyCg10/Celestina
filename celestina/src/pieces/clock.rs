//! The Clock piece: the short date ("vie 17") and the Clock page, which holds
//! the month calendar and the weather reading. A click opens the page. The
//! Island's own capsule clock is unchanged. Without a date the piece is
//! absent.

use celestina_paint::{Role, Weight};
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy, glance,
    island::{
        pages::clock::Clock as Page,
        view::{centred, text_style},
    },
};

/// The space around the text that still counts as the widget.
const HIT_PAD: f32 = 5.0;

/// The Clock piece.
#[derive(Clone, Copy, Debug)]
pub struct Clock;

fn widget_key() -> Key {
    Key::named("clock-date")
}

impl Piece for Clock {
    fn id(&self) -> PieceId {
        PieceId(glance::CLOCK_PIECE)
    }

    fn name(&self) -> &'static str {
        copy::PAGE_CLOCK
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let today = cx.today?;
        Some(
            centred(view::row(widget_key()))
                .padding(HIT_PAD)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .child(view::text(
                    Key::named("clock-date-text"),
                    &copy::short_date(today.weekday, today.day),
                    text_style(
                        cx.theme.type_scale.body,
                        Weight::Regular,
                        true,
                        Role::TextHi,
                    ),
                )),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::CLOCK,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == widget_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::CLOCK),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_shell::glance::Today;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{island::pages::NameOwner, pieces::Axis};

    fn with<R>(today: Option<Today>, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
        })
    }

    /// Saturday 2026-10-17.
    fn saturday() -> Today {
        Today::from_unix(1_792_281_480, 0).expect("a date")
    }

    #[test]
    fn without_a_date_the_piece_is_absent() {
        assert!(with(None, |cx| Clock.bar(cx)).is_none());
    }

    #[test]
    fn the_widget_is_the_short_date() {
        let today = Today {
            year: 2026,
            month: 10,
            day: 16,
            weekday: 4,
        };
        let v = with(Some(today), |cx| Clock.bar(cx)).expect("a widget");
        assert_eq!(v.key, widget_key());
        assert!(v.interactive && v.focusable);
        let ViewKind::Text { text, style, .. } = &v.children[0].kind else {
            panic!("a text");
        };
        assert_eq!(text, &copy::short_date(4, 16));
        assert!(style.tabular);
        let other = with(Some(saturday()), |cx| Clock.bar(cx)).expect("a widget");
        let ViewKind::Text { text, .. } = &other.children[0].kind else {
            panic!("a text");
        };
        assert_eq!(text, &copy::short_date(5, 17));
    }

    #[test]
    fn a_click_opens_the_clock_page_and_nothing_else_does() {
        with(Some(saturday()), |cx| {
            let act = |key, gesture| Clock.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(widget_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::CLOCK)
            );
            assert_eq!(act(widget_key(), Gesture::Middle), PieceAction::None);
            assert_eq!(act(widget_key(), Gesture::Context), PieceAction::None);
            assert_eq!(
                act(widget_key(), Gesture::Scroll(Axis::Horizontal, -1)),
                PieceAction::None
            );
            assert_eq!(act(Key::named("x"), Gesture::Primary), PieceAction::None);
        });
    }

    #[test]
    fn the_piece_is_named_and_has_its_page() {
        assert_eq!(Clock.id().as_str(), "clock");
        assert_eq!(Clock.name(), "Reloj");
        assert_eq!(Clock.page().map(|e| e.id), Some(PageId::CLOCK));
    }
}
