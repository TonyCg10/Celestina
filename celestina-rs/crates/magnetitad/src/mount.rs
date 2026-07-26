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

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use magnetita_core::SftpMount;

/// A live sshfs mount of one phone. Unmounts when dropped.
pub struct Mount {
    mountpoint: PathBuf,
}

impl Mount {
    /// Mount the phone's sftp root at its per-device runtime path. `host` is the
    /// phone's address (from the link). Blocks until sshfs has established the
    /// mount (it then backgrounds itself).
    pub fn open(device_id: &str, host: &str, sftp: &SftpMount) -> io::Result<Mount> {
        let mountpoint = mountpoint_for(device_id);
        std::fs::create_dir_all(&mountpoint)?;
        // Clear any stale mount left by a previous crash before remounting.
        let _ = unmount(&mountpoint);

        let remote = format!("{}@{}:{}", sftp.user, host, sftp.path);
        let mut child = Command::new("sshfs")
            .arg(&remote)
            .arg(&mountpoint)
            .args(["-p", &sftp.port.to_string()])
            .args(SSHFS_OPTIONS)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        // sshfs reads the one-session password from stdin (-o password_stdin).
        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{}", sftp.password)?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let reason = String::from_utf8_lossy(&output.stderr);
            let reason = reason.trim();
            let reason = if reason.is_empty() { "sshfs failed" } else { reason };
            return Err(io::Error::other(reason.to_owned()));
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
pub fn mountpoint_for(device_id: &str) -> PathBuf {
    base_dir().join(device_id)
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
    Command::new("fusermount3")
        .arg("-u")
        .arg(mountpoint)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

/// sshfs options: the phone's one-session password over stdin, no host-key
/// checks (the server is ephemeral and already trusted over TLS), auto-reconnect
/// with liveness probes, and acceptance of the ssh-rsa host key some phones
/// still present (added, not forced).
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
    "reconnect",
    "-o",
    "ServerAliveInterval=15",
    "-o",
    "ServerAliveCountMax=3",
];
