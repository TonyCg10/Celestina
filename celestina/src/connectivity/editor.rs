//! The editing UIs the connectivity pages hand over to (D11): the `Editar…`
//! rows start `nm-connection-editor` or `blueman-manager`, fixed programs
//! the shell names itself ([`Editor::program`]), looked up only in the
//! absolute directories of `PATH` and started with no arguments and no
//! environment from data, detached through the launcher's spawn path
//! ([`crate::launcher::spawn::spawn_program`]). A program that is not found
//! is remembered in the state, and the row says so; nothing else happens.
//!
//! The lookup reads at most [`MAX_PATH_DIRS`] directory entries' metadata on
//! the loop's thread, once per deliberate activation of an `Editar…` row.

use std::{
    ffi::OsStr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use celestina_shell::connectivity::Editor;

use super::state::ConnectivityState;
use crate::{error_chain, launcher::spawn::SpawnError};

/// The most `PATH` entries a lookup reads; the rest are ignored.
pub const MAX_PATH_DIRS: usize = 64;

/// Why an editor did not start.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    /// The program is in no absolute directory of `PATH`.
    #[error("{0} is not on PATH")]
    Missing(&'static str),
    /// It was found but could not be started.
    #[error(transparent)]
    Start(#[from] SpawnError),
}

/// The executable file `program` in the first absolute directory of `path`
/// (a `PATH` value) that holds one. Relative and empty entries, which would
/// name the shell's working directory, are skipped.
#[must_use]
pub fn find(program: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .take(MAX_PATH_DIRS)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Starts `editor` from the `PATH` value `path` through `start` (the
/// launcher's spawn path in production).
///
/// # Errors
///
/// [`EditorError`] naming why it did not start.
pub fn open_with(
    editor: Editor,
    path: Option<&OsStr>,
    start: impl FnOnce(&Path, &str) -> Result<(), SpawnError>,
) -> Result<(), EditorError> {
    let program = editor.program();
    let found = find(program, path).ok_or(EditorError::Missing(program))?;
    start(&found, program)?;
    Ok(())
}

/// The `Editar…` row was chosen: starts `editor` and records in `state`
/// whether its program is missing. Returns whether `state` changed (the
/// row's note appears or goes).
pub fn launch(state: &mut ConnectivityState, editor: Editor) -> bool {
    let path = std::env::var_os("PATH");
    let opened = open_with(
        editor,
        path.as_deref(),
        crate::launcher::spawn::spawn_program,
    );
    record(state, editor, opened)
}

fn record(state: &mut ConnectivityState, editor: Editor, opened: Result<(), EditorError>) -> bool {
    match opened {
        Ok(()) => {
            tracing::info!(program = editor.program(), "connectivity: editor started");
            state.set_editor_missing(editor, false)
        }
        Err(EditorError::Missing(program)) => {
            tracing::info!(program, "connectivity: the editor is not installed");
            state.set_editor_missing(editor, true)
        }
        Err(err @ EditorError::Start(_)) => {
            tracing::warn!("connectivity: {}", error_chain(&err));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ffi::OsString;
    use std::fs;

    use super::*;

    /// A directory of the test's own, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("celestina-editor-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch dir");
            Self(dir)
        }

        fn program(&self, name: &str, mode: u32) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, "#!/bin/sh\n").expect("program");
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("mode");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn joined(dirs: &[&Path]) -> OsString {
        std::env::join_paths(dirs).expect("PATH")
    }

    #[test]
    fn only_an_executable_file_in_an_absolute_path_directory_is_found() {
        let empty = Scratch::new("empty");
        let plain = Scratch::new("plain");
        let bin = Scratch::new("bin");
        plain.program("nm-connection-editor", 0o644);
        let want = bin.program("nm-connection-editor", 0o755);
        fs::create_dir_all(empty.0.join("blueman-manager")).expect("a directory");

        let path = joined(&[&empty.0, &plain.0, &bin.0]);
        assert_eq!(find("nm-connection-editor", Some(&path)), Some(want));
        // A directory named like the program is not the program.
        assert_eq!(find("blueman-manager", Some(&path)), None);
        // No PATH, and relative or empty entries (the working directory).
        assert_eq!(find("nm-connection-editor", None), None);
        let relative = OsString::from(":.:bin");
        assert_eq!(find("nm-connection-editor", Some(&relative)), None);
    }

    #[test]
    fn the_fixed_program_starts_with_no_arguments_or_is_noted_missing() {
        let bin = Scratch::new("start");
        let program = bin.program("blueman-manager", 0o755);
        let path = joined(&[&bin.0]);
        let started = RefCell::new(Vec::new());
        let opened = open_with(Editor::Bluetooth, Some(&path), |found, name| {
            started
                .borrow_mut()
                .push((found.to_owned(), name.to_owned()));
            Ok(())
        });
        assert!(opened.is_ok());
        assert_eq!(
            started.into_inner(),
            [(program, "blueman-manager".to_owned())]
        );

        let mut state = ConnectivityState::default();
        let missing = open_with(Editor::Network, Some(&path), |_, _| {
            panic!("a missing program is never started")
        });
        assert!(matches!(
            missing,
            Err(EditorError::Missing("nm-connection-editor"))
        ));
        assert!(record(&mut state, Editor::Network, missing));
        assert!(state.editor_missing(Editor::Network));
        // Found later (installed meanwhile): the note goes.
        assert!(record(&mut state, Editor::Network, Ok(())));
        assert!(!state.editor_missing(Editor::Network));
        // A start failure is not a missing program.
        let failed = Err(EditorError::Start(SpawnError::NoProgram {
            name: "x".to_owned(),
        }));
        assert!(!record(&mut state, Editor::Bluetooth, failed));
        assert!(!state.editor_missing(Editor::Bluetooth));
    }
}
