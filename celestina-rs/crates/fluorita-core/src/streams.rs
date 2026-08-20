//! The streams inside one file, and which of them is being used.
//!
//! Called streams and not tracks on purpose: in this crate a *track* is a song
//! in the Music projection, and one word for two unrelated things is how a
//! catalogue ends up sorting subtitles by album.
//!
//! A film often carries more than one audio track and more than one set of
//! subtitles, and until now Fluorita played whichever the backend picked and
//! offered no way to say otherwise. This module owns what a track *is* to this
//! application and what may be asked about it; the backend reports the list and
//! confirms every change, exactly as it does for position and volume.
//!
//! Everything here is bounded before it is allocated. A track list is read out
//! of a file, which makes it hostile input: the count is capped, each label is
//! capped, and a list that claims more than the cap is truncated rather than
//! believed.

/// What kind of stream a track is. Video tracks are listed by the backend too
/// and are deliberately absent: choosing between video streams is not something
/// this player offers, and a kind nothing can act on has no place in a model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StreamKind {
    Audio,
    Subtitle,
}

impl StreamKind {
    /// The backend property that selects a track of this kind.
    #[must_use]
    pub const fn selector(self) -> &'static str {
        match self {
            Self::Audio => "aid",
            Self::Subtitle => "sid",
        }
    }
}

/// The most tracks of one kind this will carry. Far past any real film, and
/// bounded because the number comes from the file.
pub const MAX_STREAMS: usize = 64;

/// The longest label kept from a track's own title. A title is free text from
/// the container and is shown in a menu.
pub const MAX_LABEL_CHARACTERS: usize = 120;

/// One stream a person can choose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    /// The backend's own identifier for it. Opaque here: this crate never
    /// invents one and never renumbers.
    pub id: i64,
    pub kind: StreamKind,
    /// The title the container gave it, already bounded. Empty when it gave
    /// none, which is common and not a failure.
    pub title: String,
    /// Its language tag, as the container spells it. Empty when absent.
    pub language: String,
}

impl Stream {
    /// Builds a track, trimming what the file claims down to what may be held.
    #[must_use]
    pub fn new(id: i64, kind: StreamKind, title: &str, language: &str) -> Self {
        Self {
            id,
            kind,
            title: bounded(title),
            language: bounded(language),
        }
    }

    /// Whether this track carries nothing a person could tell it by. The
    /// surface names such a track by its position instead.
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        self.title.trim().is_empty() && self.language.trim().is_empty()
    }
}

fn bounded(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_LABEL_CHARACTERS)
        .collect()
}

/// The tracks one file offers and which of them is in use.
///
/// Selection is `None` when nothing of that kind is playing — the ordinary
/// state for subtitles, and a real one for audio in a silent film.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamSet {
    tracks: Vec<Stream>,
    audio: Option<i64>,
    subtitle: Option<i64>,
}

impl StreamSet {
    /// Takes what the backend reported, discarding kinds this does not offer
    /// and anything past the ceiling.
    #[must_use]
    pub fn from_reported(reported: impl IntoIterator<Item = Stream>) -> Self {
        let mut audio = Vec::new();
        let mut subtitle = Vec::new();
        for track in reported {
            let bucket = match track.kind {
                StreamKind::Audio => &mut audio,
                StreamKind::Subtitle => &mut subtitle,
            };
            if bucket.len() < MAX_STREAMS {
                bucket.push(track);
            }
        }
        audio.append(&mut subtitle);
        Self {
            tracks: audio,
            ..Self::default()
        }
    }

    /// The tracks of one kind, in the order the file lists them.
    pub fn of(&self, kind: StreamKind) -> impl Iterator<Item = &Stream> {
        self.tracks.iter().filter(move |track| track.kind == kind)
    }

    /// How many of one kind there are.
    #[must_use]
    pub fn count(&self, kind: StreamKind) -> usize {
        self.of(kind).count()
    }

    /// The track of one kind at `index` in the published order.
    #[must_use]
    pub fn at(&self, kind: StreamKind, index: usize) -> Option<&Stream> {
        self.of(kind).nth(index)
    }

    /// Which track of one kind is in use.
    #[must_use]
    pub const fn selected(&self, kind: StreamKind) -> Option<i64> {
        match kind {
            StreamKind::Audio => self.audio,
            StreamKind::Subtitle => self.subtitle,
        }
    }

    /// Where the selected track sits in the published order, for a surface that
    /// marks one row of a menu.
    #[must_use]
    pub fn selected_index(&self, kind: StreamKind) -> Option<usize> {
        let selected = self.selected(kind)?;
        self.of(kind).position(|track| track.id == selected)
    }

    /// Records a confirmed selection. Only the engine calls this: a request is
    /// not a selection until the backend says the track changed.
    pub fn confirm(&mut self, kind: StreamKind, id: Option<i64>) {
        // A confirmation naming a track this file does not have is ignored
        // rather than stored: it would leave the menu marking a row that is not
        // there.
        if let Some(id) = id {
            if !self.of(kind).any(|track| track.id == id) {
                return;
            }
        }
        match kind {
            StreamKind::Audio => self.audio = id,
            StreamKind::Subtitle => self.subtitle = id,
        }
    }

    /// Whether choosing between tracks of this kind is worth offering. One
    /// audio track is not a choice; one subtitle track is, because it can also
    /// be turned off.
    #[must_use]
    pub fn is_choosable(&self, kind: StreamKind) -> bool {
        match kind {
            StreamKind::Audio => self.count(kind) > 1,
            StreamKind::Subtitle => self.count(kind) > 0,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

/// How fast playback runs. Bounded so a request cannot ask the backend for a
/// rate that stops it or makes it unlistenable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speed(f64);

impl Speed {
    pub const NORMAL: Self = Self(1.0);
    pub const SLOWEST: f64 = 0.25;
    pub const FASTEST: f64 = 4.0;

    /// The rates the surface offers. A list rather than a slider: playback
    /// speed is chosen from a few known values, and a continuous control would
    /// invite 1.03× by accident.
    pub const OFFERED: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

    /// Clamps a requested rate into what is allowed.
    #[must_use]
    pub fn new(rate: f64) -> Self {
        if rate.is_finite() {
            Self(rate.clamp(Self::SLOWEST, Self::FASTEST))
        } else {
            Self::NORMAL
        }
    }

    #[must_use]
    pub const fn rate(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn is_normal(self) -> bool {
        (self.0 - 1.0).abs() < f64::EPSILON
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[cfg(test)]
mod tests {
    use super::{Speed, Stream, StreamKind, StreamSet, MAX_LABEL_CHARACTERS, MAX_STREAMS};

    fn audio(id: i64, title: &str) -> Stream {
        Stream::new(id, StreamKind::Audio, title, "es")
    }

    fn subtitle(id: i64, title: &str) -> Stream {
        Stream::new(id, StreamKind::Subtitle, title, "en")
    }

    #[test]
    fn a_label_a_file_claims_is_bounded_and_stripped_of_control_characters() {
        let stream = Stream::new(
            1,
            StreamKind::Audio,
            &format!("  come\u{7}dy{}  ", "x".repeat(MAX_LABEL_CHARACTERS)),
            "es",
        );
        assert_eq!(stream.title.chars().count(), MAX_LABEL_CHARACTERS);
        assert!(!stream.title.contains('\u{7}'));
        assert!(stream.title.starts_with("comedy"));
    }

    #[test]
    fn a_file_claiming_thousands_of_tracks_is_truncated_rather_than_believed() {
        let reported = (0..MAX_STREAMS as i64 * 3).map(|id| audio(id, "stream"));
        let tracks = StreamSet::from_reported(reported);
        assert_eq!(tracks.count(StreamKind::Audio), MAX_STREAMS);
    }

    #[test]
    fn tracks_keep_the_order_the_file_lists_them_in_within_their_kind() {
        let tracks = StreamSet::from_reported([
            subtitle(4, "english"),
            audio(1, "dubbed"),
            subtitle(5, "spanish"),
            audio(2, "original"),
        ]);
        assert_eq!(
            tracks
                .of(StreamKind::Audio)
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            tracks.at(StreamKind::Subtitle, 1).map(|track| track.id),
            Some(5)
        );
    }

    #[test]
    fn nothing_is_selected_until_the_engine_confirms_it() {
        let mut tracks = StreamSet::from_reported([audio(1, "a"), audio(2, "b")]);
        assert_eq!(tracks.selected(StreamKind::Audio), None);

        tracks.confirm(StreamKind::Audio, Some(2));
        assert_eq!(tracks.selected(StreamKind::Audio), Some(2));
        assert_eq!(tracks.selected_index(StreamKind::Audio), Some(1));

        tracks.confirm(StreamKind::Audio, None);
        assert_eq!(tracks.selected(StreamKind::Audio), None);
    }

    #[test]
    fn a_confirmation_for_a_track_this_file_lacks_is_ignored() {
        let mut tracks = StreamSet::from_reported([audio(1, "a")]);
        tracks.confirm(StreamKind::Audio, Some(9));
        assert_eq!(
            tracks.selected(StreamKind::Audio),
            None,
            "marking a row that is not in the menu is worse than marking none"
        );
    }

    #[test]
    fn one_audio_track_is_not_a_choice_and_one_subtitle_track_is() {
        let single = StreamSet::from_reported([audio(1, "a")]);
        assert!(!single.is_choosable(StreamKind::Audio));
        assert!(!single.is_choosable(StreamKind::Subtitle));

        let dubbed = StreamSet::from_reported([audio(1, "a"), audio(2, "b")]);
        assert!(dubbed.is_choosable(StreamKind::Audio));

        let subtitled = StreamSet::from_reported([audio(1, "a"), subtitle(3, "en")]);
        assert!(
            subtitled.is_choosable(StreamKind::Subtitle),
            "one set of subtitles is still on or off"
        );
    }

    #[test]
    fn a_track_with_nothing_to_call_it_says_so() {
        assert!(Stream::new(1, StreamKind::Audio, "  ", "").is_anonymous());
        assert!(!Stream::new(1, StreamKind::Audio, "", "es").is_anonymous());
    }

    #[test]
    fn a_rate_is_clamped_and_a_nonsensical_one_becomes_normal() {
        assert_eq!(Speed::new(1.5).rate(), 1.5);
        assert_eq!(Speed::new(100.0).rate(), Speed::FASTEST);
        assert_eq!(Speed::new(0.0).rate(), Speed::SLOWEST);
        assert!(Speed::new(f64::NAN).is_normal());
        assert!(Speed::default().is_normal());
        assert!(Speed::OFFERED.contains(&1.0));
    }

    #[test]
    fn the_selector_is_the_backend_property_for_that_kind() {
        assert_eq!(StreamKind::Audio.selector(), "aid");
        assert_eq!(StreamKind::Subtitle.selector(), "sid");
    }
}
