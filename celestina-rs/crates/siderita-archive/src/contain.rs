//! The extraction root, and the one rule every write into it obeys: nothing is
//! written through a link, and no link the extraction leaves behind resolves
//! outside the root.
//!
//! [`crate::member::safe_relative`] validates a stored name as *text*, which is
//! necessary and not sufficient. Text cannot see that `esc`, one member earlier,
//! became a link to `d/up/..`, nor that `d/up` is itself a link to `..`: every
//! hop looks inside and the chain still leaves (SID-1 of the 2026-09-26 audit).
//! So every path below the root is walked on the real filesystem, one component
//! at a time:
//!
//! - every write happens *through an open folder*, never through a name looked
//!   up again ([`Folder`]); a folder a member needs is created only where
//!   nothing exists, and an existing component is entered only when it is a
//!   real folder, never a link ([`Root::make_dirs`], [`Root::place`]);
//! - a link target is resolved the way the kernel resolves it — each link met
//!   on the way is expanded, `..` climbs the real parent — and refused the
//!   moment it would climb above the root ([`Root::link_stays_inside`]);
//! - a finished tree that some other writer produced is checked the same way,
//!   plus the one thing a hard link can hide: a file shared with an inode
//!   outside the tree ([`Root::verify_tree`]).
//!
//! ## Why folders are held open
//!
//! Checking a name and then using it leaves a window: between the two, another
//! process can replace a checked folder with a link, and the use follows it.
//! A test that swaps a folder for a link in a loop wrote outside the root in a
//! handful of trials when each write re-resolved its path as text. So each
//! component is entered by *opening* it — `O_DIRECTORY | O_NOFOLLOW`, so a
//! link or a FIFO in its place fails the open instead of being followed or
//! blocking — and the opened handle's `(device, inode)` is compared with the
//! `lstat` that approved it. Children are then made through
//! `/proc/self/fd/<fd>/<name>`: the kernel resolves that prefix to the folder
//! the descriptor holds, whatever its name has become, and the final `<name>`
//! is created with calls that never follow a link there (`O_CREAT | O_EXCL`,
//! `mkdir`, `symlink`, `link`). This is `openat` and its siblings spelled with
//! the standard library only, since the crate takes no system-call dependency.
//!
//! What stays open: a folder the extraction already entered can be *moved*
//! out of the root by another process while it is being filled, and what is
//! written lands where the folder went. That process moved the folder; the
//! archive did not steer anything.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

use siderita_ops::OpError;

use crate::error::ArchiveError;

/// How many links one resolution may expand before it is refused, as the
/// kernel's own `ELOOP` limit.
const MAX_HOPS: u32 = 40;

/// `O_DIRECTORY | O_NOFOLLOW` for this architecture, where the crate knows it.
/// The two flags are the same everywhere except on the ARM and PowerPC
/// families, where Linux keeps older values; Android shares the kernel's ABI.
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const FOLDER_ONLY: Option<i32> = Some(0o40000 | 0o100000);
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc64"
    )
))]
const FOLDER_ONLY: Option<i32> = Some(0o200000 | 0o400000);
/// Anywhere else the flags are not known, and a folder that cannot be opened
/// without following a link is not opened at all: every extraction there
/// fails with an "unsupported" error before it writes.
#[cfg(not(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc64"
    )
)))]
const FOLDER_ONLY: Option<i32> = None;

/// A folder below the root, held open for as long as something is written in
/// it.
pub(crate) struct Folder {
    handle: File,
    /// Where it was when it was opened, for messages a person reads.
    shown: PathBuf,
}

impl Folder {
    /// Opens `path`, which `approved` (an `lstat` of it) said is a real folder,
    /// and proves the handle is that very folder.
    fn open(
        path: &Path,
        approved: &fs::Metadata,
        shown: PathBuf,
        member: &Path,
    ) -> Result<Self, ArchiveError> {
        let handle = match open_folder(path) {
            Ok(handle) => handle,
            Err(error) => {
                // Refused by `O_NOFOLLOW` or `O_DIRECTORY`: something that is
                // not the approved folder took its place.
                let now = fs::symlink_metadata(path);
                if error.kind() == io::ErrorKind::NotADirectory
                    || now.map(|data| !data.file_type().is_dir()).unwrap_or(false)
                {
                    return Err(unsafe_member(member));
                }
                return Err(OpError::io(&shown, &error).into());
            }
        };
        let opened = handle
            .metadata()
            .map_err(|error| OpError::io(&shown, &error))?;
        if !opened.is_dir() || identity(&opened) != identity(approved) {
            return Err(unsafe_member(member));
        }
        Ok(Self { handle, shown })
    }

    /// The path through which the kernel reaches `name` inside *this* folder,
    /// whatever the folder's own name has become since it was opened.
    #[cfg(unix)]
    fn entry(&self, name: &OsStr) -> PathBuf {
        use std::os::fd::AsRawFd;
        let mut path = PathBuf::from("/proc/self/fd");
        path.push(self.handle.as_raw_fd().to_string());
        path.push(name);
        path
    }

    #[cfg(not(unix))]
    fn entry(&self, name: &OsStr) -> PathBuf {
        self.shown.join(name)
    }

    fn identity(&self) -> Result<(u64, u64), ArchiveError> {
        let data = self
            .handle
            .metadata()
            .map_err(|error| OpError::io(&self.shown, &error))?;
        Ok(identity(&data))
    }
}

#[cfg(unix)]
fn open_folder(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(flags) = FOLDER_ONLY else {
        return Err(io::Error::from(io::ErrorKind::Unsupported));
    };
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
}

#[cfg(not(unix))]
fn open_folder(_path: &Path) -> io::Result<File> {
    let _ = FOLDER_ONLY;
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// The `(device, inode)` pair that names one file whatever path reaches it.
#[cfg(unix)]
fn identity(data: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (data.dev(), data.ino())
}

#[cfg(not(unix))]
fn identity(_data: &fs::Metadata) -> (u64, u64) {
    (0, 0)
}

/// One name inside an open folder: where a member is created.
///
/// The folder stays open for as long as this value lives, so the name keeps
/// meaning "inside the folder that was checked" however the tree around it
/// changes.
pub(crate) struct Placed {
    folder: Folder,
    name: OsString,
}

impl Placed {
    /// The path the kernel resolves through the held folder; for system calls.
    pub(crate) fn path(&self) -> PathBuf {
        self.folder.entry(&self.name)
    }

    /// Where the entry is, for messages a person reads.
    pub(crate) fn shown(&self) -> PathBuf {
        self.folder.shown.join(&self.name)
    }

    /// Whether this is the same name in the same folder as `other`.
    pub(crate) fn same_entry(&self, other: &Placed) -> Result<bool, ArchiveError> {
        Ok(self.name == other.name && self.folder.identity()? == other.folder.identity()?)
    }
}

/// The folder an extraction writes into, held open by its canonical path.
pub(crate) struct Root {
    real: PathBuf,
    top: Folder,
    id: (u64, u64),
}

impl Root {
    /// Resolves `folder`, which must exist, to its canonical path and holds it
    /// open.
    ///
    /// The folder the person chose may itself be reached through a link; that is
    /// their choice and is resolved once here. Everything *below* it is the
    /// archive's, and is never allowed to be a link that is followed.
    pub(crate) fn open(folder: &Path) -> Result<Self, ArchiveError> {
        let real = fs::canonicalize(folder).map_err(|error| OpError::io(folder, &error))?;
        let approved = fs::symlink_metadata(&real).map_err(|error| OpError::io(&real, &error))?;
        if !approved.is_dir() {
            return Err(OpError::io(&real, &io::Error::from(io::ErrorKind::NotADirectory)).into());
        }
        let top = Folder::open(&real, &approved, real.clone(), Path::new("."))?;
        Ok(Self {
            real,
            top,
            id: identity(&approved),
        })
    }

    /// The canonical path of the root.
    pub(crate) fn path(&self) -> &Path {
        &self.real
    }

    /// The root folder, for one walk. Its name is checked to still be the
    /// folder that was opened: a root that was moved or replaced is not
    /// written into any more.
    fn top(&self) -> Result<Folder, ArchiveError> {
        let now =
            fs::symlink_metadata(&self.real).map_err(|error| OpError::io(&self.real, &error))?;
        if now.file_type().is_symlink() || identity(&now) != self.id {
            return Err(unsafe_member(Path::new(".")));
        }
        let handle = self
            .top
            .handle
            .try_clone()
            .map_err(|error| OpError::io(&self.real, &error))?;
        Ok(Folder {
            handle,
            shown: self.real.clone(),
        })
    }

    /// Walks `relative` from the root through real folders, each held open,
    /// and answers the last one. With `create`, a missing folder is made; without
    /// it, a missing or non-folder component answers `None`. A link on the way
    /// is always an [`ArchiveError::UnsafeMember`].
    fn walk(
        &self,
        relative: &Path,
        create: bool,
        member: &Path,
    ) -> Result<Option<Folder>, ArchiveError> {
        let mut folder = self.top()?;
        for component in relative.components() {
            let Component::Normal(part) = component else {
                return Err(unsafe_member(member));
            };
            let entry = folder.entry(part);
            let shown = folder.shown.join(part);
            let data = match fs::symlink_metadata(&entry) {
                Ok(data) => data,
                Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                    match fs::create_dir(&entry) {
                        // Made by someone else between the two calls: the same
                        // rule applies to what is there now.
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(OpError::io(&shown, &error).into()),
                    }
                    fs::symlink_metadata(&entry).map_err(|error| OpError::io(&shown, &error))?
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(OpError::io(&shown, &error).into()),
            };
            let kind = data.file_type();
            if kind.is_symlink() {
                return Err(unsafe_member(member));
            }
            if !kind.is_dir() {
                if !create {
                    return Ok(None);
                }
                return Err(
                    OpError::io(&shown, &io::Error::from(io::ErrorKind::NotADirectory)).into(),
                );
            }
            folder = Folder::open(&entry, &data, shown, member)?;
        }
        Ok(Some(folder))
    }

    /// Makes every folder of `relative` below the root and answers the last
    /// one, held open.
    ///
    /// An existing component is entered only when it is a real folder. A link
    /// is refused as an [`ArchiveError::UnsafeMember`] — writing through it is
    /// how a crafted archive leaves — and a file in the way is the IO failure
    /// "not a directory". `member` is the stored name reported on refusal.
    pub(crate) fn make_dirs(&self, relative: &Path, member: &Path) -> Result<Folder, ArchiveError> {
        self.walk(relative, true, member)?
            .ok_or_else(|| unsafe_member(member))
    }

    /// The name a member takes, inside its folder held open, with every folder
    /// above it made by [`Root::make_dirs`]. The final component itself is left
    /// to the caller, which creates it without following anything already
    /// there.
    pub(crate) fn place(&self, relative: &Path) -> Result<Placed, ArchiveError> {
        let parent = relative.parent().unwrap_or(Path::new(""));
        let folder = self.make_dirs(parent, relative)?;
        let Some(name) = relative.file_name() else {
            return Err(unsafe_member(relative));
        };
        Ok(Placed {
            folder,
            name: name.to_os_string(),
        })
    }

    /// The existing regular file `relative` names, reached only through real
    /// folders, with the `lstat` that found it; `None` when there is no such
    /// file.
    ///
    /// This is what a hard link member may point at. Hard links are made before
    /// any of the archive's symlinks exist, so one that names an archive symlink
    /// (or a folder, or nothing) finds no file here and is skipped by the
    /// caller as unsupported. A link met on the way can only be one something
    /// else put there, and is an [`ArchiveError::UnsafeMember`].
    pub(crate) fn existing_file(
        &self,
        relative: &Path,
        member: &Path,
    ) -> Result<Option<(Placed, fs::Metadata)>, ArchiveError> {
        let parent = relative.parent().unwrap_or(Path::new(""));
        let Some(name) = relative.file_name() else {
            return Ok(None);
        };
        let Some(folder) = self.walk(parent, false, member)? else {
            return Ok(None);
        };
        let placed = Placed {
            folder,
            name: name.to_os_string(),
        };
        let data = match fs::symlink_metadata(placed.path()) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(OpError::io(&placed.shown(), &error).into()),
        };
        if data.file_type().is_symlink() {
            return Err(unsafe_member(member));
        }
        Ok(data.file_type().is_file().then_some((placed, data)))
    }

    /// Whether a link stored at `link` (relative to the root) with `target`
    /// resolves, on the filesystem as it is now, to a place inside the root.
    ///
    /// Resolved as the kernel resolves it: from the link's folder, each link
    /// met on the way is expanded in place and `..` climbs the real parent. A
    /// component that does not exist yet ends the lookup's use of the disk and
    /// the rest is read as text, which is still refused the moment it climbs
    /// above the root. An absolute target, and a chain longer than the kernel
    /// follows, are refused outright.
    ///
    /// This answers what a *reader* of the finished tree will reach; it is not
    /// what keeps writes inside, which never follow a link at all.
    pub(crate) fn link_stays_inside(&self, link: &Path, target: &Path) -> bool {
        let mut position: Vec<OsString> = Vec::new();
        if let Some(parent) = link.parent() {
            for component in parent.components() {
                match component {
                    Component::Normal(part) => position.push(part.to_os_string()),
                    Component::CurDir => {}
                    _ => return false,
                }
            }
        }
        let mut hops = 0;
        self.resolve(&mut position, target, &mut hops)
    }

    /// Walks `target` from `position`, expanding links, and answers whether it
    /// stays inside. `position` is left at the resolved place.
    fn resolve(&self, position: &mut Vec<OsString>, target: &Path, hops: &mut u32) -> bool {
        for component in target.components() {
            match component {
                Component::RootDir | Component::Prefix(_) => return false,
                Component::CurDir => {}
                Component::ParentDir => {
                    if position.pop().is_none() {
                        return false;
                    }
                }
                Component::Normal(part) => {
                    position.push(part.to_os_string());
                    let here = self.join(position);
                    let is_link = fs::symlink_metadata(&here)
                        .map(|data| data.file_type().is_symlink())
                        .unwrap_or(false);
                    if !is_link {
                        continue;
                    }
                    *hops += 1;
                    if *hops > MAX_HOPS {
                        return false;
                    }
                    let Ok(next) = fs::read_link(&here) else {
                        return false;
                    };
                    // A link resolves from the folder that holds it.
                    position.pop();
                    if !self.resolve(position, &next, hops) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn join(&self, position: &[OsString]) -> PathBuf {
        let mut path = self.real.clone();
        path.extend(position);
        path
    }

    /// Checks a finished tree that another writer produced: every symlink must
    /// resolve inside the root, and no file may share its inode with anything
    /// outside it.
    ///
    /// This is the guard for a delegated extraction, where the tool wrote the
    /// tree itself. A hard link to a file outside the root is recognised by its
    /// link count: every name of an inode that the tree holds is counted, and an
    /// inode with more names than that has one elsewhere.
    pub(crate) fn verify_tree(&self) -> Result<(), ArchiveError> {
        let mut shared = Shared::default();
        let mut pending = vec![self.real.clone()];
        while let Some(directory) = pending.pop() {
            let entries =
                fs::read_dir(&directory).map_err(|error| OpError::io(&directory, &error))?;
            for entry in entries {
                let entry = entry.map_err(|error| OpError::io(&directory, &error))?;
                let path = entry.path();
                let kind = entry
                    .file_type()
                    .map_err(|error| OpError::io(&path, &error))?;
                let relative = path.strip_prefix(&self.real).unwrap_or(&path).to_path_buf();
                if kind.is_symlink() {
                    let target =
                        fs::read_link(&path).map_err(|error| OpError::io(&path, &error))?;
                    if !self.link_stays_inside(&relative, &target) {
                        return Err(unsafe_member(&relative));
                    }
                } else if kind.is_dir() {
                    pending.push(path);
                } else if kind.is_file() {
                    let data =
                        fs::symlink_metadata(&path).map_err(|error| OpError::io(&path, &error))?;
                    shared.count(relative, &data);
                }
            }
        }
        shared.none_outside()
    }
}

/// The refusal for a member whose write would leave the root.
pub(crate) fn unsafe_member(member: &Path) -> ArchiveError {
    ArchiveError::UnsafeMember {
        name: member.display().to_string(),
    }
}

/// The names each multiply-linked inode has inside the tree.
#[derive(Default)]
struct Shared {
    inodes: HashMap<(u64, u64), (u64, u64, PathBuf)>,
}

impl Shared {
    #[cfg(unix)]
    fn count(&mut self, relative: PathBuf, data: &fs::Metadata) {
        use std::os::unix::fs::MetadataExt;
        if data.nlink() <= 1 {
            return;
        }
        let seen =
            self.inodes
                .entry((data.dev(), data.ino()))
                .or_insert((data.nlink(), 0, relative));
        seen.1 += 1;
    }

    #[cfg(not(unix))]
    fn count(&mut self, _relative: PathBuf, _data: &fs::Metadata) {}

    fn none_outside(&self) -> Result<(), ArchiveError> {
        match self
            .inodes
            .values()
            .find(|(links, inside, _)| inside < links)
        {
            Some((_, _, name)) => Err(unsafe_member(name)),
            None => Ok(()),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::Root;
    use crate::error::ArchiveError;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-contain-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(path.join("root")).expect("create scratch");
            Self(path)
        }

        fn root(&self) -> Root {
            Root::open(&self.0.join("root")).expect("open root")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_folder_is_never_made_through_a_link() {
        let scratch = Scratch::new("make-dirs");
        let root = scratch.root();
        fs::create_dir(scratch.0.join("outside")).expect("mk outside");
        symlink("../outside", root.path().join("shortcut")).expect("link");

        let refused = root.make_dirs(Path::new("shortcut/inside"), Path::new("shortcut/inside/x"));

        assert!(matches!(refused, Err(ArchiveError::UnsafeMember { .. })));
        assert!(!scratch.0.join("outside/inside").exists());
        let made = root
            .make_dirs(Path::new("a/b"), Path::new("a/b/x"))
            .expect("real folders");
        assert!(made.shown.is_dir() && made.shown.starts_with(root.path()));
    }

    #[test]
    fn a_link_is_resolved_through_every_link_it_meets() {
        let scratch = Scratch::new("resolve");
        let root = scratch.root();
        fs::create_dir(root.path().join("d")).expect("mk d");
        symlink("..", root.path().join("d/up")).expect("d/up");

        // Each hop looks inside as text; resolved, the chain leaves.
        assert!(!root.link_stays_inside(Path::new("esc"), Path::new("d/up/..")));
        assert!(root.link_stays_inside(Path::new("esc"), Path::new("d/up")));
        assert!(root.link_stays_inside(Path::new("d/x"), Path::new("up/d")));
        assert!(!root.link_stays_inside(Path::new("x"), Path::new("/etc/passwd")));
        // A link that does not exist yet is read as text, and still bounded.
        assert!(root.link_stays_inside(Path::new("x"), Path::new("new/../d")));
        assert!(!root.link_stays_inside(Path::new("x"), Path::new("new/../..")));
    }

    #[test]
    fn a_loop_of_links_is_refused_rather_than_followed_forever() {
        let scratch = Scratch::new("loop");
        let root = scratch.root();
        symlink("b", root.path().join("a")).expect("a");
        symlink("a", root.path().join("b")).expect("b");

        assert!(!root.link_stays_inside(Path::new("c"), Path::new("a")));
    }

    /// The delegated tool's result: the same chain the auditor wrote as a tar,
    /// laid down by "the tool", is refused after the fact.
    #[test]
    fn a_tree_a_tool_wrote_is_refused_when_a_link_chain_leaves_it() {
        let scratch = Scratch::new("tool-chain");
        let root = scratch.root();
        fs::create_dir(root.path().join("d")).expect("mk d");
        symlink("..", root.path().join("d/up")).expect("d/up");
        symlink("d/up/..", root.path().join("esc")).expect("esc");

        assert!(matches!(
            root.verify_tree(),
            Err(ArchiveError::UnsafeMember { .. })
        ));
    }

    #[test]
    fn a_tree_a_tool_wrote_is_refused_when_a_file_is_a_hard_link_to_outside() {
        let scratch = Scratch::new("tool-hardlink");
        let root = scratch.root();
        let secret = scratch.0.join("secret");
        fs::write(&secret, b"do not touch").expect("write secret");
        fs::create_dir(root.path().join("d")).expect("mk d");
        fs::hard_link(&secret, root.path().join("d/hard")).expect("hard link");

        assert!(matches!(
            root.verify_tree(),
            Err(ArchiveError::UnsafeMember { .. })
        ));
    }

    #[test]
    fn an_honest_tree_with_links_inside_passes() {
        let scratch = Scratch::new("tool-honest");
        let root = scratch.root();
        fs::create_dir(root.path().join("d")).expect("mk d");
        fs::write(root.path().join("d/one"), b"one").expect("write");
        fs::hard_link(root.path().join("d/one"), root.path().join("two")).expect("hard link");
        symlink("../two", root.path().join("d/upward")).expect("link");
        symlink("d", root.path().join("shortcut")).expect("link");

        root.verify_tree().expect("stays inside");
    }
}
