//! Wayland clipboard process adaptation for the daemon.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::thread;

/// Watch desktop clipboard changes without tying this adapter to daemon state.
/// `wl-paste --watch` prints each value followed by NUL so multiline text stays
/// one event. Missing tools are a best-effort degradation.
pub(crate) fn spawn_watch(on_change: impl Fn(String) + Send + 'static) {
    thread::spawn(move || {
        let mut child = match Command::new("wl-paste")
            .args(["--watch", "sh", "-c", "cat; printf '\\0'"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                eprintln!("magnetitad: clipboard watch unavailable: {error}");
                return;
            }
        };
        let Some(stdout) = child.stdout.take() else {
            return;
        };
        let mut reader = std::io::BufReader::new(stdout);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(0, &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    if buffer.last() == Some(&0) {
                        buffer.pop();
                    }
                    on_change(String::from_utf8_lossy(&buffer).into_owned());
                }
                Err(_) => break,
            }
        }
    });
}

/// Read current desktop clipboard text, or empty when unavailable.
pub(crate) fn read() -> String {
    Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// Put text on the desktop and reap wl-copy's foreground process.
pub(crate) fn write(text: &str) -> bool {
    let Ok(mut child) = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            let _ = child.wait();
            return false;
        }
    }
    child.wait().is_ok()
}
