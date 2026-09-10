//! Bounded, disposable cache for album art received from a trusted phone,
//! under the user's runtime directory with generated names, never a
//! peer-provided path.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

/// Remove artwork left by a killed previous run. The cache is disposable and
/// rebuilt from the phone's current player state.
pub fn sweep() -> io::Result<()> {
    let root = root_dir()?;
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    Ok(())
}

/// Delete one generated cache file, but only when it still belongs to our
/// runtime cache root.
pub fn discard(path: &Path) {
    if root_dir().ok().is_some_and(|root| path.starts_with(root)) {
        let _ = fs::remove_file(path);
    }
}

/// Delete every cover for a disconnected device.
pub fn clear_device(device_id: &str) {
    if let Ok(directory) = device_dir(device_id) {
        let _ = fs::remove_dir_all(directory);
    }
}

fn root_dir() -> io::Result<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|path| path.join("magnetita").join("artwork"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "XDG_RUNTIME_DIR is unavailable for the album-art cache",
            )
        })
}

fn device_dir(device_id: &str) -> io::Result<PathBuf> {
    Ok(root_dir()?.join(format!("{:016x}", hash(device_id))))
}

fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
