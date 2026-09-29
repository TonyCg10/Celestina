//! The icon inside a Windows executable.
//!
//! A PE file keeps its icons in a resource tree three levels deep — type, then
//! name, then language — and keeps them in two pieces: a `RT_GROUP_ICON`
//! directory listing the sizes available, and one `RT_ICON` per size holding the
//! actual image. Neither piece is an icon file on its own: the group is a table
//! of contents with no image data, and each image is a bare bitmap with the
//! `.ico` header removed. So the header is put back here, around the largest
//! image the group offers, and what comes out is a file any image reader
//! recognises.
//!
//! Every offset in that tree is a virtual address, which means it points at
//! where the section *would* be once loaded, not at where it sits in the file.
//! Translating those is most of what this module does.
//!
//! The file is never read whole. An installer carries gigabytes of payload
//! behind a few kilobytes of headers, and a folder of them used to make the
//! thumbnail pool read every one into memory; each table is now read where it
//! sits, within one budget per lookup.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::{u16_at, u32_at, MAX_IMAGE};

/// Resource type 3 in the PE specification: one icon image.
const RT_ICON: u32 = 3;
/// Resource type 14: the directory of sizes that belong to one icon.
const RT_GROUP_ICON: u32 = 14;

/// How far into the file the PE header may start. Real linkers put it within
/// the first few hundred bytes; a header claimed further out is not one.
const MAX_HEADER_AT: u64 = 64 * 1024;

/// Everything one icon lookup may read, headers and image together: the one
/// image a lookup keeps is capped at [`MAX_IMAGE`], and the tables around it
/// are a few kilobytes.
const READ_BUDGET: u64 = MAX_IMAGE as u64 + 1024 * 1024;

/// The largest icon in `path`, as the bytes of an `.ico` file.
///
/// An executable can be many gigabytes — installers and games carry their
/// payload inside — while its icon is a few kilobytes near the front. So the
/// file is never read whole: the headers, the section table and each resource
/// directory are read where they sit, and every read is checked against the
/// file's real length and one budget for the whole lookup.
pub(crate) fn icon(path: &Path) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mut probe = Probe::new(file, metadata.len());
    icon_in(&mut probe)
}

fn icon_in<R: Read + Seek>(probe: &mut Probe<R>) -> Option<Vec<u8>> {
    let sections = sections_of(probe)?;
    let resources = resource_root(probe, &sections)?;

    // The first group is the program's own icon: Windows shows it for the file,
    // and later groups belong to whatever else the binary carries.
    let group = first_entry_of_type(probe, resources, RT_GROUP_ICON)?;
    let group = leaf_bytes(probe, resources, &sections, group)?;

    let (id, _size) = largest_in_group(&group)?;
    let image = icon_by_id(probe, resources, &sections, id)?;
    Some(wrap_as_ico(&image))
}

/// Bounded reads at file offsets.
///
/// Every read names its offset and length; one that runs past the end of the
/// file, asks for more than [`MAX_IMAGE`] at once, or would take the lookup
/// past [`READ_BUDGET`] answers `None`, whatever the headers claim.
struct Probe<R> {
    source: R,
    len: u64,
    read: u64,
}

impl<R: Read + Seek> Probe<R> {
    fn new(source: R, len: u64) -> Self {
        Self {
            source,
            len,
            read: 0,
        }
    }

    fn bytes(&mut self, at: u64, len: usize) -> Option<Vec<u8>> {
        if len > MAX_IMAGE {
            return None;
        }
        let wanted = u64::try_from(len).ok()?;
        if at.checked_add(wanted)? > self.len || self.read.checked_add(wanted)? > READ_BUDGET {
            return None;
        }
        self.source.seek(SeekFrom::Start(at)).ok()?;
        let mut buffer = vec![0; len];
        self.source.read_exact(&mut buffer).ok()?;
        self.read += wanted;
        Some(buffer)
    }

    fn u16(&mut self, at: u64) -> Option<u16> {
        u16_at(&self.bytes(at, 2)?, 0)
    }

    fn u32(&mut self, at: u64) -> Option<u32> {
        u32_at(&self.bytes(at, 4)?, 0)
    }
}

/// Where each section lands, so a virtual address can be turned into a position
/// in the file.
struct Sections {
    entries: Vec<(u32, u32, u32)>,
}

impl Sections {
    /// The file offset a virtual address points at, if any section covers it.
    fn offset_of(&self, rva: u32) -> Option<u64> {
        for (virtual_address, raw_size, raw_pointer) in &self.entries {
            if rva >= *virtual_address && rva < virtual_address.saturating_add(*raw_size) {
                return Some(u64::from(*raw_pointer) + u64::from(rva - virtual_address));
            }
        }
        None
    }
}

/// The offset of the `PE\0\0` signature, when the file has one.
fn pe_header<R: Read + Seek>(probe: &mut Probe<R>) -> Option<u64> {
    if probe.bytes(0, 2)? != b"MZ" {
        return None;
    }
    let pe_at = u64::from(probe.u32(0x3c)?);
    if pe_at > MAX_HEADER_AT || probe.bytes(pe_at, 4)? != b"PE\0\0" {
        return None;
    }
    Some(pe_at)
}

fn sections_of<R: Read + Seek>(probe: &mut Probe<R>) -> Option<Sections> {
    let coff = pe_header(probe)? + 4;
    let section_count = usize::from(probe.u16(coff + 2)?).min(96);
    let optional_size = u64::from(probe.u16(coff + 16)?);
    let table = probe.bytes(coff + 20 + optional_size, section_count * 40)?;

    let mut entries = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let at = index * 40;
        entries.push((
            u32_at(&table, at + 12)?,
            u32_at(&table, at + 16)?,
            u32_at(&table, at + 20)?,
        ));
    }
    Some(Sections { entries })
}

/// The file offset of the resource tree's root directory.
fn resource_root<R: Read + Seek>(probe: &mut Probe<R>, sections: &Sections) -> Option<u64> {
    let optional = pe_header(probe)? + 4 + 20;
    // The data-directory array starts at a different offset in 32- and 64-bit
    // images, and its third entry is the resource table.
    let directories = match probe.u16(optional)? {
        0x10b => optional + 96,
        0x20b => optional + 112,
        _ => return None,
    };
    let rva = probe.u32(directories + 2 * 8)?;
    if rva == 0 {
        return None;
    }
    sections.offset_of(rva)
}

/// The first entry of a given type in the resource root, as the offset of its
/// subdirectory.
fn first_entry_of_type<R: Read + Seek>(
    probe: &mut Probe<R>,
    root: u64,
    wanted: u32,
) -> Option<u64> {
    for (id, offset, is_directory) in directory_entries(probe, root)? {
        if id == wanted && is_directory {
            return root.checked_add(offset);
        }
    }
    None
}

/// The entries of one resource directory: `(id, offset, is_directory)`.
fn directory_entries<R: Read + Seek>(
    probe: &mut Probe<R>,
    at: u64,
) -> Option<Vec<(u32, u64, bool)>> {
    let header = probe.bytes(at, 16)?;
    let named = usize::from(u16_at(&header, 12)?);
    let by_id = usize::from(u16_at(&header, 14)?);
    let total = named.checked_add(by_id)?;
    // A directory with thousands of entries is not one this crate needs to walk.
    if total > 4096 {
        return None;
    }
    let table = probe.bytes(at + 16, total * 8)?;
    let mut out = Vec::with_capacity(total);
    for index in 0..total {
        let entry = index * 8;
        let name = u32_at(&table, entry)?;
        let offset = u32_at(&table, entry + 4)?;
        let is_directory = offset & 0x8000_0000 != 0;
        out.push((
            name & 0x7fff_ffff,
            u64::from(offset & 0x7fff_ffff),
            is_directory,
        ));
    }
    Some(out)
}

/// Walks a resource subdirectory down to its data and reads those bytes.
fn leaf_bytes<R: Read + Seek>(
    probe: &mut Probe<R>,
    root: u64,
    sections: &Sections,
    directory: u64,
) -> Option<Vec<u8>> {
    let (at, len) = leaf_location(probe, root, directory)?;
    let offset = sections.offset_of(at)?;
    probe.bytes(offset, usize::try_from(len).ok()?)
}

/// The `(rva, size)` of the first data entry under a subdirectory.
fn leaf_location<R: Read + Seek>(
    probe: &mut Probe<R>,
    root: u64,
    directory: u64,
) -> Option<(u32, u32)> {
    let mut cursor = directory;
    // Name level, then language level: two hops at most before the data entry.
    for _ in 0..2 {
        let (_id, offset, is_directory) = *directory_entries(probe, cursor)?.first()?;
        let next = root.checked_add(offset)?;
        if !is_directory {
            let entry = probe.bytes(next, 8)?;
            return Some((u32_at(&entry, 0)?, u32_at(&entry, 4)?));
        }
        cursor = next;
    }
    None
}

/// The id and size of the largest image a group directory offers.
fn largest_in_group(group: &[u8]) -> Option<(u32, u32)> {
    let count = usize::from(u16_at(group, 4)?);
    let mut best: Option<(u32, u32, u32)> = None;
    for index in 0..count.min(64) {
        let entry = 6 + index * 14;
        let width = u32::from(*group.get(entry)?);
        let height = u32::from(*group.get(entry + 1)?);
        // Zero means 256 in this format, which is the largest of all.
        let side = if width == 0 { 256 } else { width }.max(if height == 0 { 256 } else { height });
        let size = u32_at(group, entry + 8)?;
        let id = u32::from(u16_at(group, entry + 12)?);
        if best.is_none_or(|(chosen, _, _)| side > chosen) {
            best = Some((side, id, size));
        }
    }
    best.map(|(_side, id, size)| (id, size))
}

/// The image bytes of the `RT_ICON` with this id.
fn icon_by_id<R: Read + Seek>(
    probe: &mut Probe<R>,
    root: u64,
    sections: &Sections,
    id: u32,
) -> Option<Vec<u8>> {
    let icons = first_entry_of_type(probe, root, RT_ICON)?;
    for (entry_id, offset, is_directory) in directory_entries(probe, icons)? {
        if entry_id != id || !is_directory {
            continue;
        }
        return leaf_bytes(probe, root, sections, root.checked_add(offset)?);
    }
    None
}

/// Puts an `.ico` header back around a single image.
///
/// Modern executables store PNG data here, which needs no interpretation; older
/// ones store a device-independent bitmap whose header claims twice its real
/// height (it counts a mask that may not exist). Both are left exactly as they
/// are: an `.ico` file is defined as this header plus whichever of the two, and
/// image readers handle the rest.
fn wrap_as_ico(image: &[u8]) -> Vec<u8> {
    let (width, height) = dimensions_of(image);
    let mut out = Vec::with_capacity(image.len() + 22);
    out.extend_from_slice(&[0, 0, 1, 0, 1, 0]); // reserved, type 1 (icon), one image
    out.push(width);
    out.push(height);
    out.extend_from_slice(&[0, 0, 1, 0, 32, 0]); // colours, reserved, planes, bit depth
    out.extend_from_slice(&(image.len() as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes()); // the image starts after this header
    out.extend_from_slice(image);
    out
}

/// The side lengths an `.ico` entry declares, where 0 means 256.
fn dimensions_of(image: &[u8]) -> (u8, u8) {
    if image.starts_with(&[0x89, b'P', b'N', b'G']) {
        // A PNG's own header carries the real size; the directory entry may say
        // 0/0, which is exactly what "256 or larger" means here.
        return (0, 0);
    }
    // A DIB header states its width, and a height that counts the mask twice.
    let width = crate::u32_at(image, 4).unwrap_or(0);
    let height = crate::u32_at(image, 8).unwrap_or(0) / 2;
    (
        u8::try_from(width).unwrap_or(0),
        u8::try_from(height).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::{dimensions_of, largest_in_group, wrap_as_ico};

    #[test]
    fn the_group_names_its_largest_image() {
        // Two entries: 16×16 with id 1, and 256×256 (written as 0×0) with id 7.
        let mut group = vec![0, 0, 1, 0, 2, 0];
        group.extend_from_slice(&[16, 16, 0, 0, 1, 0, 32, 0]);
        group.extend_from_slice(&100u32.to_le_bytes()[..4]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
        group.extend_from_slice(&900u32.to_le_bytes()[..4]);
        group.extend_from_slice(&7u16.to_le_bytes());
        // The entry layout above is 14 bytes each: 8 fields, size, id.
        assert_eq!(largest_in_group(&group), Some((7, 900)));
    }

    #[test]
    fn an_ico_header_is_put_back_around_the_image() {
        let png = [0x89, b'P', b'N', b'G', 1, 2, 3];
        let ico = wrap_as_ico(&png);
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(&ico[22..], &png);
        // A PNG entry declares 0×0, which this format reads as 256.
        assert_eq!(dimensions_of(&png), (0, 0));
    }

    /// A minimal PE32 whose resource section holds one icon group naming one
    /// PNG image, laid out by hand so the test states every offset it relies on.
    pub(super) fn tiny_pe(png: &[u8]) -> Vec<u8> {
        fn put16(file: &mut [u8], at: usize, value: u16) {
            file[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        fn put32(file: &mut [u8], at: usize, value: u32) {
            file[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        const RSRC: usize = 0x200;
        const VA: u32 = 0x1000;
        let mut file = vec![0u8; 0x400];
        file[..2].copy_from_slice(b"MZ");
        put32(&mut file, 0x3c, 0x40);
        file[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        put16(&mut file, coff, 0x14c);
        put16(&mut file, coff + 2, 1); // one section
        put16(&mut file, coff + 16, 0xe0); // optional header size
        let optional = coff + 20;
        put16(&mut file, optional, 0x10b); // PE32
        put32(&mut file, optional + 96 + 16, VA); // resource table rva
        put32(&mut file, optional + 96 + 20, 0x200);
        let table = optional + 0xe0;
        file[table..table + 5].copy_from_slice(b".rsrc");
        put32(&mut file, table + 8, 0x200);
        put32(&mut file, table + 12, VA);
        put32(&mut file, table + 16, 0x200);
        put32(&mut file, table + 20, RSRC as u32);
        let r = RSRC;
        // Root: RT_ICON and RT_GROUP_ICON, each a directory.
        put16(&mut file, r + 14, 2);
        put32(&mut file, r + 16, 3);
        put32(&mut file, r + 20, 0x8000_0000 | 0x30);
        put32(&mut file, r + 24, 14);
        put32(&mut file, r + 28, 0x8000_0000 | 0x60);
        // RT_ICON → id 1 → language → data entry at 0x90.
        put16(&mut file, r + 0x30 + 14, 1);
        put32(&mut file, r + 0x30 + 16, 1);
        put32(&mut file, r + 0x30 + 20, 0x8000_0000 | 0x48);
        put16(&mut file, r + 0x48 + 14, 1);
        put32(&mut file, r + 0x48 + 16, 0x409);
        put32(&mut file, r + 0x48 + 20, 0x90);
        // RT_GROUP_ICON → id 1 → language → data entry at 0xa0.
        put16(&mut file, r + 0x60 + 14, 1);
        put32(&mut file, r + 0x60 + 16, 1);
        put32(&mut file, r + 0x60 + 20, 0x8000_0000 | 0x78);
        put16(&mut file, r + 0x78 + 14, 1);
        put32(&mut file, r + 0x78 + 16, 0x409);
        put32(&mut file, r + 0x78 + 20, 0xa0);
        // Data entries: the image at 0xd0, the group at 0xb0.
        put32(&mut file, r + 0x90, VA + 0xd0);
        put32(&mut file, r + 0x94, png.len() as u32);
        put32(&mut file, r + 0xa0, VA + 0xb0);
        put32(&mut file, r + 0xa4, 20);
        // The group: one 256×256 entry (written 0×0) naming icon id 1.
        let g = r + 0xb0;
        file[g..g + 6].copy_from_slice(&[0, 0, 1, 0, 1, 0]);
        file[g + 6..g + 14].copy_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
        put32(&mut file, g + 14, png.len() as u32);
        put16(&mut file, g + 18, 1);
        file[r + 0xd0..r + 0xd0 + png.len()].copy_from_slice(png);
        file
    }

    /// Bytes this thread has read through `read(2)` so far, from the kernel's
    /// own per-thread counter, so the test measures what the reader did rather
    /// than what it says it did.
    fn thread_rchar() -> u64 {
        let io = std::fs::read_to_string("/proc/thread-self/io").expect("per-thread io counters");
        io.lines()
            .find_map(|line| line.strip_prefix("rchar: "))
            .and_then(|value| value.trim().parse().ok())
            .expect("an rchar line")
    }

    const SPARSE_SIZE: u64 = 4 * 1024 * 1024 * 1024;

    /// SID-12: an installer of several gigabytes used to be read whole into
    /// memory to draw a 64-pixel tile. The icon of a sparse executable of that
    /// size comes out of a bounded number of bytes.
    #[test]
    fn a_huge_executable_is_read_in_bounded_bytes() {
        let dir = std::env::temp_dir().join(format!("siderita-pe-big-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("instalador.exe");
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4];
        let file = std::fs::File::create(&path).expect("create");
        std::io::Write::write_all(&mut &file, &tiny_pe(&png)).expect("write the headers");
        file.set_len(SPARSE_SIZE).expect("grow the file sparsely");
        drop(file);

        let before = thread_rchar();
        let icon = super::icon(&path);
        let read = thread_rchar() - before;
        let _ = std::fs::remove_dir_all(&dir);

        let icon = icon.expect("the icon is found");
        assert_eq!(&icon[22..], &png, "the image comes back whole");
        assert!(
            read < 64 * 1024,
            "read {read} bytes of a {SPARSE_SIZE}-byte file"
        );
    }

    /// A resource entry that claims gigabytes, or points past the end of the
    /// file, is refused without reading what it claims.
    #[test]
    fn a_resource_claiming_gigabytes_is_refused_unread() {
        let png = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        let mut claimed = tiny_pe(&png);
        // The image's data entry now says it is 2 GiB long.
        claimed[0x200 + 0x94..0x200 + 0x98].copy_from_slice(&0x7fff_ffffu32.to_le_bytes());
        let mut probe = super::Probe::new(std::io::Cursor::new(claimed.clone()), 1 << 33);
        assert_eq!(super::icon_in(&mut probe), None);
        assert!(probe.read < 4096, "read {} bytes", probe.read);

        // And one that points past the end of what is really there.
        let len = claimed.len() as u64;
        claimed[0x200 + 0x94..0x200 + 0x98].copy_from_slice(&0x1000u32.to_le_bytes());
        let mut probe = super::Probe::new(std::io::Cursor::new(claimed), len);
        assert_eq!(super::icon_in(&mut probe), None);
    }

    #[test]
    fn nothing_is_read_out_of_a_file_that_is_not_one() {
        let dir = std::env::temp_dir().join(format!("siderita-pe-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("no-soy-un-exe.exe");
        std::fs::write(&path, b"esto no es un ejecutable").expect("write");
        assert_eq!(super::icon(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
