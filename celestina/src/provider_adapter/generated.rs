//! The two files this shell offers the session, and never installs itself.
//!
//! Niri's colours and the portal backend registration both live in files the
//! shell can *write* but must not *apply*: one belongs to the author's
//! compositor configuration and the other to the session's portal setup, and
//! editing either would be this shell changing something it does not own.
//!
//! So both are written under the shell's own state directory, with a comment
//! saying what to do with them. Referencing them is the author's step, and
//! removing that reference is the whole rollback.

use std::path::{Path, PathBuf};

use celestina_shell_core::niri_colours;

use super::portal_settings;

/// Where the generated files go: the shell's own data directory, never the
/// portal's and never Niri's.
fn directory() -> Option<PathBuf> {
    celestina_core::xdg::data_home().map(|home| home.join("celestina/generated"))
}

/// Writes both files, replacing them atomically so a half-written include can
/// never be what Niri reads at session start.
///
/// Failures are reported and otherwise ignored: a shell that could not write a
/// file the author has not referenced yet is still a working shell.
pub fn write_all() {
    let Some(directory) = directory() else {
        return;
    };

    if let Some(colours) = niri_colours::include_text() {
        write_one(&directory.join("niri-colours.kdl"), colours.as_bytes());
    }
    write_one(
        &directory.join(portal_settings::PORTAL_FILE_NAME),
        portal_settings::portal_file_text().as_bytes(),
    );
}

fn write_one(path: &Path, bytes: &[u8]) {
    if let Err(error) = celestina_core::atomic_file::replace(path, bytes) {
        eprintln!(
            "celestina-provider-adapter: could not write {}: {error}",
            path.display()
        );
    }
}
