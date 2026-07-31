//! The Music projection: artists, albums and tracks derived from tags.
//!
//! Tags are frequently absent or partial, so the projection keeps an explicit
//! unknown bucket at every level instead of dropping a track or inventing a
//! name. Unknown sorts last; everything else sorts case-insensitively with a
//! byte-order tiebreak, so the same catalogue always produces the same tree.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::catalogue::Catalogue;
use crate::media::{MediaId, MediaKind};

/// One playable track.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Track {
    pub id: MediaId,
    pub path: PathBuf,
    /// The tagged title, or the filename stem when nothing was tagged.
    pub display_name: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration: Option<Duration>,
    pub available: bool,
}

/// One album of one artist. `title` is `None` for the untagged bucket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Album {
    pub title: Option<String>,
    pub year: Option<i32>,
    pub tracks: Vec<Track>,
}

impl Album {
    /// Total duration, or `None` as soon as one track's duration is unknown —
    /// a partial sum presented as a total would be a lie.
    #[must_use]
    pub fn total_duration(&self) -> Option<Duration> {
        self.tracks
            .iter()
            .try_fold(Duration::ZERO, |sum, track| Some(sum + track.duration?))
    }
}

/// One artist. `name` is `None` for the untagged bucket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Artist {
    pub name: Option<String>,
    pub albums: Vec<Album>,
}

impl Artist {
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.albums.iter().map(|album| album.tracks.len()).sum()
    }
}

/// The whole Music tree.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MusicLibrary {
    pub artists: Vec<Artist>,
}

impl MusicLibrary {
    /// Projects every audio record in the catalogue.
    #[must_use]
    pub fn project(catalogue: &Catalogue) -> Self {
        let mut grouped: BTreeMap<Option<String>, BTreeMap<Option<String>, Album>> =
            BTreeMap::new();

        for record in catalogue
            .records()
            .filter(|record| record.kind() == MediaKind::Audio)
        {
            let metadata = record.metadata();
            let artist = metadata.grouping_artist().map(str::to_owned);
            let album_title = metadata.album_title().map(str::to_owned);

            let album = grouped
                .entry(artist)
                .or_default()
                .entry(album_title.clone())
                .or_insert_with(|| Album {
                    title: album_title,
                    year: metadata.year,
                    tracks: Vec::new(),
                });
            // The first tagged year wins; a later untagged track cannot erase it.
            if album.year.is_none() {
                album.year = metadata.year;
            }
            album.tracks.push(Track {
                id: record.id().clone(),
                path: record.path().to_path_buf(),
                display_name: record.display_name(),
                track_number: metadata.track_number,
                disc_number: metadata.disc_number,
                duration: metadata.duration,
                available: record.is_available(),
            });
        }

        let mut artists: Vec<Artist> = grouped
            .into_iter()
            .map(|(name, albums)| {
                let mut albums: Vec<Album> = albums
                    .into_values()
                    .map(|mut album| {
                        album.tracks.sort_by(compare_tracks);
                        album
                    })
                    .collect();
                albums.sort_by(|left, right| compare_optional_names(&left.title, &right.title));
                Artist { name, albums }
            })
            .collect();
        artists.sort_by(|left, right| compare_optional_names(&left.name, &right.name));

        Self { artists }
    }

    /// Every track in projection order — what "play everything" queues.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.artists
            .iter()
            .flat_map(|artist| artist.albums.iter().flat_map(|album| album.tracks.iter()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.artists.is_empty()
    }
}

/// Untagged sorts last; tagged names compare case-insensitively, then by bytes
/// so two spellings that differ only in case keep a stable order.
fn compare_optional_names(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => left
            .to_lowercase()
            .cmp(&right.to_lowercase())
            .then_with(|| left.cmp(right)),
    }
}

/// Disc, then track number, then name — with untagged numbers last, so a
/// numbered album keeps its running order and an untagged one stays readable.
fn compare_tracks(left: &Track, right: &Track) -> Ordering {
    compare_optional_numbers(left.disc_number, right.disc_number)
        .then_with(|| compare_optional_numbers(left.track_number, right.track_number))
        .then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
        .then_with(|| left.path.cmp(&right.path))
}

fn compare_optional_numbers(left: Option<u32>, right: Option<u32>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => left.cmp(&right),
    }
}

#[cfg(test)]
mod tests {
    use super::MusicLibrary;
    use crate::catalogue::{Catalogue, MediaMetadata, MediaRecord, SourceIdentity};
    use crate::media::{MediaId, MediaKind};
    use crate::source::{KindSet, SourceSet};
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn add(catalogue: &mut Catalogue, inode: u64, path: &str, metadata: MediaMetadata) {
        add_kind(catalogue, inode, path, MediaKind::Audio, metadata);
    }

    fn add_kind(
        catalogue: &mut Catalogue,
        inode: u64,
        path: &str,
        kind: MediaKind,
        metadata: MediaMetadata,
    ) {
        let mut sources = SourceSet::new();
        let source = sources
            .add(PathBuf::from("/home/toni/Música"), KindSet::all())
            .expect("absolute root");
        catalogue.upsert(
            MediaRecord::new(
                MediaId::filesystem(66, inode),
                source,
                PathBuf::from(path),
                kind,
                SourceIdentity::new(1, SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
            )
            .with_metadata(metadata),
        );
    }

    fn tagged(artist: &str, album: &str, title: &str, number: u32) -> MediaMetadata {
        MediaMetadata {
            title: Some(title.to_owned()),
            artist: Some(artist.to_owned()),
            album: Some(album.to_owned()),
            track_number: Some(number),
            duration: Some(Duration::from_secs(200)),
            ..MediaMetadata::default()
        }
    }

    #[test]
    fn tracks_group_into_artists_and_albums() {
        let mut catalogue = Catalogue::new();
        add(
            &mut catalogue,
            1,
            "/m/1.flac",
            tagged("Beta", "Uno", "A", 1),
        );
        add(
            &mut catalogue,
            2,
            "/m/2.flac",
            tagged("Beta", "Uno", "B", 2),
        );
        add(
            &mut catalogue,
            3,
            "/m/3.flac",
            tagged("Alfa", "Dos", "C", 1),
        );

        let library = MusicLibrary::project(&catalogue);

        assert_eq!(
            library
                .artists
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>(),
            vec![Some("Alfa".to_owned()), Some("Beta".to_owned())]
        );
        assert_eq!(library.artists[1].track_count(), 2);
        assert_eq!(
            library.artists[1].albums[0]
                .tracks
                .iter()
                .map(|track| track.display_name.clone())
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn untagged_tracks_land_in_an_unknown_bucket_that_sorts_last() {
        let mut catalogue = Catalogue::new();
        add(
            &mut catalogue,
            1,
            "/m/x.flac",
            tagged("Alfa", "Uno", "A", 1),
        );
        add(
            &mut catalogue,
            2,
            "/m/sin etiquetas.flac",
            MediaMetadata::default(),
        );

        let library = MusicLibrary::project(&catalogue);

        assert_eq!(library.artists.len(), 2);
        assert_eq!(library.artists[1].name, None);
        assert_eq!(library.artists[1].albums[0].title, None);
        assert_eq!(
            library.artists[1].albums[0].tracks[0].display_name, "sin etiquetas",
            "an untagged track keeps a name from its filename"
        );
    }

    #[test]
    fn video_and_images_never_enter_the_music_projection() {
        let mut catalogue = Catalogue::new();
        add_kind(
            &mut catalogue,
            1,
            "/m/clip.mkv",
            MediaKind::Video,
            MediaMetadata::default(),
        );
        add_kind(
            &mut catalogue,
            2,
            "/m/photo.png",
            MediaKind::Image,
            MediaMetadata::default(),
        );

        assert!(MusicLibrary::project(&catalogue).is_empty());
    }

    #[test]
    fn a_partially_tagged_album_keeps_its_year_and_orders_untagged_numbers_last() {
        let mut catalogue = Catalogue::new();
        add(
            &mut catalogue,
            1,
            "/m/con numero.flac",
            MediaMetadata {
                album: Some("Uno".to_owned()),
                artist: Some("Alfa".to_owned()),
                track_number: Some(2),
                year: Some(1999),
                ..MediaMetadata::default()
            },
        );
        add(
            &mut catalogue,
            2,
            "/m/sin numero.flac",
            MediaMetadata {
                album: Some("Uno".to_owned()),
                artist: Some("Alfa".to_owned()),
                ..MediaMetadata::default()
            },
        );

        let library = MusicLibrary::project(&catalogue);
        let album = &library.artists[0].albums[0];

        assert_eq!(album.year, Some(1999));
        assert_eq!(
            album
                .tracks
                .iter()
                .map(|track| track.track_number)
                .collect::<Vec<_>>(),
            vec![Some(2), None]
        );
        assert_eq!(
            album.total_duration(),
            None,
            "one unknown duration makes the total unknown"
        );
    }

    #[test]
    fn a_fully_tagged_album_reports_its_total_duration() {
        let mut catalogue = Catalogue::new();
        add(
            &mut catalogue,
            1,
            "/m/1.flac",
            tagged("Alfa", "Uno", "A", 1),
        );
        add(
            &mut catalogue,
            2,
            "/m/2.flac",
            tagged("Alfa", "Uno", "B", 2),
        );

        let library = MusicLibrary::project(&catalogue);

        assert_eq!(
            library.artists[0].albums[0].total_duration(),
            Some(Duration::from_secs(400))
        );
        assert_eq!(library.tracks().count(), 2);
    }

    #[test]
    fn projection_is_stable_and_keeps_unavailable_tracks_visible() {
        let mut catalogue = Catalogue::new();
        add(
            &mut catalogue,
            1,
            "/m/1.flac",
            tagged("Alfa", "Uno", "A", 1),
        );
        add(
            &mut catalogue,
            2,
            "/m/2.flac",
            tagged("beta", "Dos", "B", 1),
        );
        catalogue.reconcile(&BTreeSet::new());

        let first = MusicLibrary::project(&catalogue);
        let second = MusicLibrary::project(&catalogue);

        assert_eq!(first, second);
        assert_eq!(first.tracks().count(), 2);
        assert!(first.tracks().all(|track| !track.available));
    }
}
