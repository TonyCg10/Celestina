//! The Phone piece: the author's phone as a glyph with its battery level,
//! and the Phone page. It draws nothing (and so takes no room in its zone)
//! while no phone is connected, which is also what a duty that is off
//! reads. A click opens the page; the page holds the media buttons, so
//! every action of the piece is reachable from there.

use celestina_paint::{Role, Weight};
use celestina_shell::phone::PhoneState;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{pages::phone::Phone as Page, view::centred, view::text_style},
};

/// The glyph's size, logical px.
const GLYPH: f32 = 18.0;
/// The space around the glyph that still counts as the piece.
const HIT_PAD: f32 = 5.0;
/// The glyph of a phone on battery.
const PHONE_ICON: &str = "phone";
/// The glyph of a phone that is charging.
const CHARGING_ICON: &str = "battery-charging";

/// The Phone piece.
#[derive(Clone, Copy, Debug)]
pub struct Phone;

fn phone_key() -> Key {
    Key::named("phone")
}

impl Phone {
    /// The accessible name of the widget built from `cx`; `None` while it
    /// draws nothing.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> Option<String> {
        match cx.phone {
            PhoneState::Absent => None,
            PhoneState::Present(phone) => {
                Some(copy::bar_phone_state(phone.battery, phone.charging))
            }
        }
    }
}

impl Piece for Phone {
    fn id(&self) -> PieceId {
        PieceId("phone")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_PHONE
    }

    /// The phone glyph (the charging battery while it charges, in the
    /// accent) and, when the device reported a charge, the percentage.
    /// Nothing without a phone.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let PhoneState::Present(phone) = cx.phone else {
            return None;
        };
        let (name, role) = if phone.charging {
            (CHARGING_ICON, Role::Accent)
        } else {
            (PHONE_ICON, Role::TextHi)
        };
        let level = phone.battery.map(|level| {
            view::text(
                Key::named("phone-level"),
                &copy::charge_percent(level),
                text_style(
                    cx.theme.type_scale.caption,
                    Weight::Regular,
                    true,
                    Role::TextHi,
                ),
            )
        });
        Some(
            centred(view::row(phone_key()))
                .padding(HIT_PAD)
                .gap(cx.theme.spacing.xs)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .child(view::icon(Key::named("phone-icon"), name, GLYPH).tint(role))
                .children(level),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::PHONE,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == phone_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::PHONE),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::{IconKey, Theme};
    use celestina_shell::phone::Phone as Device;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        island::pages::NameOwner,
        pieces::{piece, Axis},
    };

    fn device(battery: Option<u8>, charging: bool) -> PhoneState {
        PhoneState::Present(Device {
            id: "dev".to_owned(),
            name: "Pixel".to_owned(),
            battery,
            charging,
            media: None,
        })
    }

    fn with<R>(phone: &PhoneState, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
            phone,
            bubbles: &crate::bubbles::NONE,
            sysmon: None,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
        })
    }

    fn texts(v: &View) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let ViewKind::Text { text, .. } = &n.kind {
                out.push(text.clone());
            }
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn icon_name(v: &View) -> &'static str {
        match &v.children[0].kind {
            ViewKind::Icon {
                key: IconKey::Semantic(name),
                ..
            } => name,
            other => panic!("not an icon: {other:?}"),
        }
    }

    #[test]
    fn without_a_phone_the_piece_draws_nothing() {
        assert!(with(&PhoneState::Absent, |cx| Phone.bar(cx)).is_none());
        assert_eq!(with(&PhoneState::Absent, Phone::label), None);
    }

    #[test]
    fn a_phone_shows_its_glyph_and_its_charge() {
        let v = with(&device(Some(80), false), |cx| Phone.bar(cx)).expect("phone");
        assert_eq!(v.key, phone_key());
        assert!(v.interactive && v.focusable);
        assert_eq!(icon_name(&v), "phone");
        assert_eq!(texts(&v), [copy::charge_percent(80)]);
        for level in [0, 100] {
            let v = with(&device(Some(level), false), |cx| Phone.bar(cx)).expect("phone");
            assert_eq!(texts(&v), [copy::charge_percent(level)]);
        }
    }

    #[test]
    fn an_unknown_charge_leaves_only_the_glyph() {
        let v = with(&device(None, false), |cx| Phone.bar(cx)).expect("phone");
        assert!(texts(&v).is_empty());
        assert_eq!(v.children.len(), 1);
    }

    #[test]
    fn charging_changes_the_glyph_and_the_label() {
        let on = with(&device(Some(50), true), |cx| Phone.bar(cx)).expect("phone");
        let off = with(&device(Some(50), false), |cx| Phone.bar(cx)).expect("phone");
        assert_eq!(icon_name(&on), "battery-charging");
        assert_ne!(icon_name(&on), icon_name(&off));
        let labels: Vec<Option<String>> = [(Some(50), true), (Some(50), false), (None, true)]
            .into_iter()
            .map(|(b, c)| with(&device(b, c), Phone::label))
            .collect();
        let distinct: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(distinct.len(), 3, "{labels:?}");
        assert!(labels
            .iter()
            .flatten()
            .all(|l| l.starts_with(copy::PAGE_PHONE)));
    }

    #[test]
    fn a_hostile_name_does_not_reach_the_bar() {
        let state = PhoneState::Present(Device {
            id: "x".repeat(500),
            name: "😀\u{202e}".repeat(200),
            battery: Some(1),
            charging: false,
            media: None,
        });
        let v = with(&state, |cx| Phone.bar(cx)).expect("phone");
        assert_eq!(texts(&v), [copy::charge_percent(1)]);
    }

    fn act(key: Key, gesture: Gesture) -> PieceAction {
        with(&device(Some(10), false), |cx| {
            Phone.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn a_primary_input_opens_the_page_and_nothing_else_does_anything() {
        assert_eq!(
            act(phone_key(), Gesture::Primary),
            PieceAction::OpenPage(PageId::PHONE)
        );
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Vertical, 1),
            Gesture::Scroll(Axis::Horizontal, -1),
        ] {
            assert_eq!(act(phone_key(), gesture), PieceAction::None);
        }
        assert_eq!(
            act(Key::named("phone-icon"), Gesture::Primary),
            PieceAction::None
        );
    }

    #[test]
    fn the_piece_has_a_page_and_the_table_finds_it() {
        assert_eq!(Phone.page().map(|e| e.id), Some(PageId::PHONE));
        assert_eq!(
            piece("phone").map(|p| p.id().as_str()),
            Some("phone"),
            "registered in the table"
        );
    }
}
