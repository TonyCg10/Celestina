//! The Volume piece: the default output's icon and level in the bar, absent
//! until the audio worker has read it (so while the `osd` duty is off, and a
//! piece that merely exists touches nothing). The wheel moves the level, a
//! middle or right click flips the mute and a click opens the slider popup,
//! which also holds the microphone.
//!
//! The piece only describes and asks: the popup is built and the worker
//! commanded by the app.

use celestina_shell::osd::{Kind, Reading, Shown};
use celestina_ui::{Key, View};

use super::{
    level, wheel, AudioAction, Axis, BarCx, Gesture, PageEntry, Piece, PieceAction, PieceId,
    PieceInput,
};
use crate::{copy, osd::view::icon_of};

/// The Volume piece.
#[derive(Clone, Copy, Debug)]
pub struct Volume;

fn key() -> Key {
    Key::named("volume")
}

impl Volume {
    /// The accessible name of the widget built from `cx`: the piece's name
    /// with the level, or that it is muted.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> Option<String> {
        let level = cx.levels.audio.output?;
        Some(copy::bar_volume_state(level.percent, level.muted))
    }
}

impl Piece for Volume {
    fn id(&self) -> PieceId {
        PieceId("volume")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_VOLUME
    }

    /// The output's icon (the muted one while muted) and level; absent while
    /// the output has no reading.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let level = cx.levels.audio.output?;
        let icon = icon_of(Shown {
            kind: Kind::Volume,
            reading: Reading {
                percent: level.percent,
                muted: level.muted,
            },
        });
        Some(level::widget(cx, key(), icon, level.percent, level.muted))
    }

    fn page(&self) -> Option<PageEntry> {
        None
    }

    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key != key() {
            return PieceAction::None;
        }
        match input.gesture {
            Gesture::Primary => PieceAction::Audio(AudioAction::Popup),
            Gesture::Middle | Gesture::Context => PieceAction::Audio(AudioAction::ToggleMute),
            Gesture::Scroll(Axis::Vertical, steps) => wheel(steps)
                .map_or(PieceAction::None, |(direction, count)| {
                    PieceAction::Audio(AudioAction::Step(direction, count))
                }),
            Gesture::Scroll(Axis::Horizontal, _) => PieceAction::None,
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
        pieces::{AudioLevels, Level, Levels},
    };

    fn with<R>(audio: AudioLevels, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
                audio,
                brightness: None,
            },
        })
    }

    fn output(percent: u8, muted: bool) -> AudioLevels {
        AudioLevels {
            output: Some(Level { percent, muted }),
            input: None,
        }
    }

    fn ask(audio: AudioLevels, key: Key, gesture: Gesture) -> PieceAction {
        with(audio, |cx| {
            Volume.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn no_reading_means_no_widget() {
        assert!(with(AudioLevels::default(), |cx| Volume.bar(cx)).is_none());
        // A microphone alone does not make the output's widget.
        let mic = AudioLevels {
            output: None,
            input: Some(Level {
                percent: 40,
                muted: false,
            }),
        };
        assert!(with(mic, |cx| Volume.bar(cx)).is_none());
        assert!(with(mic, Volume::label).is_none());
    }

    #[test]
    fn the_widget_is_one_focusable_node_with_the_icon_and_the_level() {
        let v = with(output(60, false), |cx| Volume.bar(cx)).expect("a widget");
        assert_eq!(v.key, key());
        assert!(v.interactive && v.focusable);
        assert_eq!(v.children.len(), 2);
        let muted = with(output(60, true), |cx| Volume.bar(cx)).expect("a widget");
        assert_ne!(v.children[0], muted.children[0], "the muted icon differs");
    }

    #[test]
    fn the_label_names_the_level_or_the_mute() {
        let name = |muted| with(output(60, muted), Volume::label);
        assert_eq!(name(false), Some(copy::bar_volume_state(60, false)));
        assert_eq!(name(true), Some(copy::bar_volume_state(60, true)));
        assert_ne!(name(false), name(true));
    }

    #[test]
    fn gestures_ask_what_the_contract_says() {
        let a = output(60, false);
        assert_eq!(
            ask(a, key(), Gesture::Primary),
            PieceAction::Audio(AudioAction::Popup)
        );
        for gesture in [Gesture::Middle, Gesture::Context] {
            assert_eq!(
                ask(a, key(), gesture),
                PieceAction::Audio(AudioAction::ToggleMute)
            );
        }
        // The Wayland sign: down is positive, and down lowers the volume.
        assert_eq!(
            ask(a, key(), Gesture::Scroll(Axis::Vertical, 1)),
            PieceAction::Audio(AudioAction::Step(Direction::Down, 1))
        );
        assert_eq!(
            ask(a, key(), Gesture::Scroll(Axis::Vertical, -2)),
            PieceAction::Audio(AudioAction::Step(Direction::Up, 2))
        );
        assert_eq!(
            ask(a, key(), Gesture::Scroll(Axis::Vertical, 0)),
            PieceAction::None
        );
        assert_eq!(
            ask(a, key(), Gesture::Scroll(Axis::Horizontal, 1)),
            PieceAction::None
        );
        // Another piece's node is not this piece's.
        assert_eq!(
            ask(a, Key::named("bell"), Gesture::Primary),
            PieceAction::None
        );
    }
}
