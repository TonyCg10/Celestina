//! The Brightness piece: the sun and the level of the monitor of the bar's
//! output, absent until a reading of that monitor exists (so merely placing
//! the piece in a layout reads nothing: the DDC worker is asked to detect by
//! the app, once, when the duty is on and a layout holds the piece). The
//! wheel moves the level and a click opens the slider popup.
//!
//! The piece only describes and asks: the popup is built and the workers
//! commanded by the app.

use celestina_ui::{Key, View};

use super::{
    level, wheel, Axis, BarCx, BrightnessAction, Gesture, PageEntry, Piece, PieceAction, PieceId,
    PieceInput,
};
use crate::copy;

/// The semantic icon of a brightness.
const ICON: &str = "sun";

/// The Brightness piece.
#[derive(Clone, Copy, Debug)]
pub struct Brightness;

fn key() -> Key {
    Key::named("brightness")
}

impl Brightness {
    /// The accessible name of the widget built from `cx`: the piece's name
    /// with the level.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> Option<String> {
        Some(copy::bar_brightness_state(cx.levels.brightness?))
    }
}

impl Piece for Brightness {
    fn id(&self) -> PieceId {
        PieceId("brightness")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_BRIGHTNESS
    }

    /// The sun and the level; absent while the output's monitor is unknown.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let percent = cx.levels.brightness?;
        Some(level::widget(cx, key(), ICON, percent, false))
    }

    fn page(&self) -> Option<PageEntry> {
        None
    }

    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key != key() {
            return PieceAction::None;
        }
        match input.gesture {
            Gesture::Primary => PieceAction::Brightness(BrightnessAction::Popup),
            Gesture::Scroll(Axis::Vertical, steps) => {
                wheel(steps).map_or(PieceAction::None, |(direction, count)| {
                    PieceAction::Brightness(BrightnessAction::Step(direction, count))
                })
            }
            Gesture::Middle | Gesture::Context | Gesture::Scroll(Axis::Horizontal, _) => {
                PieceAction::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_shell::audio::Direction;

    use super::*;
    use crate::{
        island::pages::NameOwner,
        pieces::{AudioLevels, Levels},
    };

    fn with<R>(brightness: Option<u8>, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: Levels {
                audio: AudioLevels::default(),
                brightness,
            },
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
        })
    }

    fn ask(level: Option<u8>, key: Key, gesture: Gesture) -> PieceAction {
        with(level, |cx| {
            Brightness.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn an_unknown_monitor_means_no_widget() {
        assert!(with(None, |cx| Brightness.bar(cx)).is_none());
        assert!(with(None, Brightness::label).is_none());
    }

    #[test]
    fn the_widget_is_one_focusable_node_and_the_label_names_the_level() {
        let v = with(Some(70), |cx| Brightness.bar(cx)).expect("a widget");
        assert_eq!(v.key, key());
        assert!(v.interactive && v.focusable);
        assert_eq!(
            with(Some(70), Brightness::label),
            Some(copy::bar_brightness_state(70))
        );
    }

    #[test]
    fn gestures_ask_what_the_contract_says() {
        let l = Some(50);
        assert_eq!(
            ask(l, key(), Gesture::Primary),
            PieceAction::Brightness(BrightnessAction::Popup)
        );
        assert_eq!(
            ask(l, key(), Gesture::Scroll(Axis::Vertical, -1)),
            PieceAction::Brightness(BrightnessAction::Step(Direction::Up, 1))
        );
        assert_eq!(
            ask(l, key(), Gesture::Scroll(Axis::Vertical, 2)),
            PieceAction::Brightness(BrightnessAction::Step(Direction::Down, 2))
        );
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Horizontal, 1),
            Gesture::Scroll(Axis::Vertical, 0),
        ] {
            assert_eq!(ask(l, key(), gesture), PieceAction::None, "{gesture:?}");
        }
        assert_eq!(
            ask(l, Key::named("volume"), Gesture::Primary),
            PieceAction::None
        );
    }
}
