//! The annotations and form fields of a PDF, read and edited without Qt.
//!
//! QtPdf draws a page (with its annotations, through Calcita's page
//! provider) but exposes none of its annotation objects; this module reads
//! them with `lopdf`, off the Qt thread, as Calcita's own model, keeps the
//! edits made to them ([`edit`]) and writes those as an incremental update
//! with their appearance streams ([`write`], [`appearance`]); [`form`] reads
//! the form's fields, gives QtPdf a copy that shows its widgets and fills
//! text fields and checkboxes. The
//! coordinate conversion between PDF user space and the page QtPdf shows
//! lives in [`coords`].

pub mod appearance;
pub mod coords;
pub mod edit;
pub mod form;
mod model;
mod read;
pub mod write;

pub use model::{AnnotId, Annotation, Field, FieldKind, Kind, Point, Quad, Rect, Rgb};
pub use read::{read_annotations, read_fields, read_page_boxes, AnnotError, MAX_FILE_BYTES};
