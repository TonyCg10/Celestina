//! Which file system a path lives on, for the properties dialog's «Formato»
//! row: `NTFS · /dev/sda1`.
//!
//! The answer comes from `/proc/self/mountinfo` — the mount whose mount point
//! is the longest prefix of the entry's real path — so it needs no daemon. A
//! few types say little by themselves (`fuseblk` is how ntfs-3g and exfat-fuse
//! show up, `vfat` covers FAT12/16/32), so for those the udev database that
//! UDisks itself reads (`/run/udev/data/b<major>:<minor>`) is asked for the
//! real `ID_FS_TYPE` / `ID_FS_VERSION`. The key is the source node's own
//! `major:minor` (`stat` of `/dev/sdb1`), because mountinfo's device number of
//! a FUSE mount is anonymous (`0:95`) and has no udev record. Every step is a
//! small file read; the
//! caller runs it off the Qt thread all the same, because a path on a share
//! that stopped answering would block `canonicalize`.
//!
//! Paths are compared as bytes: a mount point is not necessarily UTF-8.

use std::os::unix::ffi::OsStringExt;
use std::path::Path;

/// One line of `/proc/self/mountinfo`, reduced to what the lookup needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mount {
    /// `major:minor` of the mount's own device as mountinfo reports it. For a
    /// FUSE mount this is an anonymous `0:N`, not the drive; see [`udev_key`].
    pub device_id: String,
    /// Where it is mounted, unescaped, as raw bytes.
    pub mount_point: Vec<u8>,
    pub fstype: String,
    /// The device node (`/dev/sda1`), or whatever the kernel reports for a
    /// pseudo or network file system (`tmpfs`, `host:/export`).
    pub source: String,
}

/// What udev knows about the file system on a device.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Probe {
    pub fs_type: Option<String>,
    pub fs_version: Option<String>,
}

/// `«NTFS · /dev/sda1»` for the volume holding `path`, or an empty string when
/// it cannot be told (the path vanished, no mountinfo).
pub fn describe(path: &Path) -> String {
    let Ok(real) = std::fs::canonicalize(path) else {
        return String::new();
    };
    let Ok(text) = std::fs::read("/proc/self/mountinfo") else {
        return String::new();
    };
    let mounts = parse_mountinfo(&text);
    let Some(mount) = owning_mount(&mounts, &real.into_os_string().into_vec()) else {
        return String::new();
    };
    let probe = needs_probe(&mount.fstype).then(|| probe_device(&probe_key(mount)));
    label(mount, probe.as_ref())
}

/// The label for `mount`: the display name, then the device node when the
/// source is one.
pub fn label(mount: &Mount, probe: Option<&Probe>) -> String {
    let name = display_name(&mount.fstype, probe);
    if mount.source.starts_with("/dev/") {
        format!("{name} · {}", mount.source)
    } else {
        name
    }
}

/// Whether `fstype` is vague enough to be worth a udev lookup.
pub fn needs_probe(fstype: &str) -> bool {
    matches!(fstype, "fuseblk" | "vfat" | "msdos")
}

/// The udev key of `mount`: its source node's device number when that can be
/// read, else the mountinfo one.
fn probe_key(mount: &Mount) -> String {
    let rdev = std::fs::metadata(&mount.source)
        .ok()
        .map(|meta| std::os::unix::fs::MetadataExt::rdev(&meta));
    udev_key(&mount.source, rdev, &mount.device_id)
}

/// `major:minor` to look up in udev: from `rdev` of the source node when the
/// source is a `/dev/` node that was statted, else the mountinfo id.
pub fn udev_key(source: &str, rdev: Option<u64>, mountinfo_id: &str) -> String {
    match rdev {
        Some(rdev) if source.starts_with("/dev/") && rdev != 0 => {
            format!("{}:{}", rustix::fs::major(rdev), rustix::fs::minor(rdev))
        }
        _ => mountinfo_id.to_owned(),
    }
}

/// Reads udev's record for a block device (`major:minor`); empty when none.
fn probe_device(device_id: &str) -> Probe {
    std::fs::read_to_string(format!("/run/udev/data/b{device_id}"))
        .map(|text| parse_udev(&text))
        .unwrap_or_default()
}

/// Pulls `ID_FS_TYPE` and `ID_FS_VERSION` out of a udev database record.
pub fn parse_udev(text: &str) -> Probe {
    let mut probe = Probe::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("E:ID_FS_TYPE=") {
            probe.fs_type = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("E:ID_FS_VERSION=") {
            probe.fs_version = Some(value.to_owned());
        }
    }
    probe
}

/// The name a person expects for a kernel file system type.
pub fn display_name(fstype: &str, probe: Option<&Probe>) -> String {
    match fstype {
        "fuseblk" => match probe.and_then(|probe| probe.fs_type.as_deref()) {
            Some(real) if real != "fuseblk" => display_name(real, None),
            _ => "FUSE".to_owned(),
        },
        "vfat" | "msdos" => {
            let fat32 = probe.and_then(|probe| probe.fs_version.as_deref()) == Some("FAT32");
            if fat32 { "FAT32" } else { "FAT" }.to_owned()
        }
        "ntfs" | "ntfs3" | "ntfs-3g" => "NTFS".to_owned(),
        "exfat" => "exFAT".to_owned(),
        "xfs" => "XFS".to_owned(),
        "f2fs" => "F2FS".to_owned(),
        "zfs" => "ZFS".to_owned(),
        "fuse" => "FUSE".to_owned(),
        other => match other.strip_prefix("fuse.") {
            Some(inner) if !inner.is_empty() => display_name(inner, None),
            // btrfs, ext2/3/4, tmpfs, nfs, nfs4, cifs, smb3, sshfs and anything
            // unknown keep the kernel's own spelling.
            _ => other.to_owned(),
        },
    }
}

/// Parses `/proc/self/mountinfo`; lines that do not have the expected shape
/// are skipped.
pub fn parse_mountinfo(text: &[u8]) -> Vec<Mount> {
    text.split(|&byte| byte == b'\n')
        .filter_map(parse_line)
        .collect()
}

fn parse_line(line: &[u8]) -> Option<Mount> {
    let fields: Vec<&[u8]> = line
        .split(|&byte| byte == b' ')
        .filter(|field| !field.is_empty())
        .collect();
    // id parent major:minor root mount-point options [optional...] - fstype source super
    let separator = fields.iter().position(|field| *field == b"-")?;
    if separator < 6 {
        return None;
    }
    let fstype = fields.get(separator + 1)?;
    let source = fields.get(separator + 2)?;
    Some(Mount {
        device_id: String::from_utf8_lossy(fields[2]).into_owned(),
        mount_point: unescape(fields[4]),
        fstype: String::from_utf8_lossy(fstype).into_owned(),
        source: String::from_utf8_lossy(&unescape(source)).into_owned(),
    })
}

/// Undoes mountinfo's `\040`-style octal escapes (space, tab, newline, `\`).
fn unescape(field: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.len());
    let mut index = 0;
    while index < field.len() {
        let octal = (field[index] == b'\\')
            .then(|| field.get(index + 1..index + 4))
            .flatten()
            .filter(|digits| digits.iter().all(|digit| (b'0'..=b'7').contains(digit)))
            .map(|digits| {
                digits
                    .iter()
                    .fold(0u32, |acc, digit| acc * 8 + u32::from(digit - b'0'))
            })
            .and_then(|value| u8::try_from(value).ok());
        match octal {
            Some(byte) => {
                out.push(byte);
                index += 4;
            }
            None => {
                out.push(field[index]);
                index += 1;
            }
        }
    }
    out
}

/// The mount whose mount point is the longest prefix of `path` (on component
/// boundaries). Mounts stacked on one point resolve to the later one, which is
/// the one in effect.
pub fn owning_mount<'a>(mounts: &'a [Mount], path: &[u8]) -> Option<&'a Mount> {
    let mut best: Option<&Mount> = None;
    for mount in mounts {
        if !covers(&mount.mount_point, path) {
            continue;
        }
        if best.is_none_or(|current| mount.mount_point.len() >= current.mount_point.len()) {
            best = Some(mount);
        }
    }
    best
}

fn covers(mount_point: &[u8], path: &[u8]) -> bool {
    let point = mount_point.strip_suffix(b"/").unwrap_or(mount_point);
    match path.strip_prefix(point) {
        Some(rest) => rest.is_empty() || rest.starts_with(b"/"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        covers, display_name, label, needs_probe, owning_mount, parse_mountinfo, parse_udev,
        udev_key, Mount, Probe,
    };
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    const SAMPLE: &[u8] = b"\
22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/@\n\
372 22 8:1 / /mnt/wd4t rw,relatime shared:663 - ntfs3 /dev/sda1 rw,uid=1000\n\
238 22 8:17 / /mnt/samsungT7 rw,relatime shared:203 - exfat /dev/sdb1 rw\n\
300 22 8:33 / /run/media/toni/My\\040Stick rw - vfat /dev/sdc1 rw\n\
301 22 0:50 / /mnt/nas rw - nfs4 nas:/export rw\n\
302 22 0:51 / /tmp rw - tmpfs tmpfs rw\n\
garbage line\n";

    fn mounts() -> Vec<Mount> {
        parse_mountinfo(SAMPLE)
    }

    #[test]
    fn parses_fields_and_skips_garbage() {
        let mounts = mounts();
        assert_eq!(mounts.len(), 6);
        assert_eq!(mounts[1].device_id, "8:1");
        assert_eq!(mounts[1].mount_point, b"/mnt/wd4t");
        assert_eq!(mounts[1].fstype, "ntfs3");
        assert_eq!(mounts[1].source, "/dev/sda1");
    }

    #[test]
    fn optional_fields_do_not_shift_the_type() {
        // Two optional fields (shared:, master:) before the separator.
        let mounts = parse_mountinfo(b"1 0 8:2 / /x rw shared:1 master:2 - ext4 /dev/sdd2 rw\n");
        assert_eq!(mounts[0].fstype, "ext4");
        assert_eq!(mounts[0].source, "/dev/sdd2");
    }

    #[test]
    fn escaped_spaces_in_mount_points_are_decoded() {
        let mounts = mounts();
        assert_eq!(mounts[3].mount_point, b"/run/media/toni/My Stick");
        let found = owning_mount(&mounts, b"/run/media/toni/My Stick/docs/a.txt").unwrap();
        assert_eq!(found.source, "/dev/sdc1");
    }

    #[test]
    fn non_utf8_mount_points_match_byte_for_byte() {
        let mounts = parse_mountinfo(b"1 0 8:2 / /mnt/\xff\\040x rw - ext4 /dev/sdd2 rw\n");
        let path = OsStr::from_bytes(b"/mnt/\xff x/file");
        assert_eq!(
            owning_mount(&mounts, path.as_bytes()).unwrap().source,
            "/dev/sdd2"
        );
    }

    #[test]
    fn the_longest_prefix_wins() {
        let mounts = mounts();
        assert_eq!(
            owning_mount(&mounts, b"/mnt/wd4t/film.mkv").unwrap().fstype,
            "ntfs3"
        );
        assert_eq!(
            owning_mount(&mounts, b"/home/toni").unwrap().fstype,
            "btrfs"
        );
        assert_eq!(owning_mount(&mounts, b"/tmp/x").unwrap().fstype, "tmpfs");
    }

    #[test]
    fn a_sibling_with_a_shared_prefix_is_not_inside_the_mount() {
        let mounts = mounts();
        assert_eq!(
            owning_mount(&mounts, b"/mnt/wd4t2/x").unwrap().fstype,
            "btrfs"
        );
        assert!(covers(b"/mnt/wd4t", b"/mnt/wd4t"));
        assert!(covers(b"/", b"/anything"));
        assert!(!covers(b"/mnt/wd4t", b"/mnt/wd4"));
    }

    #[test]
    fn a_mount_stacked_on_the_same_point_resolves_to_the_later_one() {
        let mounts = parse_mountinfo(
            b"1 0 0:1 / /a rw - ext4 /dev/old rw\n2 1 0:2 / /a rw - xfs /dev/new rw\n",
        );
        assert_eq!(owning_mount(&mounts, b"/a/f").unwrap().source, "/dev/new");
    }

    #[test]
    fn no_mount_means_no_answer() {
        assert!(owning_mount(&[], b"/x").is_none());
    }

    #[test]
    fn names_follow_the_table() {
        let none = |fstype| display_name(fstype, None);
        assert_eq!(none("ntfs3"), "NTFS");
        assert_eq!(none("ntfs"), "NTFS");
        assert_eq!(none("exfat"), "exFAT");
        assert_eq!(none("btrfs"), "btrfs");
        assert_eq!(none("ext4"), "ext4");
        assert_eq!(none("ext3"), "ext3");
        assert_eq!(none("ext2"), "ext2");
        assert_eq!(none("xfs"), "XFS");
        assert_eq!(none("f2fs"), "F2FS");
        assert_eq!(none("zfs"), "ZFS");
        assert_eq!(none("tmpfs"), "tmpfs");
        assert_eq!(none("nfs4"), "nfs4");
        assert_eq!(none("cifs"), "cifs");
        assert_eq!(none("smb3"), "smb3");
        assert_eq!(none("fuse.sshfs"), "sshfs");
        assert_eq!(none("fuse"), "FUSE");
        assert_eq!(none("squashfs"), "squashfs");
    }

    #[test]
    fn fat_is_fat32_only_when_udev_says_so() {
        let fat32 = Probe {
            fs_type: Some("vfat".into()),
            fs_version: Some("FAT32".into()),
        };
        let fat16 = Probe {
            fs_type: Some("vfat".into()),
            fs_version: Some("FAT16".into()),
        };
        assert_eq!(display_name("vfat", Some(&fat32)), "FAT32");
        assert_eq!(display_name("msdos", Some(&fat32)), "FAT32");
        assert_eq!(display_name("vfat", Some(&fat16)), "FAT");
        assert_eq!(display_name("vfat", None), "FAT");
    }

    #[test]
    fn fuseblk_asks_udev_and_falls_back_to_fuse() {
        let ntfs = Probe {
            fs_type: Some("ntfs".into()),
            fs_version: None,
        };
        let exfat = Probe {
            fs_type: Some("exfat".into()),
            fs_version: None,
        };
        assert_eq!(display_name("fuseblk", Some(&ntfs)), "NTFS");
        assert_eq!(display_name("fuseblk", Some(&exfat)), "exFAT");
        assert_eq!(display_name("fuseblk", Some(&Probe::default())), "FUSE");
        assert_eq!(display_name("fuseblk", None), "FUSE");
        assert!(needs_probe("fuseblk") && needs_probe("vfat") && !needs_probe("ext4"));
    }

    #[test]
    fn udev_records_are_read() {
        let probe = parse_udev(
            "S:disk/by-uuid/X\nE:ID_FS_TYPE=vfat\nE:ID_FS_VERSION=FAT32\nE:ID_FS_LABEL=K\n",
        );
        assert_eq!(probe.fs_type.as_deref(), Some("vfat"));
        assert_eq!(probe.fs_version.as_deref(), Some("FAT32"));
        assert_eq!(parse_udev("E:ID_FS_LABEL=K\n"), Probe::default());
    }

    #[test]
    fn the_device_follows_a_dot_only_when_the_source_is_a_device() {
        let mounts = mounts();
        assert_eq!(label(&mounts[1], None), "NTFS · /dev/sda1");
        assert_eq!(label(&mounts[0], None), "btrfs · /dev/nvme0n1p2");
        assert_eq!(label(&mounts[4], None), "nfs4");
        assert_eq!(label(&mounts[5], None), "tmpfs");
    }

    #[test]
    fn the_udev_key_follows_the_source_node_not_the_anonymous_fuse_device() {
        // /dev/sdb1 is 8:17; the fuseblk mount itself reports 0:95.
        let rdev = rustix::fs::makedev(8, 17);
        assert_eq!(udev_key("/dev/sdb1", Some(rdev), "0:95"), "8:17");
        // No node to stat, a source that is not a device, or a zero rdev.
        assert_eq!(udev_key("/dev/sdb1", None, "0:95"), "0:95");
        assert_eq!(udev_key("nas:/export", Some(rdev), "0:50"), "0:50");
        assert_eq!(udev_key("/dev/null-ish", Some(0), "8:1"), "8:1");
    }
}
