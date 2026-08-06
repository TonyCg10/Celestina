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

/// A still whose path could not be expressed as a URL for the toolkit.
pub(crate) const UNRESOLVED_IMAGE: &str = "No se pudo resolver la ruta de la imagen";

/// The render surface exists but could not be prepared, so no picture will
/// ever arrive. Said out loud rather than left as an "opening" that never ends.
pub(crate) const SURFACE_UNAVAILABLE: &str = "No se pudo preparar la superficie de vídeo";
