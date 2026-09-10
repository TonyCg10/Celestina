//! Where a phone's storage appears: `$XDG_RUNTIME_DIR/magnetita/<device-id>/`,
//! the path Siderita browses. The mount itself is the link session's
//! (`link_wire::storage`), served over the own wire; this module owns the
//! path rule and the sweep of anything a killed daemon left mounted.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::subprocess;

/// How long releasing a mountpoint may take: the sweep must never be the
/// thing that keeps the daemon from starting.
const UNMOUNT_TIMEOUT: Duration = Duration::from_secs(5);

/// The directory holding every device's mount: `$XDG_RUNTIME_DIR/magnetita/`.
fn base_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("magnetita")
}

/// The mount path for a device: `$XDG_RUNTIME_DIR/magnetita/<device-id>/`.
/// The id arrives off the network and becomes a single path component, so
/// anything that could walk out of the base directory (separators, `..`,
/// `.`, emptiness, a NUL) is refused.
pub fn mountpoint_for(device_id: &str) -> io::Result<PathBuf> {
    if device_id.is_empty()
        || device_id == "."
        || device_id == ".."
        || device_id.contains(['/', '\\', '\0'])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing device id {device_id:?} as a mount path component"),
        ));
    }
    Ok(base_dir().join(device_id))
}

/// Unmount anything left under the runtime dir by a previous run. A graceful
/// disconnect unmounts as the session drops, but a *killed* daemon cannot run
/// destructors, so the sweep at startup gives a clean slate.
pub fn clear_stale() {
    let Ok(entries) = std::fs::read_dir(base_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let _ = unmount(&entry.path());
    }
}

fn unmount(mountpoint: &Path) -> io::Result<()> {
    let stopping = AtomicBool::new(false);
    let path = mountpoint.to_string_lossy().into_owned();
    let (mut child, group) =
        subprocess::spawn_grouped("fusermount3", &["-u", path.as_str()], Stdio::null())?;
    subprocess::wait_bounded(
        &mut child,
        group,
        Instant::now() + UNMOUNT_TIMEOUT,
        &stopping,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "fusermount3 did not finish in time",
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::mountpoint_for;

    #[test]
    fn a_device_id_is_one_path_component_or_nothing() {
        assert!(mountpoint_for("abc123")
            .unwrap()
            .ends_with("magnetita/abc123"));
        for bad in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert!(mountpoint_for(bad).is_err(), "{bad:?}");
        }
    }
}
