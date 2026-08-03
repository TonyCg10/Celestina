//! The persistent catalogue: what the library knows about each media file.
//!
//! The catalogue records identity, location, kind, source identity (size and
//! mtime) and whatever metadata was extracted — with every metadata field
//! optional, because an untagged file must stay visible instead of quietly
//! disappearing. Removing a record never removes a file: reconciliation marks a
//! record unavailable, and forgetting it is a separate, explicit decision.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::media::{MediaId, MediaKind};
use crate::source::SourceId;

/// Size and mtime, the pair that decides whether derived resources are stale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIdentity {
    pub size: u64,
    pub modified: SystemTime,
}

impl SourceIdentity {
    #[must_use]
    pub const fn new(size: u64, modified: SystemTime) -> Self {
        Self { size, modified }
    }

    /// Whether derived work produced for `self` still describes `current`.
    #[must_use]
    pub fn still_describes(self, current: SourceIdentity) -> bool {
        self == current
    }
}

/// Extracted tags. Every field is optional and an absent one stays absent:
/// Music shows an honest "unknown" bucket rather than inventing a name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub duration: Option<Duration>,
}

impl MediaMetadata {
    /// The name Music groups by: the album artist when tagged, else the track
    /// artist, else nothing — the unknown bucket.
    #[must_use]
    pub fn grouping_artist(&self) -> Option<&str> {
        self.album_artist
            .as_deref()
            .or(self.artist.as_deref())
            .map(str::trim)
            .filter(|name| !name.is_empty())
    }

    /// The album title, trimmed, or nothing.
    #[must_use]
    pub fn album_title(&self) -> Option<&str> {
        self.album
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
    }

    /// The tagged title, trimmed, or nothing. A host with nothing to show falls
    /// back to the filename — the catalogue does not fabricate a title.
    #[must_use]
    pub fn track_title(&self) -> Option<&str> {
        self.title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
    }
}

/// Whether the file behind a record was seen by the most recent reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Available,
    /// The file was not found. The record is kept — a removable drive or an
    /// unmounted share comes back — and the source file is untouched.
    Missing,
}

/// One catalogued media file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRecord {
    id: MediaId,
    source: SourceId,
    path: PathBuf,
    kind: MediaKind,
    identity: SourceIdentity,
    availability: Availability,
    metadata: MediaMetadata,
}

impl MediaRecord {
    #[must_use]
    pub fn new(
        id: MediaId,
        source: SourceId,
        path: PathBuf,
        kind: MediaKind,
        identity: SourceIdentity,
    ) -> Self {
        Self {
            id,
            source,
            path,
            kind,
            identity,
            availability: Availability::Available,
            metadata: MediaMetadata::default(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: MediaMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Restores a recorded availability.
    ///
    /// For a catalogue read back from disk: an entry that was missing when the
    /// app closed must not come back looking available before a scan has
    /// actually seen it again.
    #[must_use]
    pub const fn with_availability(mut self, availability: Availability) -> Self {
        self.availability = availability;
        self
    }

    #[must_use]
    pub fn id(&self) -> &MediaId {
        &self.id
    }

    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> MediaKind {
        self.kind
    }

    #[must_use]
    pub const fn identity(&self) -> SourceIdentity {
        self.identity
    }

    #[must_use]
    pub const fn availability(&self) -> Availability {
        self.availability
    }

    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.availability, Availability::Available)
    }

    #[must_use]
    pub const fn metadata(&self) -> &MediaMetadata {
        &self.metadata
    }

    /// The label a host shows when nothing is tagged: the filename without its
    /// extension. Lossy only for display — never rebuild a path from it.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.metadata.track_title().map_or_else(
            || {
                self.path
                    .file_stem()
                    .unwrap_or(self.path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            },
            str::to_owned,
        )
    }
}

/// What one reconciliation pass changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReconcileSummary {
    /// Records whose file was not seen this pass.
    pub marked_missing: usize,
    /// Records that were missing and came back.
    pub restored: usize,
}

/// What absorbing one scan changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AbsorbSummary {
    /// Files the catalogue had never seen.
    pub added: usize,
    /// Files whose size or mtime moved, so anything derived from their content
    /// — tags above all — is no longer trustworthy and was dropped.
    pub replaced: usize,
    /// Files that are exactly as they were: their extracted metadata is kept,
    /// which is the entire reason for persisting a catalogue.
    pub unchanged: usize,
    pub marked_missing: usize,
    pub restored: usize,
}

/// The catalogue itself: identity-keyed, deterministically ordered.
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    records: BTreeMap<MediaId, MediaRecord>,
}

impl Catalogue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces a record, returning the one it replaced. A replaced
    /// record's availability does not survive: the caller just saw the file.
    pub fn upsert(&mut self, record: MediaRecord) -> Option<MediaRecord> {
        self.records.insert(record.id.clone(), record)
    }

    #[must_use]
    pub fn get(&self, id: &MediaId) -> Option<&MediaRecord> {
        self.records.get(id)
    }

    /// The record whose file lives at `path`, if any.
    ///
    /// A linear walk on purpose: the catalogue is keyed by identity, which is
    /// what survives a rename, and a second index by path would be a second
    /// truth to keep in step. An incremental update asks this once per changed
    /// file, not once per file in the library.
    #[must_use]
    pub fn find_by_path(&self, path: &Path) -> Option<&MediaRecord> {
        self.records.values().find(|record| record.path() == path)
    }

    /// Marks one record unavailable — its file went away, and nothing else was
    /// looked at. This is what an incremental update uses instead of
    /// [`Catalogue::reconcile`], which needs a complete pass to be truthful.
    ///
    /// Returns whether anything changed. The source file is never touched.
    pub fn mark_missing(&mut self, id: &MediaId) -> bool {
        match self.records.get_mut(id) {
            Some(record) if record.availability == Availability::Available => {
                record.availability = Availability::Missing;
                true
            }
            _ => false,
        }
    }

    /// Drops a record from the library. This is the user forgetting an entry —
    /// it never touches the file the record described.
    pub fn forget(&mut self, id: &MediaId) -> Option<MediaRecord> {
        self.records.remove(id)
    }

    /// Every record, in stable identity order.
    pub fn records(&self) -> impl Iterator<Item = &MediaRecord> {
        self.records.values()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Folds a scan into what is already known.
    ///
    /// The rule that matters: a file whose identity *and* size/mtime are
    /// unchanged keeps the metadata already extracted for it. Re-reading tags
    /// costs a decoder probe per track, so a library that threw them away on
    /// every launch would be paying minutes to learn what it already knew. A
    /// file that changed underneath loses that metadata, because it now
    /// describes bytes that are gone.
    ///
    /// `complete` must be false for a truncated or cancelled scan: only a pass
    /// that actually finished may conclude that a file has disappeared.
    pub fn absorb(
        &mut self,
        scanned: impl IntoIterator<Item = MediaRecord>,
        complete: bool,
    ) -> AbsorbSummary {
        let mut summary = AbsorbSummary::default();
        let mut seen: BTreeSet<MediaId> = BTreeSet::new();

        for record in scanned {
            seen.insert(record.id.clone());
            match self.records.get(&record.id) {
                Some(known) if known.identity.still_describes(record.identity) => {
                    summary.unchanged += 1;
                    // Same bytes: keep what was learned, take the new location
                    // in case it was renamed.
                    let metadata = known.metadata.clone();
                    let mut kept = record.with_metadata(metadata);
                    kept.availability = Availability::Available;
                    self.records.insert(kept.id.clone(), kept);
                }
                Some(_) => {
                    summary.replaced += 1;
                    self.records.insert(record.id.clone(), record);
                }
                None => {
                    summary.added += 1;
                    self.records.insert(record.id.clone(), record);
                }
            }
        }

        if complete {
            let reconciled = self.reconcile(&seen);
            summary.marked_missing = reconciled.marked_missing;
            summary.restored = reconciled.restored;
        }
        summary
    }

    /// Applies the result of a completed scan: everything in `seen` is
    /// available, everything else becomes [`Availability::Missing`].
    ///
    /// Only call this with the ids of a *complete* pass — a cancelled scan would
    /// mark every unvisited file missing.
    pub fn reconcile(&mut self, seen: &BTreeSet<MediaId>) -> ReconcileSummary {
        let mut summary = ReconcileSummary::default();
        for (id, record) in &mut self.records {
            match (seen.contains(id), record.availability) {
                (true, Availability::Missing) => {
                    record.availability = Availability::Available;
                    summary.restored += 1;
                }
                (false, Availability::Available) => {
                    record.availability = Availability::Missing;
                    summary.marked_missing += 1;
                }
                _ => {}
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Availability, Catalogue, MediaMetadata, MediaRecord, ReconcileSummary, SourceIdentity,
    };
    use crate::media::{MediaId, MediaKind};
    use crate::source::{KindSet, SourceSet};
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn identity(secs: u64) -> SourceIdentity {
        SourceIdentity::new(4_096, SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn record(inode: u64, path: &str, kind: MediaKind) -> MediaRecord {
        let mut sources = SourceSet::new();
        let source = sources
            .add(PathBuf::from("/home/toni/Música"), KindSet::all())
            .expect("absolute root");
        MediaRecord::new(
            MediaId::filesystem(66, inode),
            source,
            PathBuf::from(path),
            kind,
            identity(1_000),
        )
    }

    #[test]
    fn an_untagged_file_keeps_a_display_name_from_its_filename() {
        let untagged = record(1, "/home/toni/Música/01 pista.flac", MediaKind::Audio);
        assert_eq!(untagged.display_name(), "01 pista");
        assert_eq!(untagged.metadata().grouping_artist(), None);
        assert_eq!(untagged.metadata().album_title(), None);

        let tagged =
            record(2, "/home/toni/Música/x.flac", MediaKind::Audio).with_metadata(MediaMetadata {
                title: Some("  Canción  ".to_owned()),
                artist: Some("Intérprete".to_owned()),
                ..MediaMetadata::default()
            });
        assert_eq!(tagged.display_name(), "Canción");
        assert_eq!(tagged.metadata().grouping_artist(), Some("Intérprete"));
    }

    #[test]
    fn blank_tags_are_unknown_rather_than_an_empty_name() {
        let blank = MediaMetadata {
            artist: Some("   ".to_owned()),
            album: Some(String::new()),
            title: Some(" ".to_owned()),
            ..MediaMetadata::default()
        };

        assert_eq!(blank.grouping_artist(), None);
        assert_eq!(blank.album_title(), None);
        assert_eq!(blank.track_title(), None);
    }

    #[test]
    fn the_album_artist_wins_over_the_track_artist() {
        let metadata = MediaMetadata {
            artist: Some("Invitada".to_owned()),
            album_artist: Some("Grupo".to_owned()),
            ..MediaMetadata::default()
        };

        assert_eq!(metadata.grouping_artist(), Some("Grupo"));
    }

    #[test]
    fn upsert_replaces_by_identity_not_by_path() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(
            7,
            "/home/toni/Música/antiguo.flac",
            MediaKind::Audio,
        ));
        // Same inode, renamed file: one record, new path.
        let replaced =
            catalogue.upsert(record(7, "/home/toni/Música/nuevo.flac", MediaKind::Audio));

        assert!(replaced.is_some());
        assert_eq!(catalogue.len(), 1);
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 7))
                .map(|found| found.path().to_path_buf()),
            Some(PathBuf::from("/home/toni/Música/nuevo.flac"))
        );
    }

    #[test]
    fn reconciliation_marks_missing_and_restores_without_deleting() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/home/toni/Música/a.flac", MediaKind::Audio));
        catalogue.upsert(record(2, "/home/toni/Música/b.flac", MediaKind::Audio));

        let seen: BTreeSet<_> = [MediaId::filesystem(66, 1)].into_iter().collect();
        assert_eq!(
            catalogue.reconcile(&seen),
            ReconcileSummary {
                marked_missing: 1,
                restored: 0,
            }
        );
        assert_eq!(catalogue.len(), 2, "a missing file keeps its record");
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 2))
                .map(MediaRecord::availability),
            Some(Availability::Missing)
        );

        let both: BTreeSet<_> = [MediaId::filesystem(66, 1), MediaId::filesystem(66, 2)]
            .into_iter()
            .collect();
        assert_eq!(
            catalogue.reconcile(&both),
            ReconcileSummary {
                marked_missing: 0,
                restored: 1,
            }
        );
        assert!(catalogue
            .get(&MediaId::filesystem(66, 2))
            .is_some_and(MediaRecord::is_available));
    }

    #[test]
    fn records_iterate_in_a_stable_order() {
        let mut catalogue = Catalogue::new();
        for inode in [9, 3, 5] {
            catalogue.upsert(record(inode, "/home/toni/Música/x.flac", MediaKind::Audio));
        }

        let first: Vec<_> = catalogue.records().map(MediaRecord::id).cloned().collect();
        let second: Vec<_> = catalogue.records().map(MediaRecord::id).cloned().collect();

        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
    }

    #[test]
    fn forgetting_a_record_leaves_the_rest_intact() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/home/toni/Música/a.flac", MediaKind::Audio));
        catalogue.upsert(record(2, "/home/toni/Música/b.flac", MediaKind::Audio));

        assert!(catalogue.forget(&MediaId::filesystem(66, 1)).is_some());
        assert_eq!(catalogue.len(), 1);
        assert!(catalogue.forget(&MediaId::filesystem(66, 1)).is_none());
        assert!(!catalogue.is_empty());
    }

    #[test]
    fn source_identity_detects_an_edited_file() {
        assert!(identity(1_000).still_describes(identity(1_000)));
        assert!(!identity(1_000).still_describes(identity(1_001)));
        assert!(
            !identity(1_000).still_describes(SourceIdentity::new(4_097, identity(1_000).modified))
        );
    }
}

#[cfg(test)]
mod absorb_tests {
    use super::{
        AbsorbSummary, Availability, Catalogue, MediaMetadata, MediaRecord, SourceIdentity,
    };
    use crate::media::{MediaId, MediaKind};
    use crate::source::SourceId;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn record(inode: u64, path: &str, seconds: u64) -> MediaRecord {
        MediaRecord::new(
            MediaId::filesystem(66, inode),
            SourceId::from_value(0),
            PathBuf::from(path),
            MediaKind::Audio,
            SourceIdentity::new(1_024, SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
        )
    }

    fn tagged(title: &str) -> MediaMetadata {
        MediaMetadata {
            title: Some(title.to_owned()),
            ..MediaMetadata::default()
        }
    }

    #[test]
    fn an_unchanged_file_keeps_the_metadata_already_extracted() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.flac", 100).with_metadata(tagged("Conocida")));

        let summary = catalogue.absorb([record(1, "/m/a.flac", 100)], true);

        assert_eq!(
            summary,
            AbsorbSummary {
                unchanged: 1,
                ..AbsorbSummary::default()
            }
        );
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 1))
                .and_then(|found| found.metadata().title.clone()),
            Some("Conocida".to_owned()),
            "una sonda por pista es justo lo que persistir evita"
        );
    }

    #[test]
    fn a_renamed_file_keeps_its_metadata_and_takes_the_new_path() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/antes.flac", 100).with_metadata(tagged("Conocida")));

        catalogue.absorb([record(1, "/m/después.flac", 100)], true);

        let found = catalogue.get(&MediaId::filesystem(66, 1)).expect("record");
        assert_eq!(found.path(), PathBuf::from("/m/después.flac"));
        assert_eq!(found.metadata().title.as_deref(), Some("Conocida"));
    }

    #[test]
    fn a_file_edited_underneath_loses_metadata_that_no_longer_describes_it() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.flac", 100).with_metadata(tagged("Antigua")));

        let summary = catalogue.absorb([record(1, "/m/a.flac", 200)], true);

        assert_eq!(summary.replaced, 1);
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 1))
                .map(|found| found.metadata().clone()),
            Some(MediaMetadata::default())
        );
    }

    #[test]
    fn a_complete_pass_marks_what_it_did_not_see_and_a_truncated_one_does_not() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.flac", 100));
        catalogue.upsert(record(2, "/m/b.flac", 100));

        let truncated = catalogue.absorb([record(1, "/m/a.flac", 100)], false);
        assert_eq!(truncated.marked_missing, 0);
        assert!(catalogue
            .get(&MediaId::filesystem(66, 2))
            .is_some_and(MediaRecord::is_available));

        let complete = catalogue.absorb([record(1, "/m/a.flac", 100)], true);
        assert_eq!(complete.marked_missing, 1);
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 2))
                .map(MediaRecord::availability),
            Some(Availability::Missing)
        );
    }

    #[test]
    fn a_file_that_came_back_is_available_again() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(
            record(1, "/mnt/externo/a.flac", 100)
                .with_metadata(tagged("Del disco externo"))
                .with_availability(Availability::Missing),
        );

        let summary = catalogue.absorb([record(1, "/mnt/externo/a.flac", 100)], true);

        assert_eq!(summary.unchanged, 1);
        assert!(catalogue
            .get(&MediaId::filesystem(66, 1))
            .is_some_and(MediaRecord::is_available));
        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 1))
                .and_then(|found| found.metadata().title.clone()),
            Some("Del disco externo".to_owned())
        );
    }

    #[test]
    fn a_new_file_is_added() {
        let mut catalogue = Catalogue::new();

        let summary = catalogue.absorb([record(9, "/m/nueva.flac", 100)], true);

        assert_eq!(summary.added, 1);
        assert_eq!(catalogue.len(), 1);
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::{Availability, Catalogue, MediaRecord, SourceIdentity};
    use crate::media::{MediaId, MediaKind};
    use crate::source::SourceId;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn record(inode: u64, path: &str) -> MediaRecord {
        MediaRecord::new(
            MediaId::filesystem(66, inode),
            SourceId::from_value(0),
            PathBuf::from(path),
            MediaKind::Image,
            SourceIdentity::new(1, SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
        )
    }

    #[test]
    fn a_record_is_found_by_the_path_its_file_lives_at() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.png"));
        catalogue.upsert(record(2, "/m/b.png"));

        assert_eq!(
            catalogue
                .find_by_path(Path::new("/m/b.png"))
                .map(MediaRecord::id),
            Some(&MediaId::filesystem(66, 2))
        );
        assert!(catalogue
            .find_by_path(Path::new("/m/no-existe.png"))
            .is_none());
    }

    #[test]
    fn marking_one_missing_leaves_every_other_record_alone() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.png"));
        catalogue.upsert(record(2, "/m/b.png"));

        assert!(catalogue.mark_missing(&MediaId::filesystem(66, 1)));

        assert_eq!(
            catalogue
                .get(&MediaId::filesystem(66, 1))
                .map(MediaRecord::availability),
            Some(Availability::Missing)
        );
        assert!(catalogue
            .get(&MediaId::filesystem(66, 2))
            .is_some_and(MediaRecord::is_available));
        assert_eq!(catalogue.len(), 2, "desaparecer no es borrar");
    }

    #[test]
    fn marking_twice_reports_that_nothing_changed() {
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/a.png"));

        assert!(catalogue.mark_missing(&MediaId::filesystem(66, 1)));
        assert!(!catalogue.mark_missing(&MediaId::filesystem(66, 1)));
        assert!(!catalogue.mark_missing(&MediaId::filesystem(66, 9)));
    }
}
