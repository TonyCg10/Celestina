//! The storage section's starting points: the mounted filesystems a person
//! keeps files on, with how full each one is.
//!
//! Everything here reads the disk, so it runs on a worker thread; the hub
//! only receives the finished list. Kinds are tokens; a disk's name is the
//! device's own model string or the mount point's last segment, shown raw.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use hematita_core::usage::mounts::{self, Mount};

use crate::sampler;

const MOUNTS: &str = "/proc/self/mounts";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocationKind {
    System,
    Home,
    Disk,
}

impl LocationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Home => "home",
            Self::Disk => "disk",
        }
    }

    fn of_target(target: &Path) -> Self {
        if target == Path::new("/") {
            Self::System
        } else if target == Path::new("/home") {
            Self::Home
        } else {
            Self::Disk
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    /// For a disk, the model or the mount's last segment; empty for the
    /// system and home kinds, which QML names itself.
    pub name: String,
    pub path: PathBuf,
    pub kind: LocationKind,
    pub used: u64,
    pub total: u64,
    pub readable: bool,
}

/// The block device behind a mount source: `/dev/nvme0n1p1` → `nvme0n1`,
/// `/dev/sda1` → `sda`. `None` for a source that is not a `/dev/` node.
pub fn disk_of_source(source: &str) -> Option<String> {
    let name = source.strip_prefix("/dev/")?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    // A disk whose own name ends in a digit (`nvme0n1`, `mmcblk0`) numbers
    // its partitions after a `p`; one whose name is letters (`sda`, `vda`)
    // numbers them straight after.
    let disk = match stem.strip_suffix('p') {
        Some(base) if stem.len() < name.len() && base.ends_with(|c: char| c.is_ascii_digit()) => {
            base
        }
        _ if !stem.is_empty() && !stem.contains(|c: char| c.is_ascii_digit()) => stem,
        _ => name,
    };
    Some(disk.to_owned())
}

/// A disk's shown name: the device's model when it has one, otherwise the
/// mount point's last segment.
fn disk_name(mount: &Mount, model_of: &dyn Fn(&str) -> Option<String>) -> String {
    disk_of_source(&mount.source)
        .and_then(|disk| model_of(&disk))
        .unwrap_or_else(|| {
            mount
                .target
                .file_name()
                .map_or_else(OsString::new, OsString::from)
                .to_string_lossy()
                .into_owned()
        })
}

/// Orders locations: system, home, then the rest by path; a mount point seen
/// twice (a filesystem mounted over another) is kept once.
fn ordered(mut locations: Vec<Location>) -> Vec<Location> {
    locations.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.path.cmp(&b.path)));
    locations.dedup_by(|a, b| a.path == b.path);
    locations
}

/// Builds the list from parsed mounts with the given probes; pure apart from
/// what the probes do, so the rules are tested without a disk.
fn locations_from(
    parsed: Vec<Mount>,
    capacity_of: &dyn Fn(&Path) -> Option<(u64, u64)>,
    readable: &dyn Fn(&Path) -> bool,
    model_of: &dyn Fn(&str) -> Option<String>,
) -> Vec<Location> {
    let locations = parsed
        .into_iter()
        .filter(mounts::is_shown_location)
        .filter_map(|mount| {
            let kind = LocationKind::of_target(&mount.target);
            let is_readable = readable(&mount.target);
            // `/` is offered only when it can be listed at all.
            if kind == LocationKind::System && !is_readable {
                return None;
            }
            let (used, total) = capacity_of(&mount.target).unwrap_or((0, 0));
            let name = if kind == LocationKind::Disk {
                disk_name(&mount, model_of)
            } else {
                String::new()
            };
            Some(Location {
                name,
                path: mount.target,
                kind,
                used,
                total,
                readable: is_readable,
            })
        })
        .collect();
    ordered(locations)
}

/// `(used, total)` bytes of the filesystem holding `path`.
fn capacity(path: &Path) -> Option<(u64, u64)> {
    let stat = rustix::fs::statvfs(path).ok()?;
    let total = stat.f_blocks.saturating_mul(stat.f_frsize);
    let used = stat
        .f_blocks
        .saturating_sub(stat.f_bfree)
        .saturating_mul(stat.f_frsize);
    Some((used, total))
}

/// Reads the mount table and every shown mount's capacity. Blocking: call it
/// on a worker thread. An unreadable mount table is an empty list.
pub fn read_locations() -> Vec<Location> {
    let text = std::fs::read_to_string(MOUNTS).unwrap_or_default();
    locations_from(
        mounts::parse_mounts(&text),
        &capacity,
        &|path| std::fs::read_dir(path).is_ok(),
        &sampler::disk_model_of,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(source: &str, target: &str) -> Mount {
        Mount {
            source: source.to_owned(),
            target: PathBuf::from(target),
            fstype: "ext4".to_owned(),
        }
    }

    #[test]
    fn a_partition_names_its_disk() {
        assert_eq!(disk_of_source("/dev/nvme0n1p1").as_deref(), Some("nvme0n1"));
        assert_eq!(
            disk_of_source("/dev/nvme0n1p12").as_deref(),
            Some("nvme0n1")
        );
        assert_eq!(disk_of_source("/dev/mmcblk0p2").as_deref(), Some("mmcblk0"));
        assert_eq!(disk_of_source("/dev/sda1").as_deref(), Some("sda"));
        assert_eq!(disk_of_source("/dev/sdb").as_deref(), Some("sdb"));
        assert_eq!(disk_of_source("/dev/nvme0n1").as_deref(), Some("nvme0n1"));
        assert_eq!(disk_of_source("/dev/mapper/root"), None);
        assert_eq!(disk_of_source("tmpfs"), None);
    }

    #[test]
    fn locations_are_system_home_then_disks_by_path() {
        let parsed = vec![
            mount("/dev/sdb1", "/mnt/zeta"),
            mount("/dev/sdc1", "/mnt/alpha"),
            mount("/dev/nvme0n1p3", "/home"),
            mount("/dev/nvme0n1p2", "/"),
            mount("/dev/nvme0n1p4", "/var"),
        ];
        let found = locations_from(parsed, &|_| Some((1, 2)), &|_| true, &|disk| {
            (disk == "sdb").then(|| "Model B".to_owned())
        });
        let kinds: Vec<_> = found.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LocationKind::System,
                LocationKind::Home,
                LocationKind::Disk,
                LocationKind::Disk
            ]
        );
        assert_eq!(found[2].path, PathBuf::from("/mnt/alpha"));
        assert_eq!(found[2].name, "alpha", "no model falls back to the segment");
        assert_eq!(found[3].name, "Model B");
        assert_eq!(found[0].name, "", "QML names the system kind");
        assert_eq!((found[0].used, found[0].total), (1, 2));
    }

    #[test]
    fn an_unreadable_root_is_not_offered_but_an_unreadable_disk_is_marked() {
        let parsed = vec![mount("/dev/sda2", "/"), mount("/dev/sdb1", "/mnt/b")];
        let found = locations_from(parsed, &|_| None, &|_| false, &|_| None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, PathBuf::from("/mnt/b"));
        assert!(!found[0].readable);
        assert_eq!((found[0].used, found[0].total), (0, 0));
    }

    #[test]
    fn a_mount_point_mounted_twice_is_listed_once() {
        let parsed = vec![mount("/dev/sdb1", "/mnt/b"), mount("/dev/sdc1", "/mnt/b")];
        let found = locations_from(parsed, &|_| None, &|_| true, &|_| None);
        assert_eq!(found.len(), 1);
    }
}
