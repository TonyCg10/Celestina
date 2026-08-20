// language-contract: product-copy
//
//! The words the player shows.
//!
//! The library already gives its own product copy one owner in
//! `library/copy.rs`; this is the same rule for the other adapter, which had
//! its sentences spelled inline among the lifecycle code. Everything here is
//! text a person reads on screen, which is why it is Spanish under
//! [ADR 0007](../../docs/decisions/0007-spanish-product-copy.md); the comments
//! and names around it are development truth and stay English, and the marker
//! at the head of this file exempts nothing else.
//!
//! What is *not* here: the confirmed-state words QML binds and compares against
//! (`inactivo`, `reproduciendo`, …). Those are shown *and* matched on, so they
//! stay beside the state they translate, and the refusal messages a still image
//! produces stay with the budget that decides them.

/// A file whose name says nothing this player can open. Refusing by name is
/// what keeps browsing from starting a decoder for a text file.
pub(crate) const UNKNOWN_KIND: &str = "Fluorita no reconoce este tipo de archivo";

/// A value handed to `open` that is not a path key, so it names no file. Said
/// out loud rather than opened as whatever its characters happen to spell.
pub(crate) const UNREADABLE_KEY: &str = "Fluorita no pudo interpretar la ruta de este elemento";

/// The word a kept frame's name is marked with. The domain owns the collision
/// search; this owns the wording.
pub(crate) const FRAME_MARKER: &str = "fotograma";

/// Asked to keep a frame of something that has no picture.
pub(crate) const NO_FRAME: &str = "Esto no tiene imagen de la que guardar un fotograma";

/// What a kept frame says. The name is what a person then looks for in the
/// folder, so it is what the sentence carries.
pub(crate) fn frame_kept(kept: &fluorita_engine::FrameExtracted) -> String {
    let name = kept
        .written
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("Fotograma guardado en {name}")
}

/// A still whose path could not be expressed as a URL for the toolkit.
pub(crate) const UNRESOLVED_IMAGE: &str = "No se pudo resolver la ruta de la imagen";

/// The render surface exists but could not be prepared, so no picture will
/// ever arrive. Said out loud rather than left as an "opening" that never ends.
pub(crate) const SURFACE_UNAVAILABLE: &str = "No se pudo preparar la superficie de vídeo";

/// What a stream with no title and no language is called: its position, so a
/// menu row is still something a person can point at.
pub fn stream_position(index: usize) -> String {
    format!("Pista {}", index + 1)
}

/// Where a pacing report could not be written, because this desktop gave no
/// cache directory to put it in.
pub const NO_REPORT_DIRECTORY: &str = "No hay dónde escribir el informe";

/// The report could not be written.
pub const REPORT_NOT_WRITTEN: &str = "No se pudo escribir el informe";

/// What the recording says so far, in one line.
///
/// The verdict first, because it is the answer; the numbers after it, because
/// they are the reason. A recording with nothing in it says so rather than
/// showing four zeros that would read as a perfect picture.
pub fn pacing_line(
    summary: &fluorita_core::PacingSummary,
    verdict: fluorita_core::Verdict,
) -> String {
    let word = match verdict {
        fluorita_core::Verdict::TooEarly => "Midiendo…",
        fluorita_core::Verdict::Smooth => "Estable",
        fluorita_core::Verdict::Delayed => "Fotogramas tardíos",
        fluorita_core::Verdict::Dropping => "Perdiendo fotogramas",
    };
    if summary.samples < 2 {
        return word.to_owned();
    }
    let display = summary
        .display_fps
        .map_or_else(|| "?".to_owned(), |value| format!("{value:.1}"));
    format!(
        "{word} · {:.0} perdidos/min · {:.0} tardíos/min · pantalla {display} Hz · {:.0} s",
        summary.dropped_per_minute,
        summary.delayed_per_minute,
        summary.span.as_secs_f64()
    )
}
