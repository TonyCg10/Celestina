//! The Media page (CEL-2-D): the shown player's cover, title, artist and
//! album, a read-only progress bar, the previous / play-pause / next
//! buttons and, with more than one player, one row per player that pins it.
//! While the media duty is off the page says so and nothing else; with no
//! player it says nothing plays. The page only describes: the Island turns
//! a press on a keyed node into an [`IslandAction::Media`] with
//! [`action_for`].

use celestina_paint::{PixmapId, Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::{Dimension, JustifyContent};
use unicode_segmentation::UnicodeSegmentation;

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style},
        IslandAction,
    },
    media::{MediaCommand, PlayerView},
    notifications::banners::line,
};

/// The cover's side, logical px.
pub const COVER_SIZE: f32 = 96.0;
/// The most player rows the page draws.
pub const MAX_PLAYER_ROWS: usize = 6;
/// The longest line of text the page shows, in grapheme clusters, the
/// ellipsis included.
pub const MAX_LINE_GRAPHEMES: usize = 48;
/// A transport button's side, logical px.
const TRANSPORT_SIZE: f32 = 36.0;
/// A transport button's glyph, logical px.
const TRANSPORT_GLYPH: f32 = 18.0;
/// The progress track's height, logical px.
const TRACK_H: f32 = 4.0;
/// The glyph of a player row, logical px.
const ROW_GLYPH: f32 = 14.0;

/// What the shell knows of the players, as the bar and the page read it:
/// the one typed, read-only slice of the shell's state they share.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaState {
    /// The players on the bus, sorted by bus name.
    pub players: Vec<PlayerView>,
    /// The bus name of the shown player.
    pub active: Option<String>,
    /// The uploaded cover of the shown player, if it has one.
    pub cover: Option<PixmapId>,
    /// Whether the media duty is on.
    pub duty_on: bool,
    /// Whether the session bus cannot be followed right now: the players
    /// above are the last known ones and are not shown as live.
    pub unavailable: bool,
}

/// The state of a shell whose media duty is off, for a context that has no
/// players to show.
pub static NO_MEDIA: MediaState = MediaState {
    players: Vec::new(),
    active: None,
    cover: None,
    duty_on: false,
    unavailable: false,
};

impl MediaState {
    /// The shown player, if the duty is on, the bus can be followed and one
    /// is on it.
    #[must_use]
    pub fn shown(&self) -> Option<&PlayerView> {
        if !self.duty_on || self.unavailable {
            return None;
        }
        let active = self.active.as_deref()?;
        self.players.iter().find(|p| p.bus_name == active)
    }
}

/// `text` whole up to `max` grapheme clusters, else its first `max - 1` and
/// an ellipsis, so the result never has more than `max`.
#[must_use]
pub fn ellipsise(text: &str, max: usize) -> String {
    let kept: Vec<&str> = text.graphemes(true).take(max + 1).collect();
    if kept.len() <= max {
        return text.to_owned();
    }
    let mut cut = kept[..max.saturating_sub(1)].concat();
    cut.push('\u{2026}');
    cut
}

/// A position or length as `m:ss` (`h:mm:ss` from an hour), or nothing
/// when it is unknown.
#[must_use]
pub fn clock(ms: i64) -> String {
    if ms < 0 {
        return String::new();
    }
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The share of the track played, 0..=1.
fn fraction(p: &PlayerView) -> f32 {
    if p.length_ms <= 0 || p.position_ms < 0 {
        return 0.0;
    }
    (p.position_ms as f64 / p.length_ms as f64).clamp(0.0, 1.0) as f32
}

/// The Media page.
#[derive(Clone, Copy, Debug)]
pub struct Media;

/// The key of the previous-track button.
#[must_use]
pub fn prev_key() -> Key {
    Key::named("media-prev")
}

/// The key of the play-pause button.
#[must_use]
pub fn play_key() -> Key {
    Key::named("media-play")
}

/// The key of the next-track button.
#[must_use]
pub fn next_key() -> Key {
    Key::named("media-next")
}

/// The key of the row of the player `bus_name`: stable per player (FNV-1a
/// of the bus name, which the worker bounds), not per position, so focus
/// and hover stay on the same player when another appears or leaves.
#[must_use]
pub fn player_key(bus_name: &str) -> Key {
    Key::indexed("media-player", player_tag(bus_name))
}

/// The number that tells a player's nodes apart from the others'.
fn player_tag(bus_name: &str) -> u64 {
    bus_name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The accessible name of the page's node `key`: a transport button, with
/// the play button naming what it does now.
#[must_use]
pub fn button_name(state: &MediaState, key: Key) -> Option<&'static str> {
    let shown = state.shown()?;
    if key == prev_key() {
        Some(copy::MEDIA_PREVIOUS)
    } else if key == next_key() {
        Some(copy::MEDIA_NEXT)
    } else if key == play_key() {
        Some(if shown.playing {
            copy::MEDIA_PAUSE
        } else {
            copy::MEDIA_PLAY
        })
    } else {
        None
    }
}

/// The width the texts beside the cover may take.
fn text_width(theme: &Theme) -> f32 {
    PAGE_SIZE.0 - 2.0 * theme.spacing.lg - COVER_SIZE - theme.spacing.md
}

/// The height of the progress row.
fn progress_height(theme: &Theme) -> f32 {
    line(theme.type_scale.caption)
}

/// What the page spends above its player rows.
fn fixed_height(theme: &Theme) -> f32 {
    COVER_SIZE + theme.spacing.sm + progress_height(theme) + theme.spacing.sm + TRANSPORT_SIZE
}

/// How many player rows the page draws on a surface of `max_size`: none
/// with a single player, else as many as fit, so no row is built that
/// could be focused unseen.
fn rows_fit(state: &MediaState, theme: &Theme, max_size: (f32, f32)) -> usize {
    if state.players.len() < 2 {
        return 0;
    }
    let room = max_size.1 - chrome_height(theme) - fixed_height(theme) - theme.spacing.sm;
    let each = theme.row_height + theme.spacing.xs;
    let fit = ((room + theme.spacing.xs) / each).floor().max(0.0) as usize;
    fit.min(MAX_PLAYER_ROWS).min(state.players.len())
}

impl PageView for Media {
    fn title(&self) -> &str {
        copy::PAGE_MEDIA
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let state = cx.media;
        let root = view::col(Key::named("media")).gap(theme.spacing.sm);
        if !state.duty_on {
            return root.child(note_list(
                theme,
                Key::named("media-rows"),
                copy::MEDIA_DUTY_OFF,
            ));
        }
        if state.unavailable {
            // The last players are stale: no buttons that would command one.
            return root.child(note_list(
                theme,
                Key::named("media-rows"),
                copy::MEDIA_UNAVAILABLE,
            ));
        }
        let Some(shown) = state.shown() else {
            return root.child(note_list(
                theme,
                Key::named("media-rows"),
                copy::MEDIA_NOTHING_PLAYING,
            ));
        };
        let rows = rows_fit(state, theme, cx.max_size);
        root.child(header(theme, shown, state.cover))
            .child(progress(theme, shown))
            .child(transport(theme, shown.playing))
            .children((rows > 0).then(|| chooser(theme, state, rows)))
    }

    /// The default size, taller when the player rows need it.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        let theme = cx.theme;
        if cx.media.shown().is_none() {
            return PAGE_SIZE;
        }
        let rows = rows_fit(cx.media, theme, cx.max_size);
        let list = if rows == 0 {
            0.0
        } else {
            theme.spacing.sm + rows as f32 * theme.row_height + (rows - 1) as f32 * theme.spacing.xs
        };
        let needed = chrome_height(theme) + fixed_height(theme) + list;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed).min(cx.max_size.1))
    }
}

/// What a click or Enter on the node `key` asks of the shell, if it is a
/// button or row the page draws for `state` on a surface of `max_size`.
#[must_use]
pub fn action_for(
    state: &MediaState,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<IslandAction> {
    state.shown()?;
    if key == prev_key() {
        return Some(IslandAction::Media(MediaCommand::Previous));
    }
    if key == play_key() {
        return Some(IslandAction::Media(MediaCommand::PlayPause));
    }
    if key == next_key() {
        return Some(IslandAction::Media(MediaCommand::Next));
    }
    let rows = rows_fit(state, theme, max_size);
    let player = state
        .players
        .iter()
        .take(rows)
        .find(|p| key == player_key(&p.bus_name))?;
    // Pressing the pinned player's row lets go of the pin.
    let pin = (!player.pinned).then(|| player.bus_name.clone());
    Some(IslandAction::Media(MediaCommand::Pin(pin)))
}

/// The cover and, beside it, title, artist and album.
fn header(theme: &Theme, p: &PlayerView, cover: Option<PixmapId>) -> View {
    let tile = centred(view::row(Key::named("media-cover")))
        .size(COVER_SIZE, COVER_SIZE)
        .radius(theme.radius_md)
        .fill(Role::ControlFill)
        .clip();
    let tile = match cover {
        Some(id) => tile.child(
            view::image(Key::named("media-cover-image"), id, 1.0).size(COVER_SIZE, COVER_SIZE),
        ),
        None => {
            let mut tile = tile;
            tile.style.justify_content = JustifyContent::CENTER;
            tile.child(
                view::icon(Key::named("media-cover-icon"), "music", COVER_SIZE / 2.0)
                    .tint(Role::TextLo),
            )
        }
    };
    let width = text_width(theme);
    let title = if p.title.is_empty() {
        copy::MEDIA_UNTITLED
    } else {
        &p.title
    };
    let mut texts = view::col(Key::named("media-texts"))
        .gap(theme.spacing.xs)
        .grow()
        .child(one_line(
            "media-title",
            title,
            width,
            theme.type_scale.row_title,
            Weight::Semibold,
            Role::TextHi,
        ));
    for (name, text, size, role) in [
        (
            "media-artist",
            &p.artist,
            theme.type_scale.body,
            Role::TextLo,
        ),
        (
            "media-album",
            &p.album,
            theme.type_scale.caption,
            Role::TextFaint,
        ),
    ] {
        if !text.is_empty() {
            texts = texts.child(one_line(name, text, width, size, Weight::Regular, role));
        }
    }
    view::row(Key::named("media-header"))
        .gap(theme.spacing.md)
        .child(tile)
        .child(texts)
}

/// `text` on one clipped line of at most `width`.
fn one_line(name: &str, text: &str, width: f32, size: f32, weight: Weight, role: Role) -> View {
    view::col(Key::named(name)).height(line(size)).clip().child(
        view::text(
            Key::indexed(name, 1),
            &ellipsise(text, MAX_LINE_GRAPHEMES),
            text_style(size, weight, false, role),
        )
        .max_width(width),
    )
}

/// The read-only progress: elapsed, the track and the length.
fn progress(theme: &Theme, p: &PlayerView) -> View {
    let caption = |name: &str, text: String| {
        view::text(
            Key::named(name),
            &text,
            text_style(
                theme.type_scale.caption,
                Weight::Regular,
                true,
                Role::TextLo,
            ),
        )
    };
    let mut done = view::row(Key::named("media-progress-done"))
        .fill(Role::Accent)
        .height(TRACK_H);
    done.style.size.width = Dimension::percent(fraction(p));
    let track = view::row(Key::named("media-progress-track"))
        .height(TRACK_H)
        .radius(TRACK_H / 2.0)
        .fill(Role::ControlFill)
        .clip()
        .grow()
        .child(done);
    centred(view::row(Key::named("media-progress")))
        .height(progress_height(theme))
        .gap(theme.spacing.sm)
        .child(caption("media-elapsed", clock(p.position_ms)))
        .child(track)
        .child(caption("media-length", clock(p.length_ms)))
}

/// The three transport buttons, centred.
fn transport(theme: &Theme, playing: bool) -> View {
    let play_icon = if playing { "media-pause" } else { "media-play" };
    let mut row = centred(view::row(Key::named("media-transport"))).gap(theme.spacing.md);
    row.style.justify_content = JustifyContent::CENTER;
    row.child(button(theme, prev_key(), "media-skip-back"))
        .child(button(theme, play_key(), play_icon))
        .child(button(theme, next_key(), "media-skip-forward"))
}

/// One icon button: a control, focusable, Enter and Space act. Its name is
/// the accessible name ([`button_name`]), never a label (no tooltips).
fn button(theme: &Theme, key: Key, icon: &'static str) -> View {
    let mut b = centred(view::row(key))
        .size(TRANSPORT_SIZE, TRANSPORT_SIZE)
        .radius(theme.radius_button)
        .fill(Role::ControlFill)
        .interactive()
        .focusable();
    b.style.justify_content = JustifyContent::CENTER;
    b.child(
        view::icon(
            Key::indexed("media-glyph", key_tag(key)),
            icon,
            TRANSPORT_GLYPH,
        )
        .tint(Role::TextHi),
    )
}

/// A small number telling the three buttons' glyph keys apart.
fn key_tag(key: Key) -> u64 {
    [prev_key(), play_key(), next_key()]
        .iter()
        .position(|k| *k == key)
        .map_or(0, |i| i as u64)
}

/// One row per player, the shown one lifted, each pinning its player.
fn chooser(theme: &Theme, state: &MediaState, rows: usize) -> View {
    let active = state.active.as_deref();
    view::col(Key::named("media-players"))
        .gap(theme.spacing.xs)
        .children(state.players.iter().take(rows).map(|p| {
            let mut row = centred(view::row(player_key(&p.bus_name)))
                .height(theme.row_height)
                .padding_xy(theme.spacing.md, 0.0)
                .gap(theme.spacing.md)
                .radius(theme.radius_md)
                .interactive()
                .focusable();
            if active == Some(p.bus_name.as_str()) {
                row = row.fill(Role::ControlFill);
            }
            // A row only pins its player (it never plays or pauses), so its
            // glyph says what the player is doing; the transport button
            // beside the cover shows the action a press would take, which
            // is the opposite glyph. Two vocabularies, one per meaning.
            let glyph = if p.playing {
                "media-play"
            } else {
                "media-pause"
            };
            row.child(
                view::text(
                    Key::indexed("media-player-name", player_tag(&p.bus_name)),
                    &ellipsise(&p.identity, 24),
                    text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
                )
                .grow(),
            )
            .child(
                view::icon(
                    Key::indexed("media-player-state", player_tag(&p.bus_name)),
                    glyph,
                    ROW_GLYPH,
                )
                .tint(Role::TextLo),
            )
        }))
}

#[cfg(test)]
mod tests {
    use celestina_paint::IconKey;
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::NameOwner;

    fn player(n: usize, playing: bool) -> PlayerView {
        PlayerView {
            bus_name: bus(n),
            identity: format!("player {n}"),
            playing,
            title: format!("Title {n}"),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            art_url: String::new(),
            length_ms: 200_000,
            position_ms: 50_000,
            pinned: false,
        }
    }

    fn bus(n: usize) -> String {
        format!("org.mpris.MediaPlayer2.p{n}")
    }

    fn state(n: usize) -> MediaState {
        let players: Vec<PlayerView> = (0..n).map(|i| player(i, i == 0)).collect();
        MediaState {
            active: players.first().map(|p| p.bus_name.clone()),
            players,
            cover: None,
            duty_on: true,
            unavailable: false,
        }
    }

    fn view_of(media: &MediaState, max: (f32, f32)) -> View {
        let owners = NameOwner::default();
        Media.view(&PageCx {
            theme: &Theme::dark(),
            max_size: max,
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            media,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
        })
    }

    fn wants_of(media: &MediaState, max: (f32, f32)) -> (f32, f32) {
        let owners = NameOwner::default();
        Media.wants(&PageCx {
            theme: &Theme::dark(),
            max_size: max,
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            media,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
        })
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn texts(v: &View, out: &mut Vec<String>) {
        if let ViewKind::Text { text, .. } = &v.kind {
            out.push(text.clone());
        }
        for c in &v.children {
            texts(c, out);
        }
    }

    fn all_texts(v: &View) -> Vec<String> {
        let mut out = Vec::new();
        texts(v, &mut out);
        out
    }

    const MAX: (f32, f32) = (760.0, 560.0);

    #[test]
    fn the_default_slice_is_empty_and_off() {
        let s = MediaState::default();
        assert!(s.players.is_empty() && s.active.is_none() && s.cover.is_none() && !s.duty_on);
        assert!(s.shown().is_none());
    }

    #[test]
    fn duty_off_and_no_player_each_say_one_line_and_draw_no_button() {
        let mut off = state(2);
        off.duty_on = false;
        assert_eq!(all_texts(&view_of(&off, MAX)), [copy::MEDIA_DUTY_OFF]);
        let none = MediaState {
            duty_on: true,
            ..MediaState::default()
        };
        assert_eq!(
            all_texts(&view_of(&none, MAX)),
            [copy::MEDIA_NOTHING_PLAYING]
        );
        let mut gone = state(1);
        gone.active = Some("org.mpris.MediaPlayer2.gone".to_owned());
        let v = view_of(&gone, MAX);
        for key in [prev_key(), play_key(), next_key()] {
            assert!(find(&v, key).is_none());
            assert_eq!(button_name(&gone, key), None);
        }
        assert_eq!(wants_of(&off, MAX), PAGE_SIZE);
        assert_eq!(wants_of(&none, MAX), PAGE_SIZE);
    }

    #[test]
    fn an_unavailable_bus_says_so_and_draws_no_button() {
        let mut s = state(2);
        s.unavailable = true;
        let v = view_of(&s, MAX);
        assert_eq!(all_texts(&v), [copy::MEDIA_UNAVAILABLE]);
        for key in [prev_key(), play_key(), next_key(), player_key(&bus(0))] {
            assert!(find(&v, key).is_none());
            assert_eq!(action_for(&s, &Theme::dark(), MAX, key), None);
        }
        assert_eq!(button_name(&s, play_key()), None);
        assert_eq!(wants_of(&s, MAX), PAGE_SIZE);
    }

    #[test]
    fn a_row_keeps_its_key_when_another_player_appears_or_leaves() {
        let two = state(2);
        let mut three = state(3);
        // A new player sorting first moves every other row down one place.
        three.players.rotate_right(1);
        let key_of = |s: &MediaState, n: usize| {
            let v = view_of(s, MAX);
            let row = find(&v, player_key(&bus(n))).expect("row").key;
            (row, row_position(&v, row))
        };
        let (key1, at_two) = key_of(&two, 1);
        let (key1_again, at_three) = key_of(&three, 1);
        assert_eq!(key1, key1_again, "the same player, the same key");
        assert_ne!(at_two, at_three, "although its place changed");
        assert_ne!(player_key(&bus(0)), player_key(&bus(1)));
    }

    fn row_position(v: &View, key: Key) -> usize {
        find(v, Key::named("media-players"))
            .and_then(|list| list.children.iter().position(|row| row.key == key))
            .unwrap_or(usize::MAX)
    }

    #[test]
    fn a_player_shows_its_cover_square_texts_progress_and_three_buttons() {
        let s = state(1);
        let v = view_of(&s, MAX);
        let cover = find(&v, Key::named("media-cover")).expect("cover");
        assert_eq!(cover.style.size.width, taffy::Dimension::length(COVER_SIZE));
        assert_eq!(
            cover.style.size.height,
            taffy::Dimension::length(COVER_SIZE)
        );
        // No cover: a themed icon.
        assert!(matches!(
            find(&v, Key::named("media-cover-icon")).map(|n| &n.kind),
            Some(ViewKind::Icon {
                key: IconKey::Semantic("music"),
                ..
            })
        ));
        let shown = all_texts(&v);
        for want in ["Title 0", "Artist", "Album", "0:50", "3:20"] {
            assert!(shown.iter().any(|t| t == want), "{want}: {shown:?}");
        }
        for key in [prev_key(), play_key(), next_key()] {
            let b = find(&v, key).expect("button");
            assert!(b.interactive && b.focusable);
        }
        assert!(
            find(&v, player_key(&bus(0))).is_none(),
            "one player, no chooser"
        );
    }

    #[test]
    fn a_cover_replaces_the_icon() {
        let mut s = state(1);
        s.cover = Some(PixmapId(9));
        let v = view_of(&s, MAX);
        assert!(find(&v, Key::named("media-cover-icon")).is_none());
        assert!(matches!(
            find(&v, Key::named("media-cover-image")).map(|n| &n.kind),
            Some(ViewKind::Image {
                id: PixmapId(9),
                ..
            })
        ));
    }

    #[test]
    fn the_play_button_names_and_draws_what_it_does_now() {
        let mut s = state(1);
        let glyph = |s: &MediaState| {
            let v = view_of(s, MAX);
            let b = find(&v, play_key()).expect("play");
            match &b.children[0].kind {
                ViewKind::Icon {
                    key: IconKey::Semantic(name),
                    ..
                } => *name,
                _ => "",
            }
        };
        assert_eq!(glyph(&s), "media-pause");
        assert_eq!(button_name(&s, play_key()), Some(copy::MEDIA_PAUSE));
        s.players[0].playing = false;
        assert_eq!(glyph(&s), "media-play");
        assert_eq!(button_name(&s, play_key()), Some(copy::MEDIA_PLAY));
        assert_eq!(button_name(&s, prev_key()), Some(copy::MEDIA_PREVIOUS));
        assert_eq!(button_name(&s, next_key()), Some(copy::MEDIA_NEXT));
        assert_eq!(button_name(&s, Key::named("x")), None);
    }

    #[test]
    fn several_players_get_one_row_each_up_to_the_bound() {
        let s = state(3);
        let v = view_of(&s, MAX);
        for i in 0..3 {
            let row = find(&v, player_key(&bus(i))).expect("row");
            assert!(row.interactive && row.focusable);
        }
        assert!(find(&v, player_key(&bus(3))).is_none());
        let many = state(10);
        let v = view_of(&many, (760.0, 900.0));
        assert!(find(&v, player_key(&bus(MAX_PLAYER_ROWS - 1))).is_some());
        assert!(find(&v, player_key(&bus(MAX_PLAYER_ROWS))).is_none());
    }

    #[test]
    fn rows_that_do_not_fit_are_not_built_and_the_page_never_outgrows_the_surface() {
        let theme = Theme::dark();
        let s = state(6);
        let tight = (760.0, chrome_height(&theme) + fixed_height(&theme) + 60.0);
        let v = view_of(&s, tight);
        assert!(find(&v, player_key(&bus(0))).is_some());
        assert!(
            find(&v, player_key(&bus(1))).is_none(),
            "no room for a second"
        );
        assert!(wants_of(&s, tight).1 <= tight.1);
        assert_eq!(
            action_for(&s, &theme, tight, player_key(&bus(1))),
            None,
            "an unbuilt row cannot be pressed"
        );
        // Roomy: the default width, tall enough for the rows.
        let (w, h) = wants_of(&s, MAX);
        assert_eq!(w, PAGE_SIZE.0);
        assert!(h >= PAGE_SIZE.1 && h <= MAX.1);
        assert_eq!(wants_of(&state(1), MAX), PAGE_SIZE);
    }

    #[test]
    fn presses_become_media_commands() {
        let theme = Theme::dark();
        let mut s = state(2);
        let ask = |s: &MediaState, k| action_for(s, &theme, MAX, k);
        assert_eq!(
            ask(&s, prev_key()),
            Some(IslandAction::Media(MediaCommand::Previous))
        );
        assert_eq!(
            ask(&s, play_key()),
            Some(IslandAction::Media(MediaCommand::PlayPause))
        );
        assert_eq!(
            ask(&s, next_key()),
            Some(IslandAction::Media(MediaCommand::Next))
        );
        assert_eq!(
            ask(&s, player_key(&bus(1))),
            Some(IslandAction::Media(MediaCommand::Pin(Some(
                "org.mpris.MediaPlayer2.p1".to_owned()
            ))))
        );
        // Pressing the pinned player's row lets go.
        s.players[1].pinned = true;
        assert_eq!(
            ask(&s, player_key(&bus(1))),
            Some(IslandAction::Media(MediaCommand::Pin(None)))
        );
        assert_eq!(ask(&s, player_key(&bus(2))), None);
        assert_eq!(ask(&s, Key::named("close")), None);
        s.duty_on = false;
        assert_eq!(ask(&s, play_key()), None);
        assert_eq!(ask(&MediaState::default(), play_key()), None);
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis_on_a_grapheme_boundary() {
        assert_eq!(ellipsise("short", 8), "short");
        assert_eq!(ellipsise("abcdefgh", 8), "abcdefgh");
        assert_eq!(ellipsise("abcdefghi", 8), "abcdefg\u{2026}");
        assert_eq!(ellipsise("e\u{301}e\u{301}e\u{301}", 2), "e\u{301}\u{2026}");
        let cut = ellipsise(&"y".repeat(500), MAX_LINE_GRAPHEMES);
        assert_eq!(cut.chars().count(), MAX_LINE_GRAPHEMES);
    }

    #[test]
    fn times_and_fractions_are_defensive() {
        assert_eq!(clock(-1), "");
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(59_999), "0:59");
        assert_eq!(clock(61_000), "1:01");
        assert_eq!(clock(3_661_000), "1:01:01");
        let mut p = player(0, true);
        assert_eq!(fraction(&p), 0.25);
        p.position_ms = 999_999;
        assert_eq!(fraction(&p), 1.0);
        p.length_ms = -1;
        assert_eq!(fraction(&p), 0.0);
        p.length_ms = 0;
        assert_eq!(fraction(&p), 0.0);
        p.length_ms = 10;
        p.position_ms = -1;
        assert_eq!(fraction(&p), 0.0);
    }

    #[test]
    fn hostile_text_stays_on_one_clipped_line_each() {
        let mut s = state(1);
        s.players[0].title = "t".repeat(10_000);
        s.players[0].artist = "a".repeat(10_000);
        let v = view_of(&s, MAX);
        for t in all_texts(&v) {
            assert!(t.chars().count() <= MAX_LINE_GRAPHEMES, "{}", t.len());
        }
    }
}
