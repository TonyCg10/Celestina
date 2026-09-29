//! Copying an entry, and knowing afterwards exactly what the copy made.
//!
//! A copy creates its destination exclusively (`create_new`, `create_dir`,
//! `symlink`, all of which fail when the name is taken), so the name it lands
//! on is this call's alone. Everything it creates is written into a
//! [`Journal`] as it goes, with the identity (device and inode) the new entry
//! was given. A failure or a cancellation then removes the journal's entries
//! and nothing else: a name another writer took first, or an entry somebody
//! added inside a folder this copy created, is not this call's to delete.
//!
//! The journal also records what each source entry looked like when it was
//! copied, which is what lets a move across filesystems remove only what it
//! really copied (see [`crate::relocate`]).

use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;

/// Bytes moved per read/write step; also the cancellation granularity for a
/// single large file.
const CHUNK: usize = 64 * 1024;

/// Cumulative progress of a copy (or the copy half of a cross-device move).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    /// Bytes of file content written so far.
    pub bytes: u64,
    /// Entries (files, directories, symlinks) finished so far.
    pub items: u64,
}

/// Copies `source` — a file, directory tree or symlink — into `into_dir`,
/// keeping the source's own file name, and returns the created destination.
///
/// Loss-free by construction: it refuses to overwrite an existing destination
/// ([`OpError::AlreadyExists`]) and removes nothing. A copy that is cancelled or
/// fails part-way removes the entries it created and only those, so a half
/// copy is never left behind claiming to be complete, and a name another
/// writer took in the meantime is reported, never deleted. Symlinks are copied
/// as links, never followed. `progress` receives the running total after each
/// chunk and item.
pub fn copy(
    source: &Path,
    into_dir: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf, OpError> {
    let destination = plan_destination(source, into_dir)?;
    copy_to(
        source,
        &destination,
        cancellation,
        progress,
        Durability::Buffered,
    )?;
    Ok(destination)
}

/// Copies `source` onto an exact `destination` path (not into a directory),
/// choosing the target name explicitly. This is conflict resolution's "keep
/// both": the caller supplies a freed name so the copy lands beside — never on
/// top of — the entry it collided with. Refuses an existing destination and one
/// that lies inside the source, exactly like [`copy`].
pub fn copy_as(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    if let Some(parent) = destination.parent() {
        guard_not_inside(source, parent, destination)?;
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(OpError::AlreadyExists {
                path: destination.to_path_buf(),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(OpError::io(destination, &error)),
    }
    copy_to(
        source,
        destination,
        cancellation,
        progress,
        Durability::Buffered,
    )
    .map(drop)
}

/// Whether a copy must be on disk, not only in the page cache, before it
/// reports success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Durability {
    /// A plain copy: its source still exists, so a copy lost to a power cut
    /// loses nothing that was not also elsewhere.
    Buffered,
    /// The copy half of a move: its source is about to be removed, so every
    /// file, every directory and the new top-level name are synced first.
    Synced,
}

/// Copies `source` onto the exact, non-existent path `destination`, and
/// returns the journal of what it created.
///
/// On any failure the journal's entries are removed, newest first, and the
/// error is returned; nothing the copy did not create is touched. When the
/// destination name itself is taken the error is
/// [`OpError::AlreadyExists`] and nothing at all is removed. Shared with the
/// cross-device move.
pub(crate) fn copy_to(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
    durability: Durability,
) -> Result<Journal, OpError> {
    let mut context = CopyContext {
        cancel: cancellation,
        progress,
        total: Progress::default(),
        durability,
        journal: Journal::default(),
    };
    let copied = copy_tree(&mut context, source, destination).and_then(|()| {
        if durability == Durability::Synced {
            // The new top-level name lives in its parent directory.
            sync_directory(parent_of(destination))
        } else {
            Ok(())
        }
    });
    match copied {
        Ok(()) => Ok(context.journal),
        Err(error) => {
            context.journal.roll_back();
            Err(error)
        }
    }
}

/// Resolves `into_dir/<source name>` and refuses the two structural hazards: a
/// destination that already exists, and a destination inside the source.
pub(crate) fn plan_destination(source: &Path, into_dir: &Path) -> Result<PathBuf, OpError> {
    let name = source.file_name().ok_or_else(|| OpError::Io {
        path: source.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the source has no file name to copy".to_owned(),
    })?;
    let destination = into_dir.join(name);
    guard_not_inside(source, into_dir, &destination)?;

    match fs::symlink_metadata(&destination) {
        Ok(_) => Err(OpError::AlreadyExists { path: destination }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(OpError::io(&destination, &error)),
    }
}

pub(crate) fn guard_not_inside(
    source: &Path,
    into_dir: &Path,
    destination: &Path,
) -> Result<(), OpError> {
    // Canonical prefix comparison resolves symlinks and `..`; if either path
    // cannot be canonicalized yet, fall back to a lexical check.
    let inside = match (fs::canonicalize(source), fs::canonicalize(into_dir)) {
        (Ok(canon_source), Ok(canon_into)) => canon_into.starts_with(&canon_source),
        _ => into_dir == source || into_dir.starts_with(source),
    };
    if inside {
        return Err(OpError::DestinationInsideSource {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
        });
    }
    Ok(())
}

/// The kind of entry a copy created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKind {
    File,
    Directory,
    Symlink,
}

/// Who an entry is and what it looked like, as far as removing it safely
/// needs to know.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Stamp {
    #[cfg(unix)]
    fn of(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            links: metadata.nlink(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    #[cfg(not(unix))]
    fn of(metadata: &Metadata) -> Self {
        Self {
            device: 0,
            inode: 0,
            length: metadata.len(),
            links: 1,
            modified: (0, 0),
            changed: (0, 0),
        }
    }

    /// Whether `other` is the same entry (device and inode), whatever it
    /// holds now.
    fn same_entry(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }

    /// Whether `other` is the same entry with the same length and the same
    /// modification and change times: nothing was written to it since.
    ///
    /// For an entry with more than one name the change time is not compared:
    /// removing one of its names (which a move does) changes it for all of
    /// them, and says nothing about the content. Length and modification
    /// time still catch a write.
    fn unchanged(&self, other: &Self) -> bool {
        self.same_entry(other)
            && self.length == other.length
            && self.modified == other.modified
            && (self.links > 1 || self.changed == other.changed)
    }
}

/// One entry a copy created.
#[derive(Clone, Debug)]
pub(crate) struct Created {
    pub(crate) kind: EntryKind,
    pub(crate) source: PathBuf,
    pub(crate) destination: PathBuf,
    /// The destination as this copy created it.
    made: Stamp,
    /// The source as this copy read it: for a file, as it was when it was
    /// opened, before its first byte was read, so that any write during the
    /// copy — into a part already read, too — shows as a change.
    read: Stamp,
}

/// Every entry a copy created, in creation order: a folder always comes
/// before its contents.
#[derive(Debug, Default)]
pub(crate) struct Journal {
    entries: Vec<Created>,
}

impl Journal {
    fn record(
        &mut self,
        kind: EntryKind,
        source: &Path,
        destination: &Path,
        made: Stamp,
        read: Stamp,
    ) {
        self.entries.push(Created {
            kind,
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            made,
            read,
        });
    }

    /// Removes, newest first, every destination entry this copy created and
    /// that is still the entry it created. A folder that somebody else put
    /// something into is left in place with that something.
    pub(crate) fn roll_back(&self) {
        for entry in self.entries.iter().rev() {
            let Ok(now) = fs::symlink_metadata(&entry.destination) else {
                continue;
            };
            if !entry.made.same_entry(&Stamp::of(&now)) {
                continue;
            }
            let _ = match entry.kind {
                EntryKind::Directory => fs::remove_dir(&entry.destination),
                EntryKind::File | EntryKind::Symlink => fs::remove_file(&entry.destination),
            };
        }
    }

    /// Removes, newest first, every source entry this copy read, as long as
    /// it is still what was read, and returns the source entries it left:
    /// an entry that changed after it was copied, and an entry that arrived
    /// in a source folder during the copy. A folder that still holds one of
    /// them stays too; the entries themselves are what is reported.
    ///
    /// Called by a move only after the copy is synced and verified.
    pub(crate) fn remove_sources(&self) -> Vec<PathBuf> {
        let mut left_behind = Vec::new();
        // Folders that could not be removed, and files kept on purpose: the
        // listing of a kept folder must not report them a second time.
        let mut kept: Vec<&Path> = Vec::new();
        for entry in self.entries.iter().rev() {
            let now = match fs::symlink_metadata(&entry.source) {
                Ok(now) => Stamp::of(&now),
                // Somebody else removed it already; there is nothing to keep.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => {
                    left_behind.push(entry.source.clone());
                    kept.push(&entry.source);
                    continue;
                }
            };
            match entry.kind {
                EntryKind::File | EntryKind::Symlink => {
                    if !entry.read.unchanged(&now) || remove_entry_file(&entry.source).is_err() {
                        left_behind.push(entry.source.clone());
                        kept.push(&entry.source);
                    }
                }
                EntryKind::Directory => {
                    if !entry.read.same_entry(&now) {
                        // Replaced by a different folder since: not ours.
                        left_behind.push(entry.source.clone());
                        kept.push(&entry.source);
                        continue;
                    }
                    if remove_entry_dir(&entry.source).is_ok() {
                        continue;
                    }
                    kept.push(&entry.source);
                    let before = left_behind.len();
                    if let Ok(children) = fs::read_dir(&entry.source) {
                        for child in children.flatten() {
                            let path = child.path();
                            if !kept.contains(&path.as_path()) {
                                left_behind.push(path);
                            }
                        }
                    }
                    if left_behind.len() == before && !kept_inside(&kept, &entry.source) {
                        // Nothing new inside, yet it would not go: report
                        // the folder itself so the failure is not silent.
                        left_behind.push(entry.source.clone());
                    }
                }
            }
        }
        left_behind
    }
}

/// Whether a kept entry (already reported) sits inside `folder`.
fn kept_inside(kept: &[&Path], folder: &Path) -> bool {
    kept.iter()
        .any(|known| *known != folder && known.starts_with(folder))
}

fn remove_entry_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn remove_entry_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn parent_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Syncs a directory, so the names created in it survive a power loss.
fn sync_directory(directory: &Path) -> Result<(), OpError> {
    File::open(directory)
        .and_then(|handle| handle.sync_all())
        .map_err(|error| OpError::io(directory, &error))
}

/// A create failure, with a taken name reported as the collision it is.
fn create_error(destination: &Path, error: &io::Error) -> OpError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        OpError::AlreadyExists {
            path: destination.to_path_buf(),
        }
    } else {
        OpError::io(destination, error)
    }
}

struct CopyContext<'a> {
    cancel: &'a CancellationToken,
    progress: &'a mut dyn FnMut(Progress),
    total: Progress,
    durability: Durability,
    journal: Journal,
}

impl CopyContext<'_> {
    fn check(&self) -> Result<(), OpError> {
        if self.cancel.is_cancelled() {
            Err(OpError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn add_bytes(&mut self, bytes: u64) {
        self.total.bytes = self.total.bytes.saturating_add(bytes);
        (self.progress)(self.total);
    }

    fn finish_item(&mut self) {
        self.total.items = self.total.items.saturating_add(1);
        (self.progress)(self.total);
    }
}

/// Recursively copies `source` onto the non-existent `destination`, by kind.
fn copy_tree(context: &mut CopyContext, source: &Path, destination: &Path) -> Result<(), OpError> {
    context.check()?;
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            OpError::SourceMissing {
                path: source.to_path_buf(),
            }
        } else {
            OpError::io(source, &error)
        }
    })?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        copy_symlink(context, source, destination, &metadata)
    } else if file_type.is_dir() {
        copy_directory(context, source, destination, &metadata)
    } else if file_type.is_file() {
        copy_file(context, source, destination, &metadata)
    } else {
        Err(OpError::UnsupportedFileType {
            path: source.to_path_buf(),
        })
    }
}

fn copy_directory(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), OpError> {
    fs::create_dir(destination).map_err(|error| create_error(destination, &error))?;
    let made =
        fs::symlink_metadata(destination).map_err(|error| OpError::io(destination, &error))?;
    context.journal.record(
        EntryKind::Directory,
        source,
        destination,
        Stamp::of(&made),
        Stamp::of(metadata),
    );
    context.finish_item();

    for entry in fs::read_dir(source).map_err(|error| OpError::io(source, &error))? {
        context.check()?;
        let entry = entry.map_err(|error| OpError::io(source, &error))?;
        let child_destination = destination.join(entry.file_name());
        copy_tree(context, &entry.path(), &child_destination)?;
    }

    // Sync before the source's permissions are applied: a read-only or
    // unlistable source folder would otherwise refuse the open the sync needs.
    if context.durability == Durability::Synced {
        sync_directory(destination)?;
    }
    // Apply the source's permissions only after the contents are in, so a
    // read-only source directory never blocks writing its children.
    let _ = fs::set_permissions(destination, metadata.permissions());
    Ok(())
}

fn copy_file(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), OpError> {
    let mut reader = File::open(source).map_err(|error| OpError::io(source, &error))?;
    // The source as it is before its first byte is read. Taken from the open
    // file, so it is the very inode being copied; a write at any point after
    // this — even into a region already read — changes what a move compares
    // it with before removing the source.
    let opened = reader
        .metadata()
        .map_err(|error| OpError::io(source, &error))?;
    let mut writer =
        File::create_new(destination).map_err(|error| create_error(destination, &error))?;
    // Journaled before a byte is written, so a failure below removes the
    // partial file.
    let made = writer
        .metadata()
        .map_err(|error| OpError::io(destination, &error))?;
    context.journal.record(
        EntryKind::File,
        source,
        destination,
        Stamp::of(&made),
        Stamp::of(&opened),
    );
    let mut buffer = vec![0u8; CHUNK];

    loop {
        context.check()?;
        let read = reader
            .read(&mut buffer)
            .map_err(|error| OpError::io(source, &error))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| OpError::io(destination, &error))?;
        context.add_bytes(read as u64);
    }

    if context.durability == Durability::Synced {
        writer
            .sync_all()
            .map_err(|error| OpError::io(destination, &error))?;
    }
    drop(writer);
    let _ = fs::set_permissions(destination, metadata.permissions());
    context.finish_item();
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), OpError> {
    let target = fs::read_link(source).map_err(|error| OpError::io(source, &error))?;
    std::os::unix::fs::symlink(&target, destination)
        .map_err(|error| create_error(destination, &error))?;
    let made =
        fs::symlink_metadata(destination).map_err(|error| OpError::io(destination, &error))?;
    context.journal.record(
        EntryKind::Symlink,
        source,
        destination,
        Stamp::of(&made),
        Stamp::of(metadata),
    );
    context.finish_item();
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(
    _context: &mut CopyContext,
    source: &Path,
    _destination: &Path,
    _metadata: &Metadata,
) -> Result<(), OpError> {
    Err(OpError::UnsupportedFileType {
        path: source.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant};

    use celestina_core::CancellationToken;

    use super::{copy_as, copy_to, Durability};
    use crate::error::OpError;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-copyas-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copy_as_places_a_copy_under_the_chosen_name() {
        let dir = TestDir::new("basic");
        let source = dir.path().join("orig.txt");
        fs::write(&source, b"keep both").expect("seed");
        let destination = dir.path().join("orig (copia).txt");

        copy_as(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("copy_as");

        assert_eq!(fs::read(&source).expect("source kept"), b"keep both");
        assert_eq!(fs::read(&destination).expect("copy made"), b"keep both");
    }

    #[test]
    fn copy_as_refuses_an_existing_destination() {
        let dir = TestDir::new("exists");
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("b.txt");
        fs::write(&source, b"a").expect("seed source");
        fs::write(&destination, b"do not clobber").expect("seed dest");

        let error = copy_as(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect_err("must refuse");
        assert!(matches!(error, OpError::AlreadyExists { .. }));
        assert_eq!(
            fs::read(&destination).expect("dest intact"),
            b"do not clobber"
        );
    }

    /// SID-2, deterministically: another writer took the name between the
    /// look and the create. The copy must report the collision and leave the
    /// other writer's file exactly as it was.
    #[test]
    fn a_copy_that_loses_its_name_leaves_the_other_file_alone() {
        let dir = TestDir::new("lost-file");
        let source = dir.path().join("mine.txt");
        fs::write(&source, b"mine").expect("seed source");
        let destination = dir.path().join("taken.txt");
        fs::write(&destination, b"theirs").expect("seed the other writer");

        let error = copy_to(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
            Durability::Buffered,
        )
        .expect_err("the name is taken");

        assert!(
            matches!(error, OpError::AlreadyExists { ref path } if path == &destination),
            "{error:?}"
        );
        assert_eq!(fs::read(&destination).expect("theirs kept"), b"theirs");
    }

    /// The same for a folder: a directory that appeared under the name is not
    /// this copy's to remove, and neither is anything inside it.
    #[test]
    fn a_folder_copy_that_loses_its_name_leaves_the_other_folder_alone() {
        let dir = TestDir::new("lost-folder");
        let source = dir.path().join("mine");
        fs::create_dir(&source).expect("seed source");
        fs::write(source.join("a.txt"), b"a").expect("seed child");
        let destination = dir.path().join("taken");
        fs::create_dir(&destination).expect("seed the other folder");
        fs::write(destination.join("theirs.txt"), b"theirs").expect("seed theirs");

        let error = copy_to(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
            Durability::Buffered,
        )
        .expect_err("the name is taken");

        assert!(matches!(error, OpError::AlreadyExists { .. }), "{error:?}");
        assert_eq!(
            fs::read(destination.join("theirs.txt")).expect("theirs kept"),
            b"theirs"
        );
    }

    /// SID-2 as the audit reproduced it: two copies of one file race for one
    /// name. At most one may win, the loser reports the collision, and a
    /// reported success must leave a complete file behind. The audit saw the
    /// winner's file deleted by the loser's rollback in 160 of 200 rounds.
    #[test]
    fn racing_copies_never_report_success_for_a_file_that_is_gone() {
        const ROUNDS: usize = 200;
        const SIZE: usize = 256 * 1024;
        let dir = TestDir::new("race");
        let source = dir.path().join("source.bin");
        fs::write(&source, vec![7u8; SIZE]).expect("seed source");
        let started = Instant::now();
        let mut lost = Vec::new();
        let mut wrong_errors = Vec::new();

        for round in 0..ROUNDS {
            let destination = dir.path().join(format!("race-{round}.bin"));
            let barrier = Arc::new(Barrier::new(2));
            let racers: Vec<_> = (0..2)
                .map(|_| {
                    let (source, destination, barrier) =
                        (source.clone(), destination.clone(), Arc::clone(&barrier));
                    std::thread::spawn(move || {
                        barrier.wait();
                        copy_as(
                            &source,
                            &destination,
                            &CancellationToken::new(),
                            &mut |_| {},
                        )
                    })
                })
                .collect();
            let results: Vec<Result<(), OpError>> = racers
                .into_iter()
                .map(|racer| racer.join().expect("a racer panicked"))
                .collect();

            let winners = results.iter().filter(|result| result.is_ok()).count();
            assert_eq!(winners, 1, "round {round}: {results:?}");
            if results
                .iter()
                .any(|result| matches!(result, Err(error) if !matches!(error, OpError::AlreadyExists { .. })))
            {
                wrong_errors.push(round);
            }
            let complete = fs::metadata(&destination)
                .map(|metadata| metadata.len() == SIZE as u64)
                .unwrap_or(false);
            if !complete {
                lost.push(round);
            }
            let _ = fs::remove_file(&destination);
        }

        assert!(
            lost.is_empty(),
            "a reported success left no complete file in {} of {ROUNDS} rounds: {lost:?}",
            lost.len()
        );
        assert!(
            wrong_errors.is_empty(),
            "the loser failed with something other than AlreadyExists in rounds {wrong_errors:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "the race took {:?}",
            started.elapsed()
        );
    }
}
