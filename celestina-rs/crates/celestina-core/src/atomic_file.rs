//! Small files written whole and read bounded: suite state, and user media
//! landed next to its source.
//!
//! Every write here takes the same shape: the complete new bytes go into a
//! unique hidden sibling, the sibling is synced, a rename publishes it, and the
//! containing directory is synced. The destination is never opened for writing,
//! so a failure before the rename leaves it exactly as it was, and a failed
//! attempt removes only its own sibling.
//!
//! There are three entry points, because the three kinds of file want different
//! things from the result, and a single function with a mode flag would hide
//! that choice from the call site:
//!
//! - [`replace`] — the original entry point, unchanged: the new file gets the
//!   process umask's mode and missing parents are created at the umask too. It
//!   reports an `Err` when syncing the directory fails *after* the rename,
//!   although the new bytes are then already in place.
//! - [`replace_private`] — suite state that only its owner may read (clipboard
//!   history, recent documents, bookmarks, settings): the file is `0600` from
//!   its creation, and missing parents are created `0700` as the XDG base
//!   directory spec asks.
//! - [`land_media`] (and its two-step form [`stage_media`]) — a person's file
//!   written beside its source (an edited photo, an exported frame): the new
//!   file takes the source's permission bits and, where the filesystem allows
//!   it, its group, and the publish *refuses* to replace an existing name, so a
//!   "keep both" copy can never clobber a file created in the meantime.
//!
//! The two new entry points return [`Published`], which tells a caller the truth
//! after the rename: the bytes are in place either way, and only the directory
//! sync, which protects the new name across a power loss, may have failed.
//!
//! [`read_bounded`] is the matching reader for small state files: it refuses
//! anything but a regular file (a FIFO named like a state file would otherwise
//! block its reader for ever) and reads at most one byte past the limit, so the
//! bound holds even when the file grows while it is read.
//!
//! # Adoption
//!
//! No consumer calls the new entry points yet (ruling R-A3 keeps the unit that
//! introduced them purely additive). The shell's clipboard history
//! (`SURF-1-E`, P-10) and magnetita-net's `write_private` (`MAG-D1-D`, P-11)
//! move to [`replace_private`]; Fluorita's edits and frame exports land through
//! [`stage_media`] and [`land_media`] (`FLU-H1-A`, P-7); the shell's settings
//! reader adopts [`read_bounded`] (`SURF-1-F`, P-17), and the other unbounded
//! state readers follow in their owners' units.

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

/// Write and sync a unique sibling, then atomically replace `path`. The old
/// file remains intact until the complete new bytes are durable enough to
/// rename, and a failed attempt cleans only its own temporary file.
///
/// The new file and any missing parent are created at the process umask, and
/// an `Err` from the final directory sync arrives after the rename has already
/// published the new bytes. New private state uses [`replace_private`], which
/// answers both.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let (temporary, mut file) = create_temporary(path, None)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        // The file sync protects its bytes; syncing the containing directory
        // protects the renamed directory entry across a sudden power loss.
        fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// What a write that reached its rename achieved.
///
/// Both variants mean the new bytes are at the destination: every later open
/// of that name reads them, and the caller must treat the write as done.
#[must_use]
#[derive(Debug)]
pub enum Published {
    /// The containing directory was synced too, so the new name survives a
    /// sudden power loss.
    Durable,
    /// Syncing the containing directory failed. The new bytes are in place
    /// now, but a power loss before the filesystem's next commit may bring the
    /// previous directory entry back.
    DirectoryNotSynced(io::Error),
}

impl Published {
    /// Whether the directory sync succeeded as well.
    #[must_use]
    pub fn is_durable(&self) -> bool {
        matches!(self, Self::Durable)
    }
}

/// The step of a write that failed. Every step before [`WriteStep::Publish`]
/// leaves the destination untouched, and so does a failed publish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteStep {
    /// Creating a missing parent directory.
    CreateParent,
    /// Reading the permission bits of the file a media write copies them from.
    ReadSourceMode,
    /// Creating the hidden sibling.
    CreateTemporary,
    /// Writing the bytes into the sibling.
    Write,
    /// Setting the sibling's mode.
    SetMode,
    /// Syncing the sibling's bytes.
    Sync,
    /// Moving the sibling to the destination name.
    Publish,
}

impl WriteStep {
    fn describe(self) -> &'static str {
        match self {
            Self::CreateParent => "creating the parent of",
            Self::ReadSourceMode => "reading the mode of the source of",
            Self::CreateTemporary => "creating the temporary for",
            Self::Write => "writing the temporary for",
            Self::SetMode => "setting the mode of the temporary for",
            Self::Sync => "syncing the temporary for",
            Self::Publish => "publishing",
        }
    }
}

/// Why a write through [`replace_private`] or [`land_media`] did not publish.
/// The destination is unchanged in every case.
#[derive(Debug)]
pub enum WriteError {
    /// A media write found the destination name taken and did not replace it.
    TargetExists { path: PathBuf },
    /// A filesystem call failed at `step` while writing `path`.
    Io {
        step: WriteStep,
        path: PathBuf,
        source: io::Error,
    },
}

impl WriteError {
    fn io(step: WriteStep, path: &Path, source: io::Error) -> Self {
        Self::Io {
            step,
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetExists { path } => {
                write!(formatter, "{} already exists", path.display())
            }
            Self::Io { step, path, source } => {
                write!(
                    formatter,
                    "{} {}: {source}",
                    step.describe(),
                    path.display()
                )
            }
        }
    }
}

impl Error for WriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TargetExists { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

type SyncDirectory = fn(&Path) -> io::Result<()>;

fn sync_directory(directory: &Path) -> io::Result<()> {
    fs::File::open(directory)?.sync_all()
}

fn parent_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Replaces `path` with `bytes` as a file only its owner can read or write.
///
/// The sibling is created `0600` and set to exactly `0600` before it is
/// synced, so there is no moment at which the new contents are readable by
/// anyone else. Missing parents are created `0700`; existing parents keep
/// their mode (see [`crate::xdg::ensure_private_dir`] for a directory the
/// caller owns). An existing file at `path` is replaced whatever its mode.
///
/// # Errors
///
/// A [`WriteError::Io`] naming the step that failed; `path` is then unchanged.
pub fn replace_private(path: &Path, bytes: &[u8]) -> Result<Published, WriteError> {
    replace_private_with(path, bytes, sync_directory)
}

fn replace_private_with(
    path: &Path,
    bytes: &[u8],
    sync: SyncDirectory,
) -> Result<Published, WriteError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent_of(path))
        .map_err(|source| WriteError::io(WriteStep::CreateParent, path, source))?;
    let staged = Staged::write(path, bytes, fs::Permissions::from_mode(0o600), None)?;
    staged.publish(sync, rename_replacing)
}

fn rename_replacing(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

/// Lands `bytes` at `destination`, a new name, as a sibling of the person's
/// file `mode_source`, whose permission bits (and group, where the filesystem
/// allows it) the new file takes. An existing `destination` is never replaced.
///
/// This is [`stage_media`] followed at once by [`StagedMedia::publish`].
///
/// # Errors
///
/// [`WriteError::TargetExists`] when the name is taken, or a
/// [`WriteError::Io`] naming the failed step; `destination` is unchanged.
pub fn land_media(
    destination: &Path,
    bytes: &[u8],
    mode_source: &Path,
) -> Result<Published, WriteError> {
    stage_media(destination, bytes, mode_source)?.publish()
}

/// Writes and syncs the complete new file for `destination` under a hidden
/// sibling name, without publishing it yet.
///
/// Two steps exist for the edit that replaces its original: the caller stages
/// the new bytes, moves the original out of the way (to the Trash) only once
/// they are durable, and then publishes into the name the original left. At no
/// point is the person left with neither file. Dropping the [`StagedMedia`]
/// without publishing removes the sibling.
///
/// The sibling is private (`0600`) while it is written and takes the source's
/// permission bits, with set-id and sticky bits cleared, just before it is
/// synced. Owner and extended attributes (ACLs, labels) are not copied; the
/// group is copied when the filesystem and the caller's groups allow it.
/// Missing parent directories are not created: media lands beside its source.
///
/// # Errors
///
/// A [`WriteError::Io`] naming the failed step.
pub fn stage_media(
    destination: &Path,
    bytes: &[u8],
    mode_source: &Path,
) -> Result<StagedMedia, WriteError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let source = fs::metadata(mode_source)
        .map_err(|error| WriteError::io(WriteStep::ReadSourceMode, destination, error))?;
    let permissions = fs::Permissions::from_mode(source.permissions().mode() & 0o777);
    let staged = Staged::write(destination, bytes, permissions, Some(source.gid()))?;
    Ok(StagedMedia { staged })
}

/// Media bytes that are durable under a hidden sibling name and not published.
#[derive(Debug)]
pub struct StagedMedia {
    staged: Staged,
}

impl StagedMedia {
    /// The name [`StagedMedia::publish`] will land the file at.
    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.staged.destination
    }

    /// Moves the staged file to its destination name, refusing to replace
    /// anything that holds that name by then.
    ///
    /// The move is a hard link followed by removing the sibling name; the
    /// kernel refuses the link atomically when the name exists. A filesystem
    /// without hard links (FAT, exFAT, some FUSE mounts) gets an exclusive
    /// create of the destination as a reservation, and the rename then replaces
    /// only that empty reservation.
    ///
    /// # Errors
    ///
    /// [`WriteError::TargetExists`] when the name is taken, or a
    /// [`WriteError::Io`] for the failed publish; the destination is unchanged
    /// and the sibling is removed.
    pub fn publish(self) -> Result<Published, WriteError> {
        self.staged
            .publish(sync_directory, publish_without_replacing)
    }
}

fn publish_without_replacing(temporary: &Path, destination: &Path) -> io::Result<()> {
    match fs::hard_link(temporary, destination) {
        Ok(()) => {
            // Both names hold the file now and the publish has happened; a
            // hidden name left behind is clutter, not a lost or wrong file.
            let _ = fs::remove_file(temporary);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(error),
        Err(_) => {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            fs::rename(temporary, destination).inspect_err(|_| {
                let _ = fs::remove_file(destination);
            })
        }
    }
}

/// A complete, synced sibling of `destination`, removed on drop unless it was
/// published.
#[derive(Debug)]
struct Staged {
    temporary: PathBuf,
    destination: PathBuf,
    published: bool,
}

impl Staged {
    fn write(
        destination: &Path,
        bytes: &[u8],
        permissions: fs::Permissions,
        group: Option<u32>,
    ) -> Result<Self, WriteError> {
        let (temporary, mut file) = create_temporary(destination, Some(0o600))
            .map_err(|error| WriteError::io(WriteStep::CreateTemporary, destination, error))?;
        let staged = Self {
            temporary,
            destination: destination.to_path_buf(),
            published: false,
        };
        file.write_all(bytes)
            .map_err(|error| WriteError::io(WriteStep::Write, destination, error))?;
        if let Some(group) = group {
            // Best effort by design: a person may edit a file whose group they
            // are not a member of, and then the new file keeps their own group.
            let _ = std::os::unix::fs::fchown(&file, None, Some(group));
        }
        // After the ownership change, which may clear set-id bits, and before
        // the sync, so the synced inode already carries its final mode.
        file.set_permissions(permissions)
            .map_err(|error| WriteError::io(WriteStep::SetMode, destination, error))?;
        file.sync_all()
            .map_err(|error| WriteError::io(WriteStep::Sync, destination, error))?;
        Ok(staged)
    }

    fn publish(
        mut self,
        sync: SyncDirectory,
        rename: fn(&Path, &Path) -> io::Result<()>,
    ) -> Result<Published, WriteError> {
        if let Err(error) = rename(&self.temporary, &self.destination) {
            return Err(if error.kind() == io::ErrorKind::AlreadyExists {
                WriteError::TargetExists {
                    path: self.destination.clone(),
                }
            } else {
                WriteError::io(WriteStep::Publish, &self.destination, error)
            });
        }
        self.published = true;
        Ok(match sync(parent_of(&self.destination)) {
            Ok(()) => Published::Durable,
            Err(error) => Published::DirectoryNotSynced(error),
        })
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.temporary);
        }
    }
}

fn create_temporary(path: &Path, mode: Option<u32>) -> io::Result<(PathBuf, fs::File)> {
    use std::os::unix::fs::OpenOptionsExt;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    for _ in 0..10_000 {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.{}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        if let Some(mode) = mode {
            options.mode(mode);
        }
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve an atomic state-file temporary",
    ))
}

/// Why [`read_bounded`] returned no bytes for a file that exists.
#[derive(Debug)]
pub enum ReadError {
    /// The path names a FIFO, a socket, a device or a directory.
    NotRegular { path: PathBuf },
    /// The file holds more than `limit` bytes.
    TooLarge { path: PathBuf, limit: u64 },
    /// Opening or reading the file failed.
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegular { path } => {
                write!(formatter, "{} is not a regular file", path.display())
            }
            Self::TooLarge { path, limit } => {
                write!(formatter, "{} is larger than {limit} bytes", path.display())
            }
            Self::Io { path, source } => {
                write!(formatter, "reading {}: {source}", path.display())
            }
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// `O_NONBLOCK` on the architectures that use the generic value from
/// `include/uapi/asm-generic/fcntl.h`. Opening a FIFO with it returns at once
/// instead of waiting for a writer; on a regular file it changes nothing.
/// `std` names no such constant and this crate takes no dependency for one.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const OPEN_NONBLOCK: i32 = 0o4000;

/// Elsewhere only the type check before the open guards against a FIFO.
#[cfg(not(all(
    target_os = "linux",
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
)))]
const OPEN_NONBLOCK: i32 = 0;

/// Reads a whole small file, or `Ok(None)` when it does not exist.
///
/// Symlinks are followed, as for any configuration file a person may keep
/// elsewhere. The target must be a regular file: the type is checked before the
/// open, the open does not wait for a FIFO writer, and the opened file is
/// checked again, so a file swapped for a FIFO in between is refused rather
/// than blocking the reader. At most `limit + 1` bytes are read.
///
/// # Errors
///
/// [`ReadError::NotRegular`] or [`ReadError::TooLarge`] for a file this
/// refuses, and [`ReadError::Io`] for a failed open or read.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, ReadError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(ReadError::NotRegular {
                path: path.to_path_buf(),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ReadError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    }
    match open_nonblocking(path) {
        Ok(file) => read_open_file(path, file, limit).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ReadError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn open_nonblocking(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_NONBLOCK)
        .open(path)
}

fn read_open_file(path: &Path, file: fs::File, limit: u64) -> Result<Vec<u8>, ReadError> {
    let io_error = |source| ReadError::Io {
        path: path.to_path_buf(),
        source,
    };
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ReadError::NotRegular {
            path: path.to_path_buf(),
        });
    }
    let expected = usize::try_from(metadata.len().min(limit)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(expected);
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(ReadError::TooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    use super::{
        land_media, open_nonblocking, read_bounded, read_open_file, replace, replace_private,
        replace_private_with, stage_media, Published, ReadError, WriteError, WriteStep,
    };
    use crate::scratch::Scratch;

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path).expect("metadata").permissions().mode() & 0o7777
    }

    fn names_in(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .expect("listing")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn mkfifo(path: &Path) -> bool {
        let made = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .is_ok_and(|status| status.success());
        if !made {
            eprintln!("mkfifo is unavailable; the FIFO case is not exercised");
        }
        made
    }

    #[test]
    fn replacement_publishes_complete_bytes_and_leaves_no_temporary() {
        let scratch = Scratch::new("atomic-file");
        let root = scratch.path();
        let path = root.join("state.json");
        fs::write(&path, b"old").unwrap();

        replace(&path, b"complete-new-state").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"complete-new-state");
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
    }

    #[test]
    fn private_state_is_0600_in_0700_parents_even_over_a_wider_file() {
        let scratch = Scratch::new("private-state");
        let parent = scratch.path().join("state").join("celestina");
        let path = parent.join("clipboard.json");

        assert!(replace_private(&path, b"first")
            .expect("written")
            .is_durable());
        assert_eq!(fs::read(&path).expect("read"), b"first");
        assert_eq!(mode_of(&path), 0o600);
        assert_eq!(mode_of(&parent), 0o700);
        assert_eq!(mode_of(&scratch.path().join("state")), 0o700);

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert!(replace_private(&path, b"second")
            .expect("written")
            .is_durable());
        assert_eq!(fs::read(&path).expect("read"), b"second");
        assert_eq!(mode_of(&path), 0o600);
        assert_eq!(names_in(&parent), ["clipboard.json"]);
    }

    #[test]
    fn a_failed_directory_sync_after_the_rename_is_reported_as_published() {
        let scratch = Scratch::new("dir-sync");
        let path = scratch.path().join("settings.json");
        fs::write(&path, b"old").expect("old");

        let outcome = replace_private_with(&path, b"new", |_| {
            Err(io::Error::other("injected directory fsync failure"))
        })
        .expect("the rename happened, so this is not an error");

        assert!(matches!(outcome, Published::DirectoryNotSynced(_)));
        assert!(!outcome.is_durable());
        assert_eq!(fs::read(&path).expect("read"), b"new");
        assert_eq!(names_in(scratch.path()), ["settings.json"]);
    }

    #[test]
    fn a_write_that_cannot_start_leaves_everything_alone() {
        let scratch = Scratch::new("private-fail");
        let blocker = scratch.path().join("not-a-directory");
        fs::write(&blocker, b"file").expect("file");

        let error = replace_private(&blocker.join("state.json"), b"x").expect_err("no parent");
        assert!(matches!(
            error,
            WriteError::Io {
                step: WriteStep::CreateParent,
                ..
            }
        ));
        assert_eq!(fs::read(&blocker).expect("read"), b"file");
        assert_eq!(names_in(scratch.path()), ["not-a-directory"]);
    }

    #[test]
    fn landed_media_keeps_the_source_mode_and_group() {
        let scratch = Scratch::new("media-mode");
        let source = scratch.path().join("photo.jpg");
        fs::write(&source, b"original").expect("source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).expect("chmod");
        let copy = scratch.path().join("photo (edited).jpg");

        assert!(land_media(&copy, b"edited", &source)
            .expect("landed")
            .is_durable());

        assert_eq!(fs::read(&copy).expect("read"), b"edited");
        assert_eq!(mode_of(&copy), 0o640);
        assert_eq!(
            fs::metadata(&copy).expect("copy").gid(),
            fs::metadata(&source).expect("source").gid()
        );
        assert_eq!(fs::read(&source).expect("source"), b"original");
        assert_eq!(
            names_in(scratch.path()),
            ["photo (edited).jpg", "photo.jpg"]
        );
    }

    #[test]
    fn a_private_source_lands_private_and_set_id_bits_are_not_copied() {
        let scratch = Scratch::new("media-private");
        let source = scratch.path().join("private.jpg");
        fs::write(&source, b"original").expect("source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o6700)).expect("chmod");
        let copy = scratch.path().join("private (edited).jpg");

        assert!(land_media(&copy, b"edited", &source)
            .expect("landed")
            .is_durable());

        assert_eq!(mode_of(&copy), 0o700);
    }

    #[test]
    fn landing_media_refuses_to_replace_an_existing_name() {
        let scratch = Scratch::new("media-noreplace");
        let source = scratch.path().join("photo.jpg");
        fs::write(&source, b"original").expect("source");
        let taken = scratch.path().join("photo (1).jpg");
        fs::write(&taken, b"someone else's").expect("taken");

        let error = land_media(&taken, b"edited", &source).expect_err("the name is taken");

        assert!(matches!(error, WriteError::TargetExists { .. }));
        assert_eq!(fs::read(&taken).expect("taken"), b"someone else's");
        assert_eq!(names_in(scratch.path()), ["photo (1).jpg", "photo.jpg"]);
    }

    #[test]
    fn staged_media_replaces_its_original_only_after_the_original_moved_away() {
        let scratch = Scratch::new("media-staged");
        let source = scratch.path().join("photo.jpg");
        fs::write(&source, b"original").expect("source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).expect("chmod");
        let trash = scratch.path().join("trashed-photo.jpg");

        let staged = stage_media(&source, b"edited", &source).expect("staged");
        assert_eq!(staged.destination(), source.as_path());
        // Staging publishes nothing: the original is still the original.
        assert_eq!(fs::read(&source).expect("source"), b"original");

        fs::rename(&source, &trash).expect("the caller moves the original away");
        assert!(staged.publish().expect("published").is_durable());

        assert_eq!(fs::read(&source).expect("new"), b"edited");
        assert_eq!(mode_of(&source), 0o600);
        assert_eq!(fs::read(&trash).expect("trashed"), b"original");
        assert_eq!(names_in(scratch.path()), ["photo.jpg", "trashed-photo.jpg"]);
    }

    #[test]
    fn staged_media_over_a_name_still_taken_is_refused() {
        let scratch = Scratch::new("media-staged-taken");
        let source = scratch.path().join("photo.jpg");
        fs::write(&source, b"original").expect("source");

        let staged = stage_media(&source, b"edited", &source).expect("staged");
        let error = staged.publish().expect_err("the original was never moved");

        assert!(matches!(error, WriteError::TargetExists { .. }));
        assert_eq!(fs::read(&source).expect("source"), b"original");
        assert_eq!(names_in(scratch.path()), ["photo.jpg"]);
    }

    #[test]
    fn dropping_staged_media_removes_its_sibling() {
        let scratch = Scratch::new("media-drop");
        let source = scratch.path().join("photo.jpg");
        fs::write(&source, b"original").expect("source");

        let staged =
            stage_media(&scratch.path().join("copy.jpg"), b"edited", &source).expect("staged");
        assert_eq!(names_in(scratch.path()).len(), 2);
        drop(staged);

        assert_eq!(names_in(scratch.path()), ["photo.jpg"]);
    }

    #[test]
    fn a_missing_mode_source_stages_nothing() {
        let scratch = Scratch::new("media-nosource");
        let error = land_media(
            &scratch.path().join("copy.jpg"),
            b"edited",
            &scratch.path().join("missing.jpg"),
        )
        .expect_err("no source");

        assert!(matches!(
            error,
            WriteError::Io {
                step: WriteStep::ReadSourceMode,
                ..
            }
        ));
        assert!(names_in(scratch.path()).is_empty());
    }

    #[test]
    fn a_bounded_read_returns_the_bytes_or_nothing_for_a_missing_file() {
        let scratch = Scratch::new("read-bounded");
        let path = scratch.path().join("settings.json");
        assert!(read_bounded(&path, 16)
            .expect("a missing file is not an error")
            .is_none());

        fs::write(&path, b"exactly-16-bytes").expect("write");
        assert_eq!(
            read_bounded(&path, 16).expect("read").as_deref(),
            Some(&b"exactly-16-bytes"[..])
        );
    }

    #[test]
    fn a_file_over_the_limit_is_refused() {
        let scratch = Scratch::new("read-oversize");
        let path = scratch.path().join("settings.json");
        fs::write(&path, b"seventeen bytes!!").expect("write");

        assert!(matches!(
            read_bounded(&path, 16),
            Err(ReadError::TooLarge { limit: 16, .. })
        ));
    }

    #[test]
    fn a_directory_or_a_fifo_is_not_a_state_file() {
        let scratch = Scratch::new("read-irregular");
        assert!(matches!(
            read_bounded(scratch.path(), 16),
            Err(ReadError::NotRegular { .. })
        ));

        let fifo = scratch.path().join("settings.json");
        if mkfifo(&fifo) {
            assert!(matches!(
                read_bounded(&fifo, 16),
                Err(ReadError::NotRegular { .. })
            ));
        }
    }

    #[test]
    fn a_fifo_swapped_in_after_the_type_check_does_not_block_the_reader() {
        let scratch = Scratch::new("read-swap");
        let fifo = scratch.path().join("x.desktop");
        if !mkfifo(&fifo) {
            return;
        }
        // The open and the second check, as `read_bounded` runs them after a
        // type check that saw a regular file under this name.
        let (sender, receiver) = std::sync::mpsc::channel();
        let path = fifo.clone();
        std::thread::spawn(move || {
            let refused = match open_nonblocking(&path) {
                Ok(file) => matches!(
                    read_open_file(&path, file, 16),
                    Err(ReadError::NotRegular { .. })
                ),
                Err(_) => false,
            };
            let _ = sender.send(refused);
        });
        let refused = receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("opening a FIFO with no writer must not wait for one");
        assert!(refused);
    }
}
