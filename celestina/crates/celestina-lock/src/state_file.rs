// language-contract: allow-non-english
// (the tests type international input: accents, symbols, emoji)
//! The pid file for the author, and nothing more (design §4.2, §5 "Nothing
//! outside the compositor is trusted about coverage", TM-6b, F31).
//!
//! `$XDG_RUNTIME_DIR/celestina/lock-<WAYLAND_DISPLAY>.state` holds `pid <n>`
//! so the author can find a stuck lock from another console (design §8). It is
//! **never read** by this program, and nothing reads it to decide whether the
//! session is covered, whether to lock, release an inhibitor or suspend: the
//! compositor is the only authority. The name carries the display so a nested
//! lock in a shared runtime directory never overwrites the live session's.
//!
//! Creating it is best effort and holds an `flock` for the life of the
//! process; a second lock that cannot take the lock leaves the first one's file
//! alone (the compositor refuses the second lock anyway).
#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rustix::fs::{flock, FlockOperation, OFlags};

/// The longest sanitised display name kept.
const NAME_MAX: usize = 64;

/// A display name reduced to `[A-Za-z0-9._-]`, every other byte becoming `_`.
#[must_use]
pub fn sanitise(display: &str) -> String {
    let name: String = display
        .chars()
        .take(NAME_MAX)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "_".to_owned()
    } else {
        name
    }
}

/// The file's path for a runtime directory and a display.
#[must_use]
pub fn path_for(runtime_dir: &Path, display: &str) -> PathBuf {
    runtime_dir
        .join("celestina")
        .join(format!("lock-{}.state", sanitise(display)))
}

/// The pid file while this process owns it. Removed on drop.
#[derive(Debug)]
pub struct StateFile {
    path: PathBuf,
    // Kept open: the `flock` lives as long as the descriptor.
    _file: File,
}

impl StateFile {
    /// Creates (or takes over a stale) pid file and writes `pid <pid>` in it.
    ///
    /// # Errors
    ///
    /// Anything that stops it: the directory, another live lock holding the
    /// file, a write error. The caller logs it and goes on.
    pub fn create(runtime_dir: &Path, display: &str, pid: u32) -> io::Result<Self> {
        let path = path_for(runtime_dir, display);
        if let Some(dir) = path.parent() {
            match fs::DirBuilder::new().mode(0o700).create(dir) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            // Never follow a link someone planted in place of the file.
            .custom_flags(i32::try_from(OFlags::NOFOLLOW.bits()).unwrap_or(0))
            .open(&path)?;
        flock(&file, FlockOperation::NonBlockingLockExclusive).map_err(io::Error::from)?;
        file.set_len(0)?;
        writeln!(file, "pid {pid}")?;
        file.flush()?;
        Ok(Self { path, _file: file })
    }

    /// Where the file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StateFile {
    fn drop(&mut self) {
        // Only the owner (it holds the flock) removes the file.
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "celestina-lock-state-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn the_display_name_is_reduced_to_a_safe_alphabet() {
        assert_eq!(sanitise("wayland-1"), "wayland-1");
        assert_eq!(sanitise("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitise("a b\n\0é"), "a_b___");
        assert_eq!(sanitise(""), "_");
        assert_eq!(sanitise(&"x".repeat(500)).len(), NAME_MAX);
        let path = path_for(Path::new("/run/user/1000"), "../x");
        assert_eq!(path, Path::new("/run/user/1000/celestina/lock-.._x.state"));
    }

    #[test]
    fn it_writes_the_pid_with_mode_0600_and_removes_it_on_drop() {
        let dir = scratch("write");
        let state = StateFile::create(&dir, "wayland-9", 4242).expect("create");
        let path = state.path().to_path_buf();
        assert_eq!(fs::read_to_string(&path).expect("read"), "pid 4242\n");
        let mode = fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        drop(state);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_lock_of_the_same_display_leaves_the_first_ones_file_alone() {
        let dir = scratch("second");
        let first = StateFile::create(&dir, "wayland-9", 1111).expect("first");
        let second = StateFile::create(&dir, "wayland-9", 2222);
        assert!(second.is_err(), "the flock is held");
        assert_eq!(
            fs::read_to_string(first.path()).expect("read"),
            "pid 1111\n",
            "not clobbered"
        );
        // And the failed one did not remove it either.
        let path = first.path().to_path_buf();
        assert!(path.exists());
        drop(first);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_file_is_taken_over_and_a_symlink_is_refused() {
        let dir = scratch("stale");
        let path = path_for(&dir, "wayland-9");
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(&path, "pid 1\n").expect("stale");
        let state = StateFile::create(&dir, "wayland-9", 3333).expect("takeover");
        assert_eq!(fs::read_to_string(&path).expect("read"), "pid 3333\n");
        drop(state);

        let victim = dir.join("victim");
        fs::write(&victim, "keep").expect("victim");
        std::os::unix::fs::symlink(&victim, &path).expect("link");
        assert!(StateFile::create(&dir, "wayland-9", 4444).is_err());
        assert_eq!(fs::read_to_string(&victim).expect("read"), "keep");
        let _ = fs::remove_dir_all(&dir);
    }
}
