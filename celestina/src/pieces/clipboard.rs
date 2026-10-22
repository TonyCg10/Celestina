//! The Clipboard piece: the clipboard icon, with a small pause mark while
//! recording is paused, and the Clipboard page. Absent from the bar while
//! `duties.clipboard` is off (a zone whose pieces are all absent draws no
//! capsule). A click opens the page.
//!
//! The piece reads only the [`BarCx`]'s clipboard slice, which holds
//! whether the duty is on and whether it is paused besides the page's
//! previews; the widget never draws a preview or a count of entries.

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{pages::clipboard::Clipboard as Page, view::centred},
};

/// The icon's size, logical px.
const ICON: f32 = 18.0;
/// The paused mark's size, logical px.
const MARK: f32 = 10.0;
/// The space around the icon that still counts as the icon.
const HIT_PAD: f32 = 5.0;

/// The Clipboard piece.
#[derive(Clone, Copy, Debug)]
pub struct Clipboard;

fn icon_key() -> Key {
    Key::named("clipboard")
}

impl Clipboard {
    /// The accessible name of the widget built from `cx`: the piece's name
    /// with its state.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> &'static str {
        copy::bar_clipboard_state(cx.clipboard.paused)
    }
}

impl Piece for Clipboard {
    fn id(&self) -> PieceId {
        PieceId("clipboard")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_CLIPBOARD
    }

    /// The clipboard icon, with the pause mark at its corner while paused;
    /// nothing while the duty is off.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        if !cx.clipboard.on {
            return None;
        }
        let icon =
            view::icon(Key::named("clipboard-icon"), "clipboard-paste", ICON).tint(Role::TextHi);
        let mark = cx.clipboard.paused.then(|| {
            view::icon(Key::named("clipboard-paused"), "media-pause", MARK)
                .tint(Role::Accent)
                .absolute(HIT_PAD + ICON - MARK / 2.0, HIT_PAD / 2.0)
        });
        Some(
            centred(view::row(icon_key()))
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
            id: PageId::CLIPBOARD,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == icon_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::CLIPBOARD),
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
        island::pages::{
            clipboard::{ClipboardPanel, ClipboardRow, RowBody},
            NameOwner,
        },
        pieces::Axis,
    };

    fn with<R>(panel: &ClipboardPanel, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
            today: None,
            clipboard: panel,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
        })
    }

    fn panel(on: bool, paused: bool) -> ClipboardPanel {
        ClipboardPanel {
            on,
            paused,
            rows: vec![ClipboardRow {
                index: 0,
                body: RowBody::Text("hunter2 is not shown by the bar".to_owned()),
                thumb: None,
            }],
            total: 1,
            ..ClipboardPanel::default()
        }
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
    fn the_piece_is_absent_while_the_duty_is_off() {
        for paused in [false, true] {
            assert!(with(&panel(false, paused), |cx| Clipboard.bar(cx)).is_none());
        }
        assert!(with(&ClipboardPanel::default(), |cx| Clipboard.bar(cx)).is_none());
    }

    #[test]
    fn the_icon_has_a_mark_only_while_paused() {
        let widget = Key::named("clipboard");
        let icon = Key::named("clipboard-icon");
        let mark = Key::named("clipboard-paused");
        let running = with(&panel(true, false), |cx| Clipboard.bar(cx)).expect("icon");
        assert_eq!(keys(&running), [widget, icon]);
        let paused = with(&panel(true, true), |cx| Clipboard.bar(cx)).expect("icon");
        assert_eq!(keys(&paused), [widget, icon, mark]);
        for v in [&running, &paused] {
            assert!(v.interactive && v.focusable);
        }
    }

    #[test]
    fn the_widget_draws_the_clipboard_icon_and_no_content() {
        let v = with(&panel(true, false), |cx| Clipboard.bar(cx)).expect("icon");
        match &v.children[0].kind {
            ViewKind::Icon {
                key: celestina_paint::IconKey::Semantic(name),
                ..
            } => assert_eq!(*name, "clipboard-paste"),
            other => panic!("not an icon: {other:?}"),
        }
        let mut stack = vec![&v];
        while let Some(n) = stack.pop() {
            assert!(
                !matches!(n.kind, ViewKind::Text { .. }),
                "no text in the bar"
            );
            stack.extend(n.children.iter());
        }
    }

    fn act(key: Key, gesture: Gesture) -> PieceAction {
        with(&panel(true, false), |cx| {
            Clipboard.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn a_primary_input_opens_the_page_and_nothing_else_does_anything() {
        assert_eq!(
            act(icon_key(), Gesture::Primary),
            PieceAction::OpenPage(PageId::CLIPBOARD)
        );
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Vertical, 2),
            Gesture::Scroll(Axis::Horizontal, -2),
        ] {
            assert_eq!(act(icon_key(), gesture), PieceAction::None);
        }
        assert_eq!(
            act(Key::named("other"), Gesture::Primary),
            PieceAction::None
        );
    }

    #[test]
    fn the_label_names_the_piece_and_says_when_it_is_paused() {
        let running = with(&panel(true, false), Clipboard::label);
        let paused = with(&panel(true, true), Clipboard::label);
        assert_ne!(running, paused);
        assert!(running.starts_with(copy::PAGE_CLIPBOARD));
        assert!(paused.starts_with(copy::PAGE_CLIPBOARD));
    }

    #[test]
    fn the_piece_opens_the_clipboard_page() {
        assert_eq!(Clipboard.page().map(|e| e.id), Some(PageId::CLIPBOARD));
        assert_eq!(Clipboard.id().as_str(), "clipboard");
        assert_eq!(Clipboard.name(), copy::PAGE_CLIPBOARD);
    }
}
