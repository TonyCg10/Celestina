//! Configured library roots.
//!
//! Fluorita reads media out of roots the user configured — it is not a file
//! manager and never crawls the filesystem at large. This module owns which
//! roots exist, which kinds each one contributes and whether a given path
//! belongs to one; the scan that walks them lives outside this crate.

use std::path::{Path, PathBuf};

use crate::media::MediaKind;

/// The set of kinds a root contributes, without a bitflags dependency.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KindSet(u8);

impl KindSet {
    const IMAGE: u8 = 1 << 0;
    const VIDEO: u8 = 1 << 1;
    const AUDIO: u8 = 1 << 2;

    /// Nothing — a root that contributes nothing is rejected on `add`.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// The two kinds Gallery shows. Pictures and Videos both get this: phone
    /// cameras drop clips into a pictures folder, and a root that silently
    /// ignored them would look like a scanning bug.
    #[must_use]
    pub const fn gallery() -> Self {
        Self(Self::IMAGE | Self::VIDEO)
    }

    /// What Music projects.
    #[must_use]
    pub const fn audio() -> Self {
        Self(Self::AUDIO)
    }

    #[must_use]
    pub const fn all() -> Self {
        Self(Self::IMAGE | Self::VIDEO | Self::AUDIO)
    }

    #[must_use]
    pub const fn with(self, kind: MediaKind) -> Self {
        Self(self.0 | Self::bit(kind))
    }

    #[must_use]
    pub const fn contains(self, kind: MediaKind) -> bool {
        self.0 & Self::bit(kind) != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    const fn bit(kind: MediaKind) -> u8 {
        match kind {
            MediaKind::Image => Self::IMAGE,
            MediaKind::Video => Self::VIDEO,
            MediaKind::Audio => Self::AUDIO,
        }
    }
}

/// Opaque handle for one configured root, stable for the lifetime of a
/// [`SourceSet`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(u32);

impl SourceId {
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Rebuilds a handle from a stored value.
    ///
    /// Only a catalogue being read back should need this: within a session,
    /// handles come from [`SourceSet::add`]. A value that no longer names a
    /// configured root simply owns no source, which is what
    /// [`SourceSet::get`] already reports.
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        Self(value)
    }
}

/// One configured root and the kinds it contributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaSource {
    id: SourceId,
    root: PathBuf,
    kinds: KindSet,
}

impl MediaSource {
    #[must_use]
    pub const fn id(&self) -> SourceId {
        self.id
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn kinds(&self) -> KindSet {
        self.kinds
    }

    /// Whether this root both contains `path` and wants that kind.
    #[must_use]
    pub fn accepts(&self, path: &Path, kind: MediaKind) -> bool {
        self.kinds.contains(kind) && path.starts_with(&self.root)
    }
}

/// Why a root could not be configured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRejected {
    /// A relative root cannot be resolved without a working directory, and a
    /// library that silently depended on one would scan different files per run.
    NotAbsolute,
    /// A root that contributes no kind would only cost scan time.
    NoKinds,
    /// The same root, or a root already covered by (or covering) another one:
    /// nesting would catalogue the same file under two sources.
    Overlapping,
}

/// The configured roots, in configuration order.
#[derive(Clone, Debug, Default)]
pub struct SourceSet {
    sources: Vec<MediaSource>,
    next_id: u32,
}

impl SourceSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures one root. Rejects a relative, empty-kinded or overlapping
    /// root rather than accepting a library that would double-count files.
    pub fn add(&mut self, root: PathBuf, kinds: KindSet) -> Result<SourceId, SourceRejected> {
        if !root.is_absolute() {
            return Err(SourceRejected::NotAbsolute);
        }
        if kinds.is_empty() {
            return Err(SourceRejected::NoKinds);
        }
        if self
            .sources
            .iter()
            .any(|source| root.starts_with(&source.root) || source.root.starts_with(&root))
        {
            return Err(SourceRejected::Overlapping);
        }

        let id = SourceId(self.next_id);
        self.next_id += 1;
        self.sources.push(MediaSource { id, root, kinds });
        Ok(id)
    }

    #[must_use]
    pub fn get(&self, id: SourceId) -> Option<&MediaSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    /// The configured roots, in the order they were added.
    #[must_use]
    pub fn sources(&self) -> &[MediaSource] {
        &self.sources
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The root that owns `path` for `kind`, if any. Roots cannot nest, so at
    /// most one matches.
    #[must_use]
    pub fn owner_of(&self, path: &Path, kind: MediaKind) -> Option<&MediaSource> {
        self.sources
            .iter()
            .find(|source| source.accepts(path, kind))
    }

    /// Seeds the initial library from the XDG media directories that exist.
    ///
    /// The caller resolves and existence-checks the directories — this crate
    /// performs no IO — and a missing one is simply not configured.
    #[must_use]
    pub fn seeded_from(dirs: &XdgMediaDirs) -> Self {
        let mut set = Self::new();
        for (root, kinds) in [
            (dirs.pictures.as_ref(), KindSet::gallery()),
            (dirs.videos.as_ref(), KindSet::gallery()),
            (dirs.music.as_ref(), KindSet::audio()),
        ] {
            if let Some(root) = root {
                // A rejected seed (relative, or nested inside an earlier one) is
                // dropped: seeding must never fail the first run.
                let _ = set.add(root.clone(), kinds);
            }
        }
        set
    }
}

/// The XDG user directories Fluorita seeds from, as resolved by the caller.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XdgMediaDirs {
    pub pictures: Option<PathBuf>,
    pub videos: Option<PathBuf>,
    pub music: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::{KindSet, SourceRejected, SourceSet, XdgMediaDirs};
    use crate::media::MediaKind;
    use std::path::{Path, PathBuf};

    fn dirs() -> XdgMediaDirs {
        XdgMediaDirs {
            pictures: Some(PathBuf::from("/home/toni/Imágenes")),
            videos: Some(PathBuf::from("/home/toni/Vídeos")),
            music: Some(PathBuf::from("/home/toni/Música")),
        }
    }

    #[test]
    fn seeding_configures_every_directory_that_exists() {
        let set = SourceSet::seeded_from(&dirs());

        assert_eq!(set.sources().len(), 3);
        assert!(set
            .owner_of(Path::new("/home/toni/Imágenes/a.png"), MediaKind::Image)
            .is_some());
        // A clip inside Pictures still belongs to Gallery.
        assert!(set
            .owner_of(Path::new("/home/toni/Imágenes/clip.mp4"), MediaKind::Video)
            .is_some());
        // Music is audio-only; a video dropped there is not catalogued.
        assert!(set
            .owner_of(Path::new("/home/toni/Música/clip.mp4"), MediaKind::Video)
            .is_none());
        assert!(set
            .owner_of(Path::new("/home/toni/Música/song.flac"), MediaKind::Audio)
            .is_some());
    }

    #[test]
    fn a_missing_directory_is_simply_not_configured() {
        let set = SourceSet::seeded_from(&XdgMediaDirs {
            pictures: Some(PathBuf::from("/home/toni/Imágenes")),
            ..XdgMediaDirs::default()
        });

        assert_eq!(set.sources().len(), 1);
        assert!(set
            .owner_of(Path::new("/home/toni/Música/song.flac"), MediaKind::Audio)
            .is_none());
    }

    #[test]
    fn a_path_outside_every_root_has_no_owner() {
        let set = SourceSet::seeded_from(&dirs());

        assert!(set
            .owner_of(Path::new("/etc/shadow.png"), MediaKind::Image)
            .is_none());
        // Prefix matching is by path component, not by string.
        assert!(set
            .owner_of(
                Path::new("/home/toni/Imágenes-privadas/a.png"),
                MediaKind::Image
            )
            .is_none());
    }

    #[test]
    fn relative_empty_and_nested_roots_are_rejected() {
        let mut set = SourceSet::new();

        assert_eq!(
            set.add(PathBuf::from("Imágenes"), KindSet::gallery()),
            Err(SourceRejected::NotAbsolute)
        );
        assert_eq!(
            set.add(PathBuf::from("/home/toni/Imágenes"), KindSet::empty()),
            Err(SourceRejected::NoKinds)
        );
        assert!(set
            .add(PathBuf::from("/home/toni/Imágenes"), KindSet::gallery())
            .is_ok());
        assert_eq!(
            set.add(
                PathBuf::from("/home/toni/Imágenes/2026"),
                KindSet::gallery()
            ),
            Err(SourceRejected::Overlapping)
        );
        assert_eq!(
            set.add(PathBuf::from("/home/toni"), KindSet::all()),
            Err(SourceRejected::Overlapping)
        );
        assert_eq!(set.sources().len(), 1);
    }

    #[test]
    fn a_stored_source_handle_round_trips() {
        let mut set = SourceSet::new();
        let identifier = set
            .add(PathBuf::from("/home/toni/Música"), KindSet::audio())
            .expect("absolute root");

        assert_eq!(super::SourceId::from_value(identifier.value()), identifier);
        // A handle from a previous configuration names nothing now.
        assert!(set.get(super::SourceId::from_value(99)).is_none());
    }

    #[test]
    fn kind_sets_answer_per_kind() {
        assert!(KindSet::gallery().contains(MediaKind::Image));
        assert!(KindSet::gallery().contains(MediaKind::Video));
        assert!(!KindSet::gallery().contains(MediaKind::Audio));
        assert!(KindSet::empty().is_empty());
        assert!(KindSet::empty()
            .with(MediaKind::Audio)
            .contains(MediaKind::Audio));
        assert!(KindSet::all().contains(MediaKind::Audio));
    }

    #[test]
    fn source_ids_stay_stable_as_roots_are_added() {
        let mut set = SourceSet::new();
        let pictures = set
            .add(PathBuf::from("/home/toni/Imágenes"), KindSet::gallery())
            .expect("absolute root");
        let music = set
            .add(PathBuf::from("/home/toni/Música"), KindSet::audio())
            .expect("absolute root");

        assert_ne!(pictures, music);
        assert_eq!(
            set.get(pictures).map(super::MediaSource::root),
            Some(Path::new("/home/toni/Imágenes"))
        );
        assert!(!set.is_empty());
    }
}
