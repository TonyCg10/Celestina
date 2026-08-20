// language-contract: product-copy
//
//! The words the metadata panel shows.
//!
//! One module so product copy has a single owner, exactly as `library/copy.rs`
//! and `editor/copy.rs` do. The marker at the head exempts the string literals
//! here and nothing else.

use fluorita_core::{MetadataRejected, PrivateFact};
use fluorita_engine::MetadataWritten;

/// The word a corrected copy's name is marked with.
pub(super) const COPY_MARKER: &str = "editado";

pub(super) const UNREADABLE_KEY: &str = "Fluorita no pudo interpretar la ruta de este elemento";
pub(super) const NO_METADATA: &str = "Este archivo no lleva datos que Fluorita pueda mostrar";
pub(super) const NOTHING_CARRIED: &str = "Esta imagen no lleva nada que quitar";

/// Said when the container's tags can be read and not written. The reason is
/// given, because "no se puede" without one reads as a fallo.
pub(super) const READ_ONLY_CONTAINER: &str =
    "Fluorita puede leer las etiquetas de este formato, pero todavía no escribirlas sin recomprimir el audio";

/// The chooser's own words.
pub(super) const COVER_TITLE: &str = "Elegir portada";
pub(super) const COVER_FILTER: &str = "Imágenes";

pub(super) fn private_fact(fact: PrivateFact) -> &'static str {
    match fact {
        PrivateFact::Location => "Dónde se hizo",
        PrivateFact::Camera => "Con qué se hizo",
        PrivateFact::Timestamp => "Cuándo se hizo",
    }
}

pub(super) fn written(written: &MetadataWritten) -> String {
    let name = written
        .written
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if written.trashed_original.is_some() {
        format!("Guardado en {name}; el original está en la papelera")
    } else {
        format!("Guardado en {name}")
    }
}

pub(super) fn rejected(rejected: MetadataRejected) -> &'static str {
    match rejected {
        MetadataRejected::NotSupported => NO_METADATA,
        MetadataRejected::ReadOnlyContainer => READ_ONLY_CONTAINER,
        MetadataRejected::ValueTooLong => "Ese valor es demasiado largo",
        MetadataRejected::ValueNotPrintable => {
            "Ese valor lleva caracteres que la etiqueta no admite"
        }
        MetadataRejected::NothingRequested => "No hay cambios que guardar",
        MetadataRejected::NoChange => "El archivo ya dice eso",
        MetadataRejected::CoverTooLarge => "Esa portada es demasiado grande",
        MetadataRejected::CoverNotAnImage => "Eso no es una imagen que se pueda incrustar",
    }
}
