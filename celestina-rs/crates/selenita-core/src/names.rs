//! Where a capture lands and what it is called.
//!
//! The name is `<stem> YYYY-MM-DD HH.MM.SS.<ext>` in local time. The stem is
//! product copy («Captura» for a screenshot) and comes from the Qt seam, so
//! this crate holds no Spanish. A name already taken in the folder gets ` (2)`,
//! ` (3)`… before the extension ([`numbered`]); the caller publishes without
//! replacing and walks that sequence, so two captures in the same second
//! never overwrite each other.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use celestina_core::xdg;
use chrono::{DateTime, Local, NaiveDateTime};

/// `<stem> 2026-10-09 14.32.05.<ext>`, `now` in the local time zone.
#[must_use]
pub fn capture_file_name(stem: &str, now: SystemTime, extension: &str) -> String {
    let local: DateTime<Local> = now.into();
    file_name_at(stem, local.naive_local(), extension)
}

/// The rule behind [`capture_file_name`], on a wall-clock time already in
/// the zone it is written in.
#[must_use]
pub fn file_name_at(stem: &str, wall_clock: NaiveDateTime, extension: &str) -> String {
    format!(
        "{stem} {}.{extension}",
        wall_clock.format("%Y-%m-%d %H.%M.%S")
    )
}

/// `name` with ` (n)` before its extension; `n` of 1 or less is `name`
/// itself.
#[must_use]
pub fn numbered(name: &str, n: u32) -> String {
    if n <= 1 {
        return name.to_owned();
    }
    match name.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() => format!("{base} ({n}).{extension}"),
        _ => format!("{name} ({n})"),
    }
}

/// The person's pictures folder: `XDG_PICTURES_DIR` from the environment when
/// set (how the tests and the smoke point Selenita at a scratch folder),
/// else from `config_home/user-dirs.dirs`, read here without running
/// `xdg-user-dir`. `None` when neither names it.
#[must_use]
pub fn pictures_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_PICTURES_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let file = xdg::config_home()?.join("user-dirs.dirs");
    let bytes = std::fs::read(file).ok()?;
    user_dir_from(&bytes, "XDG_PICTURES_DIR", &home)
}

/// The folder `key` names in a `user-dirs.dirs` file's bytes: a line
/// `KEY="$HOME/Pictures"` or `KEY="/absolute"`, the last one winning as the
/// shell that sources the file would have it. Read as bytes, so a folder
/// whose name is not UTF-8 comes back exact. A folder equal to the home
/// itself means "not set", as `xdg-user-dirs` writes it.
#[must_use]
pub fn user_dir_from(bytes: &[u8], key: &str, home: &Path) -> Option<PathBuf> {
    let mut found = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.trim_ascii();
        if line.starts_with(b"#") {
            continue;
        }
        let Some(value) = line
            .strip_prefix(key.as_bytes())
            .and_then(|rest| rest.strip_prefix(b"="))
            .and_then(|value| value.strip_prefix(b"\""))
            .and_then(|value| value.strip_suffix(b"\""))
        else {
            continue;
        };
        let path = if let Some(rest) = value.strip_prefix(b"$HOME") {
            let mut joined = home.as_os_str().as_bytes().to_vec();
            joined.extend_from_slice(rest);
            PathBuf::from(OsString::from_vec(joined))
        } else if value.starts_with(b"/") {
            PathBuf::from(OsStr::from_bytes(value))
        } else {
            continue;
        };
        found = Some(path);
    }
    found.filter(|path| path.as_path() != home && !path.as_os_str().is_empty())
}
