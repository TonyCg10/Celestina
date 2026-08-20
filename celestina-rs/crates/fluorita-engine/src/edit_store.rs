//! Remembering an edit, so a saved copy can be opened and changed again.
//!
//! **What is stored, and why it is not the picture.** A saved copy holds
//! flattened pixels: the text is in them, and reading it back would give no way
//! to move that text again. What makes a copy reopenable is the *recipe* — the
//! original it was computed from, the transformations, and the objects — so
//! that is what is written down. Reopening re-renders from the original; the
//! copy on disk is the result, never the working state.
//!
//! **Why it lives here and not beside the picture.** The folders the person
//! mapped are theirs. Fluorita does not leave files in them, so the recipes go
//! next to the catalogue in the application's own directory, keyed by the
//! result file's identity.
//!
//! **What happens when the original changes or disappears.** The base's size
//! and modification time are stored with the recipe. If either has moved, the
//! recipe describes a picture that no longer exists and is reported as stale
//! rather than applied to different bytes — the same
//! [`SourceIdentity`](fluorita_core::SourceIdentity) rule the catalogue already
//! uses for artwork.
//!
//! The format follows `catalogue_store`: one header line, one record per line,
//! tab-separated, with every free-form field percent-encoded so a path that is
//! not valid UTF-8 survives and no field can smuggle a separator into the next
//! record.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use celestina_core::{atomic_file, percent};
use fluorita_core::{
    Annotation, Area, Axis, Canvas, Ink, MediaId, Point, Quarter, Redaction, ShapeKind,
    SourceIdentity, Transform,
};

use crate::error::{EngineError, EngineResult};

const HEADER: &str = "fluorita-edits 1";

/// A ceiling on what will be read into memory. Far past any plausible number
/// of edited pictures, and a bound a corrupted file cannot argue with.
const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// The recipe that produced one saved file.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredEdit {
    /// The picture this was computed from. Still on disk for a copy; gone for
    /// a replacement, which is why a replacement stores nothing at all.
    pub base: PathBuf,
    /// What the base looked like when the recipe was written.
    pub base_identity: SourceIdentity,
    /// The base's dimensions, so the document reopens without measuring first.
    pub base_canvas: Canvas,
    /// Applied to the base, in this order.
    pub transforms: Vec<Transform>,
    /// Drawn after them, in the coordinates the transformations end at.
    pub objects: Vec<Annotation>,
}

/// Every recipe this host knows, keyed by the identity of the file it produced.
#[derive(Clone, Debug, Default)]
pub struct EditStore {
    entries: BTreeMap<MediaId, StoredEdit>,
}

impl EditStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Remembers the recipe behind `result`, replacing any earlier one.
    pub fn remember(&mut self, result: MediaId, edit: StoredEdit) {
        self.entries.insert(result, edit);
    }

    /// Forgets a recipe — after the file it describes is trashed, or after a
    /// replacement flattened it.
    pub fn forget(&mut self, result: &MediaId) {
        self.entries.remove(result);
    }

    /// The recipe behind `result`, if there is one.
    #[must_use]
    pub fn get(&self, result: &MediaId) -> Option<&StoredEdit> {
        self.entries.get(result)
    }

    /// The recipe behind `result`, but only when the base it was computed from
    /// still exists and still has the size and modification time it had.
    ///
    /// `current` is what the caller measured just now; `None` means the base is
    /// gone. Either way the answer is `None` rather than a recipe applied to
    /// bytes it does not describe.
    #[must_use]
    pub fn usable(&self, result: &MediaId, current: Option<SourceIdentity>) -> Option<&StoredEdit> {
        let edit = self.entries.get(result)?;
        current
            .is_some_and(|current| edit.base_identity.still_describes(current))
            .then_some(edit)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Where the recipes live: beside the catalogue, in the application's own
/// directory, never in the folders the person mapped.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    celestina_core::xdg::data_home().map(|data| data.join("fluorita").join("edits.tsv"))
}

/// What a load found.
#[derive(Debug, Default)]
pub struct EditLoad {
    pub store: EditStore,
    /// Records that could not be read. Counted rather than hidden: silently
    /// losing recipes would look like the editor forgetting on its own.
    pub skipped: usize,
    /// Nothing stored yet.
    pub absent: bool,
}

/// Reads the recipes at `path`.
///
/// A missing, unreadable or unrecognised file means "no recipes yet", not an
/// error: none of them should stop a picture from opening.
///
/// # Errors
///
/// Only a file past [`MAX_BYTES`], which is refused rather than read.
pub fn load(path: &Path) -> EngineResult<EditLoad> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(EditLoad {
            absent: true,
            ..EditLoad::default()
        });
    };
    if metadata.len() > MAX_BYTES {
        return Err(EngineError::OverBudget {
            what: "the stored edits",
            limit: MAX_BYTES,
            actual: metadata.len(),
        });
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(EditLoad {
            absent: true,
            ..EditLoad::default()
        });
    };

    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Ok(EditLoad {
            absent: true,
            ..EditLoad::default()
        });
    }

    let mut store = EditStore::new();
    let mut skipped = 0usize;
    let mut current: Option<(MediaId, StoredEdit)> = None;

    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.first().copied() {
            Some("E") => {
                if let Some((id, edit)) = current.take() {
                    store.remember(id, edit);
                }
                match read_entry(&fields) {
                    Some(entry) => current = Some(entry),
                    None => skipped += 1,
                }
            }
            Some("T") => match (current.as_mut(), read_transform(&fields)) {
                (Some((_, edit)), Some(transform)) => edit.transforms.push(transform),
                _ => skipped += 1,
            },
            Some("O") => match (current.as_mut(), read_object(&fields)) {
                (Some((_, edit)), Some(object)) => edit.objects.push(object),
                _ => skipped += 1,
            },
            _ => skipped += 1,
        }
    }
    if let Some((id, edit)) = current.take() {
        store.remember(id, edit);
    }

    Ok(EditLoad {
        store,
        skipped,
        absent: false,
    })
}

/// Writes every recipe to `path`, replacing what was there atomically.
///
/// # Errors
///
/// Any failure to create the directory or land the file.
pub fn save(path: &Path, store: &EditStore) -> EngineResult<()> {
    let mut text = String::from(HEADER);
    text.push('\n');
    for (id, edit) in &store.entries {
        let Some((device, inode)) = id.filesystem_parts() else {
            // A path-keyed identity does not survive a rename, so a recipe
            // stored under one would attach itself to whatever took the name.
            continue;
        };
        text.push_str(&format!(
            "E\t{device}\t{inode}\t{}\t{}\t{}\t{}\t{}\n",
            percent::encode(&percent::path_bytes(&edit.base)),
            seconds(edit.base_identity.modified),
            edit.base_identity.size,
            edit.base_canvas.width(),
            edit.base_canvas.height(),
        ));
        for transform in &edit.transforms {
            text.push_str(&write_transform(*transform));
        }
        for object in &edit.objects {
            text.push_str(&write_object(object));
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| EngineError::Io {
            operation: "creating the edit store directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    atomic_file::replace(path, text.as_bytes()).map_err(|source| EngineError::Io {
        operation: "writing the stored edits",
        path: path.to_path_buf(),
        source,
    })
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_entry(fields: &[&str]) -> Option<(MediaId, StoredEdit)> {
    if fields.len() != 8 {
        return None;
    }
    let device = fields[1].parse().ok()?;
    let inode = fields[2].parse().ok()?;
    let base = percent::path_from_bytes(&percent::decode_strict(fields[3])?);
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(fields[4].parse().ok()?);
    let size = fields[5].parse().ok()?;
    let canvas = Canvas::new(fields[6].parse().ok()?, fields[7].parse().ok()?)?;
    Some((
        MediaId::filesystem(device, inode),
        StoredEdit {
            base,
            base_identity: SourceIdentity::new(size, modified),
            base_canvas: canvas,
            transforms: Vec::new(),
            objects: Vec::new(),
        },
    ))
}

fn write_transform(transform: Transform) -> String {
    match transform {
        Transform::Rotate(quarter) => {
            let quarters = match quarter {
                Quarter::Clockwise => 1,
                Quarter::Half => 2,
                Quarter::CounterClockwise => 3,
            };
            format!("T\trotate\t{quarters}\n")
        }
        Transform::Flip(axis) => {
            let axis = match axis {
                Axis::Horizontal => "h",
                Axis::Vertical => "v",
            };
            format!("T\tflip\t{axis}\n")
        }
        Transform::Crop(area) => format!("T\tcrop\t{}\n", write_area(area)),
        Transform::Resize(canvas) => {
            format!("T\tresize\t{}\t{}\n", canvas.width(), canvas.height())
        }
    }
}

fn read_transform(fields: &[&str]) -> Option<Transform> {
    match *fields.get(1)? {
        "rotate" => Some(Transform::Rotate(
            match fields.get(2)?.parse::<u8>().ok()? {
                1 => Quarter::Clockwise,
                2 => Quarter::Half,
                3 => Quarter::CounterClockwise,
                _ => return None,
            },
        )),
        "flip" => Some(Transform::Flip(match *fields.get(2)? {
            "h" => Axis::Horizontal,
            "v" => Axis::Vertical,
            _ => return None,
        })),
        "crop" => Some(Transform::Crop(read_area(fields.get(2..6)?)?)),
        "resize" => Some(Transform::Resize(Canvas::new(
            fields.get(2)?.parse().ok()?,
            fields.get(3)?.parse().ok()?,
        )?)),
        _ => None,
    }
}

fn write_area(area: Area) -> String {
    format!(
        "{}\t{}\t{}\t{}",
        area.origin.x, area.origin.y, area.width, area.height
    )
}

fn read_area(fields: &[&str]) -> Option<Area> {
    let values = read_numbers(fields, 4)?;
    let area = Area::new(Point::new(values[0], values[1]), values[2], values[3]);
    Some(area)
}

fn read_numbers(fields: &[&str], expected: usize) -> Option<Vec<f32>> {
    if fields.len() < expected {
        return None;
    }
    let mut values = Vec::with_capacity(expected);
    for field in &fields[..expected] {
        let value: f32 = field.parse().ok()?;
        // A stored coordinate that is not a number would place an object
        // nowhere and make every later comparison false.
        if !value.is_finite() {
            return None;
        }
        values.push(value);
    }
    Some(values)
}

fn write_ink(ink: Ink) -> String {
    format!("{},{},{},{}", ink.red, ink.green, ink.blue, ink.alpha)
}

fn read_ink(field: &str) -> Option<Ink> {
    let parts: Vec<&str> = field.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Ink::new(
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        parts[3].parse().ok()?,
    ))
}

fn write_optional_ink(ink: Option<Ink>) -> String {
    ink.map_or_else(|| "-".to_owned(), write_ink)
}

fn read_optional_ink(field: &str) -> Option<Option<Ink>> {
    if field == "-" {
        Some(None)
    } else {
        read_ink(field).map(Some)
    }
}

fn write_object(object: &Annotation) -> String {
    match object {
        Annotation::Text {
            area,
            text,
            size,
            ink,
            backdrop,
            quarters,
        } => format!(
            "O\ttext\t{}\t{size}\t{}\t{}\t{quarters}\t{}\n",
            write_area(*area),
            write_ink(*ink),
            write_optional_ink(*backdrop),
            percent::encode(text.as_bytes()),
        ),
        Annotation::Stroke { points, width, ink } => {
            let points: Vec<String> = points
                .iter()
                .map(|point| format!("{},{}", point.x, point.y))
                .collect();
            format!(
                "O\tstroke\t{width}\t{}\t{}\n",
                write_ink(*ink),
                points.join(";")
            )
        }
        Annotation::Line {
            from,
            to,
            width,
            ink,
            arrow,
        } => format!(
            "O\tline\t{}\t{}\t{}\t{}\t{width}\t{}\t{}\n",
            from.x,
            from.y,
            to.x,
            to.y,
            write_ink(*ink),
            u8::from(*arrow)
        ),
        Annotation::Shape {
            kind,
            area,
            width,
            ink,
            fill,
        } => format!(
            "O\tshape\t{}\t{}\t{width}\t{}\t{}\n",
            match kind {
                ShapeKind::Rectangle => "rect",
                ShapeKind::Ellipse => "ellipse",
            },
            write_area(*area),
            write_ink(*ink),
            write_optional_ink(*fill)
        ),
        Annotation::Highlight { area, ink } => {
            format!("O\thighlight\t{}\t{}\n", write_area(*area), write_ink(*ink))
        }
        Annotation::Redact { area, style } => format!(
            "O\tredact\t{}\t{}\n",
            write_area(*area),
            match style {
                Redaction::Pixelate => "pixelate",
                Redaction::Blur => "blur",
            }
        ),
    }
}

fn read_object(fields: &[&str]) -> Option<Annotation> {
    match *fields.get(1)? {
        "text" => Some(Annotation::Text {
            area: read_area(fields.get(2..6)?)?,
            size: finite(fields.get(6)?)?,
            ink: read_ink(fields.get(7)?)?,
            backdrop: read_optional_ink(fields.get(8)?)?,
            quarters: fields.get(9)?.parse().ok()?,
            text: String::from_utf8(percent::decode_strict(fields.get(10)?)?).ok()?,
        }),
        "stroke" => {
            let mut points = Vec::new();
            for pair in fields.get(4)?.split(';') {
                let (x, y) = pair.split_once(',')?;
                points.push(Point::new(finite(x)?, finite(y)?));
            }
            Some(Annotation::Stroke {
                width: finite(fields.get(2)?)?,
                ink: read_ink(fields.get(3)?)?,
                points,
            })
        }
        "line" => {
            let values = read_numbers(fields.get(2..6)?, 4)?;
            Some(Annotation::Line {
                from: Point::new(values[0], values[1]),
                to: Point::new(values[2], values[3]),
                width: finite(fields.get(6)?)?,
                ink: read_ink(fields.get(7)?)?,
                arrow: *fields.get(8)? == "1",
            })
        }
        "shape" => Some(Annotation::Shape {
            kind: match *fields.get(2)? {
                "rect" => ShapeKind::Rectangle,
                "ellipse" => ShapeKind::Ellipse,
                _ => return None,
            },
            area: read_area(fields.get(3..7)?)?,
            width: finite(fields.get(7)?)?,
            ink: read_ink(fields.get(8)?)?,
            fill: read_optional_ink(fields.get(9)?)?,
        }),
        "highlight" => Some(Annotation::Highlight {
            area: read_area(fields.get(2..6)?)?,
            ink: read_ink(fields.get(6)?)?,
        }),
        "redact" => Some(Annotation::Redact {
            area: read_area(fields.get(2..6)?)?,
            style: match *fields.get(6)? {
                "pixelate" => Redaction::Pixelate,
                "blur" => Redaction::Blur,
                _ => return None,
            },
        }),
        _ => None,
    }
}

fn finite(field: &str) -> Option<f32> {
    let value: f32 = field.parse().ok()?;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use fluorita_core::{
        Annotation, Area, Axis, Canvas, Ink, MediaId, Point, Quarter, Redaction, ShapeKind,
        SourceIdentity, Transform,
    };

    use super::{load, save, EditStore, StoredEdit};

    fn directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fluorita-edit-store-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("test directory");
        path
    }

    fn identity() -> SourceIdentity {
        SourceIdentity::new(4096, UNIX_EPOCH + Duration::from_secs(1_700_000_000))
    }

    fn ink() -> Ink {
        Ink::new(240, 32, 16, 255)
    }

    fn full_edit(base: &str) -> StoredEdit {
        StoredEdit {
            base: PathBuf::from(base),
            base_identity: identity(),
            base_canvas: Canvas::new(4000, 3000).expect("a canvas"),
            transforms: vec![
                Transform::Rotate(Quarter::Clockwise),
                Transform::Flip(Axis::Vertical),
                Transform::Crop(Area::new(Point::new(10.5, 20.25), 300.0, 200.0)),
                Transform::Resize(Canvas::new(150, 100).expect("a canvas")),
            ],
            objects: vec![
                Annotation::Text {
                    area: Area::new(Point::new(1.0, 2.0), 30.0, 12.0),
                    text: "una nota\tcon separador".to_owned(),
                    size: 18.5,
                    ink: ink(),
                    backdrop: Some(Ink::new(0, 0, 0, 128)),
                    quarters: 3,
                },
                Annotation::Stroke {
                    points: vec![Point::new(1.0, 1.0), Point::new(2.5, 3.5)],
                    width: 4.0,
                    ink: ink(),
                },
                Annotation::Line {
                    from: Point::new(0.0, 0.0),
                    to: Point::new(50.0, 50.0),
                    width: 2.0,
                    ink: ink(),
                    arrow: true,
                },
                Annotation::Shape {
                    kind: ShapeKind::Ellipse,
                    area: Area::new(Point::new(5.0, 5.0), 20.0, 20.0),
                    width: 3.0,
                    ink: ink(),
                    fill: None,
                },
                Annotation::Highlight {
                    area: Area::new(Point::new(7.0, 7.0), 40.0, 10.0),
                    ink: Ink::new(255, 240, 0, 96),
                },
                Annotation::Redact {
                    area: Area::new(Point::new(9.0, 9.0), 60.0, 20.0),
                    style: Redaction::Blur,
                },
            ],
        }
    }

    #[test]
    fn a_whole_recipe_survives_being_written_down_and_read_back() {
        let directory = directory("round-trip");
        let path = directory.join("edits");
        let id = MediaId::filesystem(66, 4242);

        let mut store = EditStore::new();
        store.remember(id.clone(), full_edit("/fotos/original.jpg"));
        save(&path, &store).expect("the store lands");

        let loaded = load(&path).expect("the store reads");
        assert_eq!(loaded.skipped, 0);
        assert!(!loaded.absent);
        assert_eq!(
            loaded.store.get(&id),
            Some(&full_edit("/fotos/original.jpg"))
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_base_whose_name_is_not_utf8_survives() {
        use std::os::unix::ffi::OsStrExt;
        let directory = directory("bytes");
        let path = directory.join("edits");
        let name = std::ffi::OsStr::from_bytes(b"/fotos/foto\xff.jpg");
        let id = MediaId::filesystem(66, 1);

        let mut edit = full_edit("/placeholder");
        edit.base = PathBuf::from(name);
        let mut store = EditStore::new();
        store.remember(id.clone(), edit.clone());
        save(&path, &store).expect("the store lands");

        let loaded = load(&path).expect("the store reads");
        assert_eq!(
            loaded.store.get(&id).map(|it| it.base.clone()),
            Some(edit.base)
        );

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_recipe_is_unusable_when_its_base_moved_underneath_it() {
        let id = MediaId::filesystem(66, 7);
        let mut store = EditStore::new();
        store.remember(id.clone(), full_edit("/fotos/original.jpg"));

        assert!(store.usable(&id, Some(identity())).is_some());
        assert!(
            store.usable(&id, None).is_none(),
            "a base that is gone cannot be re-rendered from"
        );
        assert!(
            store
                .usable(&id, Some(SourceIdentity::new(9999, identity().modified)))
                .is_none(),
            "a base that changed size describes different bytes"
        );
        assert!(store.get(&id).is_some(), "the recipe is stale, not deleted");
    }

    #[test]
    fn forgetting_a_result_removes_only_that_recipe() {
        let kept = MediaId::filesystem(66, 1);
        let dropped = MediaId::filesystem(66, 2);
        let mut store = EditStore::new();
        store.remember(kept.clone(), full_edit("/a.jpg"));
        store.remember(dropped.clone(), full_edit("/b.jpg"));

        store.forget(&dropped);
        assert_eq!(store.len(), 1);
        assert!(store.get(&kept).is_some());
        assert!(store.get(&dropped).is_none());
    }

    #[test]
    fn nothing_stored_yet_is_not_a_failure() {
        let directory = directory("absent");
        let loaded = load(&directory.join("missing")).expect("a missing store reads");
        assert!(loaded.absent);
        assert!(loaded.store.is_empty());
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_corrupted_record_is_skipped_and_counted_rather_than_taken() {
        let directory = directory("corrupt");
        let path = directory.join("edits");
        std::fs::write(
            &path,
            "fluorita-edits 1\n\
             E\t66\t1\t%2Ffotos%2Fa.jpg\t1700000000\t10\t100\t80\n\
             O\tredact\t1\t2\t3\t4\tvanish\n\
             O\tredact\t1\t2\t3\t4\tblur\n\
             T\trotate\t7\n\
             nonsense\n",
        )
        .expect("the file");

        let loaded = load(&path).expect("the store reads");
        assert_eq!(
            loaded.skipped, 3,
            "an unknown style, an impossible turn and a stray line"
        );
        let edit = loaded
            .store
            .get(&MediaId::filesystem(66, 1))
            .expect("the entry survived its bad lines");
        assert_eq!(edit.objects.len(), 1);
        assert!(edit.transforms.is_empty());

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_store_written_over_an_older_one_replaces_it_whole() {
        let directory = directory("replace");
        let path = directory.join("edits");
        let first = MediaId::filesystem(66, 1);
        let second = MediaId::filesystem(66, 2);

        let mut store = EditStore::new();
        store.remember(first.clone(), full_edit("/a.jpg"));
        store.remember(second.clone(), full_edit("/b.jpg"));
        save(&path, &store).expect("the store lands");

        store.forget(&first);
        save(&path, &store).expect("the store lands again");

        let loaded = load(&path).expect("the store reads");
        assert_eq!(loaded.store.len(), 1);
        assert!(loaded.store.get(&first).is_none());

        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_path_keyed_identity_is_never_written() {
        let directory = directory("path-keyed");
        let path = directory.join("edits");
        let mut store = EditStore::new();
        store.remember(
            MediaId::from_path(std::path::Path::new("/fotos/a.jpg")),
            full_edit("/fotos/original.jpg"),
        );
        save(&path, &store).expect("the store lands");

        let loaded = load(&path).expect("the store reads");
        assert!(
            loaded.store.is_empty(),
            "a recipe keyed on a name would follow the name, not the file"
        );

        std::fs::remove_dir_all(&directory).ok();
    }
}
