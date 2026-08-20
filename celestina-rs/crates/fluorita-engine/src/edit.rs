//! Landing an edit on disk.
//!
//! The engine owns the *write*: which bytes, under which name, in which order,
//! and what happens to the file that was there before. It does not own the
//! drawing. Rasterising needs a toolkit that can read every format the library
//! shows and lay out text, and this crate has neither a toolkit nor a window —
//! so the caller hands in a [`Rasteriser`] and the engine calls it. That is
//! also what makes this path testable: the tests here run against a fake that
//! returns bytes, and prove the thing that actually loses people's pictures,
//! which is the order of operations around the write.
//!
//! Two rules from
//! [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md)
//! are enforced here rather than trusted:
//!
//! - **The destination is confirmed before the source moves.** A replacement
//!   writes the new file first and only then sends the original to the Trash,
//!   through the suite's one Trash implementation. Nothing here unlinks.
//! - **A turn is a header change when it can be.** A JPEG whose whole edit is
//!   an orientation change is rewritten by moving two bytes inside its EXIF
//!   segment; the pixels are copied across untouched. Falling back to a
//!   re-render is allowed, but only after that has actually been attempted.

use std::path::{Path, PathBuf};

use celestina_core::{atomic_file, CancellationToken};
use fluorita_core::{Composition, EditClass, Orientation, OutputFormat, SaveChoice};
use siderita_ops::{next_available, NameShape, OpError};

use crate::error::{EngineError, EngineResult};

/// The largest result the engine will write. A canvas is already bounded
/// before an edit is composed; this bounds what the *encoder* produced from
/// it, so a pathological output cannot be written to the author's disk.
pub const MAX_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Draws a composition over a picture and encodes the result.
///
/// Implemented by the application over its toolkit. The engine never learns
/// what a `QImage` is, and the toolkit never learns what a Trash is.
pub trait Rasteriser {
    /// Reads `source`, applies the composition's transformations in order,
    /// draws its objects, and encodes the result in `format`.
    ///
    /// `quality` is `Some` only for a lossy format, and is the fixed value the
    /// product decided; an implementation must not substitute its own.
    ///
    /// # Errors
    ///
    /// Any failure to read, draw or encode. The detail is a developer-facing
    /// message; the user-facing wording belongs to the host.
    fn render(
        &self,
        source: &Path,
        composition: &Composition,
        format: OutputFormat,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, RasterFailure>;
}

/// Where a replaced original goes.
///
/// A seam for the same reason [`Rasteriser`] is one, and for one more: it is
/// what lets a test assert the ordering this module exists to guarantee — the
/// fake records that the destination already existed when it was asked to move
/// the original. [`DesktopTrash`] is the only implementation the application
/// uses.
pub trait Bin {
    /// Sends `path` somewhere recoverable and reports where it landed.
    ///
    /// # Errors
    ///
    /// Any failure to move the file. The result is already written when this
    /// runs, so a failure here is a half-finished replacement, not a lost one.
    fn send(&self, path: &Path, cancellation: &CancellationToken) -> Result<PathBuf, OpError>;
}

/// The desktop's own Trash, through the suite's single implementation of the
/// freedesktop spec.
pub struct DesktopTrash;

impl Bin for DesktopTrash {
    fn send(&self, path: &Path, cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
        let mut progress = |_| {};
        siderita_ops::trash(path, cancellation, &mut progress).map(|trashed| trashed.trashed)
    }
}

/// Why the toolkit could not produce the result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RasterFailure {
    pub detail: String,
}

impl RasterFailure {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

/// One save, exactly as the person asked for it.
pub struct SaveRequest<'a> {
    /// The picture the edit was composed over. Always the file the objects'
    /// coordinates refer to.
    pub source: &'a Path,
    /// What to draw. Ignored when the whole edit turns out to be an
    /// orientation change a header can carry.
    pub composition: &'a Composition,
    /// `Some` when the document is nothing but an orientation change, so the
    /// lossless path may be attempted.
    pub orientation: Option<Orientation>,
    /// The format decided by the fixed output rule.
    pub format: OutputFormat,
    /// Copy beside the original, or replace it.
    pub choice: SaveChoice,
    /// The word a copy's name is marked with — product copy, so the host owns
    /// it and the engine only places it.
    pub copy_marker: &'a str,
}

/// What a save actually did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Saved {
    /// The file now holding the result.
    pub written: PathBuf,
    /// Which side of the contract the write turned out to be on. A requested
    /// lossless save that could not be performed losslessly reports
    /// [`EditClass::Raster`] here rather than claiming the original survived.
    pub class: EditClass,
    /// Where the original went, when it was replaced. `None` for a copy, and
    /// `None` when the result was written over the original's own path.
    pub trashed_original: Option<PathBuf>,
}

/// Writes an edit.
///
/// # Errors
///
/// Refuses a source that is not an absolute file, a result past
/// [`MAX_OUTPUT_BYTES`], a rasteriser failure, any filesystem failure, and a
/// cancellation. A failure never leaves the original moved: the Trash step is
/// last and runs only after the destination exists.
pub fn save(
    request: &SaveRequest<'_>,
    rasteriser: &dyn Rasteriser,
    bin: &dyn Bin,
    cancellation: &CancellationToken,
) -> EngineResult<Saved> {
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    if !request.source.is_absolute() {
        return Err(EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "an edit is saved over an absolute path",
        });
    }

    let (bytes, class) = compose_bytes(request, rasteriser)?;
    if bytes.len() as u64 > MAX_OUTPUT_BYTES {
        return Err(EngineError::OverBudget {
            what: "the edited image",
            limit: MAX_OUTPUT_BYTES,
            actual: bytes.len() as u64,
        });
    }
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }

    let destination = destination_for(request, class)?;
    atomic_file::replace(&destination, &bytes).map_err(|source| EngineError::Io {
        operation: "writing the edited image",
        path: destination.clone(),
        source,
    })?;

    // Only now, with the result on disk, may the original be moved. The order
    // is the contract: a Trash step that ran first would leave a person with
    // neither file if the write then failed.
    let trashed_original = match request.choice {
        SaveChoice::Copy => None,
        SaveChoice::Replace if destination == request.source => None,
        SaveChoice::Replace => {
            Some(
                bin.send(request.source, cancellation)
                    .map_err(|source| EngineError::Trash {
                        path: request.source.to_path_buf(),
                        source,
                    })?,
            )
        }
    };

    Ok(Saved {
        written: destination,
        class,
        trashed_original,
    })
}

/// The bytes to write, and what class producing them turned out to be.
fn compose_bytes(
    request: &SaveRequest<'_>,
    rasteriser: &dyn Rasteriser,
) -> EngineResult<(Vec<u8>, EditClass)> {
    if let Some(orientation) = request.orientation {
        if !orientation.is_identity() && request.format == OutputFormat::Jpeg {
            let original = read_source(request.source)?;
            if let Some(reoriented) = reorient_jpeg(&original, orientation) {
                return Ok((reoriented, EditClass::Lossless));
            }
        }
    }
    let bytes = rasteriser
        .render(
            request.source,
            request.composition,
            request.format,
            request.format.quality(),
        )
        .map_err(|failure| EngineError::Undecodable {
            path: request.source.to_path_buf(),
            detail: failure.detail,
        })?;
    Ok((bytes, EditClass::Raster))
}

fn read_source(path: &Path) -> EngineResult<Vec<u8>> {
    std::fs::read(path).map_err(|source| EngineError::Io {
        operation: "reading the picture to edit",
        path: path.to_path_buf(),
        source,
    })
}

/// Where the result goes.
///
/// A copy takes the next free `name (marker).ext`, which is the suite's one
/// keep-both policy rather than a second spelling of it. A replacement takes
/// the original's own path when the format has not changed, and the original's
/// name with the new extension when it has — in which case the original is
/// still a different file, and is still trashed.
fn destination_for(request: &SaveRequest<'_>, _class: EditClass) -> EngineResult<PathBuf> {
    let directory = request
        .source
        .parent()
        .ok_or_else(|| EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "a file to edit has a parent directory",
        })?;
    let name = request
        .source
        .file_name()
        .ok_or_else(|| EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "a file to edit has a name",
        })?;
    let extension = request.format.extension();
    // The name the search must avoid colliding with is the one that will
    // actually be written, extension and all. Searching under the source's
    // extension and changing it afterwards asks "is `foto (editado).gif`
    // free?" and then writes `foto (editado).png`, which is how a keep-both
    // policy silently overwrites the file it exists to protect.
    let target = Path::new(name).with_extension(extension);
    let free = |directory: &Path| {
        next_available(
            directory,
            target.as_os_str(),
            request.copy_marker,
            NameShape::File,
        )
    };

    match request.choice {
        SaveChoice::Copy => Ok(free(directory)),
        SaveChoice::Replace => {
            let same_extension = request
                .source
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension));
            if same_extension {
                return Ok(request.source.to_path_buf());
            }
            // The container changed, so the result is a different file. It
            // takes the original's name under the new extension when that is
            // free, and the next free name when it is not — the original is
            // trashed either way.
            let renamed = directory.join(&target);
            if renamed.exists() {
                Ok(free(directory))
            } else {
                Ok(renamed)
            }
        }
    }
}

/// Rewrites a JPEG's EXIF orientation tag, leaving every other byte alone.
///
/// Returns `None` when the file is not a JPEG, carries no EXIF segment, or
/// carries one with no orientation tag to change. That is not a failure: it
/// means the turn cannot be a header change for *this* file, and the caller
/// falls back to re-rendering it. Inserting a tag that is not there would mean
/// rewriting every offset in the segment, which is a different and much
/// riskier operation than moving two bytes that already exist.
///
/// The new value composes with whatever the file already claimed, so turning a
/// photograph the camera had already marked as rotated adds to it instead of
/// overwriting it.
#[must_use]
pub fn reorient_jpeg(bytes: &[u8], orientation: Orientation) -> Option<Vec<u8>> {
    let (offset, endian) = orientation_field(bytes)?;
    let existing = read_u16(bytes, offset, endian);
    let combined = Orientation::from_exif(existing).then(orientation);

    let mut rewritten = bytes.to_vec();
    write_u16(&mut rewritten, offset, endian, combined.to_exif());
    Some(rewritten)
}

/// Reads the orientation a JPEG already claims, for a host that wants to show
/// the picture the way the camera meant it.
#[must_use]
pub fn jpeg_orientation(bytes: &[u8]) -> Option<Orientation> {
    let (offset, endian) = orientation_field(bytes)?;
    Some(Orientation::from_exif(read_u16(bytes, offset, endian)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Endian {
    Little,
    Big,
}

/// Finds the two bytes holding the EXIF orientation value, if they exist.
///
/// Every read is bounds-checked against the slice rather than against a length
/// the file claims: the segment lengths, entry counts and offsets here are all
/// attacker-controlled, and a JPEG is exactly the kind of file a person
/// receives from someone else.
fn orientation_field(bytes: &[u8]) -> Option<(usize, Endian)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }

    let mut cursor = 2usize;
    let exif = loop {
        if cursor + 4 > bytes.len() || bytes[cursor] != 0xFF {
            return None;
        }
        let marker = bytes[cursor + 1];
        // Start of scan: the entropy-coded data begins and there are no more
        // headers to walk.
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 {
            return None;
        }
        let segment_start = cursor + 4;
        let segment_end = segment_start.checked_add(length - 2)?;
        if segment_end > bytes.len() {
            return None;
        }
        if marker == 0xE1 && bytes.get(segment_start..segment_start + 6) == Some(b"Exif\0\0") {
            break segment_start + 6;
        }
        cursor = segment_end;
    };

    let endian = match bytes.get(exif..exif + 2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    if read_u16(bytes, exif + 2, endian) != 42 {
        return None;
    }
    let ifd = exif.checked_add(read_u32(bytes, exif + 4, endian) as usize)?;
    if ifd + 2 > bytes.len() {
        return None;
    }
    let entries = read_u16(bytes, ifd, endian) as usize;
    for index in 0..entries {
        let entry = ifd.checked_add(2 + index * 12)?;
        if entry + 12 > bytes.len() {
            return None;
        }
        if read_u16(bytes, entry, endian) == 0x0112 {
            // A SHORT value of count one lives inline in the entry's value
            // field, which is where the orientation tag always is.
            if read_u16(bytes, entry + 2, endian) != 3 {
                return None;
            }
            return Some((entry + 8, endian));
        }
    }
    None
}

fn read_u16(bytes: &[u8], offset: usize, endian: Endian) -> u16 {
    let Some(slice) = bytes.get(offset..offset + 2) else {
        return 0;
    };
    let pair = [slice[0], slice[1]];
    match endian {
        Endian::Little => u16::from_le_bytes(pair),
        Endian::Big => u16::from_be_bytes(pair),
    }
}

fn read_u32(bytes: &[u8], offset: usize, endian: Endian) -> u32 {
    let Some(slice) = bytes.get(offset..offset + 4) else {
        return 0;
    };
    let quad = [slice[0], slice[1], slice[2], slice[3]];
    match endian {
        Endian::Little => u32::from_le_bytes(quad),
        Endian::Big => u32::from_be_bytes(quad),
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, endian: Endian, value: u16) {
    let pair = match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    };
    if let Some(slice) = bytes.get_mut(offset..offset + 2) {
        slice.copy_from_slice(&pair);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;
    use fluorita_core::{
        Annotation, Area, Canvas, Composition, EditClass, Orientation, OutputFormat, Point,
        Redaction, SaveChoice,
    };
    use siderita_ops::OpError;

    use super::{
        jpeg_orientation, reorient_jpeg, save, Bin, RasterFailure, Rasteriser, SaveRequest,
    };
    use crate::error::EngineError;

    /// A JPEG with exactly one EXIF entry: the orientation tag.
    ///
    /// Hand-built rather than checked in as a binary so the test says what it
    /// is testing. `pixels` stands in for the entropy-coded data, and the
    /// assertions below check that those bytes come out the other side
    /// untouched.
    fn jpeg_with_orientation(value: u16, pixels: &[u8]) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // count
        tiff.extend_from_slice(&value.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]); // the rest of the inline value field
        tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD

        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let length = (2 + 6 + tiff.len()) as u16;
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(b"Exif\0\0");
        bytes.extend_from_slice(&tiff);
        bytes.extend_from_slice(&[0xFF, 0xDA]);
        bytes.extend_from_slice(pixels);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        bytes
    }

    fn jpeg_without_exif() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xDA, 1, 2, 3, 0xFF, 0xD9]
    }

    struct FakeRasteriser {
        bytes: Vec<u8>,
        calls: RefCell<usize>,
    }

    impl FakeRasteriser {
        fn new() -> Self {
            Self {
                bytes: b"rendered".to_vec(),
                calls: RefCell::new(0),
            }
        }
    }

    impl Rasteriser for FakeRasteriser {
        fn render(
            &self,
            _source: &Path,
            _composition: &Composition,
            _format: OutputFormat,
            _quality: Option<u8>,
        ) -> Result<Vec<u8>, RasterFailure> {
            *self.calls.borrow_mut() += 1;
            Ok(self.bytes.clone())
        }
    }

    struct BrokenRasteriser;

    impl Rasteriser for BrokenRasteriser {
        fn render(
            &self,
            _source: &Path,
            _composition: &Composition,
            _format: OutputFormat,
            _quality: Option<u8>,
        ) -> Result<Vec<u8>, RasterFailure> {
            Err(RasterFailure::new("the toolkit refused the file"))
        }
    }

    /// Records what the destination looked like at the moment it was asked to
    /// move the original, which is the only way to prove the ordering.
    #[derive(Default)]
    struct FakeBin {
        asked_for: RefCell<Vec<PathBuf>>,
        destination_existed: RefCell<Option<bool>>,
        watching: RefCell<Option<PathBuf>>,
    }

    impl FakeBin {
        fn watching(destination: &Path) -> Self {
            Self {
                watching: RefCell::new(Some(destination.to_path_buf())),
                ..Self::default()
            }
        }
    }

    impl Bin for FakeBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            if let Some(destination) = self.watching.borrow().as_ref() {
                *self.destination_existed.borrow_mut() = Some(destination.exists());
            }
            self.asked_for.borrow_mut().push(path.to_path_buf());
            std::fs::remove_file(path).map_err(|error| OpError::io(path, &error))?;
            Ok(PathBuf::from("/trash").join(path.file_name().unwrap_or_default()))
        }
    }

    struct RefusingBin;

    impl Bin for RefusingBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            Err(OpError::io(
                path,
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ))
        }
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-edit-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("test directory");
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).expect("test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn composition() -> Composition {
        Composition {
            canvas: Canvas::new(100, 80).expect("a canvas"),
            transforms: Vec::new(),
            objects: vec![Annotation::Redact {
                area: Area::new(Point::new(1.0, 1.0), 10.0, 10.0),
                style: Redaction::Pixelate,
            }],
        }
    }

    fn request<'a>(
        source: &'a Path,
        composition: &'a Composition,
        choice: SaveChoice,
        format: OutputFormat,
        orientation: Option<Orientation>,
    ) -> SaveRequest<'a> {
        SaveRequest {
            source,
            composition,
            orientation,
            format,
            choice,
            copy_marker: "editado",
        }
    }

    fn quarter_turn() -> Orientation {
        Orientation {
            quarters: 1,
            mirrored: false,
        }
    }

    #[test]
    fn turning_a_jpeg_changes_two_bytes_and_leaves_every_other_one_alone() {
        let original = jpeg_with_orientation(1, b"entropy-coded-data");
        let turned = reorient_jpeg(&original, quarter_turn()).expect("the tag is there to change");

        assert_eq!(turned.len(), original.len());
        let differing: Vec<usize> = original
            .iter()
            .zip(&turned)
            .enumerate()
            .filter_map(|(index, (before, after))| (before != after).then_some(index))
            .collect();
        assert_eq!(
            differing.len(),
            1,
            "a quarter turn moved more than the orientation value: {differing:?}"
        );
        assert_eq!(jpeg_orientation(&turned), Some(quarter_turn()));
        assert!(
            turned
                .windows(18)
                .any(|window| window == b"entropy-coded-data"),
            "the pixels were re-encoded"
        );
    }

    #[test]
    fn a_photograph_the_camera_already_marked_is_added_to_rather_than_overwritten() {
        let already_turned = jpeg_with_orientation(6, b"data");
        let turned = reorient_jpeg(&already_turned, quarter_turn()).expect("the tag is there");
        assert_eq!(
            jpeg_orientation(&turned),
            Some(Orientation {
                quarters: 2,
                mirrored: false
            }),
            "two quarter turns are a half turn, not one quarter turn"
        );
    }

    #[test]
    fn a_file_with_no_orientation_tag_is_left_for_the_renderer() {
        assert_eq!(reorient_jpeg(&jpeg_without_exif(), quarter_turn()), None);
        assert_eq!(jpeg_orientation(&jpeg_without_exif()), None);
        assert_eq!(reorient_jpeg(b"not a jpeg at all", quarter_turn()), None);
        assert_eq!(reorient_jpeg(&[0xFF, 0xD8], quarter_turn()), None);
    }

    #[test]
    fn a_truncated_or_lying_exif_segment_is_refused_rather_than_read_past() {
        let mut lying = jpeg_with_orientation(1, b"data");
        // Claim a segment far longer than the file.
        lying[4] = 0xFF;
        lying[5] = 0xFF;
        assert_eq!(reorient_jpeg(&lying, quarter_turn()), None);

        let truncated = &jpeg_with_orientation(1, b"data")[..12];
        assert_eq!(reorient_jpeg(truncated, quarter_turn()), None);
    }

    #[test]
    fn a_turn_on_a_jpeg_is_written_losslessly_and_the_renderer_is_never_asked() {
        let directory = TestDir::new("lossless");
        let source = directory.file("foto.jpg", &jpeg_with_orientation(1, b"pixels"));
        let composition = composition();
        let rasteriser = FakeRasteriser::new();

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Replace,
                OutputFormat::Jpeg,
                Some(quarter_turn()),
            ),
            &rasteriser,
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(saved.class, EditClass::Lossless);
        assert_eq!(saved.written, source);
        assert_eq!(saved.trashed_original, None, "it replaced its own path");
        assert_eq!(*rasteriser.calls.borrow(), 0, "no pixel was re-encoded");
        assert_eq!(
            jpeg_orientation(&std::fs::read(&source).expect("the file")),
            Some(quarter_turn())
        );
    }

    #[test]
    fn a_turn_that_cannot_be_a_header_change_falls_back_and_says_so() {
        let directory = TestDir::new("fallback");
        let source = directory.file("foto.jpg", &jpeg_without_exif());
        let composition = composition();
        let rasteriser = FakeRasteriser::new();

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Copy,
                OutputFormat::Jpeg,
                Some(quarter_turn()),
            ),
            &rasteriser,
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(
            saved.class,
            EditClass::Raster,
            "a save that had to re-render never reports the original as intact"
        );
        assert_eq!(*rasteriser.calls.borrow(), 1);
    }

    #[test]
    fn a_copy_lands_beside_the_original_and_leaves_it_untouched() {
        let directory = TestDir::new("copy");
        let source = directory.file("foto.jpg", &jpeg_with_orientation(1, b"pixels"));
        let composition = composition();

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Copy,
                OutputFormat::Jpeg,
                None,
            ),
            &FakeRasteriser::new(),
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(saved.written, directory.0.join("foto (editado).jpg"));
        assert_eq!(saved.trashed_original, None);
        assert!(source.exists(), "a copy never moves the original");
        assert_eq!(
            std::fs::read(&saved.written).expect("the copy"),
            b"rendered".to_vec()
        );
    }

    #[test]
    fn a_second_copy_does_not_overwrite_the_first() {
        let directory = TestDir::new("second-copy");
        let source = directory.file("foto.jpg", &jpeg_with_orientation(1, b"pixels"));
        directory.file("foto (editado).jpg", b"the first copy");
        let composition = composition();

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Copy,
                OutputFormat::Jpeg,
                None,
            ),
            &FakeRasteriser::new(),
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(saved.written, directory.0.join("foto (editado 2).jpg"));
        assert_eq!(
            std::fs::read(directory.0.join("foto (editado).jpg")).expect("the first copy"),
            b"the first copy".to_vec()
        );
    }

    #[test]
    fn a_copy_never_overwrites_a_file_that_already_has_the_name_it_will_be_written_under() {
        let directory = TestDir::new("copy-collision");
        let source = directory.file("captura.gif", b"a gif the toolkit cannot write");
        // The name the first copy would take, already occupied by something
        // else the person owns.
        directory.file("captura (editado).png", b"do not lose me");
        let composition = composition();

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Copy,
                OutputFormat::Png,
                None,
            ),
            &FakeRasteriser::new(),
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(saved.written, directory.0.join("captura (editado 2).png"));
        assert_eq!(
            std::fs::read(directory.0.join("captura (editado).png")).expect("the earlier file"),
            b"do not lose me".to_vec(),
            "the keep-both name searched under the wrong extension and overwrote a real file"
        );
    }

    #[test]
    fn a_replacement_writes_the_destination_before_it_moves_the_original() {
        let directory = TestDir::new("replace-order");
        let source = directory.file("captura.gif", b"a gif the toolkit cannot write");
        let destination = directory.0.join("captura.png");
        let composition = composition();
        let bin = FakeBin::watching(&destination);

        let saved = save(
            &request(
                &source,
                &composition,
                SaveChoice::Replace,
                OutputFormat::Png,
                None,
            ),
            &FakeRasteriser::new(),
            &bin,
            &CancellationToken::new(),
        )
        .expect("the save lands");

        assert_eq!(saved.written, destination);
        assert_eq!(
            *bin.destination_existed.borrow(),
            Some(true),
            "the original was moved before the result existed"
        );
        assert_eq!(
            bin.asked_for.borrow().as_slice(),
            std::slice::from_ref(&source)
        );
        assert_eq!(
            saved.trashed_original,
            Some(PathBuf::from("/trash/captura.gif"))
        );
        assert!(!source.exists());
    }

    #[test]
    fn a_replacement_that_cannot_reach_the_trash_still_leaves_the_result_written() {
        let directory = TestDir::new("replace-refused");
        let source = directory.file("captura.gif", b"a gif");
        let composition = composition();

        let failure = save(
            &request(
                &source,
                &composition,
                SaveChoice::Replace,
                OutputFormat::Png,
                None,
            ),
            &FakeRasteriser::new(),
            &RefusingBin,
            &CancellationToken::new(),
        )
        .expect_err("the Trash refused");

        assert!(matches!(failure, EngineError::Trash { .. }));
        assert!(
            directory.0.join("captura.png").exists(),
            "the result must survive a failed Trash step"
        );
        assert!(source.exists(), "and so must the original");
    }

    #[test]
    fn a_toolkit_failure_writes_nothing_at_all() {
        let directory = TestDir::new("broken");
        let source = directory.file("foto.jpg", &jpeg_with_orientation(1, b"pixels"));
        let composition = composition();

        let failure = save(
            &request(
                &source,
                &composition,
                SaveChoice::Replace,
                OutputFormat::Jpeg,
                None,
            ),
            &BrokenRasteriser,
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect_err("the toolkit refused");

        assert!(matches!(failure, EngineError::Undecodable { .. }));
        assert_eq!(
            std::fs::read(&source).expect("the original"),
            jpeg_with_orientation(1, b"pixels"),
            "a failed render left the original exactly as it was"
        );
    }

    #[test]
    fn a_cancelled_save_never_writes() {
        let directory = TestDir::new("cancelled");
        let source = directory.file("foto.jpg", &jpeg_with_orientation(1, b"pixels"));
        let composition = composition();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let failure = save(
            &request(
                &source,
                &composition,
                SaveChoice::Copy,
                OutputFormat::Jpeg,
                None,
            ),
            &FakeRasteriser::new(),
            &FakeBin::default(),
            &cancellation,
        )
        .expect_err("cancelled");
        assert!(matches!(failure, EngineError::Cancelled));
        assert!(!directory.0.join("foto (editado).jpg").exists());
    }

    #[test]
    fn a_relative_source_is_refused_before_anything_is_read() {
        let composition = composition();
        let failure = save(
            &request(
                Path::new("foto.jpg"),
                &composition,
                SaveChoice::Copy,
                OutputFormat::Jpeg,
                None,
            ),
            &FakeRasteriser::new(),
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect_err("refused");
        assert!(matches!(failure, EngineError::UnusableSource { .. }));
    }
}
