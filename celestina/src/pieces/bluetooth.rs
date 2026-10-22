//! The Bluetooth piece: the Bluetooth glyph, dim while the adapter is off
//! and accented while a device is connected. A click opens the Bluetooth
//! page. Absent while BlueZ or its adapter is.

use celestina_paint::Role;
use celestina_shell::connectivity::BtSection;
use celestina_ui::{Key, View};

use super::{
    conn_glyph, BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput,
};
use crate::{copy, island::pages::bluetooth::Bluetooth as Page};

/// The Bluetooth piece.
#[derive(Clone, Copy, Debug)]
pub struct Bluetooth;

fn node_key() -> Key {
    Key::named("bluetooth")
}

fn connected(section: &BtSection) -> usize {
    section.rows().iter().filter(|row| row.connected()).count()
}

impl Piece for Bluetooth {
    fn id(&self) -> PieceId {
        PieceId("bluetooth")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_BLUETOOTH
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let section = cx.connectivity.bluetooth().ready().copied()?;
        let role = match (section.powered(), connected(section)) {
            (false, _) => Role::TextFaint,
            (true, 0) => Role::TextHi,
            (true, _) => Role::Accent,
        };
        Some(conn_glyph::tap(
            cx.theme,
            node_key(),
            [conn_glyph::icon(
                Key::named("bluetooth-glyph"),
                "bluetooth",
                role,
            )],
        ))
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::BLUETOOTH,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == node_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::BLUETOOTH),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::connectivity::Service;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{
            state::{sample, ConnectivityState},
            ConnectivityEvent,
        },
        pieces::{tests_support::with_state, Axis},
    };

    fn state_with(section: BtSection) -> ConnectivityState {
        let mut state = ConnectivityState::default();
        state.apply(&ConnectivityEvent::Bluetooth(Service::Ready(section)));
        state
    }

    fn tint(v: &View) -> Option<Role> {
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let ViewKind::Icon { tint, .. } = &n.kind {
                return *tint;
            }
            stack.extend(n.children.iter());
        }
        None
    }

    #[test]
    fn the_piece_is_absent_without_an_adapter() {
        let state = ConnectivityState::default();
        with_state(&state, |cx| {
            assert!(Bluetooth.bar(cx).is_none());
        });
    }

    #[test]
    fn off_on_and_connected_look_different() {
        let off = state_with(BtSection::new(false, false, Vec::new()));
        let on = state_with(BtSection::new(true, false, Vec::new()));
        let connected = state_with(sample::bluetooth());
        let seen: Vec<Option<Role>> = [&off, &on, &connected]
            .into_iter()
            .map(|state| {
                with_state(state, |cx| {
                    let v = Bluetooth.bar(cx).expect("piece");
                    assert!(v.interactive && v.focusable);
                    tint(&v)
                })
            })
            .collect();
        assert_eq!(seen[0], Some(Role::TextFaint));
        assert_eq!(seen[1], Some(Role::TextHi));
        assert_eq!(seen[2], Some(Role::Accent));
    }

    #[test]
    fn a_primary_input_opens_the_page_and_nothing_else_does_anything() {
        let state = state_with(sample::bluetooth());
        with_state(&state, |cx| {
            let act = |key, gesture| Bluetooth.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(node_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::BLUETOOTH)
            );
            for gesture in [
                Gesture::Middle,
                Gesture::Context,
                Gesture::Scroll(Axis::Horizontal, -1),
            ] {
                assert_eq!(act(node_key(), gesture), PieceAction::None);
            }
            assert_eq!(act(Key::named("x"), Gesture::Primary), PieceAction::None);
        });
    }

    #[test]
    fn the_piece_opens_the_bluetooth_page() {
        assert_eq!(Bluetooth.page().map(|e| e.id), Some(PageId::BLUETOOTH));
        assert_eq!(Bluetooth.id().as_str(), "bluetooth");
    }
}
