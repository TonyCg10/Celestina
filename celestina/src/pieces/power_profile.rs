//! The power-profile piece: a glyph for the active profile (a bolt for
//! performance, a gauge for balanced, a leaf for power saver). A click opens
//! the Power page, which the battery piece owns; a middle click cycles to
//! the next profile, the same request the page's rows make. Absent while
//! power-profiles-daemon is.

use celestina_paint::Role;
use celestina_shell::connectivity::Action;
use celestina_ui::{Key, View};

use super::{
    conn_glyph, BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput,
};
use crate::{copy, island::pages::power::profile_icon};

/// The power-profile piece.
#[derive(Clone, Copy, Debug)]
pub struct PowerProfile;

fn node_key() -> Key {
    Key::named("power-profile")
}

impl Piece for PowerProfile {
    fn id(&self) -> PieceId {
        PieceId("power-profile")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_POWER_PROFILE
    }

    /// Absent while the daemon is, and while it names no valid active
    /// profile (there is nothing to show).
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let section = cx.connectivity.profiles().ready().copied()?;
        let active = section.active()?;
        Some(conn_glyph::tap(
            cx.theme,
            node_key(),
            [conn_glyph::icon(
                Key::named("power-profile-glyph"),
                profile_icon(active.as_str()),
                Role::TextHi,
            )],
        ))
    }

    /// No page of its own: the Power page is the battery piece's.
    fn page(&self) -> Option<PageEntry> {
        None
    }

    /// A click opens the Power page; a middle click asks for the next
    /// profile when there is one to cycle to.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key != node_key() {
            return PieceAction::None;
        }
        match input.gesture {
            Gesture::Primary => PieceAction::OpenPage(PageId::POWER),
            Gesture::Middle => {
                let can_cycle = input
                    .cx
                    .connectivity
                    .profiles()
                    .ready()
                    .is_some_and(|section| section.cycle().is_some());
                if can_cycle {
                    PieceAction::Connectivity(Action::CycleProfile)
                } else {
                    PieceAction::None
                }
            }
            Gesture::Context | Gesture::Scroll(..) => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::connectivity::{Degraded, ProfileSection, Service};
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{
            power::PowerEvent,
            state::{sample, ConnectivityState},
        },
        pieces::{tests_support::with_state, Axis},
    };

    fn state_with(section: ProfileSection) -> ConnectivityState {
        let mut s = ConnectivityState::default();
        s.apply_power(&PowerEvent::Profiles(Service::Ready(section)));
        s
    }

    fn icon_name(v: &View) -> Option<&'static str> {
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let ViewKind::Icon {
                key: celestina_paint::IconKey::Semantic(name),
                ..
            } = &n.kind
            {
                return Some(name);
            }
            stack.extend(n.children.iter());
        }
        None
    }

    #[test]
    fn the_piece_is_absent_without_the_daemon_or_an_active_profile() {
        let state = ConnectivityState::default();
        with_state(&state, |cx| {
            assert!(PowerProfile.bar(cx).is_none());
        });
        let nameless = state_with(ProfileSection::new("", ["balanced"], Degraded::No));
        with_state(&nameless, |cx| assert!(PowerProfile.bar(cx).is_none()));
    }

    #[test]
    fn each_profile_has_its_glyph() {
        let mut seen = Vec::new();
        for name in ["performance", "balanced", "power-saver"] {
            let state = state_with(sample::profiles(name));
            with_state(&state, |cx| {
                let v = PowerProfile.bar(cx).expect("piece");
                assert!(v.interactive && v.focusable);
                seen.push(icon_name(&v));
            });
        }
        assert_eq!(seen, [Some("zap"), Some("gauge"), Some("leaf")]);
    }

    #[test]
    fn a_click_opens_the_power_page_and_a_middle_click_cycles() {
        let state = state_with(sample::profiles("balanced"));
        with_state(&state, |cx| {
            let act = |key, gesture| PowerProfile.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(node_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::POWER)
            );
            assert_eq!(
                act(node_key(), Gesture::Middle),
                PieceAction::Connectivity(Action::CycleProfile)
            );
            for gesture in [Gesture::Context, Gesture::Scroll(Axis::Vertical, 1)] {
                assert_eq!(act(node_key(), gesture), PieceAction::None);
            }
            assert_eq!(act(Key::named("x"), Gesture::Middle), PieceAction::None);
        });
    }

    #[test]
    fn a_middle_click_asks_for_nothing_when_there_is_nothing_to_cycle_to() {
        let single = state_with(ProfileSection::new("balanced", ["balanced"], Degraded::No));
        with_state(&single, |cx| {
            let ask = PowerProfile.activate(&PieceInput {
                key: node_key(),
                gesture: Gesture::Middle,
                cx,
            });
            assert_eq!(ask, PieceAction::None);
        });
        // An active profile the daemon does not list is not one to cycle from.
        let odd = state_with(ProfileSection::new(
            "mystery",
            ["balanced", "performance"],
            Degraded::No,
        ));
        with_state(&odd, |cx| {
            let ask = PowerProfile.activate(&PieceInput {
                key: node_key(),
                gesture: Gesture::Middle,
                cx,
            });
            assert_eq!(ask, PieceAction::None);
        });
    }

    #[test]
    fn the_piece_has_no_page_of_its_own() {
        assert!(PowerProfile.page().is_none());
        assert_eq!(PowerProfile.id().as_str(), "power-profile");
    }
}
