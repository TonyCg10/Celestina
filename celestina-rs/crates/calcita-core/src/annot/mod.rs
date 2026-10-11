//! The annotations and form fields of a PDF, read without Qt.
//!
//! QtPdf draws a page (with its annotations, through Calcita's page
//! provider) but exposes none of its annotation objects; this module reads
//! them with `lopdf`, off the Qt thread, as Calcita's own model. The
//! coordinate conversion between PDF user space and the page QtPdf shows
//! lives in [`coords`].

pub mod coords;
mod model;
mod read;

pub use model::{AnnotId, Annotation, Field, FieldKind, Kind, Point, Quad, Rect, Rgb};
pub use read::{read_annotations, read_fields, read_page_boxes, AnnotError, MAX_FILE_BYTES};
