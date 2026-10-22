//! The Media piece (CEL-2-D): a play-pause glyph and the one line of what
//! plays, drawn only while a player is shown. A click opens the Media
//! page; a middle click plays or pauses the shown player.

use celestina_paint::{Role, Weight};
use celestina_shell::media::now_playing_line;
use celestina_ui::{view, Key, View};

use super::{BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{
        pages::media::{ellipsise, Media as Page, MediaState},
        view::{centred, text_style},
    },
    media::MediaCommand,
    notifications::banners::line,
};

/// The glyph's size, logical px.
const GLYPH: f32 = 16.0;
/// The space around the widget that still counts as it.
const HIT_PAD: f32 = 5.0;
/// The line's fixed width, logical px.
const LINE_W: f32 = 200.0;
/// The longest line, in grapheme clusters, the ellipsis included.
const LINE_GRAPHEMES: usize = 32;

/// The Media piece.
#[derive(Clone, Copy, Debug)]
pub struct Media;

fn key() -> Key {
    Key::named("media")
}

/// The line the bar shows for the shown player: what plays, else who plays
/// it.
fn bar_line(state: &MediaState) -> Option<String> {
    let shown = state.shown()?;
    let line = now_playing_line(&shown.artist, &shown.title);
    let line = if line.is_empty() {
        shown.identity.clone()
    } else {
        line
    };
    Some(ellipsise(&line, LINE_GRAPHEMES))
}

/// Everything the bar draws of `state`, and nothing it does not: whether
/// the shown player plays and its line. `None` when the piece is absent.
/// Two states with the same signature draw the same bar, so a change of
/// position or of the cover never redraws it.
#[must_use]
pub fn bar_signature(state: &MediaState) -> Option<(bool, String)> {
    Some((state.shown()?.playing, bar_line(state)?))
}

impl Media {
    /// The accessible name of the widget built from `cx`.
    #[must_use]
    pub fn label(cx: &BarCx<'_>) -> Option<String> {
        let (playing, line) = bar_signature(cx.media)?;
        Some(copy::bar_media_name(&line, playing))
    }
}

impl Piece for Media {
    fn id(&self) -> PieceId {
        PieceId("media")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_MEDIA
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let (playing, text) = bar_signature(cx.media)?;
        let theme = cx.theme;
        let glyph = if playing { "media-pause" } else { "media-play" };
        let size = theme.type_scale.body;
        let line = view::col(Key::named("media-line"))
            .size(LINE_W, line(size))
            .clip()
            .child(
                view::text(
                    Key::named("media-line-text"),
                    &text,
                    text_style(size, Weight::Regular, false, Role::TextHi),
                )
                .max_width(LINE_W),
            );
        Some(
            centred(view::row(key()))
                .padding(HIT_PAD)
                .gap(theme.spacing.sm)
                .radius(theme.radius_md)
                .interactive()
                .focusable()
                .child(view::icon(Key::named("media-glyph"), glyph, GLYPH).tint(Role::TextHi))
                .child(line),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::MEDIA,
            view: &Page,
        })
    }

    /// Opens the page; a middle click plays or pauses; the rest do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key != key() || input.cx.media.shown().is_none() {
            return PieceAction::None;
        }
        match input.gesture {
            Gesture::Primary => PieceAction::OpenPage(PageId::MEDIA),
            Gesture::Middle => PieceAction::Media(MediaCommand::PlayPause),
            Gesture::Context | Gesture::Scroll(..) => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{island::pages::NameOwner, media::PlayerView, pieces::Axis};

    fn player(playing: bool, artist: &str, title: &str) -> PlayerView {
        PlayerView {
            bus_name: "org.mpris.MediaPlayer2.vlc".to_owned(),
            identity: "vlc".to_owned(),
            playing,
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: String::new(),
            art_url: String::new(),
            length_ms: 100_000,
            position_ms: 5_000,
            pinned: false,
        }
    }

    fn state(players: Vec<PlayerView>) -> MediaState {
        let active = players.first().map(|p| p.bus_name.clone());
        MediaState {
            players,
            active,
            cover: None,
            duty_on: true,
            unavailable: false,
        }
    }

    fn with<R>(media: &MediaState, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
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
            media,
            sysmon: None,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
        })
    }

    #[test]
    fn nothing_is_drawn_without_a_shown_player() {
        assert!(with(&MediaState::default(), |cx| Media.bar(cx)).is_none());
        let mut off = state(vec![player(true, "A", "T")]);
        off.duty_on = false;
        assert!(with(&off, |cx| Media.bar(cx)).is_none());
        let mut none = state(vec![player(true, "A", "T")]);
        none.active = None;
        assert!(with(&none, |cx| Media.bar(cx)).is_none());
    }

    #[test]
    fn the_bar_draws_a_focusable_glyph_and_the_fixed_width_line() {
        let s = state(vec![player(true, "Artist", "Title")]);
        let v = with(&s, |cx| Media.bar(cx)).expect("widget");
        assert_eq!(v.key, key());
        assert!(v.interactive && v.focusable);
        let ViewKind::Icon { key: icon, .. } = &v.children[0].kind else {
            panic!("a glyph first");
        };
        assert_eq!(
            format!("{icon:?}"),
            format!("{:?}", celestina_paint::IconKey::Semantic("media-pause"))
        );
        let ViewKind::Text { text, .. } = &v.children[1].children[0].kind else {
            panic!("the line");
        };
        assert_eq!(text, "Artist - Title");
        // Paused: the other glyph, the same line.
        let s = state(vec![player(false, "Artist", "Title")]);
        let v = with(&s, |cx| Media.bar(cx)).expect("widget");
        let ViewKind::Icon { key: icon, .. } = &v.children[0].kind else {
            panic!("a glyph first");
        };
        assert_eq!(
            format!("{icon:?}"),
            format!("{:?}", celestina_paint::IconKey::Semantic("media-play"))
        );
    }

    #[test]
    fn a_long_line_is_cut_and_a_titleless_track_shows_its_player() {
        let long = "x".repeat(200);
        let s = state(vec![player(true, "", &long)]);
        let (_, text) = bar_signature(&s).expect("signature");
        assert_eq!(text.chars().count(), LINE_GRAPHEMES);
        assert!(text.ends_with('\u{2026}'));
        let s = state(vec![player(true, "", "")]);
        assert_eq!(bar_signature(&s), Some((true, "vlc".to_owned())));
    }

    #[test]
    fn position_and_cover_do_not_change_what_the_bar_draws() {
        let a = state(vec![player(true, "A", "T")]);
        let mut b = a.clone();
        b.players[0].position_ms = 60_000;
        b.cover = Some(celestina_paint::PixmapId(3));
        assert_eq!(bar_signature(&a), bar_signature(&b));
        b.players[0].playing = false;
        assert_ne!(bar_signature(&a), bar_signature(&b));
    }

    #[test]
    fn a_click_opens_the_page_and_a_middle_click_plays_or_pauses() {
        let s = state(vec![player(true, "A", "T")]);
        let on = |gesture, key| with(&s, |cx| Media.activate(&PieceInput { key, gesture, cx }));
        assert_eq!(
            on(Gesture::Primary, key()),
            PieceAction::OpenPage(PageId::MEDIA)
        );
        assert_eq!(
            on(Gesture::Middle, key()),
            PieceAction::Media(MediaCommand::PlayPause)
        );
        for gesture in [Gesture::Context, Gesture::Scroll(Axis::Vertical, 1)] {
            assert_eq!(on(gesture, key()), PieceAction::None);
        }
        assert_eq!(on(Gesture::Primary, Key::named("other")), PieceAction::None);
        // A stale node of a bar whose player left asks for nothing.
        let none = MediaState::default();
        let action = with(&none, |cx| {
            Media.activate(&PieceInput {
                key: key(),
                gesture: Gesture::Middle,
                cx,
            })
        });
        assert_eq!(action, PieceAction::None);
    }

    #[test]
    fn the_label_names_the_piece_the_line_and_the_state() {
        let s = state(vec![player(false, "A", "T")]);
        assert_eq!(
            with(&s, Media::label),
            Some(copy::bar_media_name("A - T", false))
        );
        assert_eq!(with(&MediaState::default(), Media::label), None);
    }
}
