//! Freedesktop base directories, and the private directories the suite keeps
//! under them.
//!
//! Each `*_home` returns the environment variable when it holds an absolute
//! path, and otherwise the spec's `$HOME`-relative fallback. `siderita-ops` (the
//! home Trash) and the app (every config and data file it reads) both resolve
//! these, so they live here once instead of being copied into each module.
//!
//! [`runtime_dir`] is different on purpose: the spec gives
//! `$XDG_RUNTIME_DIR` no fallback, and the fallback the suite's copies used,
//! the world-writable `/tmp`, is exactly where another local account can
//! pre-create a predictable name (a symlink, or a directory it owns) and
//! capture a mount point, a FIFO or a lock. It therefore answers an error, never
//! a substitute, and it checks what the spec requires of the directory: absolute,
//! a directory, owned by this user, and closed to everyone else. A caller that
//! gets an error degrades the feature that needed the directory.
//!
//! [`ensure_private_dir`] creates or checks a directory the suite owns under one
//! of these bases (`$XDG_RUNTIME_DIR/magnetita`, the shell's style import root,
//! `$XDG_STATE_HOME/celestina`): created `0700`, refused when it is a symlink or
//! another user's, and tightened to `0700` when it is this user's own directory
//! from an older release that created it wider.
//!
//! # Adoption
//!
//! No consumer calls [`runtime_dir`] or [`ensure_private_dir`] yet (ruling R-A3
//! keeps the unit that introduced them purely additive). The ad-hoc
//! `XDG_RUNTIME_DIR` lookups move here in their owners' bug units: magnetitad's
//! mounts, mirror FIFOs and artwork cache and the Magnetita app's mirror view
//! (`MAG-D1-D`, P-11), and the shell's DDC lock, Melibea socket and style
//! import root (`SURF-1-E`, P-10).

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// `$XDG_CONFIG_HOME` if it is an absolute path, else `$HOME/.config`.
pub fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".config")))
}

/// `$XDG_CACHE_HOME` if it is an absolute path, else `$HOME/.cache`.
///
/// The shared thumbnail cache hangs off this one, so it is the base directory
/// two projects now resolve — Siderita through Qt, Fluorita through here.
pub fn cache_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".cache")))
}

/// `$XDG_DATA_HOME` if it is an absolute path, else `$HOME/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".local").join("share")))
}

/// `$XDG_STATE_HOME` if it is an absolute path, else `$HOME/.local/state`.
///
/// The spec's own distinction from `data_home` is the one that matters here:
/// state that should survive a restart but is not portable or precious enough
/// to belong in `data_home` — a clipboard history is exactly that, and never a
/// document.
pub fn state_home() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".local").join("state")))
}

/// Why a private directory — the runtime directory, or one the suite keeps
/// under a base directory — cannot be used.
#[derive(Debug)]
pub enum PrivateDirError {
    /// `$XDG_RUNTIME_DIR` is unset or empty.
    Unset,
    /// The path is relative, so it would resolve against the working directory.
    NotAbsolute { path: PathBuf },
    /// A filesystem call on `path` failed.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// The path is not a directory, or (for [`ensure_private_dir`]) is a
    /// symlink to one.
    NotADirectory { path: PathBuf },
    /// The directory belongs to another user.
    NotOwned {
        path: PathBuf,
        owner: u32,
        user: u32,
    },
    /// The runtime directory grants its group or other users some access.
    Shared { path: PathBuf, mode: u32 },
    /// This process's effective user id could not be read.
    UnknownUser { source: io::Error },
}

impl fmt::Display for PrivateDirError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unset => formatter.write_str("XDG_RUNTIME_DIR is not set"),
            Self::NotAbsolute { path } => {
                write!(formatter, "{} is not an absolute path", path.display())
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} {}: {source}", path.display()),
            Self::NotADirectory { path } => {
                write!(formatter, "{} is not a directory", path.display())
            }
            Self::NotOwned { path, owner, user } => write!(
                formatter,
                "{} belongs to uid {owner}, not to uid {user}",
                path.display()
            ),
            Self::Shared { path, mode } => write!(
                formatter,
                "{} has mode {mode:04o}; a private directory is 0700",
                path.display()
            ),
            Self::UnknownUser { source } => {
                write!(formatter, "cannot read the effective user id: {source}")
            }
        }
    }
}

impl Error for PrivateDirError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::UnknownUser { source } => Some(source),
            _ => None,
        }
    }
}

/// `$XDG_RUNTIME_DIR`, only when it is what the spec says it must be: an
/// absolute path to a directory owned by this user with no access for anyone
/// else. There is no fallback.
///
/// The check reads the directory's metadata, so it belongs on a worker or in
/// start-up code, not in a Qt-thread invokable.
///
/// # Errors
///
/// [`PrivateDirError::Unset`] when the variable is unset or empty, and the
/// variant naming the broken requirement otherwise.
pub fn runtime_dir() -> Result<PathBuf, PrivateDirError> {
    let user = effective_uid().map_err(|source| PrivateDirError::UnknownUser { source })?;
    checked_runtime_dir(std::env::var_os("XDG_RUNTIME_DIR"), user)
}

fn checked_runtime_dir(value: Option<OsString>, user: u32) -> Result<PathBuf, PrivateDirError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let path = value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(PrivateDirError::Unset)?;
    if !path.is_absolute() {
        return Err(PrivateDirError::NotAbsolute { path });
    }
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(source) => {
            return Err(PrivateDirError::Io {
                operation: "reading the metadata of",
                path,
                source,
            })
        }
    };
    if !metadata.is_dir() {
        return Err(PrivateDirError::NotADirectory { path });
    }
    if metadata.uid() != user {
        return Err(PrivateDirError::NotOwned {
            path,
            owner: metadata.uid(),
            user,
        });
    }
    let mode = metadata.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(PrivateDirError::Shared { path, mode });
    }
    Ok(path)
}

/// Creates `path` and any missing parents with mode `0700`, or checks the
/// directory that is already there.
///
/// An existing directory must be a real directory (not a symlink to one) owned
/// by this user. If it grants its group or other users any access it is
/// tightened to `0700`: only its owner could have created it, so this is an
/// older release's directory, not someone else's trap. Existing parents are
/// left as they are; they are the user's (`~/.local/state`) or the session's.
///
/// # Errors
///
/// The [`PrivateDirError`] naming the requirement `path` breaks, or the
/// filesystem call that failed.
pub fn ensure_private_dir(path: &Path) -> Result<(), PrivateDirError> {
    let user = effective_uid().map_err(|source| PrivateDirError::UnknownUser { source })?;
    ensure_private_dir_for(path, user)
}

fn ensure_private_dir_for(path: &Path, user: u32) -> Result<(), PrivateDirError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    if !path.is_absolute() {
        return Err(PrivateDirError::NotAbsolute {
            path: path.to_path_buf(),
        });
    }
    match std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
    {
        Ok(()) => {}
        // Something that is not a directory holds the name; the inspection
        // below names it.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(PrivateDirError::Io {
                operation: "creating",
                path: path.to_path_buf(),
                source,
            })
        }
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|source| PrivateDirError::Io {
        operation: "reading the metadata of",
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_dir() {
        return Err(PrivateDirError::NotADirectory {
            path: path.to_path_buf(),
        });
    }
    if metadata.uid() != user {
        return Err(PrivateDirError::NotOwned {
            path: path.to_path_buf(),
            owner: metadata.uid(),
            user,
        });
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |source| PrivateDirError::Io {
                operation: "restricting the mode of",
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

/// The largest `/proc/self/status` this reads. The file is a few kilobytes;
/// the bound only keeps a broken procfs from being read without end.
const MAX_STATUS_BYTES: u64 = 64 * 1024;

/// This process's effective user id, from `/proc/self/status`.
///
/// `std` exposes no `geteuid`, and this crate takes no dependency and no
/// `unsafe` to call it; the status file states the same four ids the kernel
/// keeps, in a documented, stable format.
pub(crate) fn effective_uid() -> io::Result<u32> {
    let mut text = String::new();
    std::fs::File::open("/proc/self/status")?
        .take(MAX_STATUS_BYTES)
        .read_to_string(&mut text)?;
    effective_uid_in(&text).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "/proc/self/status has no readable Uid line",
        )
    })
}

/// The second field of the `Uid:` line: real, effective, saved, filesystem.
fn effective_uid_in(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::{
        cache_home, checked_runtime_dir, effective_uid, effective_uid_in, ensure_private_dir_for,
        state_home, PrivateDirError,
    };
    use crate::scratch::Scratch;
    use std::ffi::OsString;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;

    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::symlink_metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o7777
    }

    fn owner_of(path: &std::path::Path) -> u32 {
        std::fs::symlink_metadata(path).expect("metadata").uid()
    }

    #[test]
    fn the_effective_uid_is_the_second_id_of_the_uid_line() {
        let status = "Name:\tcat\nUmask:\t0022\nUid:\t1000\t1001\t1002\t1003\nGid:\t5\t5\t5\t5\n";
        assert_eq!(effective_uid_in(status), Some(1001));
        assert_eq!(effective_uid_in("Name:\tcat\n"), None);
        assert_eq!(effective_uid_in("Uid:\t1000\n"), None);
    }

    #[test]
    fn the_effective_uid_owns_what_this_process_creates() {
        let scratch = Scratch::new("euid");
        assert_eq!(
            effective_uid().expect("an effective uid"),
            owner_of(scratch.path())
        );
    }

    #[test]
    fn an_unset_or_empty_runtime_dir_has_no_fallback() {
        assert!(matches!(
            checked_runtime_dir(None, 0),
            Err(PrivateDirError::Unset)
        ));
        assert!(matches!(
            checked_runtime_dir(Some(OsString::new()), 0),
            Err(PrivateDirError::Unset)
        ));
    }

    #[test]
    fn a_relative_runtime_dir_is_refused() {
        assert!(matches!(
            checked_runtime_dir(Some("run/user/1000".into()), 0),
            Err(PrivateDirError::NotAbsolute { .. })
        ));
    }

    #[test]
    fn the_runtime_dir_must_be_a_private_directory_of_this_user() {
        let scratch = Scratch::new("runtime");
        let runtime = scratch.path().join("runtime");
        std::fs::create_dir(&runtime).expect("runtime dir");
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        let user = owner_of(&runtime);

        assert_eq!(
            checked_runtime_dir(Some(runtime.clone().into()), user).expect("private"),
            runtime
        );
        assert!(matches!(
            checked_runtime_dir(Some(runtime.clone().into()), user.wrapping_add(1)),
            Err(PrivateDirError::NotOwned { .. })
        ));

        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(matches!(
            checked_runtime_dir(Some(runtime.clone().into()), user),
            Err(PrivateDirError::Shared { mode: 0o755, .. })
        ));

        let file = scratch.path().join("file");
        std::fs::write(&file, b"").expect("file");
        assert!(matches!(
            checked_runtime_dir(Some(file.into()), user),
            Err(PrivateDirError::NotADirectory { .. })
        ));
        assert!(matches!(
            checked_runtime_dir(Some(scratch.path().join("missing").into()), user),
            Err(PrivateDirError::Io { .. })
        ));
    }

    #[test]
    fn a_private_dir_and_its_missing_parents_are_created_0700() {
        let scratch = Scratch::new("private-create");
        let user = owner_of(scratch.path());
        let parent = scratch.path().join("state");
        let leaf = parent.join("celestina");

        ensure_private_dir_for(&leaf, user).expect("created");

        assert_eq!(mode_of(&parent), 0o700);
        assert_eq!(mode_of(&leaf), 0o700);
        // Asking again is a check, not an error.
        ensure_private_dir_for(&leaf, user).expect("already private");
    }

    #[test]
    fn an_own_wider_directory_is_tightened_and_a_foreign_one_refused() {
        let scratch = Scratch::new("private-existing");
        let user = owner_of(scratch.path());
        let wide = scratch.path().join("wide");
        std::fs::create_dir(&wide).expect("dir");
        std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        ensure_private_dir_for(&wide, user).expect("tightened");
        assert_eq!(mode_of(&wide), 0o700);

        std::fs::set_permissions(&wide, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(matches!(
            ensure_private_dir_for(&wide, user.wrapping_add(1)),
            Err(PrivateDirError::NotOwned { .. })
        ));
        // A foreign directory is not repaired, only refused.
        assert_eq!(mode_of(&wide), 0o755);
    }

    #[test]
    fn a_symlink_or_a_file_is_not_a_private_dir() {
        let scratch = Scratch::new("private-symlink");
        let user = owner_of(scratch.path());
        let target = scratch.path().join("target");
        std::fs::create_dir(&target).expect("dir");
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let file = scratch.path().join("file");
        std::fs::write(&file, b"").expect("file");

        assert!(matches!(
            ensure_private_dir_for(&link, user),
            Err(PrivateDirError::NotADirectory { .. })
        ));
        assert!(matches!(
            ensure_private_dir_for(&file, user),
            Err(PrivateDirError::NotADirectory { .. })
        ));
        assert!(matches!(
            ensure_private_dir_for(std::path::Path::new("relative"), user),
            Err(PrivateDirError::NotAbsolute { .. })
        ));
    }

    /// The environment is process-wide, so every base directory that falls
    /// back to `$HOME` shares this one test rather than racing another test
    /// function that also sets `HOME` under the harness's default threading.
    #[test]
    fn cache_home_prefers_the_variable_and_falls_back_to_home() {
        let previous_cache = std::env::var_os("XDG_CACHE_HOME");
        let previous_state = std::env::var_os("XDG_STATE_HOME");
        let previous_home = std::env::var_os("HOME");

        // SAFETY-equivalent discipline: restored before returning, and no other
        // test in this crate reads these variables.
        std::env::set_var("XDG_CACHE_HOME", "/tmp/celestina-cache");
        assert_eq!(cache_home(), Some(PathBuf::from("/tmp/celestina-cache")));

        // A relative value is not a base directory; the spec's fallback wins.
        std::env::set_var("XDG_CACHE_HOME", "relativa");
        std::env::set_var("HOME", "/home/prueba");
        assert_eq!(cache_home(), Some(PathBuf::from("/home/prueba/.cache")));

        std::env::remove_var("XDG_CACHE_HOME");
        assert_eq!(cache_home(), Some(PathBuf::from("/home/prueba/.cache")));

        std::env::remove_var("XDG_STATE_HOME");
        assert_eq!(
            state_home(),
            Some(PathBuf::from("/home/prueba/.local/state"))
        );
        std::env::set_var("XDG_STATE_HOME", "/tmp/celestina-state");
        assert_eq!(state_home(), Some(PathBuf::from("/tmp/celestina-state")));

        match previous_cache {
            Some(value) => std::env::set_var("XDG_CACHE_HOME", value),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
        match previous_state {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}
