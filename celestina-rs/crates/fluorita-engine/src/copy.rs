// language-contract: product-copy
//
//! Engine failures a person reads, when the failure itself decides the words.
//!
//! Most of [`crate::error::EngineError::user_message`] is one fixed sentence
//! per variant. A replacement whose result could not take its name is
//! different: the words must say where the result went, and every surface
//! that shows the error — the editor, the metadata panel — must say the same
//! thing. The marker at the head exempts the string literals here and nothing
//! else, under
//! [ADR 0007](../../../../docs/decisions/0007-spanish-product-copy.md).

use std::path::Path;

/// The original is in the Trash and the result is kept, under `kept`, beside
/// the name it could not take.
pub(crate) fn replacement_kept(kept: &Path) -> String {
    let name = kept
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!(
        "El original está en la Papelera y el resultado se guardó junto a él con el nombre oculto «{name}»"
    )
}

#[cfg(test)]
mod tests {
    use super::replacement_kept;
    use std::path::Path;

    #[test]
    fn the_message_names_the_kept_file() {
        let message = replacement_kept(Path::new("/m/.foto.jpg.fluorita-result-7-0"));
        assert!(message.contains(".foto.jpg.fluorita-result-7-0"));
        assert!(!message.contains("/m/"), "a surface is not handed a path");
    }
}
