//! Calcita's domain, without Qt: the zoom ladder, the page field's grammar,
//! the recent documents, the reading position each one is restored to, the
//! order a search walks its hits in, and a document's annotations and form
//! fields with the conversion to the page QtPdf shows, the edits made to
//! them and their writing as an incremental update.

pub mod annot;
pub mod pagelabel;
pub mod reading;
pub mod recent;
pub mod search;
pub mod zoom;
