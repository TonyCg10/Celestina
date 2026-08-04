//! Turning the catalogue into what QML binds to.
//!
//! Pure: it reads a catalogue and produces rows and a sentence. Everything that
//! decides *when* to project lives with the work; everything that decides what
//! the library *is* lives in `fluorita-core`.

use std::path::Path;

use fluorita_core::{
    gallery, Catalogue, GalleryFilter, GalleryOrder, MediaKind, MusicLibrary, SourceScope,
    SourceSet,
};

use super::work::{thumbnail_cache_root, MAX_ARTWORK_PER_PASS};

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
    pub(super) gallery: Vec<[String; 5]>,
    pub(super) music: Vec<[String; 5]>,
    /// The sidebar: one row per configured root, in configuration order —
    /// handle, label and the root itself.
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
    let cache_root = thumbnail_cache_root();
    let items = gallery(
        catalogue,
        scope,
        GalleryFilter::All,
        GalleryOrder::NewestFirst,
    );

    let image_count = items
        .iter()
        .filter(|item| item.kind == MediaKind::Image)
        .count();
    let video_count = items.len() - image_count;

    let gallery_rows: Vec<[String; 5]> = items
        .iter()
        .map(|item| {
            [
                item.path.to_string_lossy().into_owned(),
                item.display_name.clone(),
                kind_label(item.kind).to_owned(),
                cached_thumbnail(cache_root.as_deref(), &item.path),
                flag(item.available),
            ]
        })
        .collect();
    let missing = items.iter().filter(|item| !item.available).count();

    let music = MusicLibrary::project(catalogue, scope);
    let music_rows: Vec<[String; 5]> = music
        .artists
        .iter()
        .flat_map(|artist| {
            artist.albums.iter().flat_map(move |album| {
                album.tracks.iter().map(move |track| {
                    [
                        track.path.to_string_lossy().into_owned(),
                        track.display_name.clone(),
                        artist.name.clone().unwrap_or_else(unknown_artist),
                        album.title.clone().unwrap_or_else(unknown_album),
                        flag(track.available),
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
                // Lossy for display only. Nothing reopens a root from this: the
                // scan walks `MediaSource::root` and every item carries its own
                // byte-exact path.
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
    "Unknown artist".to_owned()
}

pub(super) fn unknown_album() -> String {
    "Unknown album".to_owned()
}

pub(super) const fn kind_label(kind: MediaKind) -> &'static str {
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
/// "no media in your folders" would be a lie told while three other folders sit
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
            "Nothing supported in this folder".to_owned()
        } else {
            "No media in your folders".to_owned()
        };
    }
    let mut parts: Vec<String> = Vec::new();
    if images > 0 {
        parts.push(format!("{images} {}", plural(images, "image", "images")));
    }
    if videos > 0 {
        parts.push(format!("{videos} {}", plural(videos, "video", "videos")));
    }
    if tracks > 0 {
        parts.push(format!("{tracks} {}", plural(tracks, "track", "tracks")));
    }
    if missing > 0 {
        parts.push(format!("{missing} not found"));
    }
    let counted = parts.join(" · ");
    if truncated {
        format!("{counted} (incomplete scan: a limit was reached)")
    } else {
        counted
    }
}

pub(super) fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 { one } else { many }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{cached_thumbnail, kind_label, summarize};
    use fluorita_core::MediaKind;
    use std::path::Path;

    #[test]
    fn the_summary_counts_what_was_found() {
        assert_eq!(summarize(86, 8, 0, 0, false, false), "86 images · 8 videos");
        assert_eq!(
            summarize(1, 1, 1, 0, false, false),
            "1 image · 1 video · 1 track"
        );
    }

    #[test]
    fn an_empty_folder_is_not_reported_as_an_empty_library() {
        // The distinction is the whole point of a sidebar: three folders with
        // content and one without must not all say the library is empty.
        assert_eq!(
            summarize(0, 0, 0, 0, false, true),
            "Nothing supported in this folder"
        );
        assert_eq!(
            summarize(0, 0, 0, 0, false, false),
            "No media in your folders"
        );
    }

    #[test]
    fn a_truncated_scan_never_reads_like_a_complete_inventory() {
        let summary = summarize(50_000, 0, 0, 0, true, false);
        assert!(summary.contains("incomplete"));
        assert!(summary.contains("50000 images"));
    }

    #[test]
    fn what_went_missing_is_counted_out_loud() {
        // A file that is no longer there stays in the grid — a disconnected
        // drive is not data loss — but the header says so.
        assert_eq!(
            summarize(2, 1, 0, 1, false, false),
            "2 images · 1 video · 1 not found"
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

    #[test]
    fn kind_labels_are_the_words_the_interface_shows() {
        assert_eq!(kind_label(MediaKind::Image), "image");
        assert_eq!(kind_label(MediaKind::Video), "video");
        assert_eq!(kind_label(MediaKind::Audio), "audio");
    }
}
