//! The freedesktop thumbnail contract Siderita already consumes.
//!
//! Siderita is the authority here, because it is the consumer that must not
//! change: it derives its cache entry as
//! `md5(QUrl::fromLocalFile(absolutePath).toEncoded())`, lowercase hex, under
//! `<cache root>/large/<key>.png`, and reuses any entry at least as new as its
//! source. Fluorita becomes the producer for video posters and audio covers, so
//! the derivation is frozen here with golden vectors measured from Qt 6 itself.
//!
//! Everything in this module is a decision, not IO: the caller opens files,
//! writes bytes and renames.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use celestina_core::percent;

/// The "large" freedesktop thumbnail edge, in pixels — the size Siderita reads
/// and writes.
pub const LARGE_THUMBNAIL_PIXELS: u32 = 256;

/// The freedesktop size directory a thumbnail lives in. Only `large` is part of
/// the contract the suite shares today; the enum exists so adding `normal` later
/// cannot silently change existing paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThumbnailSize {
    Large,
}

impl ThumbnailSize {
    #[must_use]
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Large => "large",
        }
    }

    #[must_use]
    pub const fn max_pixels(self) -> u32 {
        match self {
            Self::Large => LARGE_THUMBNAIL_PIXELS,
        }
    }
}

/// The canonical `file://` URI for an absolute local path, byte-safe.
///
/// Returns `None` for a relative path rather than emitting a key that would
/// address the wrong file. On Unix the raw bytes are encoded without a lossy
/// Unicode conversion, so a malformed filename stays addressable and
/// deterministic — that extension is Fluorita's, not Qt's.
#[must_use]
pub fn file_uri(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let encoded = percent::encode_qt_path(&percent::path_bytes(path));
    Some(format!("file://{encoded}"))
}

/// The cache entry name for a URI: MD5 of its bytes as lowercase hex.
///
/// MD5 is what the spec mandates for this key; it is an interoperability
/// contract, never a security claim.
#[must_use]
pub fn cache_key(file_uri: &str) -> String {
    format!("{:x}", md5::compute(file_uri.as_bytes()))
}

/// `<cache root>/large/<key>.png` for an absolute source path, or `None` when
/// the path cannot produce a canonical URI.
#[must_use]
pub fn large_thumbnail_path(cache_root: &Path, source: &Path) -> Option<PathBuf> {
    thumbnail_path(cache_root, source, ThumbnailSize::Large)
}

/// Like [`large_thumbnail_path`] for an explicit size directory.
#[must_use]
pub fn thumbnail_path(cache_root: &Path, source: &Path, size: ThumbnailSize) -> Option<PathBuf> {
    let uri = file_uri(source)?;
    Some(
        cache_root
            .join(size.directory())
            .join(format!("{}.png", cache_key(&uri))),
    )
}

/// Whether a cached thumbnail may be shown, or must be produced again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkValidity {
    /// Nothing cached for this key yet.
    Missing,
    /// Cached, but older than the file it depicts.
    Stale,
    /// Cached and at least as new as its source.
    Fresh,
}

impl ArtworkValidity {
    /// A thumbnail is always written after its source, so an edit that pushes
    /// the source mtime past the cache entry is exactly what forces a
    /// regenerate. This keys off filesystem mtimes, like Siderita, and therefore
    /// also honours entries other producers wrote.
    #[must_use]
    pub fn evaluate(source_mtime: SystemTime, cache_mtime: Option<SystemTime>) -> Self {
        match cache_mtime {
            None => Self::Missing,
            Some(cached) if cached >= source_mtime => Self::Fresh,
            Some(_) => Self::Stale,
        }
    }

    #[must_use]
    pub fn needs_generation(self) -> bool {
        !matches!(self, Self::Fresh)
    }
}

/// Everything an IO layer needs to publish one thumbnail without a reader ever
/// seeing half a PNG: encode into `temporary`, set the two text keys, restrict
/// the mode, then rename onto `final_path`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtworkPublication {
    /// Where the entry must end up.
    pub final_path: PathBuf,
    /// The unique sibling to write first; a failed attempt removes only this.
    pub temporary_path: PathBuf,
    /// The `Thumb::URI` text key: the same URI the entry name hashes.
    pub thumb_uri: String,
    /// The `Thumb::MTime` text key: the source mtime in whole seconds, or
    /// `None` when the source timestamp predates the epoch and cannot be
    /// spelled by the spec.
    pub thumb_mtime_seconds: Option<i64>,
    /// Owner-only permissions; a thumbnail can disclose the content of a
    /// private file.
    pub mode: u32,
}

impl ArtworkPublication {
    /// Builds the descriptor for `source` under `cache_root`.
    ///
    /// `uniquifier` distinguishes concurrent writers of the same entry — a
    /// worker id or a counter — and is spelled into the temporary name, so two
    /// producers never share a partial file.
    #[must_use]
    pub fn prepare(
        cache_root: &Path,
        source: &Path,
        source_mtime: SystemTime,
        uniquifier: u64,
    ) -> Option<Self> {
        let thumb_uri = file_uri(source)?;
        let final_path = cache_root
            .join(ThumbnailSize::Large.directory())
            .join(format!("{}.png", cache_key(&thumb_uri)));
        let mut temporary = final_path.clone().into_os_string();
        temporary.push(format!(".tmp-{uniquifier:x}"));

        Some(Self {
            final_path,
            temporary_path: PathBuf::from(temporary),
            thumb_uri,
            thumb_mtime_seconds: unix_seconds(source_mtime),
            mode: 0o600,
        })
    }

    /// The directory that must exist before the temporary file is created.
    #[must_use]
    pub fn parent_directory(&self) -> &Path {
        self.final_path.parent().unwrap_or(Path::new("."))
    }
}

/// Whole seconds since the Unix epoch, or `None` for a timestamp before it.
#[must_use]
pub fn unix_seconds(time: SystemTime) -> Option<i64> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
}

#[cfg(test)]
mod tests {
    use super::{
        cache_key, file_uri, large_thumbnail_path, unix_seconds, ArtworkPublication,
        ArtworkValidity, ThumbnailSize, LARGE_THUMBNAIL_PIXELS,
    };
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    // Measured from Qt 6 (`QUrl::fromLocalFile().toEncoded()` + MD5), not from
    // documentation. These are the whole pipeline — encode, digest, hex — and
    // they are the proof that Siderita needs no change to read what Fluorita
    // writes. Do not "fix" a divergence from GLib here: Siderita is the
    // consumer whose contract must hold.
    const QT_VECTORS: &[(&str, &str, &str)] = &[
        (
            "/home/toni/clip.mp4",
            "file:///home/toni/clip.mp4",
            "053a0fcc87f42f4b9e33ebc076783935",
        ),
        (
            "/home/toni/a b.mp4",
            "file:///home/toni/a%20b.mp4",
            "2275fc454ce0dc91ae3cfe0fe70eebb0",
        ),
        (
            "/home/toni/Vídeos/canción ñ.mp3",
            "file:///home/toni/V%C3%ADdeos/canci%C3%B3n%20%C3%B1.mp3",
            "70e33e372a4c9c1f732b967ed9df9df2",
        ),
        (
            "/home/toni/emoji 🎬.mkv",
            "file:///home/toni/emoji%20%F0%9F%8E%AC.mkv",
            "cd28796eed0cf805feb69ccd90f44154",
        ),
        (
            "/home/toni/weird#hash?q.png",
            "file:///home/toni/weird%23hash%3Fq.png",
            "40cd8e865ec08f622a53eac35cc64ab7",
        ),
        (
            "/home/toni/quote'and\"dq.jpg",
            "file:///home/toni/quote'and%22dq.jpg",
            "196259b0dd86fb879c3413eae4843ca5",
        ),
        (
            "/home/toni/paren(1)[2]{3}.webm",
            "file:///home/toni/paren(1)%5B2%5D%7B3%7D.webm",
            "db6e38d9f643b4cc7e62bd380294ee40",
        ),
        (
            "/home/toni/plus+amp&eq=semi;.flac",
            "file:///home/toni/plus+amp&eq=semi;.flac",
            "6708bf323aa2562b306e70f3ce85a20c",
        ),
        (
            "/home/toni/percent%20literal.avi",
            "file:///home/toni/percent%2520literal.avi",
            "d089d4131f54cb8a8e9624865b79052c",
        ),
        (
            "/home/toni/tilde~dash-under_dot..ogg",
            "file:///home/toni/tilde~dash-under_dot..ogg",
            "58e7b3568a80fc5afc683371f4f5657d",
        ),
        (
            "/home/toni/at@colon:comma,.opus",
            "file:///home/toni/at@colon:comma,.opus",
            "3e1260f33bf697b60462df77fcc4912a",
        ),
        (
            "/home/toni/star*bang!dollar$.wav",
            "file:///home/toni/star*bang!dollar$.wav",
            "c27427c121c671385717636ecbd22cf3",
        ),
        (
            "/home/toni/back\\slash.mp4",
            "file:///home/toni/back%5Cslash.mp4",
            "2b7c730c72cba7ae2d4f0cd19908d2d4",
        ),
        (
            "/home/toni/pipe|caret^tick`.mov",
            "file:///home/toni/pipe%7Ccaret%5Etick%60.mov",
            "7fa069aa9fd9701a792495f9134953a3",
        ),
        (
            "/home/toni/less<greater>.m4a",
            "file:///home/toni/less%3Cgreater%3E.m4a",
            "a6eac6b1d52cc409d44e60afa31fc1bf",
        ),
    ];

    #[test]
    fn golden_vectors_match_qt_byte_for_byte() {
        for (path, expected_uri, expected_key) in QT_VECTORS {
            let uri = file_uri(Path::new(path)).expect("absolute path has a URI");
            assert_eq!(&uri, expected_uri, "URI for {path}");
            assert_eq!(&cache_key(&uri), expected_key, "cache key for {path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_name_stays_addressable() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // Fluorita's own Unix extension, not a Qt-measured vector: every
        // non-ASCII byte is escaped without a lossy Unicode conversion.
        let path = PathBuf::from(OsStr::from_bytes(b"/home/toni/bad-\xFF.mp4"));
        let uri = file_uri(&path).expect("absolute path has a URI");

        assert_eq!(uri, "file:///home/toni/bad-%FF.mp4");
        assert_eq!(cache_key(&uri), "ff7e7879531a24532843de4e2ef3ead9");
    }

    #[test]
    fn a_relative_path_has_no_key_instead_of_a_wrong_one() {
        assert_eq!(file_uri(Path::new("clip.mp4")), None);
        assert_eq!(
            large_thumbnail_path(
                Path::new("/home/toni/.cache/thumbnails"),
                Path::new("clip.mp4")
            ),
            None
        );
    }

    #[test]
    fn the_cache_path_is_the_one_siderita_reads() {
        let path = large_thumbnail_path(
            Path::new("/home/toni/.cache/thumbnails"),
            Path::new("/home/toni/clip.mp4"),
        )
        .expect("absolute source");

        assert_eq!(
            path,
            PathBuf::from(
                "/home/toni/.cache/thumbnails/large/053a0fcc87f42f4b9e33ebc076783935.png"
            )
        );
        assert_eq!(ThumbnailSize::Large.directory(), "large");
        assert_eq!(ThumbnailSize::Large.max_pixels(), LARGE_THUMBNAIL_PIXELS);
    }

    #[test]
    fn validity_regenerates_only_when_the_source_moved_ahead() {
        let source = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let older = SystemTime::UNIX_EPOCH + Duration::from_secs(999);
        let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(1_001);

        assert_eq!(
            ArtworkValidity::evaluate(source, None),
            ArtworkValidity::Missing
        );
        assert_eq!(
            ArtworkValidity::evaluate(source, Some(older)),
            ArtworkValidity::Stale
        );
        assert_eq!(
            ArtworkValidity::evaluate(source, Some(source)),
            ArtworkValidity::Fresh
        );
        assert_eq!(
            ArtworkValidity::evaluate(source, Some(newer)),
            ArtworkValidity::Fresh
        );
        assert!(ArtworkValidity::Missing.needs_generation());
        assert!(ArtworkValidity::Stale.needs_generation());
        assert!(!ArtworkValidity::Fresh.needs_generation());
    }

    #[test]
    fn publication_writes_a_unique_sibling_before_the_entry() {
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let plan = ArtworkPublication::prepare(
            Path::new("/home/toni/.cache/thumbnails"),
            Path::new("/home/toni/clip.mp4"),
            mtime,
            0x2a,
        )
        .expect("absolute source");

        assert_eq!(
            plan.final_path,
            PathBuf::from(
                "/home/toni/.cache/thumbnails/large/053a0fcc87f42f4b9e33ebc076783935.png"
            )
        );
        assert_eq!(
            plan.temporary_path,
            PathBuf::from(
                "/home/toni/.cache/thumbnails/large/053a0fcc87f42f4b9e33ebc076783935.png.tmp-2a"
            )
        );
        assert_ne!(plan.temporary_path, plan.final_path);
        assert_eq!(
            plan.parent_directory(),
            Path::new("/home/toni/.cache/thumbnails/large")
        );
        assert_eq!(plan.thumb_uri, "file:///home/toni/clip.mp4");
        assert_eq!(plan.thumb_mtime_seconds, Some(1_700_000_000));
        assert_eq!(plan.mode, 0o600);

        let other = ArtworkPublication::prepare(
            Path::new("/home/toni/.cache/thumbnails"),
            Path::new("/home/toni/clip.mp4"),
            mtime,
            0x2b,
        )
        .expect("absolute source");
        assert_ne!(plan.temporary_path, other.temporary_path);
        assert_eq!(plan.final_path, other.final_path);
    }

    #[test]
    fn a_pre_epoch_timestamp_has_no_spec_mtime_instead_of_a_wrong_one() {
        let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(unix_seconds(before), None);
        assert_eq!(unix_seconds(SystemTime::UNIX_EPOCH), Some(0));
    }

    #[test]
    fn md5_matches_the_rfc_1321_vectors() {
        assert_eq!(cache_key(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(cache_key("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            cache_key("message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }
}
