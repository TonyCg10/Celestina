//! Media identity, kind and the transport a kind can honestly offer.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Stable identity for one catalogued media file.
///
/// Prefer [`MediaId::filesystem`]: the parent-independent device+inode pair
/// survives a rename inside a source root and keeps two hardlinks to the same
/// bytes a single catalogue entry. [`MediaId::from_path`] exists for callers
/// that legitimately have no metadata (a file handed in on argv before it is
/// stat'ed); it keys on the raw path bytes and therefore does not survive a
/// rename.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MediaId(Identity);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Identity {
    Filesystem { device: u64, inode: u64 },
    Path(PathBuf),
}

impl MediaId {
    /// Identity from the values a `stat` already produced.
    #[must_use]
    pub fn filesystem(device: u64, inode: u64) -> Self {
        Self(Identity::Filesystem { device, inode })
    }

    /// Identity for a file that has not been stat'ed, keyed on its raw path.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        Self(Identity::Path(path.to_path_buf()))
    }

    /// The `(device, inode)` this identity was built from, when it was.
    ///
    /// Exists so a catalogue can be written to disk and read back as the *same*
    /// identity. Nothing else should need it: comparing two `MediaId`s is what
    /// the rest of the suite does.
    #[must_use]
    pub fn filesystem_parts(&self) -> Option<(u64, u64)> {
        match self.0 {
            Identity::Filesystem { device, inode } => Some((device, inode)),
            Identity::Path(_) => None,
        }
    }

    /// The path this identity was built from, when it was not stat'ed.
    #[must_use]
    pub fn path_part(&self) -> Option<&Path> {
        match &self.0 {
            Identity::Path(path) => Some(path),
            Identity::Filesystem { .. } => None,
        }
    }

    /// Whether this identity came from filesystem metadata rather than a path.
    #[must_use]
    pub fn is_filesystem(&self) -> bool {
        matches!(self.0, Identity::Filesystem { .. })
    }
}

/// The three kinds Fluorita plays. Anything else is not media to this crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
}

impl MediaKind {
    /// Classifies by filename extension, ASCII-case-insensitively.
    ///
    /// Extension is a hint, not proof: the app may later feed a real system MIME
    /// string through its own lookup. Nothing downstream may treat a classified
    /// kind as a guarantee that the bytes decode.
    #[must_use]
    pub fn from_extension(extension: &OsStr) -> Option<Self> {
        let extension = extension.to_str()?.to_ascii_lowercase();
        if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            Some(Self::Image)
        } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            Some(Self::Video)
        } else if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
            Some(Self::Audio)
        } else {
            None
        }
    }

    /// Classifies a path by its extension. A directory-looking path or a name
    /// with no extension is simply not media.
    #[must_use]
    pub fn classify_path(path: &Path) -> Option<Self> {
        path.extension().and_then(Self::from_extension)
    }

    /// Where a static freedesktop thumbnail for this kind comes from.
    #[must_use]
    pub fn artwork_origin(self) -> ArtworkOrigin {
        match self {
            Self::Image => ArtworkOrigin::ImageDownscale,
            Self::Video => ArtworkOrigin::VideoPoster,
            Self::Audio => ArtworkOrigin::EmbeddedCover,
        }
    }

    /// What the minimal player may show for this kind. An image has no
    /// transport, so a host that draws a seek bar for one is lying.
    #[must_use]
    pub fn capabilities(self) -> MediaCapabilities {
        match self {
            Self::Image => MediaCapabilities {
                timed: false,
                seekable: false,
                has_audio: false,
                has_video: true,
            },
            Self::Video => MediaCapabilities {
                timed: true,
                seekable: true,
                has_audio: true,
                has_video: true,
            },
            Self::Audio => MediaCapabilities {
                timed: true,
                seekable: true,
                has_audio: true,
                has_video: false,
            },
        }
    }

    /// Whether the kind belongs in the Gallery projection (Music takes audio).
    #[must_use]
    pub const fn is_gallery(self) -> bool {
        matches!(self, Self::Image | Self::Video)
    }
}

/// How a static PNG thumbnail is produced for a kind. A live trailer is a
/// different thing entirely and is never published under these names — see
/// [`crate::preview`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkOrigin {
    /// A scaled read of the image itself; the toolkit can already do this.
    ImageDownscale,
    /// One representative frame of the video.
    VideoPoster,
    /// Cover art embedded in the audio file's tags; may legitimately be absent.
    EmbeddedCover,
}

/// The transport a media kind can support at all, before the engine reports
/// what this particular file actually offers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaCapabilities {
    /// Has a position and a duration, so play/pause is meaningful.
    pub timed: bool,
    /// Accepts a seek request.
    pub seekable: bool,
    /// Can produce sound, so a volume control is meaningful.
    pub has_audio: bool,
    /// Draws visible content in the player surface.
    pub has_video: bool,
}

// Images: at least the set Siderita already generates thumbnails for, so the
// two projects never disagree on what "an image" is.
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "jxl", "heic", "heif",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "wmv", "flv", "mpg", "mpeg", "m2ts", "mts", "ts",
    "ogv", "3gp",
];

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "wav", "m4a", "aac", "wma", "aiff", "aif", "ape", "wv",
    "mka",
];

#[cfg(test)]
mod tests {
    use super::{ArtworkOrigin, MediaId, MediaKind};
    use std::path::Path;

    #[test]
    fn classification_is_case_insensitive_and_kind_specific() {
        assert_eq!(
            MediaKind::classify_path(Path::new("/m/Photo.JPEG")),
            Some(MediaKind::Image)
        );
        assert_eq!(
            MediaKind::classify_path(Path::new("/m/clip.mkv")),
            Some(MediaKind::Video)
        );
        assert_eq!(
            MediaKind::classify_path(Path::new("/m/song.flac")),
            Some(MediaKind::Audio)
        );
        assert_eq!(MediaKind::classify_path(Path::new("/m/notes.txt")), None);
        assert_eq!(MediaKind::classify_path(Path::new("/m/README")), None);
    }

    #[test]
    fn every_image_kind_siderita_generates_for_is_media_here() {
        // Mirrors the list in siderita/cpp/thumbnailprovider.cpp.
        for extension in [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "jxl",
            "heic", "heif",
        ] {
            assert_eq!(
                MediaKind::classify_path(Path::new(&format!("/m/file.{extension}"))),
                Some(MediaKind::Image),
                "{extension} must classify as an image"
            );
        }
    }

    #[test]
    fn an_image_offers_no_transport() {
        let image = MediaKind::Image.capabilities();
        assert!(!image.timed);
        assert!(!image.seekable);
        assert!(!image.has_audio);

        let audio = MediaKind::Audio.capabilities();
        assert!(audio.timed && audio.seekable && audio.has_audio);
        assert!(!audio.has_video);
    }

    #[test]
    fn artwork_origin_follows_the_kind() {
        assert_eq!(
            MediaKind::Video.artwork_origin(),
            ArtworkOrigin::VideoPoster
        );
        assert_eq!(
            MediaKind::Audio.artwork_origin(),
            ArtworkOrigin::EmbeddedCover
        );
        assert_eq!(
            MediaKind::Image.artwork_origin(),
            ArtworkOrigin::ImageDownscale
        );
    }

    #[test]
    fn filesystem_identity_ignores_the_name_and_path_identity_does_not() {
        assert_eq!(
            MediaId::filesystem(66, 1234),
            MediaId::filesystem(66, 1234),
            "the same inode is the same media after a rename"
        );
        assert_ne!(MediaId::filesystem(66, 1234), MediaId::filesystem(67, 1234));
        assert_ne!(
            MediaId::from_path(Path::new("/m/a.mp3")),
            MediaId::from_path(Path::new("/m/b.mp3"))
        );
        assert!(MediaId::filesystem(66, 1).is_filesystem());
        assert!(!MediaId::from_path(Path::new("/m/a.mp3")).is_filesystem());
    }

    #[test]
    fn an_identity_survives_being_written_down_and_read_back() {
        let filesystem = MediaId::filesystem(66, 1234);
        let (device, inode) = filesystem.filesystem_parts().expect("filesystem identity");
        assert_eq!(MediaId::filesystem(device, inode), filesystem);
        assert_eq!(filesystem.path_part(), None);

        let by_path = MediaId::from_path(Path::new("/m/a.mp3"));
        let path = by_path.path_part().expect("path identity");
        assert_eq!(MediaId::from_path(path), by_path);
        assert_eq!(by_path.filesystem_parts(), None);
    }
}
