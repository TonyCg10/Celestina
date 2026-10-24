//! A `dbus-daemon` of a test's own, so no test ever touches the session bus
//! (where the author's Noctalia holds the watcher name). Shared by the tray's
//! watcher, item and menu tests; `bus.rs` keeps an earlier private copy of the
//! same recipe inside its own test module.
//!
//! The daemon runs on a configuration written here, not the session one: it
//! lists no service directory, so a test that calls an unowned activatable
//! name gets `ServiceUnknown` instead of a real desktop service started on
//! the private bus with the session's environment.

use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use zbus::blocking::connection::Builder;

/// The daemon's configuration: a session-type bus listening at `LISTEN`
/// (the test's socket), with no service directory (nothing can be
/// activated) and the session's default policy.
const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>LISTEN</listen>
  <keep_umask/>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A private session bus, removed when dropped.
pub(crate) struct PrivateBus {
    child: Child,
    dir: PathBuf,
    address: String,
}

impl PrivateBus {
    /// `None` (with a notice) where `dbus-daemon` is not installed.
    pub(crate) fn start(name: &str) -> Option<PrivateBus> {
        let dir =
            std::env::temp_dir().join(format!("celestina-tray-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let config = dir.join("bus.conf");
        let listen = format!("unix:path={}", dir.join("bus").display());
        std::fs::write(&config, CONFIG.replace("LISTEN", &listen)).ok()?;
        let spawned = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .arg("--print-address=1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else {
            eprintln!("dbus-daemon is not installed: the tray bus test is skipped");
            return None;
        };
        let mut address = String::new();
        let read = child
            .stdout
            .take()
            .map(|out| BufReader::new(out).read_line(&mut address));
        if !matches!(read, Some(Ok(n)) if n > 0) {
            let _ = child.kill();
            let _ = child.wait();
            eprintln!("dbus-daemon printed no address: the tray bus test is skipped");
            return None;
        }
        Some(PrivateBus {
            child,
            dir,
            address: address.trim().to_owned(),
        })
    }

    /// This bus's address, for a client that builds its own connection.
    pub(crate) fn address(&self) -> &str {
        &self.address
    }

    /// Kills the daemon, as a crashed bus would end: every connection to it
    /// sees its socket close.
    pub(crate) fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// A connection builder for this bus.
    pub(crate) fn builder(&self) -> Builder<'static> {
        match Builder::address(self.address.as_str()) {
            Ok(b) => b,
            Err(err) => panic!("bad bus address {}: {err}", self.address),
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
