//! Which Trash an entry belongs in.
//!
//! The freedesktop spec puts one Trash in the home directory and one on every
//! other volume, and the difference is not bookkeeping: the home Trash lives on
//! the home filesystem, so trashing a file from another disk into it is a
//! **copy of every byte** onto a disk the person did not choose. A 40 GB folder
//! deleted from an external drive would fill the system disk and take as long as
//! the copy it really is; the same delete into the drive's own Trash is a rename
//! that finishes instantly and leaves the bytes where they were.
//!
//! So the volume is found first, and only an entry that shares the home's
//! filesystem uses the home Trash.
//!
//! The spec offers two homes on a volume, and both are honoured:
//! `$topdir/.Trash/$uid` when the administrator has created a sticky, non-symlink
//! `$topdir/.Trash` for everyone to share, and `$topdir/.Trash-$uid` otherwise —
//! which this crate creates when it is missing, since that is the one a person
//! may make for themselves.
//!
//! A volume can be shared, or written by somebody else before it was plugged
//! in, so a per-user Trash directory is trusted only when it is a real
//! directory (not a symlink) owned by this user — GIO's rule. Anything else —
//! a planted `.Trash-1000` symlink, a directory another user owns — is not
//! this user's Trash: deletions fall back to the home Trash and the listing
//! ignores it. The permission bits are not judged: on a FAT, exFAT or NTFS
//! volume they come from the mount options, which the person cannot change
//! per folder, and a stricter rule there would only turn every delete into a
//! copy and hide the Trash Nautilus wrote.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::error::OpError;
use crate::trashinfo::Unrestorable;

/// Chooses the Trash for `source`: the home one when they share a filesystem,
/// the volume's own otherwise.
///
/// The filesystem is the entry's own, not what it points to: trashing a
/// symlink moves the link, which lives beside its parent's other entries, so a
/// link on `/home` that points at a USB drive belongs in the home Trash.
///
/// A volume whose Trash cannot be created or trusted — a read-only mount, a
/// directory the person cannot write, a `.Trash-$uid` somebody else planted —
/// falls back to the home Trash, because a delete that costs a copy is still
/// better than a delete that fails or lands where another user can read it.
pub(crate) fn trash_home_for(source: &Path) -> Result<PathBuf, OpError> {
    refuse_mount_point(source)?;
    let home = crate::trash::home_trash()?;
    let source_device = entry_device(source);
    let home_device = device_of(existing_ancestor(&home).as_deref().unwrap_or(&home));
    if source_device.is_none() || source_device == home_device {
        return Ok(home);
    }

    // The walk up to the mount point starts from the real folder that holds
    // the entry, so no symlinked ancestor can make it climb another volume.
    let Some(top) = entry_folder(source).and_then(|folder| top_directory(&folder)) else {
        return Ok(home);
    };
    Ok(volume_trash(&top).unwrap_or(home))
}

/// The largest `/proc/self/mountinfo` this reads. A line is about a hundred
/// bytes; a machine with tens of thousands of mounts stays under the bound.
const MAX_MOUNTINFO_BYTES: u64 = 8 * 1024 * 1024;

/// Refuses an entry that is itself where a volume is mounted. Trashing it
/// would copy the whole volume into the home Trash and then empty the volume.
///
/// Only a directory can be a mount point this matters for, and the device
/// cannot tell: on overlayfs a file reports its layer's device while its
/// folder reports the overlay's, a btrfs subvolume has a device of its own
/// without being mounted anywhere, and a bind mount of a folder from the same
/// filesystem has its parent's device. So a directory is refused when
/// `/proc/self/mountinfo` lists it as a mount point; only when that table
/// cannot be read does a device different from its folder's decide. The table
/// is read once per call, that is once per entry sent to the Trash, and only
/// for a directory.
fn refuse_mount_point(source: &Path) -> Result<(), OpError> {
    let Ok(data) = fs::symlink_metadata(source) else {
        return Ok(());
    };
    let folder = entry_folder(source);
    let entry = entry_device(source);
    let folder_device = folder.as_deref().and_then(device_of);
    let canonical = folder
        .zip(source.file_name())
        .map(|(folder, name)| folder.join(name));
    let listed = || canonical.as_deref().and_then(listed_in_mountinfo);
    if is_mount_point(data.is_dir(), entry, folder_device, listed) {
        Err(OpError::MountPoint {
            path: source.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

/// The decision behind [`refuse_mount_point`], apart from the filesystem so
/// each case can be tested: `listed` answers whether the mount table names
/// the entry, or `None` when the table cannot be read, and is asked for every
/// directory and never for anything else.
fn is_mount_point(
    is_directory: bool,
    entry_device: Option<u64>,
    folder_device: Option<u64>,
    listed: impl FnOnce() -> Option<bool>,
) -> bool {
    if !is_directory {
        return false;
    }
    match listed() {
        Some(listed) => listed,
        None => matches!(
            (entry_device, folder_device),
            (Some(entry), Some(folder)) if entry != folder
        ),
    }
}

/// Whether `/proc/self/mountinfo` lists `path` as a mount point, or `None`
/// when it cannot be read.
fn listed_in_mountinfo(path: &Path) -> Option<bool> {
    let table = celestina_core::atomic_file::read_bounded(
        Path::new("/proc/self/mountinfo"),
        MAX_MOUNTINFO_BYTES,
    )
    .ok()??;
    Some(mountinfo_lists(&table, path))
}

/// Whether a `mountinfo` table has a line whose mount point (the fifth
/// field, octal escapes undone) is `path`.
fn mountinfo_lists(table: &[u8], path: &Path) -> bool {
    table
        .split(|&byte| byte == b'\n')
        .filter_map(|line| line.split(|&byte| byte == b' ').nth(4))
        .any(|field| unescape_mount_field(field) == path)
}

/// The Trash directory on this volume, creating `.Trash-$uid` when needed, or
/// `None` when neither per-user Trash on it can be created and trusted.
fn volume_trash(top: &Path) -> Option<PathBuf> {
    let uid = current_uid()?;
    let shared = top.join(".Trash");
    if is_shared_trash(&shared) {
        let mine = shared.join(uid.to_string());
        if ensure_user_trash(&mine, uid) {
            return Some(mine);
        }
    }
    let own = top.join(format!(".Trash-{uid}"));
    ensure_user_trash(&own, uid).then_some(own)
}

/// Creates `path` as a private directory when it is missing, and answers
/// whether it is then a Trash this user can trust (see [`is_user_trash`]).
///
/// Creation is exclusive and one level only: an existing entry, a symlink
/// included, is never created through, only inspected.
#[cfg(unix)]
fn ensure_user_trash(path: &Path, uid: u32) -> bool {
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return false,
    }
    is_user_trash(path, uid)
}

#[cfg(not(unix))]
fn ensure_user_trash(_path: &Path, _uid: u32) -> bool {
    false
}

/// Whether `path` is a per-user Trash directory this user can trust: a real
/// directory, not a symlink, owned by `uid` (GIO's rule; see the module
/// documentation for why the permission bits are not judged).
#[cfg(unix)]
fn is_user_trash(path: &Path, uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(data) = fs::symlink_metadata(path) else {
        return false;
    };
    data.file_type().is_dir() && data.uid() == uid
}

#[cfg(not(unix))]
fn is_user_trash(_path: &Path, _uid: u32) -> bool {
    false
}

/// Whether `$topdir/.Trash` is the shared Trash the spec describes: a real
/// directory, not a symlink, with the sticky bit set.
///
/// The three conditions are the spec's own, and they are a security rule rather
/// than a formality: without the sticky bit any user could remove another's
/// trashed files, and a symlink could point the whole volume's Trash anywhere.
fn is_shared_trash(path: &Path) -> bool {
    let Ok(data) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !data.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        data.permissions().mode() & 0o1000 != 0
    }
    #[cfg(not(unix))]
    false
}

/// The mount point `path` lives on: the highest ancestor still on the same
/// filesystem.
///
/// Read from the entries themselves rather than from a mount table, because the
/// question is about the file at hand and `st_dev` answers it exactly,
/// including for a bind mount. Callers pass a canonical folder, so no
/// ancestor on the walk is a symlink.
pub(crate) fn top_directory(path: &Path) -> Option<PathBuf> {
    let start = existing_ancestor(path)?;
    let device = device_of(&start)?;
    let mut top = start.clone();
    let mut cursor = start;
    while let Some(parent) = cursor.parent().map(Path::to_path_buf) {
        if parent == cursor {
            break;
        }
        if device_of(&parent) != Some(device) {
            break;
        }
        top = parent.clone();
        cursor = parent;
    }
    Some(top)
}

/// The canonical folder that holds `source`: its parent with every symlink
/// and `..` resolved, the entry itself left alone.
fn entry_folder(source: &Path) -> Option<PathBuf> {
    let parent = match source.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::canonicalize(parent).ok()
}

/// The nearest ancestor that exists, so a path that has just been removed still
/// answers which volume it was on.
fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut cursor = absolute.as_path();
    loop {
        if cursor.exists() {
            return Some(cursor.to_path_buf());
        }
        cursor = cursor.parent()?;
    }
}

/// The filesystem a folder is on, following a symlink to it.
#[cfg(unix)]
fn device_of(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|data| data.dev())
}

#[cfg(not(unix))]
fn device_of(_path: &Path) -> Option<u64> {
    None
}

/// The filesystem the entry itself is on: a symlink's own, not its target's.
#[cfg(unix)]
fn entry_device(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).ok().map(|data| data.dev())
}

#[cfg(not(unix))]
fn entry_device(_path: &Path) -> Option<u64> {
    None
}

/// This process's user id, or `None` when it cannot be read — in which case no
/// volume Trash is used at all, rather than guessing somebody else's
/// (`.Trash-0` belongs to root).
fn current_uid() -> Option<u32> {
    celestina_core::xdg::effective_uid().ok()
}

/// Every Trash directory that currently exists for this user: the home one and
/// one per mounted volume that this user can trust (see [`is_user_trash`]).
///
/// Mount points come from `/proc/self/mounts`, which is the only place that
/// knows what is mounted right now. A volume with no Trash contributes nothing.
pub(crate) fn all_trash_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(home) = crate::trash::home_trash() {
        roots.push(home);
    }
    let Some(uid) = current_uid() else {
        return roots;
    };
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mounts") else {
        return roots;
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    for line in mounts.lines() {
        let Some(point) = mount_point(line) else {
            continue;
        };
        if seen.contains(&point) {
            continue;
        }
        seen.push(point.clone());
        let own = point.join(format!(".Trash-{uid}"));
        if is_user_trash(&own, uid) && own.join("info").is_dir() {
            roots.push(own);
        }
        let shared = point.join(".Trash");
        let mine = shared.join(uid.to_string());
        if is_shared_trash(&shared) && is_user_trash(&mine, uid) && mine.join("info").is_dir() {
            roots.push(mine);
        }
    }
    roots
}

/// The second field of a `/proc/self/mounts` line, with the octal escapes the
/// kernel writes for spaces and tabs undone.
fn mount_point(line: &str) -> Option<PathBuf> {
    let field = line.split_whitespace().nth(1)?;
    Some(unescape_mount_field(field.as_bytes()))
}

/// A mount-table path field with the kernel's `\ooo` escapes (space, tab,
/// newline, backslash) undone, byte by byte.
fn unescape_mount_field(field: &[u8]) -> PathBuf {
    let mut out = Vec::with_capacity(field.len());
    let mut index = 0;
    while index < field.len() {
        if field[index] == b'\\' {
            let digits = field.get(index + 1..index + 4);
            let value = digits
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 8).ok());
            if let Some(value) = value {
                out.push(value);
                index += 4;
                continue;
            }
        }
        out.push(field[index]);
        index += 1;
    }
    celestina_core::percent::path_from_bytes(&out)
}

/// The directory a relative `Path=` in the Trash rooted at `trash_root` is
/// written from.
///
/// The spec reads a relative path "from the directory in which the trash
/// directory resides": `$topdir` for `$topdir/.Trash-$uid` and
/// `$XDG_DATA_HOME` for the home Trash. For the shared `$topdir/.Trash/$uid`
/// it is `$topdir` too, which is what GIO writes and KIO reads there.
fn record_base(trash_root: &Path) -> Option<&Path> {
    let parent = trash_root.parent()?;
    if parent.file_name() == Some(OsStr::new(".Trash")) {
        return parent.parent();
    }
    Some(parent)
}

/// The top directory of the volume whose Trash is rooted at `trash_root`:
/// `$topdir` for `$topdir/.Trash-$uid` and for `$topdir/.Trash/$uid`, and
/// `None` for the home Trash, which is not a volume's.
fn volume_top(trash_root: &Path) -> Option<&Path> {
    let name = trash_root.file_name()?;
    if name.to_string_lossy().starts_with(".Trash-") {
        return trash_root.parent();
    }
    let parent = trash_root.parent()?;
    if parent.file_name() == Some(OsStr::new(".Trash")) {
        return parent.parent();
    }
    None
}

/// Where a restore may put back the entry that a record in the Trash at
/// `trash_root` describes, or why it may not.
///
/// A volume's Trash was written by whoever had the volume before, so its
/// records are hostile input: restoring one must not create a file anywhere
/// but on that volume. The rules:
///
/// - a relative record may not contain `..` (the spec allows it; nothing
///   that writes a record needs it, and it is the way out of a volume);
/// - the result must be absolute;
/// - in a volume's Trash the result must lie under `$topdir`, judged with the
///   folder that would receive it resolved, so a folder on the volume that
///   is a symlink to somewhere else does not count as inside.
///
/// The home Trash is this user's own and an absolute record there may point
/// anywhere, as it always has.
pub(crate) fn restore_target(recorded: &Path, trash_root: &Path) -> Result<PathBuf, Unrestorable> {
    if !recorded.is_absolute() && has_parent_step(recorded) {
        return Err(Unrestorable::ClimbsOut);
    }
    let original = resolve_original(recorded, trash_root);
    if !original.is_absolute() {
        return Err(Unrestorable::Relative);
    }
    if let Some(top) = volume_top(trash_root) {
        if !lies_under(&original, top) {
            return Err(Unrestorable::OutsideVolume);
        }
    }
    Ok(original)
}

/// The `Path=` to record for `original`, trashed into the Trash rooted at
/// `trash_root`: relative to `$topdir` in a volume's Trash, as GIO and KIO
/// write it, so the record still restores when the volume is mounted
/// somewhere else; absolute in the home Trash. The folder holding `original`
/// is resolved first, so a path reached through a symlink still lands under
/// the volume's real top. An entry that is somehow not under its volume's
/// top keeps its absolute path.
pub(crate) fn recorded_path(original: &Path, trash_root: &Path) -> PathBuf {
    let Some(top) = volume_top(trash_root) else {
        return original.to_path_buf();
    };
    let real = entry_folder(original)
        .zip(original.file_name())
        .map(|(folder, name)| folder.join(name));
    let real_top = fs::canonicalize(top).ok();
    match (real, real_top) {
        (Some(real), Some(real_top)) => match real.strip_prefix(&real_top) {
            Ok(relative) if !relative.as_os_str().is_empty() => relative.to_path_buf(),
            _ => original.to_path_buf(),
        },
        _ => original.to_path_buf(),
    }
}

fn has_parent_step(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

/// Whether `path` lies under `top`, both resolved: the deepest existing
/// ancestor of the folder that would hold `path` is canonicalized and the
/// rest of `path` kept as written, as a restore creates those names there. A
/// symlink anywhere above the missing part is therefore followed, and the
/// part that does not exist yet may not climb with `..`.
fn lies_under(path: &Path, top: &Path) -> bool {
    let Ok(real_top) = fs::canonicalize(top) else {
        return false;
    };
    if path.file_name().is_none() {
        return false;
    }
    let mut existing = path.parent();
    while let Some(ancestor) = existing {
        if let Ok(real) = fs::canonicalize(ancestor) {
            let Ok(rest) = path.strip_prefix(ancestor) else {
                return false;
            };
            return !has_parent_step(rest) && real.join(rest).starts_with(&real_top);
        }
        existing = ancestor.parent();
    }
    false
}

/// The original path a record in the Trash at `trash_root` points at: as
/// written when it is absolute, and resolved against the Trash's base
/// directory (see [`record_base`]) when the record used the relative form.
///
/// The result is relative only when `trash_root` itself was; a caller that
/// moves something there must refuse that.
pub(crate) fn resolve_original(recorded: &Path, trash_root: &Path) -> PathBuf {
    if recorded.is_absolute() {
        return recorded.to_path_buf();
    }
    match record_base(trash_root) {
        Some(base) => base.join(recorded),
        None => recorded.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::{mount_point, resolve_original, top_directory};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_mount_line_gives_its_point_with_escapes_undone() {
        assert_eq!(
            mount_point("/dev/nvme0n1p2 / ext4 rw,relatime 0 0"),
            Some(PathBuf::from("/"))
        );
        assert_eq!(
            mount_point("/dev/sdb1 /run/media/toni/Disco\\040Duro exfat rw 0 0"),
            Some(PathBuf::from("/run/media/toni/Disco Duro"))
        );
        assert_eq!(mount_point(""), None);
    }

    #[test]
    fn the_top_directory_of_a_path_is_a_real_mount_point() {
        // Whatever this checkout is on, walking up from it must stop somewhere
        // that exists and contains it.
        let here = std::env::current_dir().expect("cwd");
        let top = top_directory(&here).expect("top");
        assert!(top.exists());
        assert!(here.starts_with(&top));
    }

    /// The rule this module exists for: an entry on the home filesystem uses
    /// the home Trash, and one on another volume does not.
    #[test]
    fn the_home_filesystem_uses_the_home_trash() {
        let here = std::env::current_dir().expect("cwd");
        let home = super::trash_home_for(&here).expect("home");
        // This checkout and the home Trash share a filesystem in every
        // environment this suite runs in, so the volume path must not be taken.
        let expected = crate::trash::home_trash().expect("home trash");
        assert_eq!(home, expected);
    }

    #[test]
    fn a_relative_record_resolves_against_its_volume() {
        // `$topdir/.Trash-$uid`: relative to `$topdir`.
        assert_eq!(
            resolve_original(
                Path::new("fotos/uno.jpg"),
                Path::new("/run/media/toni/disco/.Trash-1000")
            ),
            PathBuf::from("/run/media/toni/disco/fotos/uno.jpg")
        );
        // `$topdir/.Trash/$uid`: relative to `$topdir` as well.
        assert_eq!(
            resolve_original(
                Path::new("fotos/uno.jpg"),
                Path::new("/run/media/toni/disco/.Trash/1000")
            ),
            PathBuf::from("/run/media/toni/disco/fotos/uno.jpg")
        );
        // The home Trash: relative to the directory it lives in.
        assert_eq!(
            resolve_original(
                Path::new("notas/dos.txt"),
                Path::new("/home/toni/.local/share/Trash")
            ),
            PathBuf::from("/home/toni/.local/share/notas/dos.txt")
        );
        // An absolute record is already the answer, whatever the volume is.
        assert_eq!(
            resolve_original(
                Path::new("/home/toni/uno.txt"),
                Path::new("/run/media/.Trash-1000")
            ),
            PathBuf::from("/home/toni/uno.txt")
        );
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "siderita-volume-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// SID-16: a `.Trash-$uid` that is a symlink is somebody else's pointer,
    /// never this user's Trash; nothing is created through it.
    #[test]
    fn a_symlinked_volume_trash_is_refused() {
        let top = scratch("symlinked-top");
        let elsewhere = scratch("symlinked-elsewhere");
        let uid = celestina_core::xdg::effective_uid().expect("uid");
        std::os::unix::fs::symlink(&elsewhere, top.join(format!(".Trash-{uid}")))
            .expect("plant link");

        let chosen = super::volume_trash(&top);

        assert_eq!(chosen, None, "the planted link was used");
        assert_eq!(
            std::fs::read_dir(&elsewhere).expect("list").count(),
            0,
            "something was created through the link"
        );
        let _ = std::fs::remove_dir_all(&top);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    /// A missing `.Trash-$uid` is created private, and then trusted.
    #[test]
    fn a_missing_volume_trash_is_created_private() {
        use std::os::unix::fs::PermissionsExt;
        let top = scratch("fresh-top");
        let uid = celestina_core::xdg::effective_uid().expect("uid");

        let chosen = super::volume_trash(&top).expect("a fresh Trash is usable");

        assert_eq!(chosen, top.join(format!(".Trash-{uid}")));
        let mode = std::fs::symlink_metadata(&chosen)
            .expect("created")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "{mode:o}");
        let _ = std::fs::remove_dir_all(&top);
    }

    /// Ruling R-A19: GIO's rule. A `.Trash-$uid` this user owns is used
    /// whatever its permission bits, which on a FAT, exFAT or NTFS volume come
    /// from the mount options (`umask=000` makes every folder 0777).
    #[test]
    fn an_owned_volume_trash_is_used_whatever_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let top = scratch("writable-top");
        let uid = celestina_core::xdg::effective_uid().expect("uid");
        let own = top.join(format!(".Trash-{uid}"));
        std::fs::create_dir(&own).expect("mk trash");
        std::fs::set_permissions(&own, std::fs::Permissions::from_mode(0o777)).expect("chmod");

        assert_eq!(super::volume_trash(&top), Some(own.clone()));
        assert!(super::is_user_trash(&own, uid));
        let _ = std::fs::remove_dir_all(&top);
    }

    /// A `.Trash-$uid` another user owns is not this user's Trash. Handing a
    /// directory to another user needs privilege, so without it the case is
    /// not constructible and the test says so instead of passing silently.
    #[test]
    fn a_volume_trash_another_user_owns_is_refused() {
        let top = scratch("foreign-top");
        let uid = celestina_core::xdg::effective_uid().expect("uid");
        let own = top.join(format!(".Trash-{uid}"));
        std::fs::create_dir(&own).expect("mk trash");
        let stranger = uid.wrapping_add(4242);
        if std::os::unix::fs::chown(&own, Some(stranger), None).is_err() {
            eprintln!("skipped: chown to another uid needs privilege");
            let _ = std::fs::remove_dir_all(&top);
            return;
        }

        assert_eq!(super::volume_trash(&top), None);
        assert!(!super::is_user_trash(&own, uid));
        let _ = std::fs::remove_dir_all(&top);
    }

    /// Inside a proper shared `.Trash`, a `$uid` entry that is a symlink is
    /// refused too, and the private `.Trash-$uid` is used instead.
    #[test]
    fn a_symlinked_uid_in_the_shared_trash_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let top = scratch("shared-top");
        let elsewhere = scratch("shared-elsewhere");
        let uid = celestina_core::xdg::effective_uid().expect("uid");
        let shared = top.join(".Trash");
        std::fs::create_dir(&shared).expect("mk shared");
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).expect("sticky");
        std::os::unix::fs::symlink(&elsewhere, shared.join(uid.to_string())).expect("plant link");

        let chosen = super::volume_trash(&top);

        assert_eq!(chosen, Some(top.join(format!(".Trash-{uid}"))));
        assert_eq!(std::fs::read_dir(&elsewhere).expect("list").count(), 0);
        let _ = std::fs::remove_dir_all(&top);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    /// Review M-5: the entry where a volume is mounted is refused, instead of
    /// copying the whole volume into the home Trash and emptying it. `/proc`
    /// is on its own filesystem under `/` on every Linux this runs on; if it
    /// is not, the case cannot be built without privilege and the test says
    /// so.
    #[test]
    fn a_mount_point_is_not_sent_to_the_trash() {
        let proc_dir = Path::new("/proc");
        let (Some(entry), Some(folder)) = (
            super::entry_device(proc_dir),
            super::device_of(Path::new("/")),
        ) else {
            eprintln!("skipped: no /proc to stand in for a mount point");
            return;
        };
        if entry == folder {
            eprintln!("skipped: /proc is not a separate filesystem here");
            return;
        }

        let refused = super::refuse_mount_point(proc_dir);
        assert!(
            matches!(refused, Err(crate::error::OpError::MountPoint { ref path }) if path == proc_dir),
            "{refused:?}"
        );
        // An ordinary entry beside it is not refused.
        let here = std::env::current_dir().expect("cwd");
        assert!(super::refuse_mount_point(&here.join("Cargo.toml")).is_ok());
        // And the Trash choice stops there, before it creates anything.
        let chosen = super::trash_home_for(proc_dir);
        assert!(
            matches!(chosen, Err(crate::error::OpError::MountPoint { .. })),
            "{chosen:?}"
        );
    }

    /// Review round 2, N-1: a regular file whose device differs from its
    /// folder's (overlayfs reports the file's layer) is never a mount point,
    /// and the mount table is not even asked.
    #[test]
    fn a_file_on_another_device_is_not_a_mount_point() {
        let asked = std::cell::Cell::new(false);
        let listed = || {
            asked.set(true);
            Some(true)
        };
        assert!(!super::is_mount_point(false, Some(44), Some(42), listed));
        assert!(!asked.get());
    }

    /// A directory with its own device that the mount table does not list (a
    /// btrfs subvolume, an overlay directory) is not a mount point either;
    /// one it lists is, and so is any when the table cannot be read.
    #[test]
    fn a_directory_is_a_mount_point_only_when_the_table_says_so() {
        assert!(!super::is_mount_point(true, Some(44), Some(42), || {
            Some(false)
        }));
        assert!(super::is_mount_point(true, Some(44), Some(42), || Some(
            true
        )));
        // Without the table, a device mismatch still decides.
        assert!(super::is_mount_point(true, Some(44), Some(42), || None));
        assert!(!super::is_mount_point(true, Some(42), Some(42), || None));
    }

    /// Review round 3: a bind mount of a folder from the same filesystem has
    /// its folder's device, and the mount table is what tells it apart.
    #[test]
    fn a_same_device_bind_mount_is_a_mount_point() {
        let table = b"36 35 253:0 /other /srv/x rw,relatime - ext4 /dev/vda rw\n";
        assert!(super::is_mount_point(true, Some(42), Some(42), || {
            Some(super::mountinfo_lists(table, Path::new("/srv/x")))
        }));
        assert!(!super::is_mount_point(true, Some(42), Some(42), || {
            Some(super::mountinfo_lists(table, Path::new("/srv/y")))
        }));
    }

    /// Review round 3, cosmetic: a restore target whose folder is missing is
    /// judged from its deepest existing ancestor, so a symlink out of the
    /// volume above the missing part is still seen.
    #[test]
    fn a_missing_folder_behind_a_symlink_out_is_not_under_the_volume() {
        let top = scratch("deep-top");
        let outside = scratch("deep-outside");
        std::os::unix::fs::symlink(&outside, top.join("fotos")).expect("plant link");

        assert!(!super::lies_under(&top.join("fotos/sub/x"), &top));
        assert!(super::lies_under(&top.join("docs/sub/x"), &top));
        let _ = std::fs::remove_dir_all(&top);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn mountinfo_names_its_mount_points_with_escapes_undone() {
        let table = b"22 1 0:5 / /proc rw,nosuid shared:12 - proc proc rw\n\
                      36 35 8:17 / /run/media/toni/Disco\\040Duro rw - exfat /dev/sdb1 rw\n";
        assert!(super::mountinfo_lists(table, Path::new("/proc")));
        assert!(super::mountinfo_lists(
            table,
            Path::new("/run/media/toni/Disco Duro")
        ));
        assert!(!super::mountinfo_lists(table, Path::new("/run/media/toni")));
        assert_eq!(
            super::unescape_mount_field(b"/a\\134b\\011c\\"),
            PathBuf::from("/a\\b\tc\\")
        );
    }
}
