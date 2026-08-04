// language-contract: product-copy
//
//! The words the library shows.
//!
//! One module so product copy has a single owner instead of being scattered
//! through the adapter, the worker and the projection. Everything here is text
//! a person reads on screen, which is why it is Spanish under
//! [ADR 0007](../../../docs/decisions/0007-spanish-product-copy.md); the
//! comments, names and doc comments around it are development truth and stay
//! English, and the marker at the head of this file exempts nothing else.
//!
//! What is *not* here: the state tokens QML compares against (`ready`,
//! `scanning`, `image`, `video`), and the diagnostics only a developer reads.
//! Those are protocol, not copy, and translating them would break a binding
//! or a log the next person greps for.

use fluorita_core::{MediaKind, SourceRejected};

/// The header while a walk is running.
pub(super) const SCANNING: &str = "Explorando tus carpetas…";

/// Why a scan produced nothing. Each names the step that failed, because
/// "could not scan" tells the reader nothing they can act on.
pub(super) const NO_SOURCES: &str = "No hay carpetas de medios que explorar";
pub(super) const SCANNER_UNAVAILABLE: &str = "No se pudo iniciar el explorador";
pub(super) const SCAN_TIMED_OUT: &str = "La exploración no terminó a tiempo";
pub(super) const SCAN_FAILED: &str = "No se pudo explorar la biblioteca";
pub(super) const SCAN_NOT_STARTED: &str = "No se pudo iniciar la exploración";

/// The folder chooser: what the desktop is asked for, and what to say when it
/// cannot be asked at all.
pub(super) const CHOOSE_FOLDER: &str = "Añadir una carpeta a tu biblioteca";
pub(super) const CHOOSER_UNAVAILABLE: &str = "El escritorio no ofreció un selector de carpetas";

/// Item actions.
pub(super) const ITEM_GONE: &str = "Ese elemento ya no está en la biblioteca";
pub(super) const TRASH_NOT_STARTED: &str = "No se pudo iniciar el envío a la papelera";
pub(super) const TRASH_FAILED: &str = "No se pudo mover a la papelera";
pub(super) const FILE_MISSING: &str =
    "Este archivo no está donde la biblioteca lo vio por última vez";

/// The kind, as a word a person reads. The token QML compares against is
/// `project::kind_label`; this is only ever shown.
pub(super) const fn kind_noun(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "imagen",
        MediaKind::Video => "vídeo",
        MediaKind::Audio => "audio",
    }
}

/// The unknown buckets. A track with no tags is still the user's music, so it
/// is named rather than hidden.
pub(super) const UNKNOWN_ARTIST: &str = "Sin artista";
pub(super) const UNKNOWN_ALBUM: &str = "Sin álbum";

/// Empty results, told apart: an empty folder is not an empty library.
pub(super) const EMPTY_FOLDER: &str = "No hay nada compatible en esta carpeta";
pub(super) const EMPTY_LIBRARY: &str = "No hay medios en tus carpetas";

/// A scan that hit a bound says so instead of reading like a full inventory.
pub(super) const TRUNCATED: &str = "(exploración incompleta: se alcanzó un límite)";

/// How many catalogued files were not found this pass.
pub(super) fn missing(count: usize) -> String {
    format!("{count} sin encontrar")
}

pub(super) fn images(count: usize) -> String {
    format!("{count} {}", plural(count, "imagen", "imágenes"))
}

pub(super) fn videos(count: usize) -> String {
    format!("{count} {}", plural(count, "vídeo", "vídeos"))
}

pub(super) fn tracks(count: usize) -> String {
    format!("{count} {}", plural(count, "pista", "pistas"))
}

/// Why the domain refused a chosen folder, in words the person who chose it can
/// act on.
pub(super) const fn rejected(rejection: SourceRejected) -> &'static str {
    match rejection {
        SourceRejected::NotAbsolute => {
            "Esa carpeta no tiene una ruta absoluta, así que no se puede mapear"
        }
        SourceRejected::NoKinds => "Esa carpeta no aportaría nada",
        SourceRejected::Overlapping => "Esa carpeta ya está dentro de otra que mapeaste",
        SourceRejected::DuplicateIdentity => "Esa carpeta choca con otra ya mapeada",
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 { one } else { many }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{images, missing, plural, tracks, videos};

    #[test]
    fn counts_agree_with_their_noun() {
        assert_eq!(images(1), "1 imagen");
        assert_eq!(images(0), "0 imágenes");
        assert_eq!(videos(2), "2 vídeos");
        assert_eq!(tracks(1), "1 pista");
        assert_eq!(missing(3), "3 sin encontrar");
    }

    #[test]
    fn only_exactly_one_takes_the_singular() {
        assert_eq!(plural(1, "uno", "varios"), "uno");
        assert_eq!(plural(0, "uno", "varios"), "varios");
        assert_eq!(plural(2, "uno", "varios"), "varios");
    }
}
