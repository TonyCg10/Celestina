//! Which mounted filesystems the storage section offers as a starting point.
//!
//! The kernel's `/proc/self/mounts` lists every mount, most of them virtual.
//! Only filesystems that hold a person's files are kept, and of those only the
//! ones mounted where a person looks for disks.

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// Filesystem types that store files on a device.
pub const SHOWN_FILESYSTEMS: &[&str] = &[
    "btrfs", "ext4", "ext3", "ext2", "xfs", "f2fs", "vfat", "exfat", "ntfs", "ntfs3", "fuseblk",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    pub source: String,
    pub target: PathBuf,
    pub fstype: String,
}

/// Parses `/proc/self/mounts` text, unescaping the kernel's octal escapes
/// (`\040` space, `\011` tab, `\012` newline, `\134` backslash), and keeps
/// only mounts of a [`SHOWN_FILESYSTEMS`] type. Malformed lines are skipped.
#[must_use]
pub fn parse_mounts(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let source = fields.next()?;
            let target = fields.next()?;
            let fstype = fields.next()?;
            if !SHOWN_FILESYSTEMS.contains(&fstype) {
                return None;
            }
            Some(Mount {
                source: String::from_utf8_lossy(&unescape(source)).into_owned(),
                // A mount point's bytes are kept exactly: the path is opened
                // later, and a lossy name would open something else.
                target: PathBuf::from(OsString::from_vec(unescape(target))),
                fstype: fstype.to_owned(),
            })
        })
        .collect()
}

/// Decodes the kernel's `\NNN` octal escapes; anything else passes through.
fn unescape(field: &str) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let digits = &bytes[i + 1..i + 4];
            if digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                if let Ok(byte) = u8::try_from(value) {
                    out.push(byte);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// `/`, `/home`, or a mount under `/mnt`, `/run/media` or `/media`.
#[must_use]
pub fn is_shown_location(mount: &Mount) -> bool {
    let target = mount.target.as_path();
    target == Path::new("/")
        || target == Path::new("/home")
        || ["/mnt", "/run/media", "/media"]
            .iter()
            .any(|base| target.starts_with(base))
}

/// One line of `/proc/self/mountinfo`: unlike `/proc/self/mounts`, it names
/// every mount point including bind mounts and btrfs subvolumes mounted from
/// the same device, which share an `st_dev` with their parent and so cannot be
/// told apart by the device number alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountPoint {
    pub id: u32,
    pub parent: u32,
    pub target: PathBuf,
    pub fstype: String,
    pub source: String,
}

/// Parses `/proc/self/mountinfo` text (`id parent major:minor root target
/// options [optional...] - fstype source superoptions`), decoding the octal
/// escapes; malformed lines are skipped. Every filesystem type is kept: a
/// boundary is a boundary whatever is mounted there.
#[must_use]
pub fn parse_mountinfo(text: &str) -> Vec<MountPoint> {
    text.lines()
        .filter_map(|line| {
            let (head, tail) = line.split_once(" - ")?;
            let mut head = head.split_whitespace();
            let id = head.next()?.parse().ok()?;
            let parent = head.next()?.parse().ok()?;
            let _device = head.next()?;
            let _root = head.next()?;
            let target = head.next()?;
            let mut tail = tail.split_whitespace();
            let fstype = tail.next()?;
            let source = tail.next()?;
            Some(MountPoint {
                id,
                parent,
                target: PathBuf::from(OsString::from_vec(unescape(target))),
                fstype: fstype.to_owned(),
                source: String::from_utf8_lossy(&unescape(source)).into_owned(),
            })
        })
        .collect()
}

/// The set of mount targets, for [`crate::usage::walk::scan`] and
/// [`crate::usage::remove::delete_tree`] to treat as boundaries.
#[must_use]
pub fn mount_targets(mounts: &[MountPoint]) -> HashSet<PathBuf> {
    mounts.iter().map(|m| m.target.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::{is_shown_location, mount_targets, parse_mountinfo, parse_mounts, Mount};
    use std::path::Path;

    /// The first lines of the author's `/proc/self/mounts` (2026-09-23), plus
    /// one USB label with a space.
    const MOUNTS: &str = "\
/dev/nvme1n1p2 / btrfs rw,noatime,compress=zstd:1,ssd,discard=async,space_cache=v2,subvolid=256,subvol=/@ 0 0
devtmpfs /dev devtmpfs rw,nosuid,size=32625660k,nr_inodes=8156415,mode=755,inode64,huge=advise 0 0
tmpfs /dev/shm tmpfs rw,nosuid,nodev,inode64,huge=advise,usrquota 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/nvme1n1p2 /home btrfs rw,noatime,compress=zstd:1,ssd,discard=async,space_cache=v2,subvolid=257,subvol=/@home 0 0
tmpfs /tmp tmpfs rw,noatime,inode64,huge=advise 0 0
/dev/nvme1n1p2 /var/cache btrfs rw,noatime,compress=zstd:1,ssd,discard=async,space_cache=v2,subvolid=260,subvol=/@cache 0 0
/dev/nvme1n1p1 /boot/efi vfat rw,relatime,fmask=0077,dmask=0077,codepage=437,iocharset=ascii,shortname=mixed,utf8,errors=remount-ro 0 0
/dev/nvme0n1p1 /mnt/samsung990 btrfs rw,noatime,compress=zstd:3,ssd,discard=async,space_cache=v2,subvolid=5,subvol=/ 0 0
/dev/sdb1 /mnt/wd4t ntfs3 rw,relatime,uid=1000,gid=1000,dmask=0022,fmask=0022,acl,iocharset=utf8,prealloc 0 0
gvfsd-fuse /run/user/1000/gvfs fuse.gvfsd-fuse rw,nosuid,nodev,relatime,user_id=1000,group_id=1000 0 0
/dev/sdc1 /run/media/toni/My\\040Disk\\134x exfat rw,relatime 0 0
broken-line-without-fields
";

    fn target(mount: &Mount) -> &Path {
        &mount.target
    }

    #[test]
    fn only_file_holding_filesystems_are_kept() {
        let mounts = parse_mounts(MOUNTS);
        let targets: Vec<_> = mounts.iter().map(target).collect();
        assert_eq!(
            targets,
            [
                "/",
                "/home",
                "/var/cache",
                "/boot/efi",
                "/mnt/samsung990",
                "/mnt/wd4t",
                "/run/media/toni/My Disk\\x",
            ]
            .map(Path::new)
        );
        assert_eq!(mounts[0].source, "/dev/nvme1n1p2");
        assert_eq!(mounts[3].fstype, "vfat");
    }

    #[test]
    fn escapes_are_decoded() {
        let mounts = parse_mounts("a /x\\011y\\012z ext4 rw 0 0\n");
        assert_eq!(mounts[0].target, Path::new("/x\ty\nz"));
    }

    #[test]
    fn only_places_a_person_looks_for_disks_are_shown() {
        let shown: Vec<_> = parse_mounts(MOUNTS)
            .into_iter()
            .filter(is_shown_location)
            .map(|m| m.target)
            .collect();
        assert_eq!(
            shown,
            [
                "/",
                "/home",
                "/mnt/samsung990",
                "/mnt/wd4t",
                "/run/media/toni/My Disk\\x"
            ]
            .map(Path::new)
        );
        let lookalike = Mount {
            source: "x".into(),
            target: "/mntx/disk".into(),
            fstype: "ext4".into(),
        };
        assert!(!is_shown_location(&lookalike));
    }

    /// Lines of the author's `/proc/self/mountinfo` shape: `/home` is the
    /// same device as `/` (a btrfs subvolume), a bind mount repeats a
    /// device, and a USB label carries a space.
    const MOUNTINFO: &str = "\
25 1 0:25 /@ / rw,noatime shared:1 - btrfs /dev/nvme1n1p2 rw,compress=zstd:1
26 25 0:5 / /dev rw,nosuid shared:2 - devtmpfs devtmpfs rw
60 25 0:25 /@home /home rw,noatime shared:30 - btrfs /dev/nvme1n1p2 rw,compress=zstd:1
61 60 0:25 /@home/toni/data /srv/data rw,noatime - btrfs /dev/nvme1n1p2 rw
90 25 8:33 / /run/media/toni/My\\040Disk rw,relatime shared:50 master:3 - exfat /dev/sdc1 rw
garbage line
27 25 0:6 / /broken rw - onlytype
";

    #[test]
    fn mountinfo_names_every_mount_point() {
        let mounts = parse_mountinfo(MOUNTINFO);
        assert_eq!(mounts.len(), 5);
        assert_eq!(mounts[0].id, 25);
        assert_eq!(mounts[0].parent, 1);
        assert_eq!(mounts[0].fstype, "btrfs");
        assert_eq!(mounts[0].source, "/dev/nvme1n1p2");
        assert_eq!(mounts[2].target, Path::new("/home"));
        assert_eq!(mounts[3].parent, 60);
        assert_eq!(mounts[4].target, Path::new("/run/media/toni/My Disk"));
        assert_eq!(mounts[4].fstype, "exfat");
        let targets = mount_targets(&mounts);
        assert!(targets.contains(Path::new("/srv/data")));
        assert!(targets.contains(Path::new("/dev")));
        assert_eq!(targets.len(), 5);
    }
}
