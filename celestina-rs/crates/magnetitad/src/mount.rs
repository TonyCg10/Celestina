//! Where a phone's storage appears: `$XDG_RUNTIME_DIR/magnetita/<device-id>/`,
//! the path Siderita browses. The mount itself is the link session's
//! (`link_wire::storage`), served over the own wire; this module owns the
//! path rule and the sweep of anything a killed daemon left mounted.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use celestina_core::xdg::PrivateDirError;

use crate::runtime::{private_runtime_base, runtime_base};
use crate::subprocess;

/// How long releasing a mountpoint may take: the sweep must never be the
/// thing that keeps the daemon from starting.
const UNMOUNT_TIMEOUT: Duration = Duration::from_secs(5);

/// The mount path for a device: `$XDG_RUNTIME_DIR/magnetita/<device-id>/`,
/// its base created `0700`. Without a runtime directory there is no mount;
/// see [`crate::runtime::runtime_base`].
pub fn mountpoint_for(device_id: &str) -> io::Result<PathBuf> {
    check_component(device_id)?;
    mountpoint_in(private_runtime_base(), device_id)
}

/// The mount path for a checked `device_id` under `base`, the daemon's
/// runtime directory or why there is none.
fn mountpoint_in(base: Result<PathBuf, PrivateDirError>, device_id: &str) -> io::Result<PathBuf> {
    Ok(base.map_err(io::Error::other)?.join(device_id))
}

/// The id is the session's (the pinned certificate's), yet it becomes a
/// single path component, so anything that could walk out of the base
/// directory (separators, `..`, `.`, emptiness, a NUL) is refused.
fn check_component(device_id: &str) -> io::Result<()> {
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
    Ok(())
}

/// Unmount anything left under the runtime dir by a previous run. A graceful
/// disconnect unmounts as the session drops, but a *killed* daemon cannot run
/// destructors, so the sweep at startup gives a clean slate.
pub fn clear_stale() {
    let Ok(base) = runtime_base() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(base) else {
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
    use super::{check_component, mountpoint_for, mountpoint_in, PrivateDirError};
    use crate::runtime::private_base_under;

    #[test]
    fn a_mount_path_never_falls_back_to_tmp() {
        use std::os::unix::fs::PermissionsExt;
        let unset = mountpoint_in(private_base_under(Err(PrivateDirError::Unset)), "abc123");
        assert!(unset.is_err(), "no runtime dir, no mount: {unset:?}");

        let runtime = std::env::temp_dir().join(format!("mag-run-private-{}", std::process::id()));
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        let owned = mountpoint_in(private_base_under(Ok(runtime.clone())), "abc123").unwrap();
        assert_eq!(owned, runtime.join("magnetita").join("abc123"));
        let base = std::fs::metadata(runtime.join("magnetita")).unwrap();
        assert_eq!(base.permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(&runtime);
    }

    #[test]
    fn a_device_id_is_one_path_component_or_nothing() {
        assert!(check_component("abc123").is_ok());
        for bad in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert!(check_component(bad).is_err(), "{bad:?}");
            assert!(mountpoint_for(bad).is_err(), "{bad:?}");
        }
    }
}
