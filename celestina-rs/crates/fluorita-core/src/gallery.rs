//! The Gallery projection: images and video from the catalogue, together.
//!
//! Gallery never hides an item because its file is currently missing — it says
//! so instead, so a disconnected drive does not look like data loss.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::catalogue::Catalogue;
use crate::media::{MediaId, MediaKind};

/// Which of the two Gallery kinds to show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GalleryFilter {
    #[default]
    All,
    Images,
    Videos,
}

impl GalleryFilter {
    #[must_use]
    pub const fn accepts(self, kind: MediaKind) -> bool {
        match self {
            Self::All => kind.is_gallery(),
            Self::Images => matches!(kind, MediaKind::Image),
            Self::Videos => matches!(kind, MediaKind::Video),
        }
    }
}

/// Deterministic orderings. Every ordering breaks ties on the path, so two runs
/// over the same catalogue produce the same grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GalleryOrder {
    #[default]
    NewestFirst,
    NameAscending,
}

/// One cell of the grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GalleryItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub kind: MediaKind,
    pub modified: SystemTime,
    pub display_name: String,
    /// `false` when the file was not seen by the last reconciliation. The host
    /// dims the cell; it does not drop it.
    pub available: bool,
}

impl GalleryItem {
    /// The filename as raw bytes are held on the record — use this, never
    /// [`GalleryItem::display_name`], to open or thumbnail the file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Projects the catalogue into the Gallery grid.
#[must_use]
pub fn gallery(
    catalogue: &Catalogue,
    filter: GalleryFilter,
    order: GalleryOrder,
) -> Vec<GalleryItem> {
    let mut items: Vec<GalleryItem> = catalogue
        .records()
        .filter(|record| filter.accepts(record.kind()))
        .map(|record| GalleryItem {
            id: record.id().clone(),
            path: record.path().to_path_buf(),
            kind: record.kind(),
            modified: record.identity().modified,
            display_name: record.display_name(),
            available: record.is_available(),
        })
        .collect();

    match order {
        GalleryOrder::NewestFirst => items.sort_by(|left, right| {
            right
                .modified
                .cmp(&left.modified)
                .then_with(|| left.path.cmp(&right.path))
        }),
        GalleryOrder::NameAscending => items.sort_by(|left, right| {
            left.path
                .file_name()
                .cmp(&right.path.file_name())
                .then_with(|| left.path.cmp(&right.path))
        }),
    }
    items
}

#[cfg(test)]
mod tests {
    use super::{gallery, GalleryFilter, GalleryOrder};
    use crate::catalogue::{Catalogue, MediaRecord, SourceIdentity};
    use crate::media::{MediaId, MediaKind};
    use crate::source::{KindSet, SourceSet};
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn catalogue() -> Catalogue {
        let mut sources = SourceSet::new();
        let source = sources
            .add(PathBuf::from("/home/toni/Imágenes"), KindSet::all())
            .expect("absolute root");
        let mut catalogue = Catalogue::new();
        for (inode, path, kind, secs) in [
            (1, "/home/toni/Imágenes/b.png", MediaKind::Image, 300),
            (2, "/home/toni/Imágenes/a.png", MediaKind::Image, 100),
            (3, "/home/toni/Imágenes/clip.mkv", MediaKind::Video, 200),
            (4, "/home/toni/Música/song.flac", MediaKind::Audio, 400),
        ] {
            catalogue.upsert(MediaRecord::new(
                MediaId::filesystem(66, inode),
                source,
                PathBuf::from(path),
                kind,
                SourceIdentity::new(1, SystemTime::UNIX_EPOCH + Duration::from_secs(secs)),
            ));
        }
        catalogue
    }

    #[test]
    fn gallery_shows_images_and_video_but_never_audio() {
        let items = gallery(&catalogue(), GalleryFilter::All, GalleryOrder::NewestFirst);

        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.kind != MediaKind::Audio));
        assert_eq!(
            items.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![MediaKind::Image, MediaKind::Video, MediaKind::Image]
        );
    }

    #[test]
    fn filters_narrow_to_one_kind() {
        let catalogue = catalogue();

        assert_eq!(
            gallery(&catalogue, GalleryFilter::Videos, GalleryOrder::NewestFirst).len(),
            1
        );
        assert_eq!(
            gallery(&catalogue, GalleryFilter::Images, GalleryOrder::NewestFirst).len(),
            2
        );
    }

    #[test]
    fn ordering_is_deterministic_and_honours_the_choice() {
        let catalogue = catalogue();

        let newest = gallery(&catalogue, GalleryFilter::All, GalleryOrder::NewestFirst);
        assert_eq!(
            newest
                .iter()
                .map(|item| item.display_name.clone())
                .collect::<Vec<_>>(),
            vec!["b", "clip", "a"]
        );

        let by_name = gallery(&catalogue, GalleryFilter::All, GalleryOrder::NameAscending);
        assert_eq!(
            by_name
                .iter()
                .map(|item| item.display_name.clone())
                .collect::<Vec<_>>(),
            vec!["a", "b", "clip"]
        );
        assert_eq!(
            by_name,
            gallery(&catalogue, GalleryFilter::All, GalleryOrder::NameAscending)
        );
    }

    #[test]
    fn a_missing_file_stays_visible_and_says_so() {
        let mut catalogue = catalogue();
        catalogue.reconcile(&BTreeSet::new());

        let items = gallery(&catalogue, GalleryFilter::All, GalleryOrder::NewestFirst);

        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| !item.available));
        assert!(items
            .iter()
            .all(|item| item.path().is_absolute() && item.path().extension().is_some()));
    }
}
