//! What a file says about itself, and which part of that can be changed.
//!
//! This is the half of editing that touches no pixel and no sample: a track's
//! tags, the cover art embedded beside them, and the EXIF a photograph carries
//! — including where it was taken. Under
//! [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md)
//! every operation here is *lossless*: the metadata block is replaced and the
//! media stream is copied across untouched. Nothing in this module recompresses
//! anything, and nothing in it can.
//!
//! Two rules shape it, and both exist because of a failure they prevent:
//!
//! - **Writability is per container, and it is honest.** A format whose block
//!   this suite cannot write answers `false` rather than being attempted and
//!   half-written. That is the same ruling `ImageFormat::carries_orientation`
//!   already makes for a lossless turn, for the same reason: a promise the
//!   writer cannot keep is worse than a missing feature.
//! - **A field a person can change is a field the library projects.** Music
//!   sorts on the artist, the album and the title, so those are what can be
//!   corrected. Lyrics, ratings and identifiers are not here, because Fluorita
//!   is not a tag database and a field nothing reads is a field nothing can
//!   verify.

use crate::media::MediaKind;
use std::ffi::OsStr;
use std::path::Path;

/// The longest a tag value may be. Generous for a real title; bounded, because
/// a tag block is attacker-controlled and this number precedes any allocation
/// the engine makes from it.
pub const MAX_TAG_CHARACTERS: usize = 512;

/// A metadata container this crate recognises by name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MetadataFormat {
    /// Vorbis comments in a FLAC metadata block.
    Flac,
    /// Vorbis comments in an Ogg stream.
    OggVorbis,
    /// ID3v2 in front of MPEG audio.
    Id3,
    /// iTunes-style atoms in an MPEG-4 container.
    Mp4,
    /// EXIF in a JPEG's APP1 segment. Not a tag container: what a photograph
    /// carries is read and removed, never written.
    JpegExif,
}

impl MetadataFormat {
    /// Classifies by filename extension, ASCII-case-insensitively, exactly as
    /// the rest of the crate does.
    #[must_use]
    pub fn from_extension(extension: &OsStr) -> Option<Self> {
        let extension = extension.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "flac" => Self::Flac,
            "ogg" | "oga" => Self::OggVorbis,
            "mp3" => Self::Id3,
            "m4a" | "aac" => Self::Mp4,
            "jpg" | "jpeg" => Self::JpegExif,
            _ => return None,
        })
    }

    #[must_use]
    pub fn classify_path(path: &Path) -> Option<Self> {
        path.extension().and_then(Self::from_extension)
    }

    /// Whether this suite can replace the container's metadata block while
    /// copying the media stream across byte for byte.
    ///
    /// Only FLAC answers `true`. Its metadata blocks state their own lengths,
    /// so replacing one is arithmetic and the audio frames after them are
    /// copied untouched.
    ///
    /// Ogg carries the same Vorbis comments and still answers `false`: they
    /// live inside a page structure whose checksums and boundaries would have
    /// to be rebuilt, and a container written by halves is worse than one left
    /// alone. ID3 and MP4 answer `false` for the same reason — an MPEG-4 atom
    /// tree has to be rebuilt with every offset in it corrected. Each is either
    /// done properly later or not claimed now.
    #[must_use]
    pub const fn writes_tags(self) -> bool {
        matches!(self, Self::Flac)
    }

    /// Whether tags can be read out of it at all. Reading is the engine's
    /// existing probe, which understands far more than this suite can write.
    #[must_use]
    pub const fn reads_tags(self) -> bool {
        matches!(self, Self::Flac | Self::OggVorbis | Self::Id3 | Self::Mp4)
    }

    /// Whether the container can carry embedded cover art that this suite can
    /// write. Reading a cover is the artwork path and is not this question.
    #[must_use]
    pub const fn writes_cover(self) -> bool {
        matches!(self, Self::Flac)
    }

    /// Whether a photograph in this container carries EXIF that can be removed.
    #[must_use]
    pub const fn strips_exif(self) -> bool {
        matches!(self, Self::JpegExif)
    }
}

/// One correctable tag field. Exactly the four Music projects on.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TagField {
    Title,
    Artist,
    Album,
    AlbumArtist,
}

impl TagField {
    pub const ALL: [Self; 4] = [Self::Title, Self::Artist, Self::Album, Self::AlbumArtist];

    /// The Vorbis comment key this field is stored under. Upper case because
    /// that is the spelling every other writer produces, and the readers are
    /// case-insensitive either way.
    #[must_use]
    pub const fn vorbis_key(self) -> &'static str {
        match self {
            Self::Title => "TITLE",
            Self::Artist => "ARTIST",
            Self::Album => "ALBUM",
            Self::AlbumArtist => "ALBUMARTIST",
        }
    }
}

/// What a photograph is carrying that a person may not want to hand over.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrivateFact {
    /// Where the picture was taken.
    Location,
    /// What took it.
    Camera,
    /// When it was taken.
    Timestamp,
}

impl PrivateFact {
    pub const ALL: [Self; 3] = [Self::Location, Self::Camera, Self::Timestamp];
}

/// What one item admits, decided from its kind and its name alone — before
/// anything is opened, so a list can answer for every visible row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataCapabilities {
    kind: MediaKind,
    format: Option<MetadataFormat>,
}

impl MetadataCapabilities {
    #[must_use]
    pub fn of(kind: MediaKind, path: &Path) -> Self {
        Self {
            kind,
            format: MetadataFormat::classify_path(path).filter(|format| match kind {
                MediaKind::Audio => format.reads_tags(),
                MediaKind::Image => format.strips_exif(),
                // A video's tags are not projected anywhere, so nothing here
                // offers to change them.
                MediaKind::Video => false,
            }),
        }
    }

    #[must_use]
    pub const fn format(&self) -> Option<MetadataFormat> {
        self.format
    }

    #[must_use]
    pub const fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Whether a tag correction can be offered for this item.
    #[must_use]
    pub fn corrects_tags(&self) -> bool {
        self.format.is_some_and(MetadataFormat::writes_tags)
    }

    /// Whether tags can be shown for it even when they cannot be changed. A
    /// surface uses this to explain rather than to hide.
    #[must_use]
    pub fn shows_tags(&self) -> bool {
        self.format.is_some_and(MetadataFormat::reads_tags)
    }

    #[must_use]
    pub fn embeds_cover(&self) -> bool {
        self.format.is_some_and(MetadataFormat::writes_cover)
    }

    #[must_use]
    pub fn strips_private_facts(&self) -> bool {
        self.format.is_some_and(MetadataFormat::strips_exif)
    }
}

/// A requested correction: the fields a person changed, and nothing else.
///
/// An absent field is untouched, which is not the same as an empty one. Setting
/// a field to the empty string removes the tag, and that difference is the
/// whole reason this is not a `MediaMetadata` with optional members.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TagChange {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
}

impl TagChange {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a field's new value.
    ///
    /// # Errors
    ///
    /// Refuses a value past [`MAX_TAG_CHARACTERS`] and one carrying a control
    /// character: a newline inside a Vorbis comment would end the field and
    /// start another one that nobody asked for.
    pub fn set(&mut self, field: TagField, value: &str) -> Result<(), MetadataRejected> {
        let value = value.trim();
        if value.chars().count() > MAX_TAG_CHARACTERS {
            return Err(MetadataRejected::ValueTooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(MetadataRejected::ValueNotPrintable);
        }
        let slot = match field {
            TagField::Title => &mut self.title,
            TagField::Artist => &mut self.artist,
            TagField::Album => &mut self.album,
            TagField::AlbumArtist => &mut self.album_artist,
        };
        *slot = Some(value.to_owned());
        Ok(())
    }

    /// The new value for one field, or `None` when it was not touched. An empty
    /// string means "remove this tag".
    #[must_use]
    pub fn value(&self, field: TagField) -> Option<&str> {
        match field {
            TagField::Title => self.title.as_deref(),
            TagField::Artist => self.artist.as_deref(),
            TagField::Album => self.album.as_deref(),
            TagField::AlbumArtist => self.album_artist.as_deref(),
        }
    }

    /// Every field this change touches, in a stable order.
    pub fn touched(&self) -> impl Iterator<Item = (TagField, &str)> {
        TagField::ALL
            .into_iter()
            .filter_map(|field| self.value(field).map(|value| (field, value)))
    }

    /// Whether anything was actually asked for.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.touched().next().is_none()
    }

    /// Whether this change would leave the file saying what it already says.
    ///
    /// A correction that corrects nothing is not written: a rewritten container
    /// with identical contents still changes the file's modification time, and
    /// the catalogue would then discard every extracted fact about it for no
    /// reason at all.
    #[must_use]
    pub fn changes_anything(&self, current: &crate::catalogue::MediaMetadata) -> bool {
        self.touched().any(|(field, value)| {
            let existing = match field {
                TagField::Title => current.title.as_deref(),
                TagField::Artist => current.artist.as_deref(),
                TagField::Album => current.album.as_deref(),
                TagField::AlbumArtist => current.album_artist.as_deref(),
            };
            let existing = existing.unwrap_or("").trim();
            existing != value
        })
    }
}

/// Why a metadata request was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataRejected {
    /// The item's kind or container carries nothing this can change.
    NotSupported,
    /// The container can be read but not written by this suite.
    ReadOnlyContainer,
    /// A tag value longer than the ceiling.
    ValueTooLong,
    /// A tag value carrying a control character, which the container's own
    /// framing would misread.
    ValueNotPrintable,
    /// Nothing was asked for.
    NothingRequested,
    /// The file already says exactly this.
    NoChange,
    /// A cover past its byte or pixel budget.
    CoverTooLarge,
    /// The chosen cover is not an image this suite can embed.
    CoverNotAnImage,
}

/// The ceilings an embedded cover may not cross.
///
/// A cover is decoration for a list: it is drawn at a card's size and never
/// larger. Embedding a 20-megapixel photograph in every track of an album would
/// multiply a library's size for pixels nothing displays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverBudget {
    pub max_bytes: u64,
    pub max_pixels: u64,
}

impl CoverBudget {
    /// The default: a generous album cover, and nothing like a photograph.
    pub const DEFAULT: Self = Self {
        max_bytes: 8 * 1024 * 1024,
        max_pixels: 4096 * 4096,
    };

    /// Judges a candidate cover before anything is read into memory.
    ///
    /// # Errors
    ///
    /// Refuses a file that is not a readable image and one past either ceiling.
    pub fn accepts(
        self,
        path: &Path,
        bytes: u64,
        pixels: Option<u64>,
    ) -> Result<(), MetadataRejected> {
        if crate::edit::ImageFormat::classify_path(path).is_none() {
            return Err(MetadataRejected::CoverNotAnImage);
        }
        if bytes > self.max_bytes {
            return Err(MetadataRejected::CoverTooLarge);
        }
        match pixels {
            Some(pixels) if pixels > self.max_pixels => Err(MetadataRejected::CoverTooLarge),
            // A file whose dimensions could not be read is refused rather than
            // embedded on the strength of its byte count alone.
            None => Err(MetadataRejected::CoverNotAnImage),
            Some(_) => Ok(()),
        }
    }
}

// How the library learns a correction: nothing here writes the catalogue. A
// rewritten container has a new size and a new modification time, which is
// exactly the `SourceIdentity` change the scan and the watch already treat as
// "this file is not the file I knew" — the record's extracted metadata is
// dropped and probed again from the file itself. Predicting the new values here
// instead would publish what this process *intended* rather than what the
// container ends up saying, which is the one thing the engine's confirmations
// exist to prevent.

#[cfg(test)]
mod tests {
    use super::{
        CoverBudget, MetadataCapabilities, MetadataFormat, MetadataRejected, PrivateFact,
        TagChange, TagField, MAX_TAG_CHARACTERS,
    };
    use crate::catalogue::MediaMetadata;
    use crate::media::MediaKind;
    use std::path::Path;

    fn track(name: &str) -> MetadataCapabilities {
        MetadataCapabilities::of(MediaKind::Audio, Path::new(name))
    }

    fn tagged() -> MediaMetadata {
        MediaMetadata {
            title: Some("Pavana".to_owned()),
            artist: Some("Ravel".to_owned()),
            album: None,
            album_artist: None,
            ..MediaMetadata::default()
        }
    }

    #[test]
    fn a_container_this_suite_cannot_write_says_so_instead_of_being_attempted() {
        assert!(track("/m/a.flac").corrects_tags());

        let ogg = track("/m/a.ogg");
        assert!(
            ogg.shows_tags() && !ogg.corrects_tags(),
            "Ogg's comments live inside pages whose checksums this cannot rebuild"
        );

        let mp3 = track("/m/a.mp3");
        assert!(
            mp3.shows_tags() && !mp3.corrects_tags(),
            "ID3 is read and not written until its writer exists"
        );
        let m4a = track("/m/a.m4a");
        assert!(m4a.shows_tags() && !m4a.corrects_tags());
    }

    #[test]
    fn a_photograph_offers_removal_and_no_tags_and_a_video_offers_nothing() {
        let photograph = MetadataCapabilities::of(MediaKind::Image, Path::new("/m/foto.jpg"));
        assert!(photograph.strips_private_facts());
        assert!(!photograph.shows_tags());
        assert!(!photograph.corrects_tags());

        let png = MetadataCapabilities::of(MediaKind::Image, Path::new("/m/captura.png"));
        assert!(!png.strips_private_facts());

        let clip = MetadataCapabilities::of(MediaKind::Video, Path::new("/m/clip.mkv"));
        assert_eq!(clip.format(), None);
        assert!(!clip.shows_tags() && !clip.corrects_tags());
        assert_eq!(clip.kind(), MediaKind::Video);
    }

    #[test]
    fn only_a_container_with_a_cover_writer_offers_one() {
        assert!(track("/m/a.flac").embeds_cover());
        assert!(!track("/m/a.ogg").embeds_cover());
        assert!(!track("/m/a.mp3").embeds_cover());
    }

    #[test]
    fn a_value_that_would_break_the_containers_framing_is_refused() {
        let mut change = TagChange::new();
        assert_eq!(
            change.set(TagField::Title, "two\nlines"),
            Err(MetadataRejected::ValueNotPrintable),
            "a newline inside a Vorbis comment starts a field nobody asked for"
        );
        assert_eq!(
            change.set(TagField::Artist, &"a".repeat(MAX_TAG_CHARACTERS + 1)),
            Err(MetadataRejected::ValueTooLong)
        );
        assert!(change.is_empty(), "a refused value is not recorded");
    }

    #[test]
    fn an_untouched_field_and_an_emptied_one_are_different_requests() {
        let mut change = TagChange::new();
        change
            .set(TagField::Album, "")
            .expect("emptying is allowed");
        assert_eq!(change.value(TagField::Album), Some(""));
        assert_eq!(change.value(TagField::Title), None);

        assert_eq!(
            change.touched().collect::<Vec<_>>(),
            vec![(TagField::Album, "")],
            "only the emptied field is part of the request"
        );
    }

    #[test]
    fn a_value_is_trimmed_and_kept_in_a_stable_order() {
        let mut change = TagChange::new();
        change.set(TagField::Artist, "  Ravel  ").expect("valid");
        change.set(TagField::Title, "Pavana").expect("valid");
        assert_eq!(change.value(TagField::Artist), Some("Ravel"));
        assert_eq!(
            change.touched().collect::<Vec<_>>(),
            vec![(TagField::Title, "Pavana"), (TagField::Artist, "Ravel")]
        );
    }

    #[test]
    fn a_correction_that_corrects_nothing_is_not_a_change() {
        let mut same = TagChange::new();
        same.set(TagField::Title, "Pavana").expect("valid");
        same.set(TagField::Artist, "Ravel").expect("valid");
        assert!(
            !same.changes_anything(&tagged()),
            "rewriting a file to say what it already says only moves its mtime"
        );

        let mut different = TagChange::new();
        different
            .set(TagField::Artist, "Maurice Ravel")
            .expect("valid");
        assert!(different.changes_anything(&tagged()));

        let mut removal = TagChange::new();
        removal.set(TagField::Title, "").expect("valid");
        assert!(removal.changes_anything(&tagged()));
    }

    #[test]
    fn a_cover_is_judged_before_it_is_read() {
        let budget = CoverBudget::DEFAULT;
        assert_eq!(
            budget.accepts(Path::new("/m/cover.jpg"), 400_000, Some(1_000_000)),
            Ok(())
        );
        assert_eq!(
            budget.accepts(Path::new("/m/notes.txt"), 10, Some(1)),
            Err(MetadataRejected::CoverNotAnImage)
        );
        assert_eq!(
            budget.accepts(Path::new("/m/cover.jpg"), budget.max_bytes + 1, Some(1)),
            Err(MetadataRejected::CoverTooLarge)
        );
        assert_eq!(
            budget.accepts(Path::new("/m/cover.jpg"), 10, Some(budget.max_pixels + 1)),
            Err(MetadataRejected::CoverTooLarge)
        );
        assert_eq!(
            budget.accepts(Path::new("/m/cover.jpg"), 10, None),
            Err(MetadataRejected::CoverNotAnImage),
            "a picture that will not say how big it is is refused, not guessed at"
        );
    }

    #[test]
    fn the_vorbis_keys_are_the_ones_every_other_writer_produces() {
        assert_eq!(TagField::Title.vorbis_key(), "TITLE");
        assert_eq!(TagField::AlbumArtist.vorbis_key(), "ALBUMARTIST");
        assert_eq!(PrivateFact::ALL.len(), 3);
        assert!(MetadataFormat::JpegExif.strips_exif());
        assert!(!MetadataFormat::JpegExif.reads_tags());
    }
}
