//! Disks, as `/proc/diskstats` and `/sys/block` report them.
//!
//! Only whole devices are shown: a partition's traffic is already counted by
//! its disk, and a person thinks in drives. The kernel's `diskstats` sectors
//! are always 512 bytes regardless of the device's own sector size.

use std::fmt;

pub const SECTOR_BYTES: u64 = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskStat {
    pub name: String,
    pub read_sectors: u64,
    pub write_sectors: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiskError {
    TooFewFields { line: String },
    UnreadableNumber { line: String },
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFields { line } => write!(formatter, "diskstats line is too short: {line}"),
            Self::UnreadableNumber { line } => {
                write!(formatter, "disk value is not a number: {line}")
            }
        }
    }
}

impl std::error::Error for DiskError {}

/// Whether a `/proc/diskstats` or `/sys/block` name is a drive rather than a
/// partition or a virtual device: `sda` yes, `sda1` no, `nvme0n1` yes,
/// `nvme0n1p2` no, `mmcblk0` yes, `mmcblk0p1` no; `loop*`, `ram*`, `zram*`,
/// `dm-*`, `sr*` and `md*` never.
#[must_use]
pub fn is_whole_device(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    for prefix in ["loop", "ram", "zram", "dm-", "sr", "md"] {
        if name.starts_with(prefix) {
            return false;
        }
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        // nvme<ctrl>n<ns>[p<part>]
        return rest.contains('n') && !rest.contains('p');
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return !rest.contains('p');
    }
    // sdX, vdX, hdX: letters only after the prefix.
    name.len() >= 3 && !name.ends_with(|c: char| c.is_ascii_digit())
}

/// Parses `/proc/diskstats`, keeping whole devices only.
///
/// # Errors
///
/// A line with fewer than the fourteen classic fields, or a non-numeric
/// sector count.
pub fn parse_diskstats(text: &str) -> Result<Vec<DiskStat>, DiskError> {
    let mut disks = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        // major minor name reads merged rsectors rms writes wmerged wsectors …
        if fields.len() < 14 {
            return Err(DiskError::TooFewFields {
                line: line.to_owned(),
            });
        }
        let name = fields[2];
        if !is_whole_device(name) {
            continue;
        }
        let number = |index: usize| -> Result<u64, DiskError> {
            fields[index]
                .parse::<u64>()
                .map_err(|_| DiskError::UnreadableNumber {
                    line: line.to_owned(),
                })
        };
        disks.push(DiskStat {
            name: name.to_owned(),
            read_sectors: number(5)?,
            write_sectors: number(9)?,
        });
    }
    Ok(disks)
}

/// `/sys/block/DEV/size`: sectors of 512 bytes.
///
/// # Errors
///
/// Not one integer.
pub fn parse_size_sectors(text: &str) -> Result<u64, DiskError> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| DiskError::UnreadableNumber {
            line: text.trim().to_owned(),
        })
}

/// `/sys/block/DEV/queue/rotational` and `/sys/block/DEV/removable`: `0` or `1`.
///
/// # Errors
///
/// Anything else.
pub fn parse_flag(text: &str) -> Result<bool, DiskError> {
    match text.trim() {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(DiskError::UnreadableNumber {
            line: other.to_owned(),
        }),
    }
}

/// `/sys/block/DEV/device/model`, trimmed and with runs of blanks collapsed.
#[must_use]
pub fn clean_model(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS: &str = "\
 259       0 nvme0n1 971829 144408 138614792 453932 609417 4538 93511328 416674 0 298818 897538 53732 0 1547679744 22193 3126 4738
 259       1 nvme0n1p1 971733 144408 138609576 453913 609417 4538 93511328 416674 0 191310 892781 53732 0 1547679744 22193 0 0
   8       0 sda 16756139 2604077 1081553769 67141404 1278548 328194 853593593 58985297 0 5007622 126145546 0 0 0 0 16999 18843
 254       0 zram0 100 0 800 0 200 0 1600 0 0 0 0 0 0 0 0 0 0
   7       0 loop0 1 0 8 0 0 0 0 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn whole_devices_are_drives_not_partitions_or_virtual_devices() {
        for name in ["sda", "sdz", "nvme0n1", "nvme12n3", "mmcblk0", "vda", "hda"] {
            assert!(is_whole_device(name), "{name}");
        }
        for name in [
            "sda1",
            "nvme0n1p1",
            "mmcblk0p2",
            "loop0",
            "ram0",
            "zram0",
            "dm-0",
            "sr0",
            "md127",
            "",
        ] {
            assert!(!is_whole_device(name), "{name}");
        }
    }

    #[test]
    fn diskstats_keeps_whole_devices_with_their_sector_counts() {
        let disks = parse_diskstats(DISKSTATS).expect("readable diskstats");
        assert_eq!(disks.len(), 2);
        assert_eq!(
            disks[0],
            DiskStat {
                name: "nvme0n1".to_owned(),
                read_sectors: 138_614_792,
                write_sectors: 93_511_328
            }
        );
        assert_eq!(disks[1].name, "sda");
        assert_eq!(disks[1].write_sectors, 853_593_593);
    }

    #[test]
    fn a_short_or_unreadable_diskstats_line_is_refused() {
        assert!(matches!(
            parse_diskstats("   8       0 sda 1 2 3\n"),
            Err(DiskError::TooFewFields { .. })
        ));
        assert!(matches!(
            parse_diskstats("   8       0 sda 1 2 x 4 5 6 7 8 9 10 11 12 13 14\n"),
            Err(DiskError::UnreadableNumber { .. })
        ));
        assert_eq!(parse_diskstats("").expect("empty is fine"), vec![]);
    }

    #[test]
    fn sysfs_helpers_read_one_value_each() {
        assert_eq!(parse_size_sectors("1953525168\n"), Ok(1_953_525_168));
        assert!(matches!(
            parse_size_sectors("big\n"),
            Err(DiskError::UnreadableNumber { .. })
        ));
        assert_eq!(parse_flag("0\n"), Ok(false));
        assert_eq!(parse_flag("1\n"), Ok(true));
        assert!(matches!(
            parse_flag("yes\n"),
            Err(DiskError::UnreadableNumber { .. })
        ));
        assert_eq!(
            clean_model("Samsung SSD 990 PRO 1TB                 \n"),
            "Samsung SSD 990 PRO 1TB"
        );
        assert_eq!(clean_model("My   Passport  2627"), "My Passport 2627");
        assert_eq!(clean_model(""), "");
    }
}
