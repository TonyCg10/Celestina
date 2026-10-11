// language-contract: product-copy
//
//! The words a trim's outcome is told in.
//!
//! The refusals and the child's failures are worded once, by
//! `fluorita_core::trim::TrimError::message_es`; a landing that failed is
//! worded by the picture editor's own sentences, because it is the same
//! landing. What is left is the success, and what a trim will leave out. The marker at the head exempts the
//! string literals here and nothing else, under
//! [ADR 0007](../../../docs/decisions/0007-spanish-product-copy.md).

use fluorita_engine::Landed;

use super::run::Failure;

/// What a landed trim says.
pub(super) fn saved(landed: &Landed) -> String {
    let name = fluorita_core::displayed_name(&landed.written);
    if landed.trashed_original.is_some() {
        format!("Guardado en {name}; el original está en la papelera")
    } else {
        format!("Guardado en {name}")
    }
}

/// What a film's trim will leave behind, said before it is saved: the
/// container, by the picture editor's own sentence, and the streams beyond
/// the main video and one audio. Empty when it keeps everything.
pub(super) fn notice(container_changes: bool, left_out: usize) -> String {
    let mut parts = Vec::new();
    if container_changes {
        parts.push(crate::editor::copy::container_change("mp4"));
    }
    match left_out {
        0 => {}
        1 => parts.push("Se quita 1 pista: solo se guardan el vídeo y un audio".to_owned()),
        many => parts.push(format!(
            "Se quitan {many} pistas: solo se guardan el vídeo y un audio"
        )),
    }
    parts.join(" · ")
}

/// What a trim that did not land says.
pub(super) fn failure(failure: &Failure) -> String {
    match failure {
        Failure::Trim(error) => error.message_es(),
        Failure::Land(error) => crate::editor::copy::failure(error),
    }
}

/// The developer's account of a failure, for the log: never shown.
pub(super) fn detail(failure: &Failure) -> String {
    match failure {
        Failure::Trim(error) => error.to_string(),
        Failure::Land(error) => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::notice;

    #[test]
    fn a_trim_that_keeps_everything_says_nothing_and_a_loss_is_said() {
        assert_eq!(notice(false, 0), "");
        assert!(notice(true, 0).contains("MP4"));
        assert!(notice(false, 1).contains("1 pista"));
        assert!(notice(false, 3).contains("3 pistas"));
        let both = notice(true, 2);
        assert!(both.contains("MP4") && both.contains("2 pistas"), "{both}");
    }
}
