//! Small process-level helpers shared by the daemon.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

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
