//! Turning the catalogue into what QML binds to.
//!
//! Pure: it reads a catalogue and produces rows and a sentence. Everything that
//! decides *when* to project lives with the work; everything that decides what
//! the library *is* lives in `fluorita-core`.

use std::path::Path;

use fluorita_core::{gallery, Catalogue, GalleryFilter, GalleryOrder, MediaKind, MusicLibrary};

use super::work::{thumbnail_cache_root, MAX_ARTWORK_PER_PASS};

/// Everything one publication produces, already shaped for QML.
#[derive(Default)]
pub(super) struct LibrarySnapshot {
    /// `guardada` while showing what was stored and the walk is still running,
    /// `lista` once the walk has been folded in, `error` when it could not be.
    pub(super) state: &'static str,
    pub(super) summary: String,
    pub(super) truncated: bool,
    pub(super) image_count: i32,
    pub(super) video_count: i32,
    pub(super) track_count: i32,
    pub(super) gallery: Vec<[String; 5]>,
    pub(super) music: Vec<[String; 5]>,
    /// What the projection was made from, kept so an explicit artwork pass has
    /// something to work on without re-walking the disk.
    pub(super) catalogue: Catalogue,
    /// How many items the shared cache has no usable thumbnail for.
    pub(super) artwork_pending: i32,
}

pub(super) fn project(
    catalogue: &Catalogue,
    truncated: bool,
    state: &'static str,
) -> LibrarySnapshot {
    let cache_root = thumbnail_cache_root();
    let items = gallery(catalogue, GalleryFilter::All, GalleryOrder::NewestFirst);

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

    let music = MusicLibrary::project(catalogue);
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

    LibrarySnapshot {
        state,
        catalogue: catalogue.clone(),
        artwork_pending,
        summary: summarize(
            image_count,
            video_count,
            music_rows.len(),
            missing,
            truncated,
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
    "Sin artista".to_owned()
}

pub(super) fn unknown_album() -> String {
    "Sin álbum".to_owned()
}

pub(super) const fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "imagen",
        MediaKind::Video => "vídeo",
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

/// What the header says. Counts are what the scan actually saw, and a
/// truncated pass says so instead of reading like a complete inventory.
/// `1`/`0` rather than a word: QML reads it as a flag, and a translated string
/// in a data column would be a translation nobody asked for.
pub(super) fn flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_owned()
}

pub(super) fn summarize(
    images: usize,
    videos: usize,
    tracks: usize,
    missing: usize,
    truncated: bool,
) -> String {
    if images == 0 && videos == 0 && tracks == 0 {
        return "No hay medios en tus carpetas".to_owned();
    }
    let mut parts: Vec<String> = Vec::new();
    if images > 0 {
        parts.push(format!("{images} {}", plural(images, "imagen", "imágenes")));
    }
    if videos > 0 {
        parts.push(format!("{videos} {}", plural(videos, "vídeo", "vídeos")));
    }
    if tracks > 0 {
        parts.push(format!("{tracks} {}", plural(tracks, "pista", "pistas")));
    }
    if missing > 0 {
        parts.push(format!("{missing} sin encontrar",));
    }
    let counted = parts.join(" · ");
    if truncated {
        format!("{counted} (exploración incompleta: se alcanzó un límite)")
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
        assert_eq!(summarize(86, 8, 0, 0, false), "86 imágenes · 8 vídeos");
        assert_eq!(summarize(1, 1, 1, 0, false), "1 imagen · 1 vídeo · 1 pista");
        assert_eq!(
            summarize(0, 0, 0, 0, false),
            "No hay medios en tus carpetas"
        );
    }

    #[test]
    fn a_truncated_scan_never_reads_like_a_complete_inventory() {
        let summary = summarize(50_000, 0, 0, 0, true);
        assert!(summary.contains("incompleta"));
        assert!(summary.contains("50000 imágenes"));
    }

    #[test]
    fn what_went_missing_is_counted_out_loud() {
        // Un archivo que ya no está sigue en la rejilla —un disco desconectado
        // no es pérdida de datos— pero el encabezado lo dice.
        assert_eq!(
            summarize(2, 1, 0, 1, false),
            "2 imágenes · 1 vídeo · 1 sin encontrar"
        );
    }

    #[test]
    fn a_missing_thumbnail_is_empty_rather_than_generated() {
        // Nothing produced this entry, so browsing must show a glyph — not
        // start a decoder to make one.
        let root = std::env::temp_dir().join("fluorita-library-tests");
        std::fs::create_dir_all(&root).expect("scratch");
        assert_eq!(
            cached_thumbnail(Some(&root), Path::new("/home/toni/Vídeos/clip.mkv")),
            ""
        );
        assert_eq!(cached_thumbnail(None, Path::new("/home/toni/x.png")), "");
    }

    #[test]
    fn an_existing_thumbnail_is_offered_as_a_url() {
        let root = std::env::temp_dir().join("fluorita-library-tests/cache");
        let source = Path::new("/home/toni/Vídeos/clip con espacio.mkv");
        let entry = fluorita_core::large_thumbnail_path(&root, source).expect("cache path");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("cache dir");
        std::fs::write(&entry, b"fake png").expect("entry");

        let url = cached_thumbnail(Some(&root), source);

        assert!(url.starts_with("file://"), "unexpected url: {url}");
        assert!(url.ends_with(".png"));
        std::fs::remove_file(&entry).ok();
    }

    #[test]
    fn kind_labels_are_the_spanish_the_interface_shows() {
        assert_eq!(kind_label(MediaKind::Image), "imagen");
        assert_eq!(kind_label(MediaKind::Video), "vídeo");
        assert_eq!(kind_label(MediaKind::Audio), "audio");
    }
}
