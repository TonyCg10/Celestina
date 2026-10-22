//! The Phone page: the connected phone's name, its battery and whether it
//! charges, what its player plays and the three transport buttons. With no
//! phone it says so. The page only describes: a press on a button is
//! decided by the Island from the buttons' keys ([`action_for`]) and becomes
//! an [`IslandAction::PhoneMedia`], which the app turns into the request
//! `phone media` makes.

use celestina_paint::{Role, Weight};
use celestina_shell::phone::{Media, Phone as Device, PhoneState};
use celestina_ui::{view, Key, View};

use super::{activity::truncate_title, note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, text_style},
        IslandAction,
    },
    providers::magnetita::MediaKind,
};

/// A transport button's icon, logical px.
const BUTTON_ICON: f32 = 20.0;
/// A transport button's side, logical px.
const BUTTON_SIZE: f32 = 36.0;

/// The Phone page.
#[derive(Clone, Copy, Debug)]
pub struct Phone;

/// The previous button.
#[must_use]
pub fn previous_key() -> Key {
    Key::named("phone-previous")
}

/// The play or pause button.
#[must_use]
pub fn play_pause_key() -> Key {
    Key::named("phone-play-pause")
}

/// The next button.
#[must_use]
pub fn next_key() -> Key {
    Key::named("phone-next")
}

/// Whether the button of `kind` takes input with `media`: the player must
/// offer it. Play or pause is always offered, since a player that cannot
/// pause can still be told to play.
fn offered(media: &Media, kind: MediaKind) -> bool {
    match kind {
        MediaKind::Previous => media.can_previous,
        MediaKind::Next => media.can_next,
        _ => true,
    }
}

/// What a press on `key` asks for with `phone`: the transport action of a
/// button the player offers, and nothing for any other node or while there
/// is no phone with a player.
#[must_use]
pub fn action_for(phone: &PhoneState, key: Key) -> Option<IslandAction> {
    let PhoneState::Present(Device {
        media: Some(media), ..
    }) = phone
    else {
        return None;
    };
    let kind = if key == previous_key() {
        MediaKind::Previous
    } else if key == play_pause_key() {
        MediaKind::PlayPause
    } else if key == next_key() {
        MediaKind::Next
    } else {
        return None;
    };
    offered(media, kind).then_some(IslandAction::PhoneMedia(kind))
}

impl PageView for Phone {
    fn title(&self) -> &str {
        copy::PAGE_PHONE
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let PhoneState::Present(phone) = cx.phone else {
            return view::col(Key::named("phone-page")).child(note_list(
                theme,
                Key::named("phone-rows"),
                copy::PHONE_NONE,
            ));
        };
        let mut battery = text_row(
            theme,
            "phone-battery",
            &copy::phone_battery(phone.battery),
            theme.type_scale.body,
            Weight::Regular,
            Role::TextHi,
        );
        if phone.charging {
            battery = battery.child(view::text(
                Key::named("phone-charging"),
                copy::PHONE_CHARGING,
                text_style(
                    theme.type_scale.caption,
                    Weight::Regular,
                    false,
                    Role::Accent,
                ),
            ));
        }
        let mut rows = view::col(Key::named("phone-rows")).child(text_row(
            theme,
            "phone-name",
            &phone.name,
            theme.type_scale.row_title,
            Weight::Semibold,
            Role::TextHi,
        ));
        rows = rows.child(battery);
        if let Some(media) = &phone.media {
            rows = media_rows(cx, rows, media);
        }
        view::col(Key::named("phone-page")).child(rows)
    }

    fn wants(&self, _: &PageCx<'_>) -> (f32, f32) {
        PAGE_SIZE
    }
}

/// A one-line row of `text`, cut to the page.
fn text_row(
    theme: &celestina_paint::Theme,
    key: &'static str,
    text: &str,
    size: f32,
    weight: Weight,
    role: Role,
) -> View {
    centred(view::row(Key::named(key)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .child(
            view::text(
                Key::indexed(key, 1),
                &truncate_title(text),
                text_style(size, weight, false, role),
            )
            .grow(),
        )
}

/// `rows` with the media section: the heading, the track and the buttons.
fn media_rows(cx: &PageCx<'_>, rows: View, media: &Media) -> View {
    let theme = cx.theme;
    let heading = if media.playing {
        copy::PHONE_PLAYING
    } else {
        copy::PHONE_PAUSED
    };
    let track = [&media.title, &media.artist, &media.player]
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut rows = rows.child(text_row(
        theme,
        "phone-media-heading",
        heading,
        theme.type_scale.caption,
        Weight::Semibold,
        Role::TextLo,
    ));
    if let Some(title) = track.first() {
        rows = rows.child(text_row(
            theme,
            "phone-media-title",
            title,
            theme.type_scale.body,
            Weight::Regular,
            Role::TextHi,
        ));
    }
    if track.len() > 1 {
        rows = rows.child(text_row(
            theme,
            "phone-media-artist",
            &track[1..].join(" \u{b7} "),
            theme.type_scale.body,
            Weight::Regular,
            Role::TextLo,
        ));
    }
    let button = |key: Key, icon: &'static str, enabled: bool| {
        let role = if enabled {
            Role::TextHi
        } else {
            Role::TextFaint
        };
        let mut b = centred(view::row(key))
            .size(BUTTON_SIZE, BUTTON_SIZE)
            .radius(BUTTON_SIZE / 2.0)
            .child(
                view::icon(
                    Key::indexed("phone-button-icon", key_index(key)),
                    icon,
                    BUTTON_ICON,
                )
                .tint(role),
            );
        if enabled {
            b = b.fill(Role::ControlFill).interactive().focusable();
        }
        b.style.justify_content = taffy::JustifyContent::CENTER;
        b
    };
    let mut buttons = centred(view::row(Key::named("phone-buttons")))
        .height(BUTTON_SIZE + theme.spacing.md)
        .gap(theme.spacing.lg)
        .child(button(
            previous_key(),
            "media-skip-back",
            offered(media, MediaKind::Previous),
        ))
        .child(button(
            play_pause_key(),
            if media.playing {
                "media-pause"
            } else {
                "media-play"
            },
            true,
        ))
        .child(button(
            next_key(),
            "media-skip-forward",
            offered(media, MediaKind::Next),
        ));
    buttons.style.justify_content = taffy::JustifyContent::CENTER;
    rows.child(buttons)
}

/// A distinct number per button, for its icon's key.
fn key_index(key: Key) -> u64 {
    if key == previous_key() {
        0
    } else if key == play_pause_key() {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::NameOwner;

    fn media(playing: bool, prev: bool, next: bool) -> Media {
        Media {
            player: "Spotify".to_owned(),
            title: "Song".to_owned(),
            artist: "Band".to_owned(),
            playing,
            can_pause: true,
            can_next: next,
            can_previous: prev,
        }
    }

    fn present(battery: Option<u8>, charging: bool, media: Option<Media>) -> PhoneState {
        PhoneState::Present(Device {
            id: "dev".to_owned(),
            name: "Pixel".to_owned(),
            battery,
            charging,
            media,
        })
    }

    fn page_of(phone: &PhoneState) -> View {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        Phone.view(&PageCx {
            theme: &theme,
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            phone,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
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

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn without_a_phone_the_page_says_so() {
        let v = page_of(&PhoneState::Absent);
        assert_eq!(texts(&v), [copy::PHONE_NONE]);
        assert!(find(&v, previous_key()).is_none());
    }

    #[test]
    fn a_phone_shows_name_battery_and_charging() {
        let v = page_of(&present(Some(64), true, None));
        let t = texts(&v);
        assert_eq!(
            t,
            [
                "Pixel".to_owned(),
                copy::phone_battery(Some(64)),
                copy::PHONE_CHARGING.to_owned()
            ]
        );
        assert!(
            find(&v, play_pause_key()).is_none(),
            "no player, no buttons"
        );
        let v = page_of(&present(None, false, None));
        assert_eq!(texts(&v), ["Pixel".to_owned(), copy::phone_battery(None)]);
    }

    #[test]
    fn media_shows_the_track_and_three_buttons() {
        let v = page_of(&present(Some(5), false, Some(media(true, true, true))));
        let t = texts(&v);
        assert!(t.contains(&copy::PHONE_PLAYING.to_owned()), "{t:?}");
        assert!(t.contains(&"Song".to_owned()));
        assert!(t
            .iter()
            .any(|s| s.contains("Band") && s.contains("Spotify")));
        for key in [previous_key(), play_pause_key(), next_key()] {
            let b = find(&v, key).expect("button");
            assert!(b.interactive && b.focusable);
        }
        let v = page_of(&present(Some(5), false, Some(media(false, true, true))));
        assert!(texts(&v).contains(&copy::PHONE_PAUSED.to_owned()));
    }

    #[test]
    fn a_button_the_player_does_not_offer_takes_no_input() {
        let v = page_of(&present(Some(5), false, Some(media(true, false, false))));
        assert!(!find(&v, previous_key()).expect("prev").interactive);
        assert!(!find(&v, next_key()).expect("next").interactive);
        assert!(find(&v, play_pause_key()).expect("pp").interactive);
    }

    #[test]
    fn hostile_text_is_cut_to_the_page() {
        let mut m = media(true, true, true);
        m.title = "\u{1f600}".repeat(5000);
        m.artist = "a".repeat(5000);
        m.player = "p\u{301}".repeat(5000);
        let state = PhoneState::Present(Device {
            id: "d".to_owned(),
            name: "n".repeat(5000),
            battery: Some(1),
            charging: false,
            media: Some(m),
        });
        let v = page_of(&state);
        for s in texts(&v) {
            assert!(
                unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).count()
                    <= super::super::activity::MAX_TITLE_GRAPHEMES,
                "{s:?}"
            );
        }
    }

    #[test]
    fn a_press_asks_for_the_transport_action() {
        let state = present(Some(5), false, Some(media(true, true, false)));
        assert_eq!(
            action_for(&state, previous_key()),
            Some(IslandAction::PhoneMedia(MediaKind::Previous))
        );
        assert_eq!(
            action_for(&state, play_pause_key()),
            Some(IslandAction::PhoneMedia(MediaKind::PlayPause))
        );
        assert_eq!(action_for(&state, next_key()), None, "not offered");
        assert_eq!(action_for(&state, Key::named("other")), None);
        assert_eq!(action_for(&PhoneState::Absent, play_pause_key()), None);
        assert_eq!(
            action_for(&present(Some(5), false, None), play_pause_key()),
            None
        );
    }

    #[test]
    fn the_page_asks_for_the_default_size() {
        assert_eq!(
            super::super::wants_of(&Phone, None, (760.0, 560.0)),
            PAGE_SIZE
        );
        assert_eq!(Phone.title(), copy::PAGE_PHONE);
    }
}
