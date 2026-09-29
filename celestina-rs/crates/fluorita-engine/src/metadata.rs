//! Writing what a file says about itself, without touching what it holds.
//!
//! Two containers, one promise: whatever this writes, the media stream comes
//! out byte for byte identical. A FLAC's audio frames are copied across after
//! its metadata blocks are rebuilt; a JPEG's entropy-coded data is copied
//! across after its EXIF segment is removed. Nothing here decodes, re-encodes
//! or re-muxes, which is what makes every operation in this module *lossless*
//! under [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md).
//!
//! **Why the parsing is hand-written.** These are two well-specified block
//! structures with stated lengths, and reading them is arithmetic. A tag
//! library would bring a whole container matrix — most of which this suite
//! deliberately refuses to write — to replace forty lines of length checks. The
//! bound that matters is stated instead: every length is checked against the
//! slice before it is used, because a metadata block is exactly the part of a
//! file a stranger controls.
//!
//! What this module does *not* do is decide anything. Which fields exist, which
//! containers may be written and what a change means for the catalogue all live
//! in `fluorita-core`'s `metadata` module.

use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use fluorita_core::{
    MetadataFormat, MetadataRejected, PrivateFact, SaveChoice, TagChange, TagField,
};

use crate::edit::Bin;
use crate::error::{EngineError, EngineResult};
use crate::landing;

/// The largest file this will read whole in order to rewrite its header.
///
/// A rewrite holds the input and the output at once, so the ceiling is stated
/// rather than discovered: a lossless audio album side is tens of megabytes and
/// a photograph far less.
pub const MAX_CONTAINER_BYTES: u64 = 512 * 1024 * 1024;

/// A cover to embed, already read and already judged against its budget.
pub struct Cover<'a> {
    /// The encoded picture, exactly as it is on disk.
    pub bytes: &'a [u8],
    /// Its media type, as the container records it.
    pub mime: &'a str,
    pub width: u32,
    pub height: u32,
}

/// One metadata write, on the same two terms every edit offers.
pub struct MetadataRequest<'a> {
    /// The file to rewrite.
    pub source: &'a Path,
    /// What to change. Empty for a request that only removes.
    pub tags: &'a TagChange,
    /// What to strip from a photograph, in order.
    pub strip: &'a [PrivateFact],
    /// The front cover to embed, when one was chosen.
    pub cover: Option<Cover<'a>>,
    /// Copy beside the original, or replace it.
    pub choice: SaveChoice,
    /// The word a copy's name is marked with — product copy, owned by the host.
    pub copy_marker: &'a str,
}

/// What a metadata write actually did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataWritten {
    pub written: PathBuf,
    /// Where the original went: `Some` for every replacement, `None` for a
    /// copy.
    pub trashed_original: Option<PathBuf>,
    /// The media stream that was carried across, in bytes. Recorded because it
    /// is the number a caller can compare against the source to see that
    /// nothing was recompressed.
    pub stream_bytes: u64,
}

/// Rewrites a file's metadata and lands the result.
///
/// # Errors
///
/// Refuses a relative source, a container this suite does not write, a file
/// past [`MAX_CONTAINER_BYTES`], a malformed container, a cancellation, and any
/// filesystem failure. The result lands exactly as an edit's save does: staged
/// and synced before the original moves to the Trash, and never over a name
/// that appeared meanwhile.
pub fn write(
    request: &MetadataRequest<'_>,
    bin: &dyn Bin,
    cancellation: &CancellationToken,
) -> EngineResult<MetadataWritten> {
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    if !request.source.is_absolute() {
        return Err(EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "metadata is written over an absolute path",
        });
    }
    let Some(format) = MetadataFormat::classify_path(request.source) else {
        return Err(EngineError::UnusableSource {
            path: request.source.to_path_buf(),
            reason: "this container carries no metadata this suite writes",
        });
    };

    let metadata = std::fs::metadata(request.source).map_err(|source| EngineError::Io {
        operation: "reading the file to retag",
        path: request.source.to_path_buf(),
        source,
    })?;
    if metadata.len() > MAX_CONTAINER_BYTES {
        return Err(EngineError::OverBudget {
            what: "the container to rewrite",
            limit: MAX_CONTAINER_BYTES,
            actual: metadata.len(),
        });
    }
    let original = std::fs::read(request.source).map_err(|source| EngineError::Io {
        operation: "reading the file to retag",
        path: request.source.to_path_buf(),
        source,
    })?;

    let rewritten = match format {
        MetadataFormat::Flac => rewrite_flac(&original, request.tags, request.cover.as_ref()),
        MetadataFormat::JpegExif => strip_exif(&original, request.strip),
        MetadataFormat::OggVorbis | MetadataFormat::Id3 | MetadataFormat::Mp4 => None,
    }
    .ok_or_else(|| EngineError::Undecodable {
        path: request.source.to_path_buf(),
        detail: "the container could not be rewritten without touching its stream".to_owned(),
    })?;

    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }

    let destination = match request.choice {
        SaveChoice::Copy => beside(request.source, request.copy_marker)?,
        SaveChoice::Replace => request.source.to_path_buf(),
    };
    let landed = landing::land(
        request.source,
        &destination,
        &rewritten.bytes,
        request.choice,
        bin,
        cancellation,
        "writing the retagged file",
    )?;

    Ok(MetadataWritten {
        written: landed.written,
        trashed_original: landed.trashed_original,
        stream_bytes: rewritten.stream_bytes,
    })
}

fn beside(source: &Path, marker: &str) -> EngineResult<PathBuf> {
    let directory = source.parent().ok_or_else(|| EngineError::UnusableSource {
        path: source.to_path_buf(),
        reason: "a file to retag has a parent directory",
    })?;
    let name = source
        .file_name()
        .ok_or_else(|| EngineError::UnusableSource {
            path: source.to_path_buf(),
            reason: "a file to retag has a name",
        })?;
    Ok(siderita_ops::next_available(
        directory,
        name,
        marker,
        siderita_ops::NameShape::File,
    ))
}

/// A rewritten container and the stream it carried across untouched.
struct Rewritten {
    bytes: Vec<u8>,
    stream_bytes: u64,
}

/// FLAC block type for Vorbis comments.
const FLAC_VORBIS_COMMENT: u8 = 4;
/// FLAC block type for embedded pictures.
const FLAC_PICTURE: u8 = 6;
/// The picture type a front cover is recorded as.
const FLAC_FRONT_COVER: u32 = 3;
/// FLAC block type for padding, which a rewrite drops rather than preserves:
/// its whole purpose is to be replaceable space.
const FLAC_PADDING: u8 = 1;
/// The flag in a block header saying this is the last one before the frames.
const FLAC_LAST_BLOCK: u8 = 0x80;

/// Replaces a FLAC's Vorbis comment block, copying every other block and every
/// audio frame across unchanged.
///
/// Returns `None` when the file is not a FLAC or its block structure does not
/// parse — never a partially rewritten file.
#[must_use]
pub fn write_flac_tags(bytes: &[u8], change: &TagChange) -> Option<Vec<u8>> {
    rewrite_flac(bytes, change, None).map(|rewritten| rewritten.bytes)
}

/// Embeds a front cover, replacing whichever one the file already carried.
///
/// Returns `None` on the same terms as [`write_flac_tags`]: not a FLAC, or a
/// block structure that does not parse.
#[must_use]
pub fn embed_flac_cover(bytes: &[u8], cover: &Cover<'_>) -> Option<Vec<u8>> {
    rewrite_flac(bytes, &TagChange::new(), Some(cover)).map(|rewritten| rewritten.bytes)
}

fn rewrite_flac(bytes: &[u8], change: &TagChange, cover: Option<&Cover<'_>>) -> Option<Rewritten> {
    if bytes.get(0..4)? != b"fLaC" {
        return None;
    }

    let mut cursor = 4usize;
    let mut kept: Vec<(u8, &[u8])> = Vec::new();
    let mut existing_comments: Option<&[u8]> = None;
    loop {
        let header = bytes.get(cursor..cursor + 4)?;
        let last = header[0] & FLAC_LAST_BLOCK != 0;
        let kind = header[0] & 0x7F;
        let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
        let start = cursor + 4;
        let end = start.checked_add(length)?;
        let body = bytes.get(start..end)?;

        match kind {
            FLAC_VORBIS_COMMENT => existing_comments = Some(body),
            FLAC_PADDING => {}
            // A new front cover replaces the one the file had rather than
            // joining it: two front covers is a file no reader agrees about.
            FLAC_PICTURE if cover.is_some() && is_front_cover(body) => {}
            _ => kept.push((kind, body)),
        }

        cursor = end;
        if last {
            break;
        }
    }

    let stream = bytes.get(cursor..)?;
    let comments = rebuild_vorbis_comments(existing_comments, change)?;
    let picture = match cover {
        Some(cover) => Some(build_picture_block(cover)?),
        None => None,
    };

    let mut out = Vec::with_capacity(bytes.len() + comments.len());
    out.extend_from_slice(b"fLaC");
    // The blocks this rewrite produces go last, so their lengths are the only
    // ones that moved and every other block is copied exactly as it was.
    let trailing = usize::from(!comments.is_empty()) + usize::from(picture.is_some());
    for (index, (kind, body)) in kept.iter().enumerate() {
        let last = index + 1 == kept.len() && trailing == 0;
        write_flac_block(&mut out, *kind, body, last)?;
    }
    if !comments.is_empty() {
        write_flac_block(&mut out, FLAC_VORBIS_COMMENT, &comments, picture.is_none())?;
    }
    if let Some(picture) = picture.as_deref() {
        write_flac_block(&mut out, FLAC_PICTURE, picture, true)?;
    }
    let stream_start = out.len();
    out.extend_from_slice(stream);

    debug_assert_eq!(&out[stream_start..], stream);
    Some(Rewritten {
        bytes: out,
        stream_bytes: stream.len() as u64,
    })
}

/// Whether an existing picture block is the front cover, which is the only one
/// a new cover replaces. A back cover or a photograph of the artist is not the
/// same picture and is left alone.
fn is_front_cover(body: &[u8]) -> bool {
    body.get(0..4)
        .and_then(|slice| slice.try_into().ok())
        .is_some_and(|quad: [u8; 4]| u32::from_be_bytes(quad) == FLAC_FRONT_COVER)
}

/// Builds a `METADATA_BLOCK_PICTURE` for a front cover.
///
/// The description is left empty and the colour count is zero, which is what
/// the specification says for a format that is not indexed. Depth is recorded
/// as 24 bits: it is advisory, and claiming a number the picture may not have
/// would be inventing metadata.
fn build_picture_block(cover: &Cover<'_>) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(cover.bytes.len() + 64);
    out.extend_from_slice(&FLAC_FRONT_COVER.to_be_bytes());
    out.extend_from_slice(&u32::try_from(cover.mime.len()).ok()?.to_be_bytes());
    out.extend_from_slice(cover.mime.as_bytes());
    // No description.
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&cover.width.to_be_bytes());
    out.extend_from_slice(&cover.height.to_be_bytes());
    out.extend_from_slice(&24u32.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&u32::try_from(cover.bytes.len()).ok()?.to_be_bytes());
    out.extend_from_slice(cover.bytes);
    Some(out)
}

fn write_flac_block(out: &mut Vec<u8>, kind: u8, body: &[u8], last: bool) -> Option<()> {
    let length = u32::try_from(body.len()).ok()?;
    if length > 0x00FF_FFFF {
        return None;
    }
    let flag = if last { FLAC_LAST_BLOCK } else { 0 };
    out.push(flag | (kind & 0x7F));
    out.extend_from_slice(&length.to_be_bytes()[1..]);
    out.extend_from_slice(body);
    Some(())
}

/// Builds the new comment block: the vendor string the file already had, every
/// comment it already carried except the ones being changed, and the new values.
///
/// A field set to the empty string is removed rather than written empty, which
/// is what makes "clear this tag" possible at all.
fn rebuild_vorbis_comments(existing: Option<&[u8]>, change: &TagChange) -> Option<Vec<u8>> {
    let mut vendor: Vec<u8> = b"Fluorita".to_vec();
    let mut carried: Vec<Vec<u8>> = Vec::new();

    if let Some(block) = existing {
        let mut cursor = 0usize;
        let vendor_length = read_u32_le(block, cursor)? as usize;
        cursor += 4;
        vendor = block
            .get(cursor..cursor.checked_add(vendor_length)?)?
            .to_vec();
        cursor += vendor_length;

        let count = read_u32_le(block, cursor)? as usize;
        cursor += 4;
        // A count is a claim, not a fact: each entry is still bounds-checked,
        // and a count larger than the block simply ends the walk.
        for _ in 0..count {
            let length = read_u32_le(block, cursor)? as usize;
            cursor += 4;
            let entry = block.get(cursor..cursor.checked_add(length)?)?;
            cursor += length;
            if !replaces(entry, change) {
                carried.push(entry.to_vec());
            }
        }
    }

    for (field, value) in change.touched() {
        if !value.is_empty() {
            carried.push(format!("{}={value}", field.vorbis_key()).into_bytes());
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(&u32::try_from(vendor.len()).ok()?.to_le_bytes());
    out.extend_from_slice(&vendor);
    out.extend_from_slice(&u32::try_from(carried.len()).ok()?.to_le_bytes());
    for entry in &carried {
        out.extend_from_slice(&u32::try_from(entry.len()).ok()?.to_le_bytes());
        out.extend_from_slice(entry);
    }
    Some(out)
}

/// Whether an existing comment is one the change replaces. Keys are compared
/// case-insensitively because the specification says they are.
fn replaces(entry: &[u8], change: &TagChange) -> bool {
    let Some(separator) = entry.iter().position(|byte| *byte == b'=') else {
        return false;
    };
    let Ok(key) = std::str::from_utf8(&entry[..separator]) else {
        return false;
    };
    TagField::ALL
        .iter()
        .any(|field| key.eq_ignore_ascii_case(field.vorbis_key()) && change.value(*field).is_some())
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// The facts a photograph is carrying, out of the ones a person is offered.
///
/// Read from every place a JPEG keeps them: the EXIF IFDs, the XMP packets
/// (standard and extended), and the secondary images a multi-picture file
/// appends after the primary one, which carry their own. A picture that
/// carries none of them answers an empty list, which is what lets a surface say
/// "this picture carries nothing" instead of offering a removal that would do
/// nothing.
#[must_use]
pub fn private_facts(bytes: &[u8]) -> Vec<PrivateFact> {
    let mut found = Vec::new();
    image_facts(bytes, &mut found);
    if let Some(primary_end) = headers(bytes).and_then(|(_, scan)| image_end(bytes, scan)) {
        let mut cursor = primary_end;
        for _ in 0..MAX_TRAILING_IMAGES {
            let Some(offset) = find(&bytes[cursor..], &[0xFF, 0xD8, 0xFF]) else {
                break;
            };
            let start = cursor + offset;
            image_facts(&bytes[start..], &mut found);
            cursor = start + 3;
        }
    }
    found
}

/// How many images appended after the primary one are read for facts. A phone
/// writes one or two; the bound is what keeps a hostile file from turning the
/// search into a walk over every byte pattern it can plant.
const MAX_TRAILING_IMAGES: usize = 16;

/// The facts in one image's header segments: its EXIF and its XMP.
fn image_facts(bytes: &[u8], found: &mut Vec<PrivateFact>) {
    let mut note = |fact| {
        if !found.contains(&fact) {
            found.push(fact);
        }
    };
    if let Some((exif, endian)) = jpeg_exif_start(bytes) {
        for tag in ifd_tags(bytes, exif, endian) {
            match tag {
                // GPS IFD pointer.
                0x8825 => note(PrivateFact::Location),
                // Make, Model, and the lens/software that recorded it.
                0x010F | 0x0110 | 0x0131 => note(PrivateFact::Camera),
                // DateTime, DateTimeOriginal, DateTimeDigitized.
                0x0132 | 0x9003 | 0x9004 => note(PrivateFact::Timestamp),
                _ => {}
            }
        }
    }
    let Some((segments, _)) = headers(bytes) else {
        return;
    };
    for packet in xmp_packets(bytes, &segments) {
        for (fact, keys) in XMP_KEYS {
            if keys.iter().any(|key| find(&packet, key).is_some()) {
                note(*fact);
            }
        }
    }
}

const XMP_STANDARD: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const XMP_EXTENDED: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
/// An extended-XMP chunk's header after its namespace: a 32-byte GUID, the
/// full packet's length and this chunk's offset in it.
const XMP_EXTENDED_HEADER: usize = 32 + 4 + 4;

/// Every XMP packet in `segments`, whole: each standard packet as it is, and
/// each extended packet reassembled from its chunks (grouped by GUID, ordered
/// by offset), so a key split across two chunks is still found. The result is
/// bounded by the segments it copies from.
fn xmp_packets(bytes: &[u8], segments: &[Segment]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    // GUID, declared full length, offset, data.
    let mut extended: Vec<(&[u8], u32, u32, &[u8])> = Vec::new();
    for segment in segments
        .iter()
        .filter(|segment| segment.kind == SegmentKind::Xmp)
    {
        let body = &bytes[segment.body.clone()];
        if let Some(packet) = body.strip_prefix(XMP_STANDARD) {
            packets.push(packet.to_vec());
        } else if let Some(chunk) = body.strip_prefix(XMP_EXTENDED) {
            match (
                chunk.get(..32),
                chunk.get(32..36),
                chunk.get(36..40),
                chunk.get(XMP_EXTENDED_HEADER..),
            ) {
                (Some(guid), Some(full), Some(offset), Some(data)) => {
                    let full = u32::from_be_bytes([full[0], full[1], full[2], full[3]]);
                    let offset = u32::from_be_bytes([offset[0], offset[1], offset[2], offset[3]]);
                    extended.push((guid, full, offset, data));
                }
                // A chunk too short for its header is still scanned as it
                // is: a malformed packet is no reason to report nothing.
                _ => packets.push(chunk.to_vec()),
            }
        }
    }
    extended.sort_by(|left, right| left.0.cmp(right.0).then(left.2.cmp(&right.2)));
    // GUID, the most this packet may grow to, chunks joined, the packet.
    let mut current: Option<(&[u8], usize, usize, Vec<u8>)> = None;
    for (guid, full, offset, data) in extended {
        if current.as_ref().is_none_or(|(held, ..)| *held != guid) {
            if let Some((.., packet)) = current.take() {
                packets.push(packet);
            }
            let declared = usize::try_from(full).unwrap_or(usize::MAX);
            current = Some((guid, declared.min(MAX_EXTENDED_XMP_BYTES), 0, Vec::new()));
        }
        let Some((_, limit, joined, packet)) = current.as_mut() else {
            continue;
        };
        // Past the declared length, the byte cap or the chunk cap a chunk is
        // not read for facts; it is still an XMP segment, so the strip still
        // removes it.
        let within = usize::try_from(offset).is_ok_and(|offset| offset < *limit);
        if !within || *joined >= MAX_EXTENDED_XMP_CHUNKS {
            continue;
        }
        let room = limit.saturating_sub(packet.len());
        packet.extend_from_slice(&data[..data.len().min(room)]);
        *joined += 1;
    }
    if let Some((.., packet)) = current {
        packets.push(packet);
    }
    packets
}

/// The most of one extended-XMP packet that is joined for facts: 64 chunks of
/// a JPEG segment's 64 KiB. A packet may declare any length; this, and the
/// length it declares, bound what is allocated.
const MAX_EXTENDED_XMP_BYTES: usize = 64 * 64 * 1024;
/// The most chunks of one extended-XMP packet that are joined.
const MAX_EXTENDED_XMP_CHUNKS: usize = 64;

/// The XMP properties that say where, with what and when a picture was taken,
/// as they appear in a packet. `:GPS` covers every EXIF GPS property under any
/// namespace prefix a writer chose.
const XMP_KEYS: &[(PrivateFact, &[&[u8]])] = &[
    (
        PrivateFact::Location,
        &[
            b":GPS",
            b"Iptc4xmpCore:Location",
            b"Iptc4xmpExt:LocationCreated",
            b"Iptc4xmpExt:LocationShown",
            b"photoshop:City",
            b"photoshop:State",
            b"photoshop:Country",
        ],
    ),
    (
        PrivateFact::Camera,
        &[
            b"tiff:Make",
            b"tiff:Model",
            b"aux:Lens",
            b"exifEX:LensModel",
            b"xmp:CreatorTool",
        ],
    ),
    (
        PrivateFact::Timestamp,
        &[
            b"xmp:CreateDate",
            b"xmp:ModifyDate",
            b"xmp:MetadataDate",
            b"exif:DateTimeOriginal",
            b"exif:DateTimeDigitized",
            b"photoshop:DateCreated",
        ],
    ),
];

/// Removes what a photograph says about where, with what and when it was
/// taken, copying every other segment and the primary picture's scans across
/// untouched.
///
/// Whole segments go, not individual tags: rewriting an IFD means correcting
/// every offset inside it, and a photograph whose camera fields were surgically
/// removed but whose GPS pointer still resolves is worse than one that simply
/// carries no EXIF. So the EXIF segment, every XMP packet (standard and
/// extended) and the multi-picture index go, and so does everything after the
/// primary picture's end-of-image marker — the secondary images a phone
/// appends each carry their own EXIF, and a Motion Photo's MP4 goes with the
/// XMP that pointed at it.
///
/// Segments between the scans of a progressive picture (tables, restart
/// intervals) are copied through and not read: cameras and editors write
/// their APP metadata before the first start-of-scan, which is where this
/// looks. `facts` therefore selects *whether* to
/// strip, not which bytes — an empty list changes nothing.
#[must_use]
pub fn strip_jpeg_exif(bytes: &[u8], facts: &[PrivateFact]) -> Option<Vec<u8>> {
    strip_exif(bytes, facts).map(|rewritten| rewritten.bytes)
}

fn strip_exif(bytes: &[u8], facts: &[PrivateFact]) -> Option<Rewritten> {
    if bytes.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    if facts.is_empty() {
        return Some(Rewritten {
            bytes: bytes.to_vec(),
            stream_bytes: bytes.len() as u64,
        });
    }

    let (segments, scan) = headers(bytes)?;
    let end = image_end(bytes, scan)?;
    let mut out: Vec<u8> = vec![0xFF, 0xD8];
    for segment in &segments {
        if !segment.kind.carries_private_facts() {
            out.extend_from_slice(&bytes[segment.start..segment.body.end]);
        }
    }
    // Start of scan: everything from here to the primary EOI is the picture.
    let stream = &bytes[scan..end];
    out.extend_from_slice(stream);
    Some(Rewritten {
        bytes: out,
        stream_bytes: stream.len() as u64,
    })
}

/// One header segment of a JPEG: where its marker starts, where its body lies
/// and what it holds.
struct Segment {
    start: usize,
    body: std::ops::Range<usize>,
    kind: SegmentKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SegmentKind {
    Exif,
    Xmp,
    MultiPicture,
    Other,
}

impl SegmentKind {
    const fn carries_private_facts(self) -> bool {
        !matches!(self, Self::Other)
    }
}

impl SegmentKind {
    fn of(marker: u8, body: &[u8]) -> Self {
        match marker {
            0xE1 if body.starts_with(b"Exif\0\0") => Self::Exif,
            0xE1 if body.starts_with(b"http://ns.adobe.com/xap/1.0/\0")
                || body.starts_with(b"http://ns.adobe.com/xmp/extension/\0") =>
            {
                Self::Xmp
            }
            0xE2 if body.starts_with(b"MPF\0") => Self::MultiPicture,
            _ => Self::Other,
        }
    }
}

/// The header segments of the JPEG at the start of `bytes`, and the offset of
/// its first start-of-scan marker. `None` for anything that is not a JPEG or
/// whose lengths do not fit the slice. Every length is checked against the
/// slice, never trusted.
fn headers(bytes: &[u8]) -> Option<(Vec<Segment>, usize)> {
    if bytes.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut segments = Vec::new();
    let mut cursor = 2usize;
    loop {
        let header = bytes.get(cursor..cursor + 4)?;
        if header[0] != 0xFF {
            return None;
        }
        let marker = header[1];
        if marker == 0xDA {
            return Some((segments, cursor));
        }
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        if length < 2 {
            return None;
        }
        let end = (cursor + 4).checked_add(length - 2)?;
        if end > bytes.len() {
            return None;
        }
        segments.push(Segment {
            start: cursor,
            body: cursor + 4..end,
            kind: SegmentKind::of(marker, &bytes[cursor + 4..end]),
        });
        cursor = end;
    }
}

/// Where the image whose first start-of-scan is at `scan` ends: just past its
/// EOI marker, or the end of `bytes` when the file stops before one.
///
/// Walks the scans and the segments between them (a progressive JPEG has
/// several). Inside a scan, `FF 00` is a stuffed data byte, `FF D0`–`FF D7` a
/// restart marker and a run of `FF` fill before a marker; only another marker
/// ends the scan. A segment whose length does not fit the slice is refused.
fn image_end(bytes: &[u8], scan: usize) -> Option<usize> {
    let mut cursor = scan;
    loop {
        // Fill bytes may precede any marker.
        while bytes.get(cursor + 1) == Some(&0xFF) {
            cursor += 1;
        }
        let Some(marker) = bytes.get(cursor..cursor + 2) else {
            return Some(bytes.len());
        };
        if marker[0] != 0xFF {
            return None;
        }
        if marker[1] == 0xD9 {
            return Some(cursor + 2);
        }
        let length = u16::from_be_bytes([*bytes.get(cursor + 2)?, *bytes.get(cursor + 3)?]);
        if length < 2 {
            return None;
        }
        let end = (cursor + 2).checked_add(usize::from(length))?;
        if end > bytes.len() {
            return None;
        }
        cursor = end;
        if marker[1] == 0xDA {
            cursor = scan_end(bytes, cursor);
        }
    }
}

/// The offset of the marker that ends the entropy-coded data starting at
/// `cursor`, or the end of `bytes`.
fn scan_end(bytes: &[u8], mut cursor: usize) -> usize {
    while let Some(offset) = bytes.get(cursor..).and_then(|rest| find(rest, &[0xFF])) {
        let at = cursor + offset;
        match bytes.get(at + 1) {
            // Stuffed data byte, restart marker, or fill before a marker.
            Some(0x00 | 0xD0..=0xD7) => cursor = at + 2,
            Some(0xFF) => cursor = at + 1,
            Some(_) => return at,
            None => return bytes.len(),
        }
    }
    bytes.len()
}

/// The first offset of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Endian {
    Little,
    Big,
}

/// The offset of a JPEG's TIFF header inside its EXIF segment, and its byte
/// order. Every read is bounds-checked against the slice rather than against a
/// length the file claims.
fn jpeg_exif_start(bytes: &[u8]) -> Option<(usize, Endian)> {
    if bytes.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut cursor = 2usize;
    let exif = loop {
        let header = bytes.get(cursor..cursor + 4)?;
        if header[0] != 0xFF || header[1] == 0xDA || header[1] == 0xD9 {
            return None;
        }
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        if length < 2 {
            return None;
        }
        let start = cursor + 4;
        let end = start.checked_add(length - 2)?;
        if end > bytes.len() {
            return None;
        }
        if header[1] == 0xE1 && bytes.get(start..start + 6) == Some(b"Exif\0\0") {
            break start + 6;
        }
        cursor = end;
    };

    let endian = match bytes.get(exif..exif + 2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    Some((exif, endian))
}

/// Every tag id in IFD0 and, when it points at one, the Exif sub-IFD.
fn ifd_tags(bytes: &[u8], exif: usize, endian: Endian) -> Vec<u16> {
    let mut tags = Vec::new();
    let mut pending = Vec::new();
    if let Some(offset) = read_u32(bytes, exif + 4, endian) {
        pending.push(offset as usize);
    }
    let mut walked = 0usize;
    while let Some(relative) = pending.pop() {
        // Two directories at most: IFD0 and the Exif sub-IFD it names. A file
        // may claim a chain of pointers, and following it is how a walk over
        // hostile input becomes unbounded.
        walked += 1;
        if walked > 2 {
            break;
        }
        let Some(ifd) = exif.checked_add(relative) else {
            break;
        };
        let Some(count) = read_u16(bytes, ifd, endian) else {
            break;
        };
        for index in 0..count as usize {
            let Some(entry) = ifd.checked_add(2 + index * 12) else {
                break;
            };
            if entry + 12 > bytes.len() {
                break;
            }
            let Some(tag) = read_u16(bytes, entry, endian) else {
                break;
            };
            tags.push(tag);
            // The Exif sub-IFD holds the original timestamps, so it is walked
            // too — but only once.
            if tag == 0x8769 {
                if let Some(offset) = read_u32(bytes, entry + 8, endian) {
                    pending.push(offset as usize);
                }
            }
        }
    }
    tags
}

fn read_u16(bytes: &[u8], offset: usize, endian: Endian) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    let pair = [slice[0], slice[1]];
    Some(match endian {
        Endian::Little => u16::from_le_bytes(pair),
        Endian::Big => u16::from_be_bytes(pair),
    })
}

fn read_u32(bytes: &[u8], offset: usize, endian: Endian) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    let quad = [slice[0], slice[1], slice[2], slice[3]];
    Some(match endian {
        Endian::Little => u32::from_le_bytes(quad),
        Endian::Big => u32::from_be_bytes(quad),
    })
}

/// Reads the comment values a FLAC currently carries, for the fields this suite
/// offers. Used to fill a correction form with what is really in the file
/// rather than with what the catalogue remembered.
#[must_use]
pub fn read_flac_tags(bytes: &[u8]) -> Option<Vec<(TagField, String)>> {
    if bytes.get(0..4)? != b"fLaC" {
        return None;
    }
    let mut cursor = 4usize;
    let mut found = Vec::new();
    loop {
        let header = bytes.get(cursor..cursor + 4)?;
        let last = header[0] & FLAC_LAST_BLOCK != 0;
        let kind = header[0] & 0x7F;
        let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
        let start = cursor + 4;
        let end = start.checked_add(length)?;
        let body = bytes.get(start..end)?;

        if kind == FLAC_VORBIS_COMMENT {
            let mut inner = 0usize;
            let vendor_length = read_u32_le(body, inner)? as usize;
            inner += 4 + vendor_length;
            let count = read_u32_le(body, inner)? as usize;
            inner += 4;
            for _ in 0..count {
                let entry_length = read_u32_le(body, inner)? as usize;
                inner += 4;
                let entry = body.get(inner..inner.checked_add(entry_length)?)?;
                inner += entry_length;
                let Ok(text) = std::str::from_utf8(entry) else {
                    continue;
                };
                let Some((key, value)) = text.split_once('=') else {
                    continue;
                };
                if let Some(field) = TagField::ALL
                    .into_iter()
                    .find(|field| key.eq_ignore_ascii_case(field.vorbis_key()))
                {
                    found.push((field, value.to_owned()));
                }
            }
        }

        cursor = end;
        if last {
            break;
        }
    }
    Some(found)
}

/// Reports a rejection a host can state, for the requests this module refuses
/// before it opens anything.
#[must_use]
pub fn judge(format: Option<MetadataFormat>, change: &TagChange) -> Result<(), MetadataRejected> {
    let Some(format) = format else {
        return Err(MetadataRejected::NotSupported);
    };
    if change.is_empty() {
        return Err(MetadataRejected::NothingRequested);
    }
    if !format.writes_tags() {
        return Err(MetadataRejected::ReadOnlyContainer);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;
    use fluorita_core::{
        MetadataFormat, MetadataRejected, PrivateFact, SaveChoice, TagChange, TagField,
    };
    use siderita_ops::OpError;

    use super::{
        judge, private_facts, read_flac_tags, strip_jpeg_exif, write, write_flac_tags, Bin,
        MetadataRequest,
    };
    use crate::error::EngineError;

    /// The audio frames every test asserts survive untouched.
    const FRAMES: &[u8] = b"\xff\xf8these are the audio frames and they must not move";

    /// A FLAC with a STREAMINFO block, an optional comment block, a padding
    /// block and some frames. Hand-built so the test says what it is testing.
    fn flac(comments: Option<&[(&str, &str)]>) -> Vec<u8> {
        let mut out = b"fLaC".to_vec();
        // STREAMINFO: kept exactly, and never last once other blocks follow.
        let streaminfo = vec![0x11u8; 34];
        out.push(0x00);
        out.extend_from_slice(&(streaminfo.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(&streaminfo);

        if let Some(entries) = comments {
            let mut block = Vec::new();
            let vendor = b"reference libFLAC";
            block.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
            block.extend_from_slice(vendor);
            block.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for (key, value) in entries {
                let entry = format!("{key}={value}").into_bytes();
                block.extend_from_slice(&(entry.len() as u32).to_le_bytes());
                block.extend_from_slice(&entry);
            }
            out.push(0x04);
            out.extend_from_slice(&(block.len() as u32).to_be_bytes()[1..]);
            out.extend_from_slice(&block);
        }

        // PADDING, last: a rewrite drops it, which is what padding is for.
        let padding = vec![0u8; 64];
        out.push(0x80 | 0x01);
        out.extend_from_slice(&(padding.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(&padding);

        out.extend_from_slice(FRAMES);
        out
    }

    fn change(field: TagField, value: &str) -> TagChange {
        let mut change = TagChange::new();
        change.set(field, value).expect("a valid value");
        change
    }

    fn tags_of(bytes: &[u8]) -> Vec<(TagField, String)> {
        read_flac_tags(bytes).expect("a readable FLAC")
    }

    fn frames_of(bytes: &[u8]) -> &[u8] {
        let start = bytes
            .windows(FRAMES.len())
            .position(|window| window == FRAMES)
            .expect("the frames are still in the file");
        &bytes[start..]
    }

    /// Records what it was sent and keeps it in `keep_in`, when one is given,
    /// so a test can find the original there afterwards.
    #[derive(Default)]
    struct FakeBin {
        asked_for: RefCell<Vec<PathBuf>>,
        keep_in: Option<PathBuf>,
    }

    impl FakeBin {
        fn keeping(directory: &Path) -> Self {
            let keep_in = directory.join("bin");
            std::fs::create_dir_all(&keep_in).expect("the bin");
            Self {
                keep_in: Some(keep_in),
                ..Self::default()
            }
        }
    }

    impl Bin for FakeBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            self.asked_for.borrow_mut().push(path.to_path_buf());
            let name = path.file_name().unwrap_or_default();
            match &self.keep_in {
                Some(directory) => {
                    let kept = directory.join(name);
                    std::fs::rename(path, &kept).map_err(|error| OpError::io(path, &error))?;
                    Ok(kept)
                }
                None => {
                    std::fs::remove_file(path).map_err(|error| OpError::io(path, &error))?;
                    Ok(PathBuf::from("/trash").join(name))
                }
            }
        }
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-metadata-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("test directory");
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).expect("test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn correcting_a_tag_leaves_the_audio_frames_byte_for_byte() {
        let original = flac(Some(&[("TITLE", "Pavana"), ("ARTIST", "Ravel")]));
        let rewritten = write_flac_tags(&original, &change(TagField::Artist, "Maurice Ravel"))
            .expect("written");

        assert_eq!(
            frames_of(&rewritten),
            FRAMES,
            "the samples must come out exactly as they went in"
        );
        assert_eq!(
            tags_of(&rewritten),
            vec![
                (TagField::Title, "Pavana".to_owned()),
                (TagField::Artist, "Maurice Ravel".to_owned())
            ]
        );
    }

    #[test]
    fn every_other_metadata_block_survives_and_padding_does_not() {
        let original = flac(Some(&[("TITLE", "Pavana")]));
        let rewritten =
            write_flac_tags(&original, &change(TagField::Album, "Suite")).expect("written");

        assert!(
            rewritten.windows(34).any(|window| window == [0x11u8; 34]),
            "STREAMINFO was not carried across"
        );
        assert!(
            rewritten.len() < original.len() + 32,
            "the dropped padding should not come back as growth"
        );
        assert_eq!(frames_of(&rewritten), FRAMES);
    }

    #[test]
    fn a_file_with_no_comment_block_gains_one() {
        let original = flac(None);
        assert!(tags_of(&original).is_empty());

        let rewritten =
            write_flac_tags(&original, &change(TagField::Title, "Pavana")).expect("written");
        assert_eq!(
            tags_of(&rewritten),
            vec![(TagField::Title, "Pavana".to_owned())]
        );
        assert_eq!(frames_of(&rewritten), FRAMES);
    }

    #[test]
    fn emptying_a_field_removes_the_comment_rather_than_writing_an_empty_one() {
        let original = flac(Some(&[("TITLE", "Pavana"), ("ALBUM", "Suite")]));
        let rewritten = write_flac_tags(&original, &change(TagField::Album, "")).expect("written");

        assert_eq!(
            tags_of(&rewritten),
            vec![(TagField::Title, "Pavana".to_owned())],
            "an emptied tag leaves no empty tag behind"
        );
    }

    #[test]
    fn a_comment_this_suite_does_not_offer_is_carried_across_untouched() {
        let original = flac(Some(&[
            ("TITLE", "Pavana"),
            ("LYRICS", "do not touch these"),
        ]));
        let rewritten =
            write_flac_tags(&original, &change(TagField::Title, "Pavane")).expect("written");

        assert!(
            String::from_utf8_lossy(&rewritten).contains("LYRICS=do not touch these"),
            "a field the library does not project must not be dropped by a correction"
        );
    }

    #[test]
    fn a_key_is_matched_however_it_was_spelled() {
        let original = flac(Some(&[("title", "Pavana")]));
        let rewritten =
            write_flac_tags(&original, &change(TagField::Title, "Pavane")).expect("written");
        assert_eq!(
            tags_of(&rewritten),
            vec![(TagField::Title, "Pavane".to_owned())],
            "the old lower-case comment was replaced, not duplicated"
        );
    }

    #[test]
    fn a_container_that_is_not_a_flac_or_that_lies_about_its_lengths_is_refused() {
        assert_eq!(
            write_flac_tags(b"not a flac", &change(TagField::Title, "x")),
            None
        );

        let mut lying = flac(Some(&[("TITLE", "Pavana")]));
        // Claim a metadata block far longer than the file.
        lying[5] = 0xFF;
        lying[6] = 0xFF;
        lying[7] = 0xFF;
        assert_eq!(write_flac_tags(&lying, &change(TagField::Title, "x")), None);

        assert_eq!(
            write_flac_tags(&flac(None)[..3], &change(TagField::Title, "x")),
            None
        );
    }

    /// A JPEG carrying an EXIF segment with the tags the surface offers to
    /// remove, plus a comment segment that must survive.
    fn photograph(with_exif: bool) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        let entries: [(u16, u32); 3] = [(0x010F, 0), (0x0132, 0), (0x8825, 0)];
        tiff.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (tag, value) in entries {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&3u16.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        }
        tiff.extend_from_slice(&0u32.to_le_bytes());

        let mut out = vec![0xFF, 0xD8];
        if with_exif {
            let length = (2 + 6 + tiff.len()) as u16;
            out.extend_from_slice(&[0xFF, 0xE1]);
            out.extend_from_slice(&length.to_be_bytes());
            out.extend_from_slice(b"Exif\0\0");
            out.extend_from_slice(&tiff);
        }
        // A comment segment: not EXIF, so it must survive the strip.
        let comment = b"made by the author";
        out.extend_from_slice(&[0xFF, 0xFE]);
        out.extend_from_slice(&((comment.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(comment);

        // A start-of-scan header with its length, as every real JPEG has: the
        // strip walks the scan to find where the primary picture ends.
        out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        out.extend_from_slice(b"entropy-coded-picture-data");
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    /// A JPEG from its header segments, one scan of `entropy` bytes and an
    /// EOI, followed by `trailing` bytes.
    fn jpeg(segments: &[(u8, Vec<u8>)], entropy: &[u8], trailing: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        for (marker, body) in segments {
            out.extend_from_slice(&[0xFF, *marker]);
            out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
            out.extend_from_slice(body);
        }
        // A start-of-scan header with one component, then the scan itself.
        out.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        out.extend_from_slice(entropy);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out.extend_from_slice(trailing);
        out
    }

    fn xmp(body: &str) -> (u8, Vec<u8>) {
        let mut segment = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        segment.extend_from_slice(body.as_bytes());
        (0xE1, segment)
    }

    fn extended_xmp(body: &str) -> (u8, Vec<u8>) {
        let mut segment = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
        segment.extend_from_slice(&[b'0'; 32]);
        segment.extend_from_slice(&[0, 0, 0, 64, 0, 0, 0, 0]);
        segment.extend_from_slice(body.as_bytes());
        (0xE1, segment)
    }

    fn comment() -> (u8, Vec<u8>) {
        (0xFE, b"made by the author".to_vec())
    }

    const GPS_XMP: &str = "<x:xmpmeta><rdf:Description exif:GPSLatitude=\"40,25.1N\" \
                           exif:GPSLongitude=\"3,42.2W\"/></x:xmpmeta>";

    /// Entropy-coded bytes a naive scan would misread: a stuffed `FF 00`, a
    /// restart marker, and fill bytes before one.
    const TRICKY_SCAN: &[u8] = b"scan\xFF\x00data\xFF\xD0more\xFF\xFF\xD1end";

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn a_location_carried_only_in_xmp_is_reported() {
        let photo = jpeg(&[xmp(GPS_XMP), comment()], b"pixels", b"");
        assert_eq!(private_facts(&photo), vec![PrivateFact::Location]);

        let dated = jpeg(
            &[xmp(
                "<rdf:Description xmp:CreateDate=\"2026\" tiff:Model=\"X\"/>",
            )],
            b"pixels",
            b"",
        );
        let facts = private_facts(&dated);
        assert!(facts.contains(&PrivateFact::Timestamp));
        assert!(facts.contains(&PrivateFact::Camera));
        assert!(!facts.contains(&PrivateFact::Location));
    }

    #[test]
    fn removing_a_location_strips_xmp_and_extended_xmp_and_keeps_the_picture() {
        let photo = jpeg(
            &[
                xmp(GPS_XMP),
                extended_xmp("<exif:GPSLatitude>40,25.1N</exif:GPSLatitude>"),
                comment(),
            ],
            TRICKY_SCAN,
            b"",
        );

        let stripped = strip_jpeg_exif(&photo, &[PrivateFact::Location]).expect("stripped");

        assert!(private_facts(&stripped).is_empty());
        assert!(!contains(&stripped, b"GPS"), "no GPS key survives");
        assert!(
            !contains(&stripped, b"ns.adobe.com"),
            "no XMP packet survives"
        );
        assert!(contains(&stripped, b"made by the author"));
        assert!(
            stripped.ends_with(&[TRICKY_SCAN, &[0xFF, 0xD9][..]].concat()),
            "the scan is copied across byte for byte"
        );
    }

    #[test]
    fn a_gps_key_split_across_extended_xmp_chunks_is_still_found() {
        let chunk = |offset: u32, text: &str| {
            let mut segment = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
            segment.extend_from_slice(&[b'A'; 32]);
            segment.extend_from_slice(&64u32.to_be_bytes());
            segment.extend_from_slice(&offset.to_be_bytes());
            segment.extend_from_slice(text.as_bytes());
            (0xE1, segment)
        };
        // Written out of order, and the key cut in the middle.
        let photo = jpeg(
            &[chunk(12, "SLatitude=\"40N\""), chunk(0, "<rdf exif:GP")],
            b"pixels",
            b"",
        );
        assert_eq!(private_facts(&photo), vec![PrivateFact::Location]);
        let stripped = strip_jpeg_exif(&photo, &private_facts(&photo)).expect("stripped");
        assert!(!contains(&stripped, b"Latitude"));
    }

    #[test]
    fn an_extended_xmp_packet_is_joined_only_as_far_as_it_is_bounded() {
        let chunk = |full: u32, offset: u32, text: &str| {
            let mut segment = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
            segment.extend_from_slice(&[b'B'; 32]);
            segment.extend_from_slice(&full.to_be_bytes());
            segment.extend_from_slice(&offset.to_be_bytes());
            segment.extend_from_slice(text.as_bytes());
            (0xE1, segment)
        };

        // A packet that declares 8 bytes: the chunk past them is not read.
        let short = jpeg(
            &[chunk(8, 0, "<rdf abc"), chunk(8, 8, "exif:GPSLatitude")],
            b"pixels",
            b"",
        );
        assert!(private_facts(&short).is_empty());
        let stripped = strip_jpeg_exif(&short, &[PrivateFact::Location]).expect("stripped");
        assert!(
            !contains(&stripped, b"GPSLatitude"),
            "a chunk not read is still stripped"
        );

        // A packet that declares far more than it holds, in more chunks than
        // are joined: what is joined stays bounded.
        let many: Vec<(u8, Vec<u8>)> = (0..100u32)
            .map(|index| chunk(u32::MAX, index * 4, "abcd"))
            .collect();
        let photo = jpeg(&many, b"pixels", b"");
        let (segments, _) = super::headers(&photo).expect("headers");
        let packets = super::xmp_packets(&photo, &segments);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].len(), super::MAX_EXTENDED_XMP_CHUNKS * 4);
    }

    #[test]
    fn secondary_images_after_the_primary_are_reported_and_dropped() {
        // A phone's multi-picture file: an MPF index in the primary, and a
        // second JPEG after the primary's EOI that carries its own location.
        let secondary = jpeg(&[xmp(GPS_XMP)], b"secondary pixels", b"");
        let mut mpf = b"MPF\0".to_vec();
        mpf.extend_from_slice(b"II*\0index");
        let photo = jpeg(&[(0xE2, mpf), comment()], TRICKY_SCAN, &secondary);

        assert_eq!(
            private_facts(&photo),
            vec![PrivateFact::Location],
            "the secondary image's location is reported"
        );

        let stripped = strip_jpeg_exif(&photo, &[PrivateFact::Location]).expect("stripped");
        assert!(!contains(&stripped, b"secondary pixels"));
        assert!(!contains(&stripped, b"MPF\0"));
        assert!(!contains(&stripped, b"GPS"));
        assert!(stripped.ends_with(&[TRICKY_SCAN, &[0xFF, 0xD9][..]].concat()));
        assert!(private_facts(&stripped).is_empty());
    }

    #[test]
    fn every_scan_of_a_progressive_primary_survives_the_strip() {
        // Two scans with a table segment between them, then the EOI.
        let mut photo = jpeg(&[xmp(GPS_XMP)], b"first scan", b"");
        photo.truncate(photo.len() - 2);
        photo.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x05, 0x10, 0x20, 0x30]);
        photo.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        photo.extend_from_slice(b"second scan");
        photo.extend_from_slice(&[0xFF, 0xD9]);
        photo.extend_from_slice(b"trailing");

        let stripped = strip_jpeg_exif(&photo, &[PrivateFact::Location]).expect("stripped");
        assert!(contains(&stripped, b"first scan"));
        assert!(contains(
            &stripped,
            &[0xFF, 0xC4, 0x00, 0x05, 0x10, 0x20, 0x30]
        ));
        assert!(stripped.ends_with(b"second scan\xFF\xD9"));
    }

    fn cover<'a>(bytes: &'a [u8]) -> super::Cover<'a> {
        super::Cover {
            bytes,
            mime: "image/jpeg",
            width: 600,
            height: 600,
        }
    }

    #[test]
    fn embedding_a_cover_keeps_the_audio_and_replaces_the_front_cover_it_had() {
        let original = flac(Some(&[("TITLE", "Pavana")]));
        let first =
            super::embed_flac_cover(&original, &cover(b"first-cover-bytes")).expect("written");
        assert_eq!(frames_of(&first), FRAMES);
        assert!(String::from_utf8_lossy(&first).contains("first-cover-bytes"));
        assert_eq!(
            tags_of(&first),
            vec![(TagField::Title, "Pavana".to_owned())],
            "embedding a cover does not disturb the tags"
        );

        let second =
            super::embed_flac_cover(&first, &cover(b"second-cover-bytes")).expect("written");
        let text = String::from_utf8_lossy(&second);
        assert!(text.contains("second-cover-bytes"));
        assert!(
            !text.contains("first-cover-bytes"),
            "a new front cover replaces the old one instead of joining it"
        );
        assert_eq!(frames_of(&second), FRAMES);
    }

    #[test]
    fn a_cover_block_records_what_the_specification_asks_for() {
        let embedded = super::embed_flac_cover(&flac(None), &cover(b"bytes")).expect("written");
        let start = embedded
            .windows(10)
            .position(|window| window == b"image/jpeg")
            .expect("the media type is recorded");
        // Four bytes of picture type, then the length of the media type.
        assert_eq!(embedded[start - 8..start - 4], 3u32.to_be_bytes());
        assert_eq!(embedded[start - 4..start], 10u32.to_be_bytes());
        assert_eq!(frames_of(&embedded), FRAMES);
    }

    #[test]
    fn a_photograph_says_what_it_is_carrying() {
        let facts = private_facts(&photograph(true));
        assert!(facts.contains(&PrivateFact::Location));
        assert!(facts.contains(&PrivateFact::Camera));
        assert!(facts.contains(&PrivateFact::Timestamp));

        assert!(
            private_facts(&photograph(false)).is_empty(),
            "a picture with no EXIF carries nothing to remove"
        );
        assert!(private_facts(b"not a jpeg").is_empty());
    }

    #[test]
    fn stripping_removes_the_exif_and_keeps_everything_else() {
        let original = photograph(true);
        let stripped = strip_jpeg_exif(&original, &[PrivateFact::Location]).expect("stripped");

        assert!(private_facts(&stripped).is_empty());
        assert!(
            String::from_utf8_lossy(&stripped).contains("made by the author"),
            "a segment that is not EXIF must survive"
        );
        assert!(
            String::from_utf8_lossy(&stripped).contains("entropy-coded-picture-data"),
            "the picture itself must survive"
        );
        assert!(stripped.len() < original.len());
    }

    #[test]
    fn stripping_nothing_changes_nothing() {
        let original = photograph(true);
        assert_eq!(
            strip_jpeg_exif(&original, &[]).as_deref(),
            Some(&original[..])
        );
        assert_eq!(
            strip_jpeg_exif(b"not a jpeg", &[PrivateFact::Location]),
            None
        );
    }

    #[test]
    fn a_copy_lands_beside_the_original_and_a_replacement_takes_its_place() {
        let directory = TestDir::new("write");
        let original = flac(Some(&[("TITLE", "Pavana")]));
        let source = directory.file("pista.flac", &original);
        let tags = change(TagField::Artist, "Ravel");
        let bin = FakeBin::keeping(&directory.0);

        let copy = write(
            &MetadataRequest {
                source: &source,
                tags: &tags,
                strip: &[],
                cover: None,
                choice: SaveChoice::Copy,
                copy_marker: "editado",
            },
            &bin,
            &CancellationToken::new(),
        )
        .expect("the copy lands");

        assert_eq!(copy.written, directory.0.join("pista (editado).flac"));
        assert_eq!(copy.trashed_original, None);
        assert!(source.exists());
        assert_eq!(copy.stream_bytes, FRAMES.len() as u64);
        assert_eq!(
            frames_of(&std::fs::read(&copy.written).expect("the copy")),
            FRAMES
        );

        let replacement = write(
            &MetadataRequest {
                source: &source,
                tags: &tags,
                strip: &[],
                cover: None,
                choice: SaveChoice::Replace,
                copy_marker: "editado",
            },
            &bin,
            &CancellationToken::new(),
        )
        .expect("the replacement lands");

        assert_eq!(replacement.written, source);
        // ADR 0009: the original reaches the Trash even though the result
        // keeps its name, so a retag can always be undone.
        let trashed = replacement
            .trashed_original
            .expect("a replacement trashes its original");
        assert_eq!(
            bin.asked_for.borrow().as_slice(),
            std::slice::from_ref(&source)
        );
        assert_eq!(
            std::fs::read(&trashed).expect("the original, in the bin"),
            original
        );
        assert_eq!(
            tags_of(&std::fs::read(&source).expect("the file")),
            vec![
                (TagField::Title, "Pavana".to_owned()),
                (TagField::Artist, "Ravel".to_owned())
            ]
        );
    }

    #[test]
    fn removing_a_location_in_place_keeps_a_private_photographs_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new("strip-in-place");
        let original = photograph(true);
        let source = directory.file("foto.jpg", &original);
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600))
            .expect("a private photograph");
        let bin = FakeBin::keeping(&directory.0);

        let written = write(
            &MetadataRequest {
                source: &source,
                tags: &TagChange::new(),
                strip: &[PrivateFact::Location],
                cover: None,
                choice: SaveChoice::Replace,
                copy_marker: "editado",
            },
            &bin,
            &CancellationToken::new(),
        )
        .expect("the replacement lands");

        assert_eq!(written.written, source);
        assert!(private_facts(&std::fs::read(&source).expect("the result")).is_empty());
        assert_eq!(
            std::fs::metadata(&source)
                .expect("the result")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "removing a location must not make the photograph readable by others"
        );
        let trashed = written.trashed_original.expect("the original is trashed");
        assert_eq!(std::fs::read(trashed).expect("the original"), original);
    }

    #[test]
    fn a_container_this_suite_cannot_write_is_refused_before_anything_lands() {
        let directory = TestDir::new("refused");
        let source = directory.file("pista.mp3", b"ID3 and then some audio");
        let tags = change(TagField::Title, "Pavana");

        let failure = write(
            &MetadataRequest {
                source: &source,
                tags: &tags,
                strip: &[],
                cover: None,
                choice: SaveChoice::Replace,
                copy_marker: "editado",
            },
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect_err("refused");

        assert!(matches!(failure, EngineError::Undecodable { .. }));
        assert_eq!(
            std::fs::read(&source).expect("the original"),
            b"ID3 and then some audio".to_vec(),
            "a refused write leaves the file exactly as it was"
        );
    }

    #[test]
    fn a_cancelled_or_relative_request_never_writes() {
        let directory = TestDir::new("cancelled");
        let source = directory.file("pista.flac", &flac(None));
        let tags = change(TagField::Title, "Pavana");
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let failure = write(
            &MetadataRequest {
                source: &source,
                tags: &tags,
                strip: &[],
                cover: None,
                choice: SaveChoice::Copy,
                copy_marker: "editado",
            },
            &FakeBin::default(),
            &cancellation,
        )
        .expect_err("cancelled");
        assert!(matches!(failure, EngineError::Cancelled));
        assert!(!directory.0.join("pista (editado).flac").exists());

        let relative = write(
            &MetadataRequest {
                source: Path::new("pista.flac"),
                tags: &tags,
                strip: &[],
                cover: None,
                choice: SaveChoice::Copy,
                copy_marker: "editado",
            },
            &FakeBin::default(),
            &CancellationToken::new(),
        )
        .expect_err("refused");
        assert!(matches!(relative, EngineError::UnusableSource { .. }));
    }

    #[test]
    fn the_refusals_a_surface_can_state_are_decided_before_a_file_is_opened() {
        let empty = TagChange::new();
        assert_eq!(judge(None, &empty), Err(MetadataRejected::NotSupported));
        assert_eq!(
            judge(Some(MetadataFormat::Flac), &empty),
            Err(MetadataRejected::NothingRequested)
        );
        assert_eq!(
            judge(Some(MetadataFormat::Id3), &change(TagField::Title, "x")),
            Err(MetadataRejected::ReadOnlyContainer)
        );
        assert_eq!(
            judge(Some(MetadataFormat::Flac), &change(TagField::Title, "x")),
            Ok(())
        );
    }
}
