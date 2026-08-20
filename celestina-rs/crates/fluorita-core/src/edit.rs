//! What may be edited, in which class, and what comes out of it.
//!
//! This module answers three questions before anything is read from disk:
//! whether an item admits an operation at all, whether that operation
//! *reorders the original bytes* or *produces a new image*, and which format
//! the result is written in. All three are contract, not implementation
//! detail: [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md)
//! requires the surface to distinguish the two classes, because an interface
//! that offers them identically lets a person believe an original survived
//! when it did not.
//!
//! Nothing here decodes, allocates a canvas or touches a file. A path and a
//! kind are enough, which is what lets a grid answer "can this be edited?" for
//! every visible card without opening one of them.

use crate::media::MediaKind;
use std::ffi::OsStr;
use std::path::Path;

/// Which side of the contract an operation falls on.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EditClass {
    /// The original bytes are reordered or re-described. Nothing is
    /// recompressed, so the picture a person keeps is the picture they had.
    Lossless,
    /// A new image is produced. The result is a different file, and saving it
    /// over the original is a decision with a consequence.
    Raster,
}

impl EditClass {
    /// Whether the original pixel data survives the operation untouched.
    #[must_use]
    pub const fn preserves_original_bytes(self) -> bool {
        matches!(self, Self::Lossless)
    }
}

/// One editing operation, as the product offers it.
///
/// This is the F7 set. Metadata editing, stream-copy trimming, frame
/// extraction and batch application are authorised in intent by ADR 0009 and
/// open as their own checkpoints; they are deliberately absent here rather
/// than present and refused, so nothing can offer an operation the engine
/// cannot perform.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Operation {
    /// A quarter-turn of the whole canvas.
    Rotate,
    /// A mirror of the whole canvas across one axis.
    Flip,
    /// Keep a rectangle of the canvas and discard the rest.
    Crop,
    /// Change the canvas dimensions.
    Resize,
    /// Place text, strokes, shapes, a highlighter or a redaction on the canvas.
    Annotate,
}

impl Operation {
    /// Every operation this checkpoint offers, in the order a surface would
    /// present them.
    pub const ALL: [Self; 5] = [
        Self::Rotate,
        Self::Flip,
        Self::Crop,
        Self::Resize,
        Self::Annotate,
    ];
}

/// An image format Fluorita recognises by name.
///
/// The list is exactly [`crate::media`]'s image extensions, so the library and
/// the editor never disagree about what a picture is.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Ico,
    Tiff,
    Avif,
    Jxl,
    Heif,
}

impl ImageFormat {
    /// Classifies by filename extension, ASCII-case-insensitively.
    ///
    /// Like [`MediaKind::from_extension`](crate::media::MediaKind::from_extension)
    /// this is a hint. A file claiming `.png` and holding something else is
    /// caught when the toolkit reads it, not here.
    #[must_use]
    pub fn from_extension(extension: &OsStr) -> Option<Self> {
        let extension = extension.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" => Self::Jpeg,
            "gif" => Self::Gif,
            "webp" => Self::Webp,
            "bmp" => Self::Bmp,
            "ico" => Self::Ico,
            "tif" | "tiff" => Self::Tiff,
            "avif" => Self::Avif,
            "jxl" => Self::Jxl,
            "heic" | "heif" => Self::Heif,
            _ => return None,
        })
    }

    /// Classifies a path by its extension.
    #[must_use]
    pub fn classify_path(path: &Path) -> Option<Self> {
        path.extension().and_then(Self::from_extension)
    }

    /// Whether the writer can turn this picture by rewriting metadata instead
    /// of touching a pixel.
    ///
    /// This is what makes turning a photograph the right way up the cheapest
    /// and safest edit in the product: a JPEG's EXIF orientation tag is a
    /// two-byte change inside a segment the pixels never see.
    ///
    /// TIFF, WebP, AVIF and HEIF also record orientation, and they answer
    /// `false` anyway: rewriting each of their containers is its own piece of
    /// work, and claiming a lossless turn the writer cannot actually perform
    /// would be exactly the lie this classification exists to prevent. They
    /// re-render like everything else until that work is done.
    #[must_use]
    pub const fn carries_orientation(self) -> bool {
        matches!(self, Self::Jpeg)
    }

    /// Whether the toolkit that reads this format can also write it.
    ///
    /// The formats that answer `false` are readable but not writable here, and
    /// under ADR 0009 that is not a reason to refuse the edit — it is the
    /// reason [`ImageFormat::output`] exists.
    #[must_use]
    pub const fn is_writable(self) -> bool {
        matches!(
            self,
            Self::Png | Self::Jpeg | Self::Webp | Self::Bmp | Self::Tiff
        )
    }

    /// The fixed output rule: keep the original's format when it can carry the
    /// result, and fall back to PNG when it cannot. There is no format
    /// dialogue, so this function is the whole decision.
    ///
    /// A GIF therefore leaves as a PNG. Any raster edit of an animated GIF
    /// already loses its animation — there is one canvas to draw on — so the
    /// rule states that loss in the extension rather than hiding it inside a
    /// file that still claims to be a GIF.
    #[must_use]
    pub const fn output(self) -> OutputFormat {
        match self {
            Self::Png => OutputFormat::Png,
            Self::Jpeg => OutputFormat::Jpeg,
            Self::Webp => OutputFormat::Webp,
            Self::Bmp => OutputFormat::Bmp,
            Self::Tiff => OutputFormat::Tiff,
            Self::Gif | Self::Ico | Self::Avif | Self::Jxl | Self::Heif => OutputFormat::Png,
        }
    }
}

/// A format the writer can actually produce.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
    Bmp,
    Tiff,
}

impl OutputFormat {
    /// The extension the written file carries.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
        }
    }

    /// The quality a lossy writer uses, or `None` for a lossless one.
    ///
    /// Fixed, because the product asks no format question and therefore asks
    /// no quality question either. High enough that one edit-and-save cycle is
    /// not visible; not 100, which mostly buys file size.
    #[must_use]
    pub const fn quality(self) -> Option<u8> {
        match self {
            Self::Jpeg | Self::Webp => Some(HIGH_QUALITY),
            Self::Png | Self::Bmp | Self::Tiff => None,
        }
    }

    /// Whether writing this format discards information the input had.
    #[must_use]
    pub const fn is_lossy(self) -> bool {
        self.quality().is_some()
    }
}

/// The quality every lossy write uses. See [`OutputFormat::quality`].
pub const HIGH_QUALITY: u8 = 95;

/// What one item admits, decided from its kind and its name alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditCapabilities {
    kind: MediaKind,
    format: Option<ImageFormat>,
}

impl EditCapabilities {
    /// Reads the matrix for one catalogued item.
    ///
    /// `kind` comes from the catalogue, which already classified the file;
    /// `path` is needed because the image format decides whether turning the
    /// picture is a header change or a re-render.
    #[must_use]
    pub fn of(kind: MediaKind, path: &Path) -> Self {
        Self {
            kind,
            format: match kind {
                MediaKind::Image => ImageFormat::classify_path(path),
                MediaKind::Video | MediaKind::Audio => None,
            },
        }
    }

    /// Whether this item can be edited at all in this checkpoint.
    ///
    /// Video and audio answer `false`: their editing is stream-copy work that
    /// ADR 0009 authorises and does not open. An image whose extension is not
    /// recognised also answers `false`, because the output rule has nothing to
    /// decide from.
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        self.format.is_some()
    }

    /// The class an operation falls in, or `None` when this item does not
    /// admit it.
    ///
    /// The one operation whose answer varies is orientation: on a format that
    /// records it as metadata, rotating and flipping are
    /// [`EditClass::Lossless`]; on one that does not, they cost a re-render
    /// like everything else.
    #[must_use]
    pub fn admits(&self, operation: Operation) -> Option<EditClass> {
        let format = self.format?;
        Some(match operation {
            Operation::Rotate | Operation::Flip if format.carries_orientation() => {
                EditClass::Lossless
            }
            Operation::Rotate
            | Operation::Flip
            | Operation::Crop
            | Operation::Resize
            | Operation::Annotate => EditClass::Raster,
        })
    }

    /// The format a saved result is written in, or `None` for an item that is
    /// not editable.
    #[must_use]
    pub fn output_format(&self) -> Option<OutputFormat> {
        self.format.map(ImageFormat::output)
    }

    /// The source format, when the item is an image this editor recognises.
    #[must_use]
    pub const fn source_format(&self) -> Option<ImageFormat> {
        self.format
    }

    /// The kind this matrix was read for.
    #[must_use]
    pub const fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Whether saving would leave the file in a different container than the
    /// one it arrived in. A surface says so before the person saves, because
    /// the name on disk changes with it.
    #[must_use]
    pub fn changes_container(&self) -> bool {
        self.format
            .is_some_and(|format| !ImageFormat::is_writable(format))
    }
}

/// The two outcomes saving offers. There is no third.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SaveChoice {
    /// Write beside the original and leave it untouched.
    Copy,
    /// Write the new bytes in place and send the original to the desktop
    /// Trash. Never an `unlink`, and never before the destination is
    /// confirmed.
    Replace,
}

impl SaveChoice {
    /// Whether the file the edit was computed from is still on disk
    /// afterwards.
    #[must_use]
    pub const fn keeps_the_base(self) -> bool {
        matches!(self, Self::Copy)
    }

    /// Whether the composed stack survives the save, so the item can be
    /// reopened with its objects intact.
    ///
    /// A replacement flattens. This is not a simplification: the stack
    /// describes operations *over a base*, and after a replacement the base is
    /// gone — reapplying it would compose the annotations onto bytes that
    /// already contain them.
    #[must_use]
    pub const fn stays_reopenable(self) -> bool {
        self.keeps_the_base()
    }
}

#[cfg(test)]
mod tests {
    use super::{EditCapabilities, EditClass, ImageFormat, Operation, OutputFormat, SaveChoice};
    use crate::media::MediaKind;
    use std::path::Path;

    fn image(name: &str) -> EditCapabilities {
        EditCapabilities::of(MediaKind::Image, Path::new(name))
    }

    #[test]
    fn turning_a_photograph_costs_no_pixels_and_cropping_it_does() {
        let photograph = image("/m/DSC_0001.JPG");
        assert_eq!(
            photograph.admits(Operation::Rotate),
            Some(EditClass::Lossless)
        );
        assert_eq!(
            photograph.admits(Operation::Flip),
            Some(EditClass::Lossless)
        );
        assert_eq!(photograph.admits(Operation::Crop), Some(EditClass::Raster));
        assert_eq!(
            photograph.admits(Operation::Resize),
            Some(EditClass::Raster)
        );
        assert_eq!(
            photograph.admits(Operation::Annotate),
            Some(EditClass::Raster)
        );
    }

    #[test]
    fn a_format_that_records_no_orientation_pays_for_the_turn() {
        let screenshot = image("/m/captura.png");
        assert_eq!(
            screenshot.admits(Operation::Rotate),
            Some(EditClass::Raster),
            "a PNG has no orientation header to rewrite"
        );
        assert!(!EditClass::Raster.preserves_original_bytes());
        assert!(EditClass::Lossless.preserves_original_bytes());
    }

    #[test]
    fn video_and_audio_are_not_editable_in_this_checkpoint() {
        let clip = EditCapabilities::of(MediaKind::Video, Path::new("/m/clip.mkv"));
        let track = EditCapabilities::of(MediaKind::Audio, Path::new("/m/pista.flac"));
        for item in [clip, track] {
            assert!(!item.is_editable());
            assert_eq!(item.output_format(), None);
            for operation in Operation::ALL {
                assert_eq!(item.admits(operation), None);
            }
        }
    }

    #[test]
    fn an_unrecognised_extension_is_not_editable() {
        let unknown = image("/m/foto.raw");
        assert!(!unknown.is_editable());
        assert_eq!(unknown.admits(Operation::Crop), None);
        assert_eq!(unknown.source_format(), None);
        assert_eq!(unknown.kind(), MediaKind::Image);
    }

    #[test]
    fn the_output_rule_keeps_the_format_it_can_write_and_falls_back_to_png() {
        assert_eq!(image("/m/a.jpeg").output_format(), Some(OutputFormat::Jpeg));
        assert_eq!(image("/m/a.png").output_format(), Some(OutputFormat::Png));
        assert_eq!(image("/m/a.webp").output_format(), Some(OutputFormat::Webp));
        assert_eq!(image("/m/a.tif").output_format(), Some(OutputFormat::Tiff));

        for name in ["/m/a.gif", "/m/a.ico", "/m/a.avif", "/m/a.jxl", "/m/a.heic"] {
            assert_eq!(
                image(name).output_format(),
                Some(OutputFormat::Png),
                "{name} is readable but not writable, so it leaves as a PNG"
            );
            assert!(image(name).changes_container());
        }
        assert!(!image("/m/a.jpg").changes_container());
    }

    #[test]
    fn every_image_extension_the_library_lists_is_classified_here() {
        // Mirrors IMAGE_EXTENSIONS in `media.rs`: the library and the editor
        // must not disagree about what a picture is.
        for extension in [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "jxl",
            "heic", "heif",
        ] {
            let name = format!("/m/file.{extension}");
            assert!(
                image(&name).is_editable(),
                "{extension} classifies as media but not as an editable image"
            );
        }
    }

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(
            ImageFormat::classify_path(Path::new("/m/Foto.JPEG")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::classify_path(Path::new("/m/Foto.HEIC")),
            Some(ImageFormat::Heif)
        );
        assert_eq!(ImageFormat::classify_path(Path::new("/m/Foto")), None);
    }

    #[test]
    fn a_lossy_output_carries_one_fixed_quality_and_a_lossless_one_carries_none() {
        assert_eq!(OutputFormat::Jpeg.quality(), Some(super::HIGH_QUALITY));
        assert_eq!(OutputFormat::Webp.quality(), Some(super::HIGH_QUALITY));
        assert_eq!(OutputFormat::Png.quality(), None);
        assert!(OutputFormat::Jpeg.is_lossy());
        assert!(!OutputFormat::Tiff.is_lossy());
        assert_eq!(OutputFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn a_copy_stays_reopenable_and_a_replacement_does_not() {
        assert!(SaveChoice::Copy.keeps_the_base());
        assert!(SaveChoice::Copy.stays_reopenable());
        assert!(!SaveChoice::Replace.keeps_the_base());
        assert!(
            !SaveChoice::Replace.stays_reopenable(),
            "the stack describes operations over a base the replacement removed"
        );
    }
}
