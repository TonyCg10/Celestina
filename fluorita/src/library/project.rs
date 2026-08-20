//! Turning the catalogue into what QML binds to.
//!
//! Pure: it reads a catalogue and produces rows and a sentence. Everything that
//! decides *when* to project lives with the work; everything that decides what
//! the library *is* lives in `fluorita-core`.

use std::path::Path;

use fluorita_core::{
    gallery, Catalogue, GalleryFilter, GalleryOrder, MediaKind, MusicLibrary, Query, SourceScope,
    SourceSet,
};

use super::copy;
use super::work::{thumbnail_cache_root, MAX_ARTWORK_PER_PASS};
use celestina_core::pathkey;

/// Everything one publication produces, already shaped for QML.
#[derive(Default)]
pub(super) struct LibrarySnapshot {
    /// `stored` while showing what was read back and the walk is still
    /// running, `ready` once the walk has been folded in, `error` when it
    /// could not be.
    pub(super) state: &'static str,
    pub(super) summary: String,
    pub(super) truncated: bool,
    pub(super) image_count: i32,
    pub(super) video_count: i32,
    pub(super) track_count: i32,
    /// Gallery rows. The first column is the item's **path key**, not its path:
    /// opaque ASCII a person never reads and every verb accepts. The display
    /// name beside it is the lossy text, and the two are never swapped.
    pub(super) gallery: Vec<[String; 5]>,
    pub(super) music: Vec<[String; 6]>,
    /// The sidebar: one row per configured root, in configuration order —
    /// handle, label and the root as display text.
    pub(super) sources: Vec<[String; 3]>,
    /// What the projection was made from, kept so an explicit artwork pass has
    /// something to work on without re-walking the disk.
    pub(super) catalogue: Catalogue,
    /// The configuration the rows were projected under, so the host answers a
    /// later add or remove from what it published rather than from a set the
    /// worker may have replaced since.
    pub(super) configured: SourceSet,
    /// How many items the shared cache has no usable thumbnail for.
    pub(super) artwork_pending: i32,
}

/// Projects one publication: the sidebar, and the content of `scope` inside it.
///
/// `scope` is the selected root. Every count and the summary describe exactly
/// what the grid and the list are showing, because a header that counted the
/// whole library while one folder was open would be describing something the
/// user cannot see.
pub(super) fn project(
    catalogue: &Catalogue,
    configured: &SourceSet,
    scope: SourceScope,
    truncated: bool,
    state: &'static str,
) -> LibrarySnapshot {
    project_matching(
        catalogue,
        configured,
        scope,
        truncated,
        state,
        &Query::default(),
    )
}

/// The same projection, keeping only what a query matches.
///
/// The query is applied here and not in the surface because *what matches* is a
/// rule — accents folded, case ignored — and a grid that re-implemented it in
/// JavaScript would be a second answer to the same question.
pub(super) fn project_matching(
    catalogue: &Catalogue,
    configured: &SourceSet,
    scope: SourceScope,
    truncated: bool,
    state: &'static str,
    query: &Query,
) -> LibrarySnapshot {
    let cache_root = thumbnail_cache_root();
    let items: Vec<_> = gallery(
        catalogue,
        scope,
        GalleryFilter::All,
        GalleryOrder::NewestFirst,
    )
    .into_iter()
    .filter(|item| query.matches(&item.display_name))
    .collect();

    let image_count = items
        .iter()
        .filter(|item| item.kind == MediaKind::Image)
        .count();
    let video_count = items.len() - image_count;

    let gallery_rows: Vec<[String; 5]> = items
        .iter()
        .map(|item| {
            [
                // The key, not the path. The catalogue already holds these
                // bytes exactly; `to_string_lossy` here was the one place that
                // threw them away, and every verb that took the result back
                // then named a file that does not exist.
                pathkey::encode(&item.path),
                item.display_name.clone(),
                kind_label(item.kind).to_owned(),
                cached_thumbnail(cache_root.as_deref(), &item.path),
                flag(item.available),
            ]
        })
        .collect();
    let missing = items.iter().filter(|item| !item.available).count();

    // Borrowed once for the nested closures: each track asks the same cache.
    let covers = cache_root.as_deref();
    let music = MusicLibrary::project(catalogue, scope);
    let music_rows: Vec<[String; 6]> = music
        .artists
        .iter()
        .flat_map(|artist| {
            artist.albums.iter().flat_map(move |album| {
                album
                    .tracks
                    .iter()
                    .filter(move |track| {
                        // A track matches on what a person would search by: its
                        // own name, the artist it is under, or the album it is on.
                        query.matches(&track.display_name)
                            || artist
                                .name
                                .as_deref()
                                .is_some_and(|name| query.matches(name))
                            || album
                                .title
                                .as_deref()
                                .is_some_and(|title| query.matches(title))
                    })
                    .map(move |track| {
                        [
                            pathkey::encode(&track.path),
                            track.display_name.clone(),
                            artist.name.clone().unwrap_or_else(unknown_artist),
                            album.title.clone().unwrap_or_else(unknown_album),
                            flag(track.available),
                            // The embedded cover, from the same shared cache the
                            // grid reads. Without it a track has nothing to light
                            // the room with, and the ambient light would be the one
                            // kind of content that does not get one.
                            cached_thumbnail(covers, &track.path),
                        ]
                    })
            })
        })
        .collect();

    let artwork_pending = cache_root.as_deref().map_or(0, |root| {
        i32::try_from(fluorita_engine::pending_artwork(catalogue, root, MAX_ARTWORK_PER_PASS).len())
            .unwrap_or(i32::MAX)
    });

    let source_rows: Vec<[String; 3]> = configured
        .sources()
        .iter()
        .map(|source| {
            [
                source.id().value().to_string(),
                source.display_name(),
                // Display text, and published as such: this is a location a
                // person reads under the folder's name, never a key. Nothing
                // reopens a root from it — the scan walks `MediaSource::root`
                // and every item carries its own key.
                source.root().to_string_lossy().into_owned(),
            ]
        })
        .collect();

    LibrarySnapshot {
        state,
        catalogue: catalogue.clone(),
        configured: configured.clone(),
        sources: source_rows,
        artwork_pending,
        summary: summarize(
            image_count,
            video_count,
            music_rows.len(),
            missing,
            truncated,
            matches!(scope, SourceScope::One(_)),
        ),
        truncated,
        image_count: i32::try_from(image_count).unwrap_or(i32::MAX),
        video_count: i32::try_from(video_count).unwrap_or(i32::MAX),
        track_count: i32::try_from(music_rows.len()).unwrap_or(i32::MAX),
        gallery: gallery_rows,
        music: music_rows,
    }
}

pub(super) fn unknown_artist() -> String {
    copy::UNKNOWN_ARTIST.to_owned()
}

pub(super) fn unknown_album() -> String {
    copy::UNKNOWN_ALBUM.to_owned()
}

/// The kind as a stable token, not as a word.
///
/// QML compares this to choose a glyph and translates it for display. Shipping
/// the Spanish noun in the data column instead would make the surface's
/// behaviour depend on the product's language, so renaming a label would break
/// an icon.
pub(crate) const fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "image",
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
    }
}

/// The shared thumbnail entry for a file, but **only if it already exists**.
///
/// Browsing never produces artwork: that would start the media backend for
/// every card in a grid, which is exactly the cost the suite's contract keeps
/// out of normal browsing. A missing thumbnail is an empty string and the
/// delegate shows a themed glyph instead.
pub(super) fn cached_thumbnail(cache_root: Option<&Path>, source: &Path) -> String {
    let Some(root) = cache_root else {
        return String::new();
    };
    let Some(entry) = fluorita_core::large_thumbnail_path(root, source) else {
        return String::new();
    };
    if !entry.is_file() {
        return String::new();
    }
    fluorita_core::file_uri(&entry).unwrap_or_default()
}

/// `1`/`0` rather than a word: QML reads it as a flag, and a translated string
/// in a data column would be a translation nobody asked for.
pub(super) fn flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_owned()
}

/// What the header says about what is on screen.
///
/// The counts describe the selected scope, not the whole library, and a
/// truncated pass says so instead of reading like a complete inventory. An
/// empty result distinguishes an empty folder from an empty library, because
/// the library-wide sentence would be a lie told while three other folders sit
/// in the sidebar with content in them.
pub(super) fn summarize(
    images: usize,
    videos: usize,
    tracks: usize,
    missing: usize,
    truncated: bool,
    scoped: bool,
) -> String {
    if images == 0 && videos == 0 && tracks == 0 {
        return if scoped {
            copy::EMPTY_FOLDER
        } else {
            copy::EMPTY_LIBRARY
        }
        .to_owned();
    }
    let mut parts: Vec<String> = Vec::new();
    if images > 0 {
        parts.push(copy::images(images));
    }
    if videos > 0 {
        parts.push(copy::videos(videos));
    }
    if tracks > 0 {
        parts.push(copy::tracks(tracks));
    }
    if missing > 0 {
        parts.push(copy::missing(missing));
    }
    let counted = parts.join(" · ");
    if truncated {
        format!("{counted} {}", copy::TRUNCATED)
    } else {
        counted
    }
}

#[cfg(test)]
mod tests {
    use super::{cached_thumbnail, copy, kind_label, project, summarize};
    use celestina_core::pathkey;
    use fluorita_core::MediaKind;
    use std::path::Path;

    #[test]
    fn the_summary_counts_what_was_found() {
        // Composed from the copy module rather than from a second copy of the
        // words: this asserts that summarize assembles the parts, and leaves
        // what the parts say to the module that owns them.
        assert_eq!(
            summarize(86, 8, 0, 0, false, false),
            format!("{} · {}", copy::images(86), copy::videos(8))
        );
        assert_eq!(
            summarize(1, 1, 1, 0, false, false),
            format!(
                "{} · {} · {}",
                copy::images(1),
                copy::videos(1),
                copy::tracks(1)
            )
        );
    }

    #[test]
    fn an_empty_folder_is_not_reported_as_an_empty_library() {
        // The distinction is the whole point of a sidebar: three folders with
        // content and one without must not all say the library is empty.
        assert_eq!(summarize(0, 0, 0, 0, false, true), copy::EMPTY_FOLDER);
        assert_eq!(summarize(0, 0, 0, 0, false, false), copy::EMPTY_LIBRARY);
        assert_ne!(copy::EMPTY_FOLDER, copy::EMPTY_LIBRARY);
    }

    #[test]
    fn a_truncated_scan_never_reads_like_a_complete_inventory() {
        let summary = summarize(50_000, 0, 0, 0, true, false);

        assert!(summary.ends_with(copy::TRUNCATED), "unexpected: {summary}");
        assert!(summary.starts_with(&copy::images(50_000)));
    }

    #[test]
    fn what_went_missing_is_counted_out_loud() {
        // A file that is no longer there stays in the grid — a disconnected
        // drive is not data loss — but the header says so.
        assert_eq!(
            summarize(2, 1, 0, 1, false, false),
            format!(
                "{} · {} · {}",
                copy::images(2),
                copy::videos(1),
                copy::missing(1)
            )
        );
    }

    #[test]
    fn a_missing_thumbnail_is_empty_rather_than_generated() {
        // Nothing produced this entry, so browsing must show a glyph — not
        // start a decoder to make one.
        let root = std::env::temp_dir().join("fluorita-library-tests");
        std::fs::create_dir_all(&root).expect("scratch");
        assert_eq!(
            cached_thumbnail(Some(&root), Path::new("/home/toni/Videos/clip.mkv")),
            ""
        );
        assert_eq!(cached_thumbnail(None, Path::new("/home/toni/x.png")), "");
    }

    #[test]
    fn an_existing_thumbnail_is_offered_as_a_url() {
        let root = std::env::temp_dir().join("fluorita-library-tests/cache");
        let source = Path::new("/home/toni/Videos/clip with space.mkv");
        let entry = fluorita_core::large_thumbnail_path(&root, source).expect("cache path");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("cache dir");
        std::fs::write(&entry, b"fake png").expect("entry");

        let url = cached_thumbnail(Some(&root), source);

        assert!(url.starts_with("file://"), "unexpected url: {url}");
        assert!(url.ends_with(".png"));
        std::fs::remove_file(&entry).ok();
    }

    /// A picture and a track whose names are not valid UTF-8, in a real
    /// configured root, catalogued the way a scan would leave them.
    #[cfg(unix)]
    fn catalogue_with_a_name_that_is_not_utf8() -> (
        fluorita_core::Catalogue,
        fluorita_core::SourceSet,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        use fluorita_core::{Catalogue, KindSet, MediaId, MediaRecord, SourceIdentity, SourceSet};
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::path::PathBuf;
        use std::time::{Duration, UNIX_EPOCH};

        let root = PathBuf::from("/home/toni/Pictures");
        let mut configured = SourceSet::new();
        let source = configured
            .add(root.clone(), KindSet::all())
            .expect("a configured root");

        // `\xff` is never valid UTF-8 in any position, so this name cannot be
        // spelled as a `String` without losing the byte.
        let picture = root.join(OsStr::from_bytes(b"na\xffme.png"));
        let track = root.join(OsStr::from_bytes(b"na\xffme.flac"));
        let identity = SourceIdentity::new(1_024, UNIX_EPOCH + Duration::from_secs(1_770_000_000));

        let mut catalogue = Catalogue::new();
        catalogue.upsert(MediaRecord::new(
            MediaId::filesystem(1, 10),
            source,
            picture.clone(),
            MediaKind::Image,
            identity,
        ));
        catalogue.upsert(MediaRecord::new(
            MediaId::filesystem(1, 11),
            source,
            track.clone(),
            MediaKind::Audio,
            identity,
        ));
        (catalogue, configured, picture, track)
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_is_published_and_stays_resolvable() {
        use fluorita_core::SourceScope;

        let (catalogue, configured, picture, track) = catalogue_with_a_name_that_is_not_utf8();

        let snapshot = project(&catalogue, &configured, SourceScope::All, false, "ready");

        // (a) It is in the projection at all — the defect never hid the row,
        // it made the row unusable.
        assert_eq!(snapshot.gallery.len(), 1);
        assert_eq!(snapshot.music.len(), 1);

        for (row, wanted) in [
            (&snapshot.gallery[0][0], &picture),
            (&snapshot.music[0][0], &track),
        ] {
            // (b) The published key round-trips to the same bytes.
            assert_eq!(&pathkey::decode(row).expect("a path"), wanted);
            // (c) And the catalogue answers for it, which is what
            // `describe_item` and `trash_item` ask.
            let found = catalogue
                .find_by_path(&pathkey::decode(row).expect("a path"))
                .expect("the record the row names");
            assert_eq!(found.path(), wanted.as_path());
        }

        // Display text is separate and still lossy, in its own column.
        assert!(snapshot.gallery[0][1].contains('\u{FFFD}'));
        assert!(!snapshot.gallery[0][0].contains('\u{FFFD}'));
    }

    #[test]
    fn a_key_that_this_process_did_not_emit_is_refused_without_panicking() {
        // What a hand-written or half-decoded value from QML would look like.
        // The catalogue is never consulted, and nothing unwinds.
        assert!(pathkey::decode("/home/toni/bad%2").is_err());
        assert!(pathkey::decode("").is_err());
        assert!(pathkey::decode("/home/toni/na\u{FFFD}me.png").is_err());
    }

    #[test]
    fn kind_labels_are_stable_tokens_rather_than_words() {
        assert_eq!(kind_label(MediaKind::Image), "image");
        assert_eq!(kind_label(MediaKind::Video), "video");
        assert_eq!(kind_label(MediaKind::Audio), "audio");
    }
}
