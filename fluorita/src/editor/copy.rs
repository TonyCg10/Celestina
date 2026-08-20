// language-contract: product-copy
//
//! The words the editor shows.
//!
//! One module so product copy has a single owner, exactly as the library's own
//! `library/copy.rs` does. The marker at the head exempts the string literals
//! here and nothing else: the comments, names and diagnostics around them stay
//! English under
//! [ADR 0007](../../../docs/decisions/0007-spanish-product-copy.md).

use fluorita_core::{EditClass, EditRejected};
use fluorita_engine::{EngineError, Saved};

/// The word a saved copy's name is marked with. The domain owns the collision
/// search; the application owns the wording.
pub(super) const COPY_MARKER: &str = "editado";
pub(super) const NOT_EDITABLE: &str = "Este archivo no se puede editar";
pub(super) const UNREADABLE_KEY: &str = "Fluorita no pudo interpretar la ruta de este elemento";
pub(super) const NOTHING_TO_SAVE: &str = "No hay cambios que guardar";

pub(super) fn class_label(class: EditClass) -> &'static str {
    match class {
        EditClass::Lossless => "Sin pérdida",
        EditClass::Raster => "Imagen nueva",
    }
}

pub(super) fn container_change(extension: &str) -> String {
    format!("Se guardará como {}", extension.to_uppercase())
}

pub(super) fn saved(saved: &Saved, remembered: bool) -> String {
    let name = saved
        .written
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    match (saved.trashed_original.is_some(), remembered) {
        (true, _) => format!("Guardado en {name}; el original está en la papelera"),
        (false, true) => format!("Guardado en {name}"),
        (false, false) => format!("Guardado en {name}, pero se abrirá sin capas"),
    }
}

/// What a failed save says. The Trash case is the one the engine cannot word
/// on its own: the result *is* on disk and only the original stayed put, which
/// is a different sentence from a write that failed.
pub(super) fn failure(error: &EngineError) -> String {
    match error {
        EngineError::Trash { .. } => {
            "Se guardó el resultado, pero el original sigue donde estaba".to_owned()
        }
        other => other.user_message(),
    }
}

pub(super) fn rejected(rejected: EditRejected) -> &'static str {
    match rejected {
        EditRejected::NotEditable => NOT_EDITABLE,
        EditRejected::OperationNotAdmitted(_) => "Esta imagen no admite esa operación",
        EditRejected::InvalidGeometry => "Esa forma no es válida",
        EditRejected::OutsideCanvas => "Eso queda fuera de la imagen",
        EditRejected::CanvasTooLarge { .. } => "La imagen resultante sería demasiado grande",
        EditRejected::EmptyCanvas => "No quedaría nada de la imagen",
        EditRejected::TooManyObjects => "Hay demasiados elementos en esta imagen",
        EditRejected::StrokeTooLong => "Ese trazo es demasiado largo",
        EditRejected::TextTooLong => "Ese texto es demasiado largo",
        EditRejected::HistoryFull => "No caben más cambios sin guardar",
        EditRejected::UnknownObject => "Ese elemento ya no está",
    }
}
