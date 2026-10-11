// language-contract: product-copy
//
//! The words a person reads when a trim is refused or fails.
//!
//! The marker at the head exempts the string literals here and nothing else,
//! under [ADR 0007](../../../../../docs/decisions/0007-spanish-product-copy.md):
//! the names and comments around them stay English.

use super::TrimError;

impl TrimError {
    /// What the edit window says, in Spanish. Never a path or an argument
    /// list: the detail of a failure is for the log.
    #[must_use]
    pub fn message_es(&self) -> String {
        match self {
            Self::Reversed => "El inicio tiene que quedar antes del final".to_owned(),
            Self::PastTheEnd => "El final queda fuera del vídeo".to_owned(),
            Self::TooShort => "El fragmento tiene que durar al menos un fotograma".to_owned(),
            Self::Unchanged => "No hay cambios que guardar".to_owned(),
            Self::NotAVideo => "Este archivo no es un vídeo".to_owned(),
            Self::ToolMissing => {
                "Falta «ffmpeg» (paquete ffmpeg): instálalo para recortar vídeos".to_owned()
            }
            Self::EncoderMissing => {
                "«ffmpeg» no tiene ningún codificador H.264 que se pueda usar".to_owned()
            }
            Self::Failed { .. } => {
                "No se pudo recortar el vídeo; el original sigue como estaba".to_owned()
            }
            Self::NoFrames => {
                "El fragmento elegido no tiene ningún fotograma; no se ha guardado nada".to_owned()
            }
            Self::Cancelled => "Recorte cancelado; el original sigue como estaba".to_owned(),
            Self::Unstarted { .. } => "No se pudo empezar a recortar el vídeo".to_owned(),
        }
    }
}
