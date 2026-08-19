//! Cover art out of a music file's tags.
//!
//! Four containers, four ways of saying the same thing:
//!
//! - **MP3** carries ID3v2 frames, and the picture is `APIC` (or `PIC` in the
//!   older v2.2). Its length is stored seven bits per byte, so a tag can never
//!   contain a false frame boundary.
//! - **FLAC** has a plain chain of metadata blocks, one of which is a picture.
//! - **MP4 / M4A** nests boxes; the cover is `moov ▸ udta ▸ meta ▸ ilst ▸ covr`.
//! - **Ogg / Opus** keeps the picture base64-encoded inside a comment field,
//!   wrapped in the same structure FLAC uses.
//!
//! All four give back the image exactly as stored — usually JPEG, sometimes PNG.

use std::path::Path;

use crate::{slice_at, u32_be_at};

/// How much of a file is read looking for a tag. Cover art lives at the front
/// (or, for MP4, in a header this finds by walking boxes), and reading a whole
/// album track to find it would cost more than the picture is worth.
const HEAD: usize = 4 * 1024 * 1024;

fn head_of(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.by_ref()
        .take(HEAD as u64)
        .read_to_end(&mut buffer)
        .ok()?;
    Some(buffer)
}

/// The picture in an MP3's ID3v2 tag.
pub(crate) fn id3_picture(path: &Path) -> Option<Vec<u8>> {
    let bytes = head_of(path)?;
    if bytes.get(..3)? != b"ID3" {
        return None;
    }
    let major = *bytes.get(3)?;
    let size = syncsafe(&bytes, 6)?;
    let end = (10 + size).min(bytes.len());

    let (id_len, size_len) = if major >= 3 { (4, 4) } else { (3, 3) };
    let mut at = 10;
    while at + id_len + size_len <= end {
        let id = slice_at(&bytes, at, id_len)?;
        if id.iter().all(|byte| *byte == 0) {
            break; // padding: the tag is over
        }
        let frame_size = if major >= 4 {
            syncsafe(&bytes, at + id_len)?
        } else if major == 3 {
            usize::try_from(u32_be_at(&bytes, at + id_len)?).ok()?
        } else {
            let raw = slice_at(&bytes, at + id_len, 3)?;
            usize::from(raw[0]) << 16 | usize::from(raw[1]) << 8 | usize::from(raw[2])
        };
        let header = id_len + size_len + if major >= 3 { 2 } else { 0 };
        let body_at = at + header;
        if id == b"APIC" || id == b"PIC" {
            let body = slice_at(&bytes, body_at, frame_size)?;
            if let Some(image) = picture_body(body, major) {
                return Some(image);
            }
        }
        at = body_at.checked_add(frame_size)?;
    }
    None
}

/// The image inside an `APIC` frame: an encoding byte, a MIME string, a picture
/// type, a description, and then the bytes.
fn picture_body(body: &[u8], major: u8) -> Option<Vec<u8>> {
    let encoding = *body.first()?;
    let mut at = 1;
    if major >= 3 {
        // MIME type, always Latin-1 and NUL-terminated.
        at += body.get(at..)?.iter().position(|byte| *byte == 0)? + 1;
    } else {
        at += 3; // v2.2 stores a three-letter format instead
    }
    at += 1; // picture type
             // The description ends in one NUL, or two when the text is UTF-16.
    let rest = body.get(at..)?;
    let terminator = if encoding == 1 || encoding == 2 { 2 } else { 1 };
    let mut cursor = 0;
    loop {
        let window = rest.get(cursor..cursor + terminator)?;
        if window.iter().all(|byte| *byte == 0) {
            break;
        }
        cursor += terminator;
    }
    let image = rest.get(cursor + terminator..)?;
    (!image.is_empty()).then(|| image.to_vec())
}

/// A 28-bit length written seven bits per byte, as ID3 stores its sizes.
fn syncsafe(bytes: &[u8], at: usize) -> Option<usize> {
    let raw = slice_at(bytes, at, 4)?;
    if raw.iter().any(|byte| *byte & 0x80 != 0) {
        return None;
    }
    Some(
        usize::from(raw[0]) << 21
            | usize::from(raw[1]) << 14
            | usize::from(raw[2]) << 7
            | usize::from(raw[3]),
    )
}

/// The picture block of a FLAC file.
pub(crate) fn flac_picture(path: &Path) -> Option<Vec<u8>> {
    let bytes = head_of(path)?;
    if bytes.get(..4)? != b"fLaC" {
        return None;
    }
    let mut at = 4;
    loop {
        let header = slice_at(&bytes, at, 4)?;
        let last = header[0] & 0x80 != 0;
        let kind = header[0] & 0x7f;
        let length =
            usize::from(header[1]) << 16 | usize::from(header[2]) << 8 | usize::from(header[3]);
        let body_at = at + 4;
        if kind == 6 {
            if let Some(image) = flac_picture_body(slice_at(&bytes, body_at, length)?) {
                return Some(image);
            }
        }
        if last {
            return None;
        }
        at = body_at.checked_add(length)?;
    }
}

/// The image inside a FLAC picture block, whose strings are length-prefixed
/// rather than terminated — which is what makes this the easiest of the four.
pub(crate) fn flac_picture_body(body: &[u8]) -> Option<Vec<u8>> {
    let mime_len = usize::try_from(u32_be_at(body, 4)?).ok()?;
    let description_at = 8 + mime_len;
    let description_len = usize::try_from(u32_be_at(body, description_at)?).ok()?;
    // width, height, depth, colours: four more counts before the data.
    let data_len_at = description_at + 4 + description_len + 16;
    let data_len = usize::try_from(u32_be_at(body, data_len_at)?).ok()?;
    let image = slice_at(body, data_len_at + 4, data_len)?;
    (!image.is_empty()).then(|| image.to_vec())
}

/// The `covr` box of an MP4 or M4A file.
pub(crate) fn mp4_cover(path: &Path) -> Option<Vec<u8>> {
    let bytes = head_of(path)?;
    let range = find_box(&bytes, 0, bytes.len(), b"moov")?;
    let range = find_box(&bytes, range.0, range.1, b"udta")?;
    let range = find_box(&bytes, range.0, range.1, b"meta")?;
    // `meta` carries four bytes of version and flags before its children.
    let range = find_box(&bytes, range.0 + 4, range.1, b"ilst")?;
    let range = find_box(&bytes, range.0, range.1, b"covr")?;
    let data = find_box(&bytes, range.0, range.1, b"data")?;
    // A `data` box starts with a type and a locale, both four bytes.
    let image = bytes.get(data.0 + 8..data.1)?;
    (!image.is_empty()).then(|| image.to_vec())
}

/// The contents of the first box with this name between `from` and `to`.
fn find_box(bytes: &[u8], from: usize, to: usize, name: &[u8; 4]) -> Option<(usize, usize)> {
    let mut at = from;
    while at + 8 <= to {
        let size = usize::try_from(u32_be_at(bytes, at)?).ok()?;
        let kind = slice_at(bytes, at + 4, 4)?;
        // A size of zero means "to the end"; one means a 64-bit size follows,
        // which no tag box uses and this parser declines to guess at.
        let end = match size {
            0 => to,
            1 => return None,
            _ => at.checked_add(size)?.min(to),
        };
        if kind == name {
            return Some((at + 8, end));
        }
        if size < 8 {
            return None;
        }
        at = end;
    }
    None
}

/// The picture inside an Ogg or Opus comment header.
///
/// The comment carries a FLAC picture block, base64-encoded, under
/// `METADATA_BLOCK_PICTURE` — so once it is decoded the FLAC reader above does
/// the rest.
pub(crate) fn ogg_picture(path: &Path) -> Option<Vec<u8>> {
    let bytes = head_of(path)?;
    if bytes.get(..4)? != b"OggS" {
        return None;
    }
    let needle = b"METADATA_BLOCK_PICTURE=";
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)?
        + needle.len();
    let rest = bytes.get(start..)?;
    let end = rest
        .iter()
        .position(
            |byte| !matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'='),
        )
        .unwrap_or(rest.len());
    let decoded = base64_decode(rest.get(..end)?)?;
    flac_picture_body(&decoded)
}

/// Base64 without a dependency: the alphabet is fixed and the input is one
/// field of one comment.
fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let value = |byte: u8| -> Option<u32> {
        Some(match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks(4) {
        let mut accumulator = 0u32;
        let mut bits = 0;
        for byte in chunk {
            if *byte == b'=' {
                break;
            }
            accumulator = accumulator << 6 | value(*byte)?;
            bits += 6;
        }
        while bits >= 8 {
            bits -= 8;
            out.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::{base64_decode, flac_picture_body, syncsafe};

    #[test]
    fn a_syncsafe_length_uses_seven_bits_a_byte() {
        assert_eq!(syncsafe(&[0, 0, 0, 0, 0, 0, 0, 0, 2, 1], 6), Some(257));
        // A byte with its top bit set is not syncsafe, and not a length either.
        assert_eq!(syncsafe(&[0, 0, 0, 0, 0, 0, 0x80, 0, 0, 0], 6), None);
    }

    #[test]
    fn a_flac_picture_block_gives_back_its_image() {
        let mut block = Vec::new();
        block.extend_from_slice(&3u32.to_be_bytes()); // picture type
        block.extend_from_slice(&9u32.to_be_bytes()); // mime length
        block.extend_from_slice(b"image/png");
        block.extend_from_slice(&0u32.to_be_bytes()); // description length
        for _ in 0..4 {
            block.extend_from_slice(&0u32.to_be_bytes()); // width, height, depth, colours
        }
        block.extend_from_slice(&4u32.to_be_bytes());
        block.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(flac_picture_body(&block), Some(vec![1, 2, 3, 4]));

        // Truncated after the header: no image, and no panic.
        assert_eq!(flac_picture_body(&block[..20]), None);
    }

    #[test]
    fn base64_comes_back_as_the_bytes_it_encoded() {
        assert_eq!(base64_decode(b"AQID"), Some(vec![1, 2, 3]));
        assert_eq!(base64_decode(b"AQI="), Some(vec![1, 2]));
        assert_eq!(base64_decode(b"!!!!"), None);
    }
}
