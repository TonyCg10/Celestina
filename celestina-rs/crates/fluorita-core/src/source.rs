//! Configured library roots.
//!
//! Fluorita reads media out of roots the user configured — it is not a file
//! manager and never crawls the filesystem at large. This module owns which
//! roots exist, which kinds each one contributes, whether a given path belongs
//! to one and which of them a projection is scoped to; the scan that walks them
//! and the file they are stored in both live outside this crate.
//!
//! The roots are the axis the library is navigated by, so a [`SourceId`] is not
//! a within-session convenience: the stored catalogue keys every record by one.
//! [`SourceSet::restore`] therefore exists beside [`SourceSet::add`], so a
//! configuration read back from disk keeps the identities its records already
//! refer to instead of being renumbered by load order.

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

    /// The label the sidebar shows: the root's final component, or the whole
    /// path when it has none. Lossy only for display — the scan and every
    /// projection use [`MediaSource::root`], never this.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.root
            .file_name()
            .unwrap_or(self.root.as_os_str())
            .to_string_lossy()
            .into_owned()
    }

    /// Whether this root both contains `path` and wants that kind.
    #[must_use]
    pub fn accepts(&self, path: &Path, kind: MediaKind) -> bool {
        self.kinds.contains(kind) && path.starts_with(&self.root)
    }
}

/// Which configured roots a projection covers.
///
/// The library is navigated by source, so every projection needs this; making
/// it an argument rather than a filter applied afterwards keeps the decision
/// with the records instead of leaving each host to re-derive it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceScope {
    /// Every configured root. What a whole-library operation reads.
    #[default]
    All,
    /// One root, whether or not it is still configured. A scope naming a root
    /// that was just removed simply matches nothing.
    One(SourceId),
}

impl SourceScope {
    #[must_use]
    pub const fn accepts(self, source: SourceId) -> bool {
        match self {
            Self::All => true,
            Self::One(scoped) => scoped.0 == source.0,
        }
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
    /// A restored identity that another configured root already holds. Two
    /// roots sharing one handle would make every stored record ambiguous.
    DuplicateIdentity,
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

    /// Configures one root under a freshly issued identity. Rejects a relative,
    /// empty-kinded or overlapping root rather than accepting a library that
    /// would double-count files.
    pub fn add(&mut self, root: PathBuf, kinds: KindSet) -> Result<SourceId, SourceRejected> {
        let id = SourceId(self.next_id);
        self.insert(id, root, kinds)
    }

    /// Configures one root under an identity it already had.
    ///
    /// This is how a stored configuration comes back: the catalogue on disk
    /// keys its records by these handles, so reissuing them in load order would
    /// silently reassign every record to the wrong root. A stored entry is
    /// validated exactly like a new one, because a configuration file is input
    /// like any other, and a duplicate handle is refused rather than shadowed.
    pub fn restore(
        &mut self,
        id: SourceId,
        root: PathBuf,
        kinds: KindSet,
    ) -> Result<SourceId, SourceRejected> {
        if self.sources.iter().any(|source| source.id == id) {
            return Err(SourceRejected::DuplicateIdentity);
        }
        self.insert(id, root, kinds)
    }

    fn insert(
        &mut self,
        id: SourceId,
        root: PathBuf,
        kinds: KindSet,
    ) -> Result<SourceId, SourceRejected> {
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

        // Never below an identity already in use: a later `add` that reissued a
        // restored handle would produce exactly the ambiguity `restore` exists
        // to prevent.
        self.next_id = self.next_id.max(id.0.saturating_add(1));
        self.sources.push(MediaSource { id, root, kinds });
        Ok(id)
    }

    /// Stops reading a root. Returns whether anything was configured under that
    /// handle.
    ///
    /// This removes the root from the library, never from the disk. Its
    /// catalogue records stop being projected because nothing scopes to a
    /// handle that no longer exists; the files themselves are untouched, and
    /// the handle is not reissued, so adding the same folder back is a new
    /// source rather than a resurrection of stale records.
    pub fn remove(&mut self, id: SourceId) -> bool {
        let before = self.sources.len();
        self.sources.retain(|source| source.id != id);
        self.sources.len() != before
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
    fn a_removed_root_stops_owning_anything_and_never_reissues_its_handle() {
        let mut set = SourceSet::new();
        let pictures = set
            .add(PathBuf::from("/home/toni/Pictures"), KindSet::gallery())
            .expect("absolute root");

        assert!(set.remove(pictures));
        // Removing again is not an error; it simply changed nothing.
        assert!(!set.remove(pictures));
        assert!(set.is_empty());
        assert!(set
            .owner_of(Path::new("/home/toni/Pictures/a.png"), MediaKind::Image)
            .is_none());

        // Adding the same folder back is a new source. Reusing the handle would
        // resurrect the stored records of a root the user removed.
        let again = set
            .add(PathBuf::from("/home/toni/Pictures"), KindSet::gallery())
            .expect("absolute root");
        assert_ne!(again, pictures);
    }

    #[test]
    fn a_restored_configuration_keeps_the_handles_its_records_refer_to() {
        let mut set = SourceSet::new();
        let music = set
            .restore(
                super::SourceId::from_value(7),
                PathBuf::from("/home/toni/Music"),
                KindSet::audio(),
            )
            .expect("absolute root");

        assert_eq!(music.value(), 7);
        // The next issued handle cannot collide with a restored one.
        let fresh = set
            .add(PathBuf::from("/home/toni/Pictures"), KindSet::gallery())
            .expect("absolute root");
        assert!(fresh.value() > 7);
    }

    #[test]
    fn a_stored_configuration_is_validated_like_any_other_input() {
        let mut set = SourceSet::new();
        set.restore(
            super::SourceId::from_value(2),
            PathBuf::from("/home/toni/Pictures"),
            KindSet::gallery(),
        )
        .expect("absolute root");

        // The same handle twice would make every stored record ambiguous.
        assert_eq!(
            set.restore(
                super::SourceId::from_value(2),
                PathBuf::from("/home/toni/Music"),
                KindSet::audio()
            ),
            Err(SourceRejected::DuplicateIdentity)
        );
        // A stored file gets no exemption from the rules a new root obeys.
        assert_eq!(
            set.restore(
                super::SourceId::from_value(3),
                PathBuf::from("Pictures"),
                KindSet::gallery()
            ),
            Err(SourceRejected::NotAbsolute)
        );
        assert_eq!(
            set.restore(
                super::SourceId::from_value(4),
                PathBuf::from("/home/toni/Pictures/2026"),
                KindSet::gallery()
            ),
            Err(SourceRejected::Overlapping)
        );
        assert_eq!(set.sources().len(), 1);
    }

    #[test]
    fn a_scope_covers_everything_or_exactly_one_root() {
        let mut set = SourceSet::new();
        let pictures = set
            .add(PathBuf::from("/home/toni/Pictures"), KindSet::gallery())
            .expect("absolute root");
        let music = set
            .add(PathBuf::from("/home/toni/Music"), KindSet::audio())
            .expect("absolute root");

        assert!(super::SourceScope::All.accepts(pictures));
        assert!(super::SourceScope::All.accepts(music));
        assert!(super::SourceScope::One(pictures).accepts(pictures));
        assert!(!super::SourceScope::One(pictures).accepts(music));
        // A scope naming a root that was just removed matches nothing rather
        // than falling back to everything.
        set.remove(pictures);
        assert!(!super::SourceScope::One(pictures).accepts(music));
    }

    #[test]
    fn a_root_is_labelled_by_its_final_component() {
        let mut set = SourceSet::new();
        let id = set
            .add(PathBuf::from("/home/toni/Videos"), KindSet::gallery())
            .expect("absolute root");

        assert_eq!(
            set.get(id).map(super::MediaSource::display_name).as_deref(),
            Some("Videos")
        );
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
