//! Drawing an edit with the toolkit that already reads these pictures.
//!
//! This is the application's half of the engine's [`Rasteriser`] seam: the
//! engine decides *whether* and *where* to write, and this decides what the
//! result looks like. The Qt calls themselves live in `cpp/imagecanvas.cpp`,
//! because cxx-qt-lib exposes no `QBrush`, no `QTransform` and no image
//! encoder; what stays here is the walk over the composition, so the shape of
//! an annotation is described in one language and drawn in the other.
//!
//! The canvas is opened by path key (ADR 0008) and with EXIF orientation
//! applied, which is the same frame `imageprobe.cpp` measures in and therefore
//! the frame every stored coordinate refers to.

use std::path::Path;

use celestina_core::pathkey;
use cxx_qt_lib::QString;
use fluorita_core::{
    Annotation, Axis, Composition, Ink, OutputFormat, Quarter, Redaction, ShapeKind, Transform,
};
use fluorita_engine::{RasterFailure, Rasteriser};

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;

        // The drawing seam. See `cpp/fluorita/imagecanvas.h` for why it is
        // hand-written C++ rather than cxx-qt-lib calls.
        include!("fluorita/imagecanvas.h");

        type FluoritaCanvas;

        #[rust_name = "open_canvas"]
        fn fluorita_open_canvas(key: &QString) -> UniquePtr<FluoritaCanvas>;

        #[cxx_name = "isEmpty"]
        fn is_empty(self: &FluoritaCanvas) -> bool;
        fn width(self: &FluoritaCanvas) -> i32;
        fn height(self: &FluoritaCanvas) -> i32;

        fn rotate(self: Pin<&mut FluoritaCanvas>, quarters: i32);
        fn flip(self: Pin<&mut FluoritaCanvas>, horizontal: bool, vertical: bool);
        fn crop(self: Pin<&mut FluoritaCanvas>, x: f32, y: f32, width: f32, height: f32);
        fn resize(self: Pin<&mut FluoritaCanvas>, width: i32, height: i32);

        #[cxx_name = "drawStroke"]
        fn draw_stroke(self: Pin<&mut FluoritaCanvas>, points: &[f32], width: f32, rgba: u32);
        #[cxx_name = "drawLine"]
        #[allow(clippy::too_many_arguments)]
        fn draw_line(
            self: Pin<&mut FluoritaCanvas>,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            width: f32,
            rgba: u32,
            arrow: bool,
        );
        #[cxx_name = "drawShape"]
        #[allow(clippy::too_many_arguments)]
        fn draw_shape(
            self: Pin<&mut FluoritaCanvas>,
            ellipse: bool,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            stroke: f32,
            rgba: u32,
            filled: bool,
            fill_rgba: u32,
        );
        #[cxx_name = "drawHighlight"]
        fn draw_highlight(
            self: Pin<&mut FluoritaCanvas>,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            rgba: u32,
        );
        fn redact(
            self: Pin<&mut FluoritaCanvas>,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            blur: bool,
        );
        #[cxx_name = "drawText"]
        #[allow(clippy::too_many_arguments)]
        fn draw_text(
            self: Pin<&mut FluoritaCanvas>,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            size: f32,
            rgba: u32,
            has_backdrop: bool,
            backdrop_rgba: u32,
            quarters: i32,
            text: &QString,
        );

        fn encode(self: &FluoritaCanvas, format: &QString, quality: i32) -> QByteArray;
    }
}

/// Draws with Qt. The only [`Rasteriser`] the application has.
pub struct ToolkitRasteriser;

impl Rasteriser for ToolkitRasteriser {
    fn render(
        &self,
        source: &Path,
        composition: &Composition,
        format: OutputFormat,
        quality: Option<u8>,
    ) -> Result<Vec<u8>, RasterFailure> {
        let key = QString::from(&pathkey::encode(source));
        let mut canvas = ffi::open_canvas(&key);
        let Some(mut canvas) = canvas.as_mut() else {
            return Err(RasterFailure::new("the toolkit returned no canvas"));
        };
        if canvas.is_empty() {
            return Err(RasterFailure::new("the toolkit could not read the picture"));
        }

        for transform in &composition.transforms {
            apply(canvas.as_mut(), *transform);
        }
        for object in &composition.objects {
            draw(canvas.as_mut(), object);
        }

        // The canvas the toolkit ends at must be the canvas the domain
        // computed. A mismatch means the two disagree about what an operation
        // does, and writing the result anyway would put an edit on disk that
        // nothing can reproduce.
        let expected = (
            i32::try_from(composition.canvas.width()).unwrap_or(i32::MAX),
            i32::try_from(composition.canvas.height()).unwrap_or(i32::MAX),
        );
        let actual = (canvas.width(), canvas.height());
        if actual != expected {
            return Err(RasterFailure::new(format!(
                "the toolkit produced {actual:?} where the document says {expected:?}"
            )));
        }

        let bytes = canvas.as_ref().encode(
            &QString::from(format.extension()),
            quality.map_or(-1, i32::from),
        );
        if bytes.is_empty() {
            return Err(RasterFailure::new("the toolkit encoded nothing"));
        }
        Ok(bytes.as_slice().to_vec())
    }
}

fn apply(mut canvas: std::pin::Pin<&mut ffi::FluoritaCanvas>, transform: Transform) {
    match transform {
        Transform::Rotate(quarter) => canvas.as_mut().rotate(match quarter {
            Quarter::Clockwise => 1,
            Quarter::Half => 2,
            Quarter::CounterClockwise => 3,
        }),
        Transform::Flip(axis) => canvas.as_mut().flip(
            matches!(axis, Axis::Horizontal),
            matches!(axis, Axis::Vertical),
        ),
        Transform::Crop(area) => {
            canvas
                .as_mut()
                .crop(area.origin.x, area.origin.y, area.width, area.height);
        }
        Transform::Resize(size) => canvas.as_mut().resize(
            i32::try_from(size.width()).unwrap_or(i32::MAX),
            i32::try_from(size.height()).unwrap_or(i32::MAX),
        ),
    }
}

fn draw(mut canvas: std::pin::Pin<&mut ffi::FluoritaCanvas>, object: &Annotation) {
    match object {
        Annotation::Text {
            area,
            text,
            size,
            ink,
            backdrop,
            quarters,
        } => canvas.as_mut().draw_text(
            area.origin.x,
            area.origin.y,
            area.width,
            area.height,
            *size,
            packed(*ink),
            backdrop.is_some(),
            backdrop.map_or(0, packed),
            i32::from(*quarters),
            &QString::from(text),
        ),
        Annotation::Stroke { points, width, ink } => {
            let flattened: Vec<f32> = points.iter().flat_map(|point| [point.x, point.y]).collect();
            canvas
                .as_mut()
                .draw_stroke(&flattened, *width, packed(*ink));
        }
        Annotation::Line {
            from,
            to,
            width,
            ink,
            arrow,
        } => canvas
            .as_mut()
            .draw_line(from.x, from.y, to.x, to.y, *width, packed(*ink), *arrow),
        Annotation::Shape {
            kind,
            area,
            width,
            ink,
            fill,
        } => canvas.as_mut().draw_shape(
            matches!(kind, ShapeKind::Ellipse),
            area.origin.x,
            area.origin.y,
            area.width,
            area.height,
            *width,
            packed(*ink),
            fill.is_some(),
            fill.map_or(0, packed),
        ),
        Annotation::Highlight { area, ink } => canvas.as_mut().draw_highlight(
            area.origin.x,
            area.origin.y,
            area.width,
            area.height,
            packed(*ink),
        ),
        Annotation::Redact { area, style } => canvas.as_mut().redact(
            area.origin.x,
            area.origin.y,
            area.width,
            area.height,
            matches!(style, Redaction::Blur),
        ),
    }
}

/// `0xRRGGBBAA`, which is how the canvas unpacks it.
const fn packed(ink: Ink) -> u32 {
    ((ink.red as u32) << 24)
        | ((ink.green as u32) << 16)
        | ((ink.blue as u32) << 8)
        | ink.alpha as u32
}

#[cfg(test)]
mod tests {
    use super::packed;
    use fluorita_core::Ink;

    #[test]
    fn an_ink_packs_in_the_order_the_canvas_unpacks() {
        assert_eq!(packed(Ink::new(0x12, 0x34, 0x56, 0x78)), 0x1234_5678);
        assert_eq!(packed(Ink::new(255, 255, 255, 255)), 0xFFFF_FFFF);
        assert_eq!(packed(Ink::new(0, 0, 0, 0)), 0);
    }
}

#[cfg(test)]
mod canvas_tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use fluorita_core::{
        Annotation, Area, Canvas, Composition, Ink, OutputFormat, Point, Quarter, Redaction,
        Transform,
    };
    use fluorita_engine::Rasteriser;

    use super::ToolkitRasteriser;

    /// A 4×4 24-bit BMP, written by hand.
    ///
    /// A fixture the test can state in full rather than a binary checked in
    /// beside it: BMP is the one format whose bytes are all header and pixels,
    /// so what the toolkit is being handed is visible here.
    fn bitmap() -> Vec<u8> {
        let width = 4i32;
        let height = 4i32;
        // Each row is padded to four bytes; 4 pixels × 3 bytes is already 12.
        let row = (width as usize) * 3;
        let pixels = row * (height as usize);
        let offset = 14 + 40;
        let mut out = Vec::with_capacity(offset + pixels);

        out.extend_from_slice(b"BM");
        out.extend_from_slice(&((offset + pixels) as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());

        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&24u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(pixels as u32).to_le_bytes());
        out.extend_from_slice(&2835u32.to_le_bytes());
        out.extend_from_slice(&2835u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());

        // Blue, green, red per pixel; the values do not matter, only that the
        // reader accepts them.
        out.extend(std::iter::repeat_n(0x40u8, pixels));
        out
    }

    fn fixture(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "fluorita-canvas-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("test directory");
        let path = directory.join("source.bmp");
        std::fs::write(&path, bitmap()).expect("test picture");
        path
    }

    fn canvas(width: u32, height: u32) -> Canvas {
        Canvas::new(width, height).expect("a canvas with area")
    }

    #[test]
    fn the_toolkit_really_turns_crops_and_encodes_a_picture() {
        let path = fixture("render");
        let composition = Composition {
            canvas: canvas(2, 2),
            transforms: vec![
                Transform::Rotate(Quarter::Clockwise),
                Transform::Crop(Area::new(Point::new(0.0, 0.0), 2.0, 2.0)),
            ],
            objects: vec![
                Annotation::Redact {
                    area: Area::new(Point::new(0.0, 0.0), 1.0, 1.0),
                    style: Redaction::Pixelate,
                },
                Annotation::Shape {
                    kind: fluorita_core::ShapeKind::Rectangle,
                    area: Area::new(Point::new(0.0, 0.0), 2.0, 2.0),
                    width: 1.0,
                    ink: Ink::new(255, 0, 0, 255),
                    fill: None,
                },
            ],
        };

        let bytes = ToolkitRasteriser
            .render(&path, &composition, OutputFormat::Png, None)
            .expect("the toolkit drew and encoded the result");

        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "the result is not a PNG");
        let _ = std::fs::remove_dir_all(path.parent().expect("the fixture directory"));
    }

    #[test]
    fn a_canvas_the_toolkit_ends_at_must_match_the_document() {
        let path = fixture("mismatch");
        // The document claims a canvas no transform produces.
        let composition = Composition {
            canvas: canvas(9, 9),
            transforms: Vec::new(),
            objects: Vec::new(),
        };

        let failure = ToolkitRasteriser
            .render(&path, &composition, OutputFormat::Png, None)
            .expect_err("a disagreement must not be written");
        assert!(
            failure.detail.contains("the document says"),
            "unexpected refusal: {}",
            failure.detail
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("the fixture directory"));
    }

    #[test]
    fn a_file_the_toolkit_cannot_read_is_refused_rather_than_drawn_on() {
        let path = fixture("unreadable");
        std::fs::write(&path, b"not a picture at all").expect("test file");
        let composition = Composition {
            canvas: canvas(4, 4),
            transforms: Vec::new(),
            objects: Vec::new(),
        };

        let failure = ToolkitRasteriser
            .render(&path, &composition, OutputFormat::Png, None)
            .expect_err("refused");
        assert!(failure.detail.contains("could not read"));

        // And a key that names no file at all.
        let missing = path.parent().expect("directory").join("gone.bmp");
        assert!(ToolkitRasteriser
            .render(&missing, &composition, OutputFormat::Png, None)
            .is_err());
        let _ = std::fs::remove_dir_all(path.parent().expect("the fixture directory"));
    }
}
