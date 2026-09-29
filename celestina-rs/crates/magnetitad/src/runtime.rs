//! Small process-level helpers shared by the daemon, and its runtime
//! directory.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::xdg::{self, PrivateDirError};

/// `$XDG_RUNTIME_DIR/magnetita`, where the phone's mounts, the mirror's FIFOs
/// and the album-art cache live. Only a runtime directory that is what the
/// spec requires counts, and nothing stands in for a missing one: a
/// predictable name under the world-writable `/tmp` is one another local
/// account can claim first. A caller without it degrades the feature that
/// needed it.
pub(crate) fn runtime_base() -> Result<PathBuf, PrivateDirError> {
    base_under(xdg::runtime_dir())
}

/// [`runtime_base`], created `0700` or checked to be this user's own, for a
/// caller about to put something in it.
pub(crate) fn private_runtime_base() -> Result<PathBuf, PrivateDirError> {
    private_base_under(xdg::runtime_dir())
}

/// The daemon's directory under `runtime`, the answer of the runtime
/// directory lookup; taking the answer rather than the environment keeps the
/// rule testable without changing the process's environment.
fn base_under(runtime: Result<PathBuf, PrivateDirError>) -> Result<PathBuf, PrivateDirError> {
    Ok(runtime?.join("magnetita"))
}

/// [`base_under`], created `0700` or checked.
pub(crate) fn private_base_under(
    runtime: Result<PathBuf, PrivateDirError>,
) -> Result<PathBuf, PrivateDirError> {
    let base = base_under(runtime)?;
    xdg::ensure_private_dir(&base)?;
    Ok(base)
}

pub(crate) fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

pub(crate) fn log(tag: &str, message: &str) {
    println!("[{tag}] {message}");
    let _ = std::io::stdout().flush();
}
