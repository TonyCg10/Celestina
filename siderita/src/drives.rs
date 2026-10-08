//! language-contract: product-copy
//!
//! Drive tools for removable volumes: rename a filesystem, format a partition,
//! or wipe a whole disk into one new partition — all through UDisks2 on the
//! system bus, like mounting. polkit asks for authorization in its own window;
//! Siderita never sees a password and nothing here runs as root.
//!
//! Every call that changes a device names it twice — the UDisks2 object and
//! the device node the person chose — and first re-checks, over a fresh look at
//! the UDisks2 object tree, that the object still is that device and that it is
//! not part of the running system (see [`backs_system`]). The pure rules —
//! label limits, partition-table choice, partition type codes, the whole-disk
//! choice, the system-drive decision — are separate functions so they can be
//! tested without touching a device.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, Value};

use crate::volumes::{
    c_string, drive_is_removable, system_bus, IFACE_BLOCK, IFACE_FILESYSTEM, UDISKS,
};

const IFACE_PARTITION: &str = "org.freedesktop.UDisks2.Partition";
const IFACE_PARTITION_TABLE: &str = "org.freedesktop.UDisks2.PartitionTable";

/// The file systems Siderita formats to, as UDisks2 names them.
pub const FS_TYPES: [&str; 5] = ["exfat", "vfat", "ntfs", "ext4", "btrfs"];

/// Mount points that make a drive part of the running system. A drive holding
/// any of them is never renamed or formatted from here.
const SYSTEM_MOUNTS: [&str; 7] = ["/", "/boot", "/boot/efi", "/efi", "/home", "/usr", "/var"];

/// Characters a FAT volume label may not hold.
const FAT_FORBIDDEN: &str = "*?.,;:/\\|+=<>[]\"";

/// The upper half of code page 850, the OEM code page FAT labels are written
/// in by mkfs.fat and read in by Windows in Western Europe (0x80 to 0xFF).
const CP850_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜø£Ø×ƒáíóúñÑªº¿®¬½¼¡«»░▒▓│┤ÁÂÀ©╣║╗╝¢¥┐└┴┬├─┼ãÃ╚╔╩╦╠═╬¤ðÐÊËÈıÍÎÏ┘┌█▄¦Ì▀ÓßÔÒõÕµþÞÚÛÙýÝ¯´\u{AD}±‗¾¶§÷¸°¨·¹³²■\u{A0}";

const MIB: u64 = 1024 * 1024;

/// The largest disk that still gets an MBR (`dos`) partition table: 2 TiB.
const MBR_LIMIT: u64 = 2 * 1024 * 1024 * MIB;

/// Where the single partition of a wiped disk starts: 1 MiB, the alignment
/// every current partitioning tool uses.
const PARTITION_OFFSET: u64 = MIB;

/// The smallest volume formatted as FAT32: mkfs.fat refuses `-F 32` when the
/// volume cannot hold FAT32's minimum cluster count, which happens below this.
const FAT32_MIN: u64 = 64 * MIB;

/// How long a freshly written partition table may take to appear on the bus.
const TABLE_WAIT: Duration = Duration::from_secs(15);

/// `label` as it is written to a `fstype` filesystem: FAT labels are stored in
/// capitals, as Windows writes them; every other file system keeps the case.
pub fn normalize_label(fstype: &str, label: &str) -> String {
    if fstype == "vfat" {
        label.to_uppercase()
    } else {
        label.to_owned()
    }
}

/// Why `label`, once normalized ([`normalize_label`]), cannot name a `fstype`
/// filesystem, or `None` when it can. The empty label is always allowed
/// (UDisks2 leaves the volume unnamed). An unknown file system cannot be named
/// from here at all.
pub fn label_error(fstype: &str, label: &str) -> Option<String> {
    let label = normalize_label(fstype, label);
    match fstype {
        "vfat" => {
            if let Some(bad) = label.chars().find(|c| FAT_FORBIDDEN.contains(*c)) {
                return Some(format!("En FAT32 el nombre no puede llevar «{bad}»"));
            }
            if let Some(bad) = label.chars().find(|c| !in_cp850(*c)) {
                return Some(format!(
                    "En FAT32 el nombre solo admite letras latinas: «{bad}» no cabe"
                ));
            }
            if label.chars().count() > 11 {
                return Some("En FAT32 el nombre admite 11 caracteres como máximo".to_owned());
            }
            None
        }
        "exfat" if label.encode_utf16().count() > 15 => {
            Some("En exFAT el nombre admite 15 caracteres como máximo".to_owned())
        }
        "ntfs" if label.encode_utf16().count() > 32 => {
            Some("En NTFS el nombre admite 32 caracteres como máximo".to_owned())
        }
        "ext4" if label.len() > 16 => {
            Some("En ext4 el nombre admite 16 bytes como máximo".to_owned())
        }
        "btrfs" if label.len() > 255 => {
            Some("En btrfs el nombre admite 255 bytes como máximo".to_owned())
        }
        "exfat" | "ntfs" | "ext4" | "btrfs" => None,
        _ => Some("Este sistema de archivos no admite cambiar el nombre".to_owned()),
    }
}

/// Whether `c` is a printable character of code page 850.
fn in_cp850(c: char) -> bool {
    (' '..='~').contains(&c) || CP850_HIGH.contains(c)
}

/// Why a `size`-byte volume cannot take `fstype`, or `None` when it can.
pub fn size_error(fstype: &str, size: u64) -> Option<String> {
    if size == 0 {
        return Some("No se pudo leer el tamaño del disco".to_owned());
    }
    if fstype == "vfat" && size < FAT32_MIN {
        return Some("FAT32 necesita un volumen de al menos 64 MiB".to_owned());
    }
    None
}

/// The extra mkfs arguments UDisks2 passes for `fstype` (`mkfs-args`): FAT is
/// always FAT32, which mkfs.fat would otherwise only choose for large volumes.
pub fn mkfs_args(fstype: &str) -> Option<[&'static str; 2]> {
    (fstype == "vfat").then_some(["-F", "32"])
}

/// The partition table a whole disk of `size` bytes gets: MBR up to 2 TiB, the
/// most widely readable; GPT above, where MBR cannot address the space.
pub fn partition_table_for(size: u64) -> &'static str {
    if size <= MBR_LIMIT {
        "dos"
    } else {
        "gpt"
    }
}

/// The partition type for a `fstype` partition in a `table` (`dos` or `gpt`):
/// an MBR type code or a GPT type GUID, or `None` for an unknown file system.
pub fn partition_type(table: &str, fstype: &str) -> Option<&'static str> {
    let windows = matches!(fstype, "vfat" | "exfat" | "ntfs");
    let linux = matches!(fstype, "ext4" | "btrfs");
    match (table, fstype) {
        ("dos", "vfat") => Some("0x0c"),
        ("dos", _) if windows => Some("0x07"),
        ("dos", _) if linux => Some("0x83"),
        // Microsoft basic data, which Windows and macOS mount.
        ("gpt", _) if windows => Some("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7"),
        // Linux filesystem data.
        ("gpt", _) if linux => Some("0fc63daf-8483-4772-8e79-3d69d8477de4"),
        _ => None,
    }
}

/// What the system-drive decision needs to know about one UDisks2 block.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockFacts {
    /// The block's object path.
    pub path: String,
    /// Its device node, e.g. `/dev/sdb1`.
    pub device: String,
    /// The drive object it sits on; empty or `/` when it has none of its own
    /// (an unlocked LUKS device, an LVM volume, a RAID array).
    pub drive: String,
    /// The block objects it is built on (LUKS backing device, device-mapper
    /// and RAID members), which lead to a drive when `drive` does not.
    pub backing: Vec<String>,
    /// Where its filesystem is mounted, if it has one.
    pub mount_points: Vec<String>,
    /// The content's UUID (`IdUUID`): every member of a multi-device
    /// filesystem such as btrfs carries the same one.
    pub uuid: String,
    /// What the content is (`IdUsage`): `filesystem`, `crypto`, `raid`…
    pub usage: String,
    /// Whether it is a partition (rather than a whole disk or a mapping).
    pub partition: bool,
    /// Whether it carries a partition table (a whole disk, including a hybrid
    /// ISO image that is a filesystem and a table at once).
    pub table: bool,
    /// Its size in bytes (`Size`).
    pub size: u64,
}

/// The drives `path` ultimately lies on, following backing devices.
pub fn drives_of(blocks: &[BlockFacts], path: &str) -> HashSet<String> {
    let by_path: HashMap<&str, &BlockFacts> = blocks.iter().map(|b| (b.path.as_str(), b)).collect();
    let mut drives = HashSet::new();
    let mut seen = HashSet::new();
    let mut pending = vec![path];
    while let Some(next) = pending.pop() {
        if !seen.insert(next) {
            continue;
        }
        let Some(block) = by_path.get(next) else {
            continue;
        };
        if !block.drive.is_empty() && block.drive != "/" {
            drives.insert(block.drive.clone());
        }
        pending.extend(block.backing.iter().map(String::as_str));
    }
    drives
}

/// Whether `drive` is part of the running system: any block on it — directly
/// or under a LUKS / LVM / RAID mapping — has a filesystem mounted at one of
/// the system mount points, is an active swap device, holds an active swap
/// file, or belongs to a multi-device filesystem (same UUID) one of whose
/// other members does any of that. An empty or `/` drive cannot be told apart
/// from the system's and counts as system.
pub fn backs_system(blocks: &[BlockFacts], swaps: &[String], drive: &str) -> bool {
    if drive.is_empty() || drive == "/" {
        return true;
    }
    let on_drive: Vec<&BlockFacts> = blocks
        .iter()
        .filter(|block| drives_of(blocks, &block.path).contains(drive))
        .collect();
    if on_drive.iter().any(|block| marks_system(block, swaps)) {
        return true;
    }
    let uuids: HashSet<&str> = on_drive
        .iter()
        .map(|block| block.uuid.as_str())
        .filter(|uuid| !uuid.is_empty())
        .collect();
    blocks
        .iter()
        .filter(|block| uuids.contains(block.uuid.as_str()))
        .any(|block| marks_system(block, swaps))
}

/// Whether this one block is in use by the running system.
fn marks_system(block: &BlockFacts, swaps: &[String]) -> bool {
    let swap_device = swaps.iter().any(|swap| swap == &block.device);
    let system_mount = block
        .mount_points
        .iter()
        .any(|mount| is_system_mount(mount));
    let swap_file = block.mount_points.iter().any(|mount| {
        let prefix = format!("{}/", mount.trim_end_matches('/'));
        swaps.iter().any(|swap| swap.starts_with(&prefix))
    });
    swap_device || system_mount || swap_file
}

fn is_system_mount(mount: &str) -> bool {
    let trimmed = mount.trim_end_matches('/');
    let normalized = if trimmed.is_empty() { "/" } else { trimmed };
    SYSTEM_MOUNTS.contains(&normalized)
}

/// The whole-disk block of `drive`: the one block on it that is neither a
/// partition nor a mapping. None, or more than one, is refused.
pub fn whole_disk_of<'a>(blocks: &'a [BlockFacts], drive: &str) -> Result<&'a BlockFacts, String> {
    let wholes: Vec<&BlockFacts> = blocks
        .iter()
        .filter(|block| block.drive == drive && !block.partition && block.backing.is_empty())
        .collect();
    match wholes[..] {
        [whole] => Ok(whole),
        _ => Err("No se encontró el disco de este volumen".to_owned()),
    }
}

/// The whole-disk block under the volume at `path`: what a whole-disk format
/// of it would erase. `None` unless the volume lies on exactly one drive and
/// that drive has exactly one whole-disk block.
pub fn disk_of<'a>(blocks: &'a [BlockFacts], path: &str) -> Option<&'a BlockFacts> {
    let drives = drives_of(blocks, path);
    let [drive] = drives.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    whole_disk_of(blocks, drive).ok()
}

/// The paths in `/proc/swaps`' first column (the header line skipped), with the
/// kernel's octal escapes for blanks decoded.
pub fn parse_swaps(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .map(|field| {
            field
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n")
                .replace("\\134", "\\")
        })
        .collect()
}

/// The active swap areas, canonicalized so a `/dev/mapper` link compares equal
/// to the `/dev/dm-N` UDisks2 reports.
fn active_swaps() -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string("/proc/swaps")
        .map_err(|error| format!("No se pudo leer /proc/swaps: {error}"))?;
    Ok(parse_swaps(&text)
        .into_iter()
        .flat_map(|swap| {
            let canonical = std::fs::canonicalize(&swap)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| swap.clone());
            [swap, canonical]
        })
        .collect())
}

/// Reads every UDisks2 block into [`BlockFacts`]. Device-mapper and RAID
/// members come from `/sys/class/block/<name>/slaves`, which UDisks2 does not
/// publish as a single property.
pub fn block_facts(connection: &Connection) -> Result<Vec<BlockFacts>, String> {
    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        connection,
        UDISKS,
        "/org/freedesktop/UDisks2",
    )
    .map_err(|error| format!("UDisks2 no disponible: {error}"))?;
    let objects = manager
        .get_managed_objects()
        .map_err(|error| format!("No se pudieron enumerar los discos: {error}"))?;

    let mut blocks = Vec::new();
    for (path, interfaces) in &objects {
        if !interfaces.contains_key(IFACE_BLOCK) {
            continue;
        }
        let path = path.as_str();
        let block = Proxy::new(connection, UDISKS, path, IFACE_BLOCK)
            .map_err(|error| format!("UDisks2: {error}"))?;
        let object = |name: &str| {
            block
                .get_property::<OwnedObjectPath>(name)
                .map(|value| value.as_str().to_owned())
                .unwrap_or_default()
        };
        let text = |name: &str| block.get_property::<String>(name).unwrap_or_default();
        let mut backing = Vec::new();
        let crypto = object("CryptoBackingDevice");
        if !crypto.is_empty() && crypto != "/" {
            backing.push(crypto);
        }
        let mount_points = if interfaces.contains_key(IFACE_FILESYSTEM) {
            Proxy::new(connection, UDISKS, path, IFACE_FILESYSTEM)
                .map_err(|error| format!("UDisks2: {error}"))?
                .get_property::<Vec<Vec<u8>>>("MountPoints")
                .map_err(|error| format!("UDisks2: {error}"))?
                .iter()
                .map(|bytes| c_string(bytes))
                .collect()
        } else {
            Vec::new()
        };
        blocks.push(BlockFacts {
            path: path.to_owned(),
            device: block
                .get_property::<Vec<u8>>("Device")
                .map(|bytes| c_string(&bytes))
                .map_err(|error| format!("UDisks2: {error}"))?,
            drive: object("Drive"),
            backing,
            mount_points,
            uuid: text("IdUUID"),
            usage: text("IdUsage"),
            partition: interfaces.contains_key(IFACE_PARTITION),
            table: interfaces.contains_key(IFACE_PARTITION_TABLE),
            size: block.get_property::<u64>("Size").unwrap_or(0),
        });
    }

    // Mapping members: `/sys/class/block/dm-0/slaves/sda2` says dm-0 sits on
    // sda2, which UDisks2 knows as the block whose device is `/dev/sda2`.
    let by_device: HashMap<String, String> = blocks
        .iter()
        .map(|block| (block.device.clone(), block.path.clone()))
        .collect();
    for block in &mut blocks {
        let Some(name) = block.device.strip_prefix("/dev/") else {
            continue;
        };
        let Ok(entries) = std::fs::read_dir(format!("/sys/class/block/{name}/slaves")) else {
            continue;
        };
        for entry in entries.flatten() {
            let member = format!("/dev/{}", entry.file_name().to_string_lossy());
            if let Some(path) = by_device.get(&member) {
                if !block.backing.contains(path) {
                    block.backing.push(path.clone());
                }
            }
        }
    }
    Ok(blocks)
}

/// One look at the whole object tree, for a listing: it answers the
/// system-drive question and names the disk under every volume listed.
pub struct Listing(Result<(Vec<BlockFacts>, Vec<String>), String>);

impl Listing {
    pub fn read(connection: &Connection) -> Self {
        Self(block_facts(connection).and_then(|blocks| Ok((blocks, active_swaps()?))))
    }

    /// Whether the block at `path` is part of the running system. When the
    /// facts cannot be read, every answer is "system" — a volume that cannot be
    /// checked is not offered.
    pub fn is_system(&self, path: &str) -> bool {
        match &self.0 {
            Ok((blocks, swaps)) => block_is_system(blocks, swaps, path),
            Err(_) => true,
        }
    }

    /// The device node and size of the disk under the block at `path` (see
    /// [`disk_of`]); empty and 0 when it cannot be told.
    pub fn disk(&self, path: &str) -> (String, u64) {
        match &self.0 {
            Ok((blocks, _)) => disk_of(blocks, path)
                .map(|disk| (disk.device.clone(), disk.size))
                .unwrap_or_default(),
            Err(_) => (String::new(), 0),
        }
    }
}

fn block_is_system(blocks: &[BlockFacts], swaps: &[String], path: &str) -> bool {
    let drives = drives_of(blocks, path);
    drives.is_empty()
        || drives
            .iter()
            .any(|drive| backs_system(blocks, swaps, drive))
}

/// Whether the volume at `object_path` lives on a drive that is part of the
/// running system. Fails closed: anything that cannot be read counts as system.
pub fn is_system_drive(connection: &Connection, object_path: &str) -> bool {
    match (block_facts(connection), active_swaps()) {
        (Ok(blocks), Ok(swaps)) => block_is_system(&blocks, &swaps, object_path),
        _ => true,
    }
}

/// The size in bytes of the block at `object_path` (UDisks2 `Block.Size`). A
/// size that cannot be read, or reads as zero, is an error: nothing is
/// formatted on a guess.
pub fn capacity(connection: &Connection, object_path: &str) -> Result<u64, String> {
    Proxy::new(connection, UDISKS, object_path, IFACE_BLOCK)
        .and_then(|block| block.get_property::<u64>("Size"))
        .ok()
        .filter(|size| *size > 0)
        .ok_or_else(|| "No se pudo leer el tamaño del disco".to_owned())
}

/// One drive operation at a time in this process, whichever tab asked: two
/// tabs must not format or rename under each other.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// The right to run a drive operation; released when dropped.
pub struct Claim(());

/// Takes the process-wide drive-operation slot, or `None` while another
/// operation holds it.
pub fn claim() -> Option<Claim> {
    IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Claim(()))
}

impl Drop for Claim {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Renames the filesystem at `object_path`, which must still be `device`. May
/// prompt for authorization. If the filesystem refuses while mounted, it is
/// unmounted, renamed and mounted again.
pub fn set_label(object_path: &str, device: &str, label: &str) -> Result<(), String> {
    let connection = system_bus()?;
    guard(&connection, object_path, device, None)?;
    let block = Proxy::new(&connection, UDISKS, object_path, IFACE_BLOCK)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let fstype = block.get_property::<String>("IdType").unwrap_or_default();
    if let Some(error) = label_error(&fstype, label) {
        return Err(error);
    }
    let label = normalize_label(&fstype, label);
    let filesystem = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let refused = "No autorizado para cambiar el nombre del volumen";
    let set = || {
        filesystem
            .call::<_, _, ()>("SetLabel", &(label.as_str(), no_options()))
            .map_err(|error| failure(error, refused))
    };
    match set() {
        Ok(()) => Ok(()),
        Err(error) if is_busy(&error) && !mount_points(&filesystem).is_empty() => {
            filesystem
                .call::<_, _, ()>("Unmount", &(no_options(),))
                .map_err(|error| failure(error, refused))?;
            let renamed = set();
            let remounted = filesystem
                .call::<_, _, String>("Mount", &(no_options(),))
                .map_err(|error| failure(error, refused));
            renamed.and(remounted.map(|_| ()))
        }
        Err(error) => Err(error),
    }
}

/// Formats the volume at `object_path` (still `device`, still holding the
/// filesystem `uuid` the person chose) in place as `fstype`, unmounting it
/// first. `quick` false overwrites it with zeros. A volume that
/// is a whole disk with a partition table of its own (a hybrid ISO image) is
/// formatted as a whole disk instead. May prompt for authorization.
pub fn format_partition(
    object_path: &str,
    device: &str,
    uuid: &str,
    fstype: &str,
    label: &str,
    quick: bool,
) -> Result<(), String> {
    let connection = system_bus()?;
    guard(&connection, object_path, device, Some(uuid))?;
    check_format(fstype, label)?;
    let blocks = block_facts(&connection)?;
    if blocks
        .iter()
        .any(|block| block.path == object_path && block.table)
    {
        return wipe_disk(&connection, &blocks, object_path, fstype, label, quick);
    }
    if let Some(error) = size_error(fstype, capacity(&connection, object_path)?) {
        return Err(error);
    }
    let refused = "No autorizado para formatear el disco";

    if let Ok(filesystem) = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM) {
        if !mount_points(&filesystem).is_empty() {
            filesystem
                .call::<_, _, ()>("Unmount", &(no_options(),))
                .map_err(|error| failure(error, refused))?;
        }
    }

    let label = normalize_label(fstype, label);
    let mut options = format_options(fstype, &label, quick);
    options.insert("update-partition-type", Value::from(true));
    Proxy::new(&connection, UDISKS, object_path, IFACE_BLOCK)
        .map_err(|error| format!("UDisks2: {error}"))?
        .call::<_, _, ()>("Format", &(fstype, options))
        .map_err(|error| failure(error, refused))
}

/// Erases the whole drive the volume at `object_path` (still `device`, still
/// holding `uuid`) lives on and creates one partition formatted as `fstype` across it, from 1 MiB: a
/// new `dos` table up to 2 TiB, a `gpt` table above. Every filesystem on the
/// drive is unmounted first. `quick` false overwrites the whole disk with
/// zeros. May prompt for authorization.
pub fn format_whole_disk(
    object_path: &str,
    device: &str,
    uuid: &str,
    fstype: &str,
    label: &str,
    quick: bool,
) -> Result<(), String> {
    let connection = system_bus()?;
    guard(&connection, object_path, device, Some(uuid))?;
    check_format(fstype, label)?;
    let blocks = block_facts(&connection)?;
    wipe_disk(&connection, &blocks, object_path, fstype, label, quick)
}

fn wipe_disk(
    connection: &Connection,
    blocks: &[BlockFacts],
    object_path: &str,
    fstype: &str,
    label: &str,
    quick: bool,
) -> Result<(), String> {
    let refused = "No autorizado para formatear el disco";
    let drives = drives_of(blocks, object_path);
    let [drive] = drives.iter().collect::<Vec<_>>()[..] else {
        return Err("No se encontró el disco de este volumen".to_owned());
    };
    let whole = whole_disk_of(blocks, drive)?;
    let size = capacity(connection, &whole.path)?;
    if let Some(error) = size_error(fstype, size.saturating_sub(PARTITION_OFFSET)) {
        return Err(error);
    }
    let table = partition_table_for(size);
    let kind = partition_type(table, fstype)
        .ok_or_else(|| "Sistema de archivos no admitido".to_owned())?;

    // Everything mounted on the drive — partitions and what is unlocked on
    // them — goes before the table does.
    for block in blocks {
        if block.mount_points.is_empty() || !drives_of(blocks, &block.path).contains(drive) {
            continue;
        }
        Proxy::new(connection, UDISKS, block.path.as_str(), IFACE_FILESYSTEM)
            .map_err(|error| format!("UDisks2: {error}"))?
            .call::<_, _, ()>("Unmount", &(no_options(),))
            .map_err(|error| failure(error, refused))?;
    }

    let mut table_options: HashMap<&str, Value> = HashMap::new();
    table_options.insert("tear-down", Value::from(true));
    table_options.insert("no-block", Value::from(false));
    if !quick {
        table_options.insert("erase", Value::from("zero"));
    }
    Proxy::new(connection, UDISKS, whole.path.as_str(), IFACE_BLOCK)
        .map_err(|error| format!("UDisks2: {error}"))?
        .call::<_, _, ()>("Format", &(table, table_options))
        .map_err(|error| failure(error, refused))?;

    wait_for_table(connection, &whole.path)?;
    // The disk was already zeroed with the table, if asked: the partition
    // itself only needs its filesystem.
    let label = normalize_label(fstype, label);
    let mut partition_options = format_options(fstype, &label, true);
    partition_options.remove("tear-down");
    let created = Proxy::new(
        connection,
        UDISKS,
        whole.path.as_str(),
        IFACE_PARTITION_TABLE,
    )
    .map_err(|error| format!("UDisks2: {error}"))?
    .call::<_, _, OwnedObjectPath>(
        "CreatePartitionAndFormat",
        &(
            PARTITION_OFFSET,
            0u64,
            kind,
            "",
            no_options(),
            fstype,
            partition_options,
        ),
    )
    .map(|_| ())
    .map_err(|error| failure(error, refused));
    created
}

/// Refuses any change unless `object_path` still is `device` — and, when the
/// caller names one, still holds the filesystem `uuid` (see [`same_content`]) —
/// UDisks2 does not hint it as system or ignored, its drive is removable, and
/// that drive is not part of the running system — whatever the caller believed
/// when it asked.
fn guard(
    connection: &Connection,
    object_path: &str,
    device: &str,
    uuid: Option<&str>,
) -> Result<(), String> {
    let block = Proxy::new(connection, UDISKS, object_path, IFACE_BLOCK)
        .map_err(|_| "El dispositivo ya no está conectado".to_owned())?;
    let current = block
        .get_property::<Vec<u8>>("Device")
        .map(|bytes| c_string(&bytes))
        .unwrap_or_default();
    if device.is_empty() || current != device {
        return Err("El dispositivo ya no está conectado".to_owned());
    }
    if let Some(uuid) = uuid {
        let now = block.get_property::<String>("IdUUID").ok();
        if !same_content(uuid, now.as_deref()) {
            return Err(CHANGED.to_owned());
        }
    }
    let refused = Err("Siderita no modifica los discos del sistema".to_owned());
    // A hint that cannot be read counts as set.
    if block.get_property::<bool>("HintSystem").unwrap_or(true)
        || block.get_property::<bool>("HintIgnore").unwrap_or(true)
    {
        return refused;
    }
    let drive = block
        .get_property::<OwnedObjectPath>("Drive")
        .map(|drive| drive.as_str().to_owned())
        .unwrap_or_default();
    if !drive_is_removable(connection, &drive) || is_system_drive(connection, object_path) {
        return refused;
    }
    Ok(())
}

/// Why a format is refused when the volume under the device node is no longer
/// the one the person chose.
pub const CHANGED: &str = "La unidad ha cambiado desde que se eligió; vuelve a abrir Formatear";

/// Whether the filesystem UUID read now (`None`: unreadable) is still the
/// `chosen` one. A volume with no UUID matches only one that still has none.
pub fn same_content(chosen: &str, now: Option<&str>) -> bool {
    now == Some(chosen)
}

fn check_format(fstype: &str, label: &str) -> Result<(), String> {
    if !FS_TYPES.contains(&fstype) {
        return Err("Sistema de archivos no admitido".to_owned());
    }
    label_error(fstype, label).map_or(Ok(()), Err)
}

fn format_options<'a>(
    fstype: &str,
    label: &'a str,
    quick: bool,
) -> HashMap<&'static str, Value<'a>> {
    let mut options: HashMap<&str, Value> = HashMap::new();
    if !label.is_empty() {
        options.insert("label", Value::from(label));
    }
    if !quick {
        options.insert("erase", Value::from("zero"));
    }
    if let Some(args) = mkfs_args(fstype) {
        options.insert("mkfs-args", Value::from(args.to_vec()));
    }
    options.insert("tear-down", Value::from(true));
    options.insert("no-block", Value::from(false));
    options
}

fn no_options() -> HashMap<&'static str, Value<'static>> {
    HashMap::new()
}

fn mount_points(filesystem: &Proxy) -> Vec<Vec<u8>> {
    filesystem
        .get_property::<Vec<Vec<u8>>>("MountPoints")
        .unwrap_or_default()
}

/// Polls until the drive's new partition table is on the bus: UDisks2 answers
/// `Format` before every listener has seen the interface appear.
fn wait_for_table(connection: &Connection, path: &str) -> Result<(), String> {
    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        connection,
        UDISKS,
        "/org/freedesktop/UDisks2",
    )
    .map_err(|error| format!("UDisks2 no disponible: {error}"))?;
    let started = Instant::now();
    loop {
        let ready = manager.get_managed_objects().is_ok_and(|objects| {
            objects.iter().any(|(object, interfaces)| {
                object.as_str() == path && interfaces.contains_key(IFACE_PARTITION_TABLE)
            })
        });
        if ready {
            return Ok(());
        }
        if started.elapsed() > TABLE_WAIT {
            return Err("La tabla de particiones nueva no apareció".to_owned());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn is_busy(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("busy") || lower.contains("mounted")
}

/// A UDisks2 error as a short message, with polkit's refusal in `refused`.
fn failure(error: zbus::Error, refused: &str) -> String {
    let text = error.to_string();
    if text.contains("NotAuthorized") {
        refused.to_owned()
    } else {
        format!("UDisks2: {text}")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        backs_system, claim, disk_of, drives_of, label_error, mkfs_args, normalize_label,
        parse_swaps, partition_table_for, partition_type, same_content, size_error, whole_disk_of,
        BlockFacts, CP850_HIGH, MIB,
    };

    fn block(path: &str, device: &str, drive: &str, mounts: &[&str]) -> BlockFacts {
        BlockFacts {
            path: path.to_owned(),
            device: device.to_owned(),
            drive: drive.to_owned(),
            mount_points: mounts.iter().map(|m| (*m).to_owned()).collect(),
            partition: true,
            ..BlockFacts::default()
        }
    }

    #[test]
    fn fat32_labels_are_uppercased_short_and_plain() {
        assert_eq!(normalize_label("vfat", "mi usb"), "MI USB");
        assert_eq!(normalize_label("exfat", "mi usb"), "mi usb");
        assert_eq!(label_error("vfat", ""), None);
        assert_eq!(label_error("vfat", "MI USB 2026"), None);
        // Lowercase is not an error: it is written in capitals, as Windows does.
        assert_eq!(label_error("vfat", "mi usb 2026"), None);
        assert_eq!(label_error("vfat", "año"), None);
        assert!(
            label_error("vfat", "mi usb 20261").is_some(),
            "12 characters"
        );
        // The length is counted after uppercasing: ß becomes SS.
        assert_eq!(label_error("vfat", "ßßßßß"), None);
        assert!(label_error("vfat", "ßßßßßß").is_some());
        for bad in [
            '*', '?', '.', ',', ';', ':', '/', '\\', '|', '+', '=', '<', '>', '[', ']', '"',
        ] {
            assert!(label_error("vfat", &format!("A{bad}B")).is_some(), "{bad}");
        }
    }

    #[test]
    fn fat32_labels_stay_inside_code_page_850() {
        assert_eq!(CP850_HIGH.chars().count(), 128);
        assert_eq!(label_error("vfat", "ÇÉÑÜ"), None);
        assert!(
            label_error("vfat", "€").is_some(),
            "the euro is not in CP850"
        );
        assert!(label_error("vfat", "Ω").is_some());
        assert!(label_error("vfat", "😀").is_some());
        assert!(label_error("vfat", "A\tB").is_some(), "control character");
    }

    #[test]
    fn exfat_counts_utf16_units() {
        assert_eq!(label_error("exfat", "Fotos de verano"), None);
        assert!(label_error("exfat", "Fotos de veranos").is_some());
        // Seven astral characters are fourteen UTF-16 units; eight are sixteen.
        assert_eq!(label_error("exfat", &"😀".repeat(7)), None);
        assert!(label_error("exfat", &"😀".repeat(8)).is_some());
        assert_eq!(label_error("exfat", "minúsculas"), None);
    }

    #[test]
    fn ntfs_ext4_and_btrfs_limits() {
        assert_eq!(label_error("ntfs", &"a".repeat(32)), None);
        assert!(label_error("ntfs", &"a".repeat(33)).is_some());
        assert_eq!(label_error("ext4", &"a".repeat(16)), None);
        assert!(label_error("ext4", &"a".repeat(17)).is_some());
        // Bytes, not characters: eight two-byte letters fill ext4's sixteen.
        assert_eq!(label_error("ext4", &"ñ".repeat(8)), None);
        assert!(label_error("ext4", &"ñ".repeat(9)).is_some());
        assert_eq!(label_error("btrfs", &"a".repeat(255)), None);
        assert!(label_error("btrfs", &"a".repeat(256)).is_some());
    }

    #[test]
    fn unknown_file_systems_cannot_be_named() {
        assert!(label_error("iso9660", "").is_some());
        assert!(label_error("", "X").is_some());
    }

    #[test]
    fn fat_is_always_fat32_and_needs_room_for_it() {
        assert_eq!(mkfs_args("vfat"), Some(["-F", "32"]));
        assert_eq!(mkfs_args("exfat"), None);
        assert!(size_error("vfat", 32 * MIB).is_some());
        assert_eq!(size_error("vfat", 64 * MIB), None);
        assert_eq!(size_error("exfat", 32 * MIB), None);
        assert!(
            size_error("exfat", 0).is_some(),
            "an unread size is refused"
        );
    }

    #[test]
    fn mbr_up_to_two_tebibytes_gpt_above() {
        let tib = 1024u64 * 1024 * 1024 * 1024;
        assert_eq!(partition_table_for(16 * 1024 * 1024 * 1024), "dos");
        assert_eq!(partition_table_for(2 * tib), "dos");
        assert_eq!(partition_table_for(2 * tib + 1), "gpt");
        assert_eq!(partition_table_for(4 * tib), "gpt");
    }

    #[test]
    fn partition_type_codes() {
        assert_eq!(partition_type("dos", "vfat"), Some("0x0c"));
        assert_eq!(partition_type("dos", "exfat"), Some("0x07"));
        assert_eq!(partition_type("dos", "ntfs"), Some("0x07"));
        assert_eq!(partition_type("dos", "ext4"), Some("0x83"));
        assert_eq!(partition_type("dos", "btrfs"), Some("0x83"));
        assert_eq!(
            partition_type("gpt", "exfat"),
            Some("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7")
        );
        assert_eq!(
            partition_type("gpt", "ext4"),
            Some("0fc63daf-8483-4772-8e79-3d69d8477de4")
        );
        assert_eq!(partition_type("dos", "xfs"), None);
    }

    #[test]
    fn the_whole_disk_is_the_one_unpartitioned_unmapped_block() {
        let mut disk = block("/b/sdb", "/dev/sdb", "/d/usb", &[]);
        disk.partition = false;
        disk.table = true;
        let mut cleartext = block("/b/dm0", "/dev/dm-0", "/d/usb", &[]);
        cleartext.partition = false;
        cleartext.backing = vec!["/b/sdb2".to_owned()];
        let blocks = vec![
            disk.clone(),
            block("/b/sdb1", "/dev/sdb1", "/d/usb", &[]),
            block("/b/sdb2", "/dev/sdb2", "/d/usb", &[]),
            cleartext,
            {
                let mut other = block("/b/sdc", "/dev/sdc", "/d/other", &[]);
                other.partition = false;
                other
            },
        ];
        assert_eq!(whole_disk_of(&blocks, "/d/usb"), Ok(&disk));
        assert!(whole_disk_of(&blocks, "/d/missing").is_err());

        let mut twice = blocks.clone();
        let mut second = disk.clone();
        second.path = "/b/sdb-again".to_owned();
        twice.push(second);
        assert!(whole_disk_of(&twice, "/d/usb").is_err(), "two candidates");
    }

    #[test]
    fn the_disk_of_a_volume_is_its_drive_whole_disk_block() {
        let mut disk = block("/b/sdb", "/dev/sdb", "/d/usb", &[]);
        disk.partition = false;
        disk.table = true;
        disk.size = 16 * 1024 * MIB;
        let mut stick = block("/b/sdc", "/dev/sdc", "/d/stick", &[]);
        stick.partition = false;
        let mut cleartext = block("/b/dm0", "/dev/dm-0", "/", &[]);
        cleartext.partition = false;
        cleartext.backing = vec!["/b/sdb2".to_owned()];
        let blocks = vec![
            disk.clone(),
            block("/b/sdb1", "/dev/sdb1", "/d/usb", &[]),
            block("/b/sdb2", "/dev/sdb2", "/d/usb", &[]),
            cleartext,
            stick.clone(),
        ];
        assert_eq!(disk_of(&blocks, "/b/sdb1"), Some(&disk));
        // An unlocked LUKS volume reaches its disk through the backing device.
        assert_eq!(disk_of(&blocks, "/b/dm0"), Some(&disk));
        // A stick formatted without a table is its own disk.
        assert_eq!(disk_of(&blocks, "/b/sdc"), Some(&stick));
        assert_eq!(disk_of(&blocks, "/b/missing"), None);
    }

    #[test]
    fn a_format_follows_the_filesystem_uuid_chosen() {
        assert!(same_content("1234-ABCD", Some("1234-ABCD")));
        // Reformatted elsewhere under the same device node.
        assert!(!same_content("1234-ABCD", Some("9999-0000")));
        assert!(!same_content("1234-ABCD", Some("")));
        assert!(
            !same_content("1234-ABCD", None),
            "an unread UUID is refused"
        );
        assert!(same_content("", Some("")));
        assert!(!same_content("", Some("1234-ABCD")));
    }

    #[test]
    fn a_plain_usb_stick_is_not_system() {
        let blocks = vec![
            block("/b/sda2", "/dev/sda2", "/d/nvme", &["/"]),
            block("/b/sdb1", "/dev/sdb1", "/d/usb", &["/run/media/toni/USB"]),
        ];
        assert!(!backs_system(&blocks, &[], "/d/usb"));
        assert!(backs_system(&blocks, &[], "/d/nvme"));
    }

    #[test]
    fn every_system_mount_point_marks_the_drive() {
        for mount in [
            "/",
            "/home",
            "/home/",
            "/boot",
            "/boot/efi",
            "/boot/efi/",
            "/efi",
            "/usr",
            "/var",
        ] {
            let blocks = vec![
                block("/b/sdb1", "/dev/sdb1", "/d/usb", &["/run/media/toni/USB"]),
                block("/b/sdb2", "/dev/sdb2", "/d/usb", &[mount]),
            ];
            assert!(backs_system(&blocks, &[], "/d/usb"), "{mount}");
        }
    }

    #[test]
    fn an_active_swap_partition_or_swap_file_marks_the_drive() {
        let blocks = vec![
            block("/b/sdb1", "/dev/sdb1", "/d/usb", &["/run/media/toni/USB"]),
            block("/b/sdb2", "/dev/sdb2", "/d/usb", &[]),
        ];
        assert!(backs_system(&blocks, &["/dev/sdb2".to_owned()], "/d/usb"));
        assert!(backs_system(
            &blocks,
            &["/run/media/toni/USB/swapfile".to_owned()],
            "/d/usb"
        ));
        assert!(!backs_system(&blocks, &["/dev/zram0".to_owned()], "/d/usb"));
    }

    #[test]
    fn a_root_inside_luks_or_lvm_reaches_its_drive() {
        let mut cleartext = block("/b/dm0", "/dev/dm-0", "/", &[]);
        cleartext.backing = vec!["/b/sdb2".to_owned()];
        let mut root = block("/b/dm1", "/dev/dm-1", "/", &["/"]);
        root.backing = vec!["/b/dm0".to_owned()];
        let blocks = vec![
            block("/b/sdb1", "/dev/sdb1", "/d/usb", &["/run/media/toni/EFI"]),
            block("/b/sdb2", "/dev/sdb2", "/d/usb", &[]),
            cleartext,
            root,
        ];
        assert!(drives_of(&blocks, "/b/dm1").contains("/d/usb"));
        assert!(backs_system(&blocks, &[], "/d/usb"));
    }

    #[test]
    fn a_member_of_a_mounted_multi_device_root_marks_its_drive() {
        // btrfs across two drives: only the internal member shows the mount.
        let mut internal = block("/b/nvme0n1p2", "/dev/nvme0n1p2", "/d/nvme", &["/"]);
        internal.uuid = "1234-btrfs".to_owned();
        internal.usage = "filesystem".to_owned();
        let mut member = block("/b/sdb1", "/dev/sdb1", "/d/usb", &[]);
        member.uuid = "1234-btrfs".to_owned();
        member.usage = "filesystem".to_owned();
        let stick = block("/b/sdc1", "/dev/sdc1", "/d/stick", &[]);
        let blocks = vec![internal, member, stick];
        assert!(backs_system(&blocks, &[], "/d/usb"));
        assert!(!backs_system(&blocks, &[], "/d/stick"), "no UUID, no link");
    }

    #[test]
    fn an_unknown_drive_counts_as_system() {
        assert!(backs_system(&[], &[], ""));
        assert!(backs_system(&[], &[], "/"));
    }

    #[test]
    fn one_drive_operation_at_a_time() {
        let first = claim().expect("the slot is free");
        assert!(claim().is_none(), "a second operation got in");
        drop(first);
        assert!(claim().is_some(), "the slot was not released");
    }

    #[test]
    fn proc_swaps_first_column() {
        let text = "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n\
                    /dev/zram0                              partition\t8388604\t\t0\t\t100\n\
                    /swap\\040file                           file\t\t1024\t\t0\t\t-2\n";
        assert_eq!(parse_swaps(text), vec!["/dev/zram0", "/swap file"]);
    }
}
