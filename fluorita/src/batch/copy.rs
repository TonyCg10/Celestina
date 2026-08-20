// language-contract: product-copy
//
//! The words a batch shows.
//!
//! One module so product copy has a single owner, as `library/copy.rs`,
//! `editor/copy.rs` and `metadata/copy.rs` already do.

use fluorita_core::BatchProgress;

/// The word each copy's name is marked with.
pub(super) const COPY_MARKER: &str = "editado";

pub(super) const NOTHING_CHOSEN: &str = "No hay nada seleccionado";

/// What a finished run amounted to.
///
/// The skipped count is said out loud whenever there is one: a run over una
/// carpeta mixta that reports only its successes reads as if it had acted on
/// everything.
pub(super) fn finished(progress: BatchProgress) -> String {
    let mut parts = Vec::new();
    if progress.done > 0 {
        parts.push(format!("{} cambiados", progress.done));
    }
    if progress.skipped > 0 {
        parts.push(format!("{} sin cambios posibles", progress.skipped));
    }
    if progress.failed > 0 {
        parts.push(format!("{} con error", progress.failed));
    }
    let tally = if parts.is_empty() {
        "sin cambios".to_owned()
    } else {
        parts.join(", ")
    };
    if progress.cancelled {
        format!("Detenido: {tally}")
    } else {
        format!("Hecho: {tally}")
    }
}
