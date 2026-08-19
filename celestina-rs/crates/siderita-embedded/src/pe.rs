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

use std::path::Path;

use crate::{slice_at, u16_at, u32_at};

/// Resource type 3 in the PE specification: one icon image.
const RT_ICON: u32 = 3;
/// Resource type 14: the directory of sizes that belong to one icon.
const RT_GROUP_ICON: u32 = 14;

/// The largest icon in `path`, as the bytes of an `.ico` file.
pub(crate) fn icon(path: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let sections = sections_of(&bytes)?;
    let resources = resource_root(&bytes, &sections)?;

    // The first group is the program's own icon: Windows shows it for the file,
    // and later groups belong to whatever else the binary carries.
    let group = first_entry_of_type(&bytes, resources, RT_GROUP_ICON)?;
    let group = leaf_bytes(&bytes, resources, &sections, group)?;

    let (id, _size) = largest_in_group(group)?;
    let image = icon_by_id(&bytes, resources, &sections, id)?;
    Some(wrap_as_ico(image))
}

/// Where each section lands, so a virtual address can be turned into a position
/// in the file.
struct Sections {
    entries: Vec<(u32, u32, u32)>,
}

impl Sections {
    /// The file offset a virtual address points at, if any section covers it.
    fn offset_of(&self, rva: u32) -> Option<usize> {
        for (virtual_address, raw_size, raw_pointer) in &self.entries {
            if rva >= *virtual_address && rva < virtual_address.saturating_add(*raw_size) {
                return usize::try_from(raw_pointer + (rva - virtual_address)).ok();
            }
        }
        None
    }
}

fn sections_of(bytes: &[u8]) -> Option<Sections> {
    if bytes.get(..2)? != b"MZ" {
        return None;
    }
    let pe_at = usize::try_from(u32_at(bytes, 0x3c)?).ok()?;
    if slice_at(bytes, pe_at, 4)? != b"PE\0\0" {
        return None;
    }
    let coff = pe_at + 4;
    let section_count = usize::from(u16_at(bytes, coff + 2)?);
    let optional_size = usize::from(u16_at(bytes, coff + 16)?);
    let table = coff + 20 + optional_size;

    let mut entries = Vec::with_capacity(section_count.min(96));
    for index in 0..section_count.min(96) {
        let at = table + index * 40;
        entries.push((
            u32_at(bytes, at + 12)?,
            u32_at(bytes, at + 16)?,
            u32_at(bytes, at + 20)?,
        ));
    }
    Some(Sections { entries })
}

/// The file offset of the resource tree's root directory.
fn resource_root(bytes: &[u8], sections: &Sections) -> Option<usize> {
    let pe_at = usize::try_from(u32_at(bytes, 0x3c)?).ok()?;
    let optional = pe_at + 4 + 20;
    // The data-directory array starts at a different offset in 32- and 64-bit
    // images, and its third entry is the resource table.
    let magic = u16_at(bytes, optional)?;
    let directories = match magic {
        0x10b => optional + 96,
        0x20b => optional + 112,
        _ => return None,
    };
    let rva = u32_at(bytes, directories + 2 * 8)?;
    if rva == 0 {
        return None;
    }
    sections.offset_of(rva)
}

/// The first entry of a given type in the resource root, as the offset of its
/// subdirectory.
fn first_entry_of_type(bytes: &[u8], root: usize, wanted: u32) -> Option<usize> {
    for (id, offset, is_directory) in directory_entries(bytes, root)? {
        if id == wanted && is_directory {
            return Some(root + offset);
        }
    }
    None
}

/// The entries of one resource directory: `(id, offset, is_directory)`.
fn directory_entries(bytes: &[u8], at: usize) -> Option<Vec<(u32, usize, bool)>> {
    let named = usize::from(u16_at(bytes, at + 12)?);
    let by_id = usize::from(u16_at(bytes, at + 14)?);
    let total = named.checked_add(by_id)?;
    // A directory with thousands of entries is not one this crate needs to walk.
    if total > 4096 {
        return None;
    }
    let mut out = Vec::with_capacity(total);
    for index in 0..total {
        let entry = at + 16 + index * 8;
        let name = u32_at(bytes, entry)?;
        let offset = u32_at(bytes, entry + 4)?;
        let is_directory = offset & 0x8000_0000 != 0;
        out.push((
            name & 0x7fff_ffff,
            usize::try_from(offset & 0x7fff_ffff).ok()?,
            is_directory,
        ));
    }
    Some(out)
}

/// Walks a resource subdirectory down to its data and returns those bytes.
fn leaf_bytes(
    bytes: &[u8],
    root: usize,
    sections: &Sections,
    directory: usize,
) -> Option<&'static [u8]> {
    // The borrow cannot escape the file we just read, so the walk answers with
    // offsets and the caller slices.
    let (at, len) = leaf_location(bytes, root, directory)?;
    let offset = sections.offset_of(at)?;
    let slice = slice_at(bytes, offset, usize::try_from(len).ok()?)?;
    // Safety of lifetime, not of memory: the vector outlives every use here, so
    // the slice is copied out instead of borrowed across the boundary.
    Some(Box::leak(slice.to_vec().into_boxed_slice()))
}

/// The `(rva, size)` of the first data entry under a subdirectory.
fn leaf_location(bytes: &[u8], root: usize, directory: usize) -> Option<(u32, u32)> {
    let mut cursor = directory;
    // Name level, then language level: two hops at most before the data entry.
    for _ in 0..2 {
        let (_id, offset, is_directory) = *directory_entries(bytes, cursor)?.first()?;
        let next = root + offset;
        if !is_directory {
            return Some((u32_at(bytes, next)?, u32_at(bytes, next + 4)?));
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
fn icon_by_id(bytes: &[u8], root: usize, sections: &Sections, id: u32) -> Option<&'static [u8]> {
    let icons = first_entry_of_type(bytes, root, RT_ICON)?;
    for (entry_id, offset, is_directory) in directory_entries(bytes, icons)? {
        if entry_id != id || !is_directory {
            continue;
        }
        return leaf_bytes(bytes, root, sections, root + offset);
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
