//! The Weather piece: the temperature in whole degrees ("12°") with a
//! condition icon when the semantic icon set has one. It has no page of its
//! own: a click opens the Clock page, which shows the full reading. Without a
//! fresh reading the piece is absent.

use celestina_paint::{Role, Weight};
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy, glance,
    island::view::{centred, text_style},
};

/// The condition icon's size, logical px.
const ICON: f32 = 16.0;
/// The space around the widget that still counts as the widget.
const HIT_PAD: f32 = 5.0;

/// The Weather piece.
#[derive(Clone, Copy, Debug)]
pub struct Weather;

fn widget_key() -> Key {
    Key::named("weather")
}

/// The semantic icon for Open-Meteo `code` by day or night, or `None` when
/// the icon set has nothing honest to show: it has a sun and a cloud but no
/// moon, so a clear night is text only.
#[must_use]
pub fn condition_icon(code: u8, daylight: bool) -> Option<&'static str> {
    match code {
        0 | 1 if daylight => Some("sun"),
        0 | 1 => None,
        _ => Some("cloud"),
    }
}

impl Piece for Weather {
    fn id(&self) -> PieceId {
        PieceId(glance::WEATHER_PIECE)
    }

    fn name(&self) -> &'static str {
        copy::PIECE_WEATHER
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let reading = cx.weather?;
        let icon = condition_icon(reading.code, reading.daylight)
            .map(|name| view::icon(Key::named("weather-icon"), name, ICON).tint(Role::TextHi));
        Some(
            centred(view::row(widget_key()))
                .padding(HIT_PAD)
                .gap(cx.theme.spacing.xs)
                .radius(cx.theme.radius_md)
                .interactive()
                .focusable()
                .children(icon)
                .child(view::text(
                    Key::named("weather-text"),
                    &copy::bar_weather(reading.celsius),
                    text_style(
                        cx.theme.type_scale.body,
                        Weight::Regular,
                        true,
                        Role::TextHi,
                    ),
                )),
        )
    }

    /// The Clock page shows the reading; the piece has no tab of its own.
    fn page(&self) -> Option<PageEntry> {
        None
    }

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
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{island::pages::NameOwner, providers::weather::WeatherReading};

    fn reading(celsius: i16, code: u8, daylight: bool) -> WeatherReading {
        WeatherReading {
            celsius,
            code,
            daylight,
            label: "Madrid".to_owned(),
            taken_unix: 0,
        }
    }

    fn with<R>(weather: Option<&WeatherReading>, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
            weather,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
        })
    }

    #[test]
    fn without_a_fresh_reading_the_piece_is_absent() {
        assert!(with(None, |cx| Weather.bar(cx)).is_none());
    }

    #[test]
    fn the_widget_is_the_temperature_with_the_icon_when_there_is_one() {
        let r = reading(12, 3, true);
        let v = with(Some(&r), |cx| Weather.bar(cx)).expect("a widget");
        assert_eq!(v.key, widget_key());
        assert!(v.interactive && v.focusable);
        assert_eq!(v.children.len(), 2);
        assert!(matches!(v.children[0].kind, ViewKind::Icon { .. }));
        let ViewKind::Text { text, .. } = &v.children[1].kind else {
            panic!("a text");
        };
        assert_eq!(text, "12°");

        // A clear night has no icon: text only, and a negative reading shows
        // its sign.
        let night = reading(-3, 0, false);
        let v = with(Some(&night), |cx| Weather.bar(cx)).expect("a widget");
        assert_eq!(v.children.len(), 1);
        let ViewKind::Text { text, .. } = &v.children[0].kind else {
            panic!("a text");
        };
        assert_eq!(text, "-3°");
    }

    #[test]
    fn the_icon_follows_the_condition_and_the_daylight() {
        assert_eq!(condition_icon(0, true), Some("sun"));
        assert_eq!(condition_icon(1, true), Some("sun"));
        assert_eq!(condition_icon(0, false), None);
        assert_eq!(condition_icon(3, false), Some("cloud"));
        assert_eq!(condition_icon(63, true), Some("cloud"));
        assert_eq!(condition_icon(95, false), Some("cloud"));
    }

    #[test]
    fn the_icons_it_names_exist_in_the_icon_set() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../celestina-style/icons");
        for code in [0, 1, 3, 63, 95] {
            for daylight in [true, false] {
                if let Some(name) = condition_icon(code, daylight) {
                    assert!(dir.join(format!("{name}.svg")).is_file(), "{name}");
                }
            }
        }
    }

    #[test]
    fn a_click_opens_the_clock_page_and_the_piece_has_no_page_of_its_own() {
        let r = reading(12, 0, true);
        with(Some(&r), |cx| {
            let act = |key, gesture| Weather.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(widget_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::CLOCK)
            );
            assert_eq!(act(widget_key(), Gesture::Context), PieceAction::None);
            assert_eq!(act(Key::named("x"), Gesture::Primary), PieceAction::None);
        });
        assert!(Weather.page().is_none());
        assert_eq!(Weather.id().as_str(), "weather");
        assert_eq!(Weather.name(), "Tiempo");
    }
}
