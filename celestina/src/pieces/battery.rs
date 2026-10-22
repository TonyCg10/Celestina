//! The battery piece: a battery glyph filled to its charge with the
//! percentage beside it, accented while charging and red when low. A click
//! opens the Power page, which the piece owns. Absent without a battery
//! (a desktop, or UPower not running).

use celestina_paint::{Role, Weight};
use celestina_shell::connectivity::{BatterySection, ChargeState};
use celestina_ui::{view, Key, View};

use super::{
    conn_glyph, BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput,
};
use crate::{
    copy,
    island::{
        pages::power::Power as Page,
        view::{centred, text_style},
    },
};

/// The battery body, logical px.
const BODY: (f32, f32) = (20.0, 11.0);
/// The space between the body's edge and its fill.
const INSET: f32 = 1.5;
/// At or under this charge, discharging, the battery is drawn red.
const LOW: u8 = 15;

/// The battery piece.
#[derive(Clone, Copy, Debug)]
pub struct Battery;

fn node_key() -> Key {
    Key::named("battery")
}

fn role(battery: &BatterySection) -> Role {
    match battery.state() {
        ChargeState::Charging => Role::Accent,
        ChargeState::Discharging if battery.percentage() <= LOW => Role::Danger,
        _ => Role::TextHi,
    }
}

/// The fill's width for `percent`: proportional, never invisible while some
/// charge is left.
fn fill_width(percent: u8) -> f32 {
    let room = BODY.0 - 2.0 * INSET;
    let width = room * f32::from(percent.min(100)) / 100.0;
    if percent == 0 {
        0.0
    } else {
        width.max(1.0)
    }
}

impl Piece for Battery {
    fn id(&self) -> PieceId {
        PieceId("battery")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_BATTERY
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let battery = cx.connectivity.battery().ready().copied()?;
        let role = role(battery);
        let level = view::row(Key::named("battery-level"))
            .fill(role)
            .radius(2.0)
            .size(fill_width(battery.percentage()), BODY.1 - 2.0 * INSET);
        let body = centred(view::row(Key::named("battery-body")))
            .size(BODY.0, BODY.1)
            .padding_xy(INSET, 0.0)
            .radius(3.0)
            .fill(Role::ControlFill)
            .child(level);
        let charging = (battery.state() == ChargeState::Charging)
            .then(|| conn_glyph::icon(Key::named("battery-charging"), "zap", Role::Accent));
        let percent = view::text(
            Key::named("battery-percent"),
            &format!("{}", battery.percentage()),
            text_style(cx.theme.type_scale.caption, Weight::Semibold, true, role),
        );
        Some(conn_glyph::tap(
            cx.theme,
            node_key(),
            charging.into_iter().chain([body, percent]),
        ))
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::POWER,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == node_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::POWER),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{
            power::PowerEvent,
            state::{sample, ConnectivityState},
        },
        pieces::{tests_support::with_state, Axis},
    };
    use celestina_shell::connectivity::Service;

    fn state_with(state: ChargeState, percent: f64) -> ConnectivityState {
        let mut s = ConnectivityState::default();
        s.apply_power(&PowerEvent::Battery(Service::Ready(
            sample::battery(state, percent).expect("battery"),
        )));
        s
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

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn fill_of(v: &View) -> Option<Role> {
        match v.kind {
            ViewKind::Box { fill, .. } => fill,
            _ => None,
        }
    }

    #[test]
    fn the_piece_is_absent_without_a_battery() {
        let state = ConnectivityState::default();
        with_state(&state, |cx| {
            assert!(Battery.bar(cx).is_none());
        });
        let mut gone = state_with(ChargeState::Full, 100.0);
        gone.apply_power(&PowerEvent::Battery(Service::Unavailable));
        with_state(&gone, |cx| assert!(Battery.bar(cx).is_none()));
    }

    #[test]
    fn the_glyph_fills_to_the_charge_and_the_state_picks_the_colour() {
        let cases = [
            (ChargeState::Discharging, 64.0, Role::TextHi, false),
            (ChargeState::Discharging, 10.0, Role::Danger, false),
            (ChargeState::Charging, 10.0, Role::Accent, true),
            (ChargeState::Full, 100.0, Role::TextHi, false),
        ];
        for (charge, percent, expected, bolt) in cases {
            let state = state_with(charge, percent);
            with_state(&state, |cx| {
                let v = Battery.bar(cx).expect("piece");
                assert!(v.interactive && v.focusable);
                let level = find(&v, Key::named("battery-level")).expect("level");
                assert_eq!(fill_of(level), Some(expected), "{charge:?} {percent}");
                assert_eq!(
                    find(&v, Key::named("battery-charging")).is_some(),
                    bolt,
                    "{charge:?}"
                );
                let ks = keys(&v);
                let set: std::collections::HashSet<Key> = ks.iter().copied().collect();
                assert_eq!(set.len(), ks.len(), "unique keys");
            });
        }
    }

    #[test]
    fn the_fill_is_proportional_and_never_invisible_while_charged() {
        assert_eq!(fill_width(0), 0.0);
        assert!(fill_width(1) >= 1.0);
        assert!(fill_width(50) < fill_width(100));
        assert_eq!(fill_width(100), BODY.0 - 2.0 * INSET);
        assert_eq!(fill_width(250), fill_width(100));
    }

    #[test]
    fn a_primary_input_opens_the_power_page_and_nothing_else_does_anything() {
        let state = state_with(ChargeState::Full, 100.0);
        with_state(&state, |cx| {
            let act = |key, gesture| Battery.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(node_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::POWER)
            );
            for gesture in [
                Gesture::Middle,
                Gesture::Context,
                Gesture::Scroll(Axis::Vertical, 1),
            ] {
                assert_eq!(act(node_key(), gesture), PieceAction::None);
            }
        });
    }

    #[test]
    fn the_battery_owns_the_power_page() {
        assert_eq!(Battery.page().map(|e| e.id), Some(PageId::POWER));
        assert_eq!(Battery.id().as_str(), "battery");
    }
}
