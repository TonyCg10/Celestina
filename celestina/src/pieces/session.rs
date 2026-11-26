//! The Session piece: a power glyph at the end of the bar. A click grows its
//! capsule into the session menu's sheet (CEL-I-006, CEL-D-003); it has no
//! page of its own.

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{
    conn_glyph, BarCx, Gesture, PageEntry, Piece, PieceAction, PieceId, PieceInput, SheetKind,
};
use crate::copy;

/// The glyph's size, logical px.
const GLYPH: f32 = 16.0;

/// The Session piece.
#[derive(Clone, Copy, Debug)]
pub struct Session;

fn node_key() -> Key {
    Key::named("session")
}

impl Piece for Session {
    fn id(&self) -> PieceId {
        PieceId("session")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_SESSION
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        Some(conn_glyph::tap(
            cx.theme,
            node_key(),
            [view::icon(Key::named("session-icon"), "power", GLYPH).tint(Role::TextHi)],
        ))
    }

    fn page(&self) -> Option<PageEntry> {
        None
    }

    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == node_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::Sheet(SheetKind::Session),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::pieces::Axis;

    #[test]
    fn the_piece_is_a_power_glyph_that_opens_the_menu() {
        crate::pieces::tests_support::with_state(
            &crate::connectivity::state::ConnectivityState::default(),
            |cx| {
                let v = Session.bar(cx).expect("always shown");
                assert_eq!(v.key, node_key());
                assert!(v.interactive && v.focusable);
                let ViewKind::Icon { key, tint, size } = &v.children[0].kind else {
                    panic!("an icon");
                };
                assert_eq!(*key, celestina_paint::IconKey::Semantic("power"));
                assert_eq!((*tint, *size), (Some(Role::TextHi), 16.0));
                let act = |key, gesture| Session.activate(&PieceInput { key, gesture, cx });
                assert_eq!(
                    act(node_key(), Gesture::Primary),
                    PieceAction::Sheet(SheetKind::Session)
                );
                for gesture in [
                    Gesture::Middle,
                    Gesture::Context,
                    Gesture::Scroll(Axis::Vertical, 1),
                ] {
                    assert_eq!(act(node_key(), gesture), PieceAction::None);
                }
                assert_eq!(act(Key::named("x"), Gesture::Primary), PieceAction::None);
            },
        );
        assert!(Session.page().is_none());
        assert_eq!(Session.id().as_str(), "session");
        assert_eq!(Session.name(), copy::PIECE_SESSION);
    }
}
