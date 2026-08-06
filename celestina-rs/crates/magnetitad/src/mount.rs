//! Mounting the phone's storage over sshfs.
//!
//! The phone's sftp plugin hands us a port, a one-session user/password and a
//! root path ([`SftpMount`]); this turns that into a real FUSE mount at
//! `$XDG_RUNTIME_DIR/magnetita/<device-id>/` so browsing the phone is plain
//! filesystem navigation. The credentials are ephemeral and the server is the
//! phone we already trust over TLS, so host-key checking is off by design — the
//! trust was established on the KDE Connect link, not on ssh's known-hosts.
//!
//! The mount is tied to the link: hold the [`Mount`] while connected and drop it
//! on disconnect — [`Drop`] unmounts, so a lost link never strands a dead mount.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use magnetita_core::SftpMount;

use crate::subprocess;

/// How long the whole mount attempt may take. [`Mount::open`] runs on the
/// thread pumping the phone link, and a phone that answers SFTP with an
/// address that accepts the TCP connection and then says nothing would
/// otherwise stall that link for as long as it likes.
const MOUNT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long releasing a mountpoint may take. Unmount runs from `Drop`, so it
/// must never be the thing that keeps a closing link alive.
const UNMOUNT_TIMEOUT: Duration = Duration::from_secs(5);

/// A live sshfs mount of one phone. Unmounts when dropped.
pub struct Mount {
    mountpoint: PathBuf,
}

impl Mount {
    /// Mount the phone's sftp root at its per-device runtime path. `host` is the
    /// phone's address (from the link). Blocks until sshfs has established the
    /// mount (it then backgrounds itself).
    pub fn open(device_id: &str, host: &str, sftp: &SftpMount) -> io::Result<Mount> {
        let mountpoint = mountpoint_for(device_id)?;
        std::fs::create_dir_all(&mountpoint)?;
        // Clear any stale mount left by a previous crash before remounting.
        let _ = unmount(&mountpoint);

        // Every component of this argument was validated at the decode
        // boundary, which is what keeps it a path rather than an sshfs option.
        let remote = format!("{}@{}:{}", sftp.user, host, sftp.path);
        let port = sftp.port.to_string();
        let mountpoint_arg = mountpoint.to_string_lossy().into_owned();
        let mut args = vec![
            remote.as_str(),
            mountpoint_arg.as_str(),
            "-p",
            port.as_str(),
        ];
        args.extend_from_slice(SSHFS_OPTIONS);

        // sshfs reads the one-session password from stdin (-o password_stdin).
        let password = format!("{}\n", sftp.password);
        let stopping = AtomicBool::new(false);
        let outcome = subprocess::run_with_input(
            "sshfs",
            &args,
            password.as_bytes(),
            Instant::now() + MOUNT_TIMEOUT,
            &stopping,
        );
        if !outcome.succeeded {
            return Err(io::Error::other(
                outcome.reason("sshfs did not establish the mount within its budget"),
            ));
        }
        Ok(Mount { mountpoint })
    }

    /// Where the phone is mounted.
    pub fn path(&self) -> &Path {
        &self.mountpoint
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        let _ = unmount(&self.mountpoint);
    }
}

/// The directory holding every device's mount: `$XDG_RUNTIME_DIR/magnetita/`.
fn base_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("magnetita")
}

/// The mount path for a device: `$XDG_RUNTIME_DIR/magnetita/<device-id>/`.
/// The id arrives off the network and becomes a single path component, so
/// anything that could walk out of the base directory — separators, `..`,
/// `.`, emptiness, a NUL — is refused. A legitimate KDE Connect id is 32 hex
/// chars; odd-but-safe ids still get a mountpoint.
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

/// Unmount anything left under our runtime dir by a previous run. A graceful
/// disconnect unmounts as the [`Mount`] drops, but a *killed* daemon cannot run
/// destructors and the backgrounded sshfs outlives it — so we sweep at startup
/// for a clean slate before mounting anew.
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

/// sshfs options: the phone's one-session password over stdin, no host-key
/// checks (the server is ephemeral and already trusted over TLS), a bounded
/// connect so an address that never answers cannot hold the attempt open,
/// auto-reconnect with liveness probes, and acceptance of the ssh-rsa host key
/// some phones still present (added, not forced).
const SSHFS_OPTIONS: &[&str] = &[
    "-o",
    "password_stdin",
    "-o",
    "StrictHostKeyChecking=no",
    "-o",
    "UserKnownHostsFile=/dev/null",
    "-o",
    "PreferredAuthentications=password",
    "-o",
    "PubkeyAuthentication=no",
    "-o",
    "HostKeyAlgorithms=+ssh-rsa",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "reconnect",
    "-o",
    "ServerAliveInterval=15",
    "-o",
    "ServerAliveCountMax=3",
];

#[cfg(test)]
mod tests {
    use super::mountpoint_for;

    #[test]
    fn a_device_id_is_a_single_path_component_or_nothing() {
        for bad in ["", ".", "..", "../..", "a/b", "a\\b", "x\0y"] {
            assert!(mountpoint_for(bad).is_err(), "{bad:?} must be refused");
        }
        let ok = mountpoint_for("689da02afffe4b1282577c0a2f0ed5e3").unwrap();
        assert!(ok.ends_with("689da02afffe4b1282577c0a2f0ed5e3"));
    }
}
