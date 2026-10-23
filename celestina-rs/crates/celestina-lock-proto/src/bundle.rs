//! The backdrop bundle, format version 1 (design §4.3 (a)).
//!
//! The shell writes it into a sealed memfd and hands it to the lock as stdin;
//! the lock reads it back with [`decode`]. Little-endian:
//!
//! ```text
//! magic           4 bytes  "CLKB"
//! version         u16      1
//! prior_failures  u8       counted failures since the last unlock
//! offset_known    u8       1 when utc_offset_s was read for this spawn, else 0
//! utc_offset_s    i32      the local UTC offset
//! outputs         u8       0..=8
//! per output:
//!   name_len      u8       1..=64, then the connector name, UTF-8
//!   width, height u16 each 1..=640
//!   pixels        width x height x 4, RGBA8, opaque
//! ```
//!
//! The format is frozen with the lock's first release (design §0): CEL-2-H-a
//! sends only the header, CEL-2-H-b adds the pixel blocks, and neither may
//! change a byte of this layout.
//!
//! The bundle comes from another process and is read hostile. Nothing is
//! allocated for a declared size before the bytes it covers are known to be
//! present, and a bad bundle degrades to black: [`decode`] never fails, it
//! says what it kept and why it dropped the rest.

use std::error::Error;
use std::fmt;

/// The first four bytes of every bundle.
pub const MAGIC: [u8; 4] = *b"CLKB";
/// The only format version.
const VERSION: u16 = 1;
/// The bytes before the first output block.
pub const HEADER_LEN: usize = 13;
/// The most outputs a bundle carries; the lock shows any other output black.
pub const MAX_OUTPUTS: usize = 8;
/// The longest connector name, in bytes.
pub const NAME_MAX: usize = 64;
/// The longest side of a block, in pixels.
pub const SIDE_MAX: u16 = 640;
/// The largest bundle the lock reads: 16 MiB. Eight maximal blocks take 13 107 765
/// bytes, so the format always fits.
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Why a block cannot exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockError {
    /// The connector name is empty or longer than [`NAME_MAX`] bytes.
    NameLength(usize),
    /// The connector name is not UTF-8.
    NameNotUtf8,
    /// A side is 0 or past [`SIDE_MAX`].
    Side(u16),
    /// The pixels are not `width x height x 4` bytes.
    PixelCount { expected: usize, found: usize },
}

impl fmt::Display for BlockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameLength(len) => {
                write!(
                    formatter,
                    "output name of {len} bytes (1..={NAME_MAX} allowed)"
                )
            }
            Self::NameNotUtf8 => formatter.write_str("output name is not UTF-8"),
            Self::Side(side) => {
                write!(
                    formatter,
                    "output side of {side} pixels (1..={SIDE_MAX} allowed)"
                )
            }
            Self::PixelCount { expected, found } => {
                write!(
                    formatter,
                    "{found} pixel bytes where {expected} were expected"
                )
            }
        }
    }
}

impl Error for BlockError {}

/// One output's picture: a connector name and an opaque RGBA8 image.
///
/// The fields are private so every block in existence passed
/// [`OutputBlock::new`]'s bounds, which are the format's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputBlock {
    name: String,
    width: u16,
    height: u16,
    pixels: Vec<u8>,
}

impl OutputBlock {
    /// A block, if its parts fit the format.
    ///
    /// # Errors
    ///
    /// The first bound the parts break: the name's length, a side, then the
    /// pixel count.
    pub fn new(
        name: impl Into<String>,
        width: u16,
        height: u16,
        pixels: Vec<u8>,
    ) -> Result<Self, BlockError> {
        let name = name.into();
        if name.is_empty() || name.len() > NAME_MAX {
            return Err(BlockError::NameLength(name.len()));
        }
        for side in [width, height] {
            if side == 0 || side > SIDE_MAX {
                return Err(BlockError::Side(side));
            }
        }
        let expected = pixel_bytes(width, height);
        if pixels.len() != expected {
            return Err(BlockError::PixelCount {
                expected,
                found: pixels.len(),
            });
        }
        Ok(Self {
            name,
            width,
            height,
            pixels,
        })
    }

    /// The connector name, as the compositor calls the output.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// `width x height x 4` bytes of RGBA8, opaque.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn encoded_len(&self) -> usize {
        1 + self.name.len() + 2 + 2 + self.pixels.len()
    }
}

/// The bytes of a `width x height` RGBA8 image. Callers pass sides already in
/// `1..=SIDE_MAX`, so the product cannot overflow.
fn pixel_bytes(width: u16, height: u16) -> usize {
    usize::from(width) * usize::from(height) * 4
}

/// What the shell tells the lock at spawn time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bundle {
    /// Counted failures since the last unlock; seeds the lock's backoff.
    pub prior_failures: u8,
    /// Whether [`Bundle::utc_offset_s`] was read for this spawn.
    pub offset_known: bool,
    /// The local UTC offset, in seconds; 0 when it is not known.
    pub utc_offset_s: i32,
    /// The pictures, one per output, at most [`MAX_OUTPUTS`].
    pub outputs: Vec<OutputBlock>,
}

impl Bundle {
    /// The bundle CEL-2-H-a sends: the carried failures, no offset, no
    /// pictures.
    #[must_use]
    pub fn header_only(prior_failures: u8) -> Self {
        Self {
            prior_failures,
            ..Self::default()
        }
    }
}

/// The bytes of `bundle`, in the canonical form: an unknown offset is written
/// as 0, and only the first [`MAX_OUTPUTS`] outputs are sent (the caller
/// orders them by connector name).
#[must_use]
pub fn encode(bundle: &Bundle) -> Vec<u8> {
    let outputs = &bundle.outputs[..bundle.outputs.len().min(MAX_OUTPUTS)];
    let size = HEADER_LEN + outputs.iter().map(OutputBlock::encoded_len).sum::<usize>();
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.push(bundle.prior_failures);
    bytes.push(u8::from(bundle.offset_known));
    let offset = if bundle.offset_known {
        bundle.utc_offset_s
    } else {
        0
    };
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.push(u8::try_from(outputs.len()).unwrap_or(u8::MAX));
    for output in outputs {
        bytes.push(u8::try_from(output.name.len()).unwrap_or(u8::MAX));
        bytes.extend_from_slice(output.name.as_bytes());
        bytes.extend_from_slice(&output.width.to_le_bytes());
        bytes.extend_from_slice(&output.height.to_le_bytes());
        bytes.extend_from_slice(&output.pixels);
    }
    bytes
}

/// What the reader knows about the file the bytes came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileMeta {
    /// The size `fstat` reported.
    pub size: u64,
    /// Whether all four seals (shrink, grow, write, seal) are present.
    pub seals_ok: bool,
}

/// Why a whole bundle was dropped: the lock shows black and carries no
/// failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ignored {
    /// Over [`MAX_FILE_BYTES`].
    TooLarge,
    /// The bytes read are not the size `fstat` reported, or the size is not
    /// the one the header's blocks imply.
    SizeMismatch,
    /// One of the four seals is missing.
    MissingSeal,
    /// Shorter than the header.
    TooShort,
    /// The first four bytes are not [`MAGIC`].
    BadMagic,
    /// A version this lock does not read; a newer shell's bundle is not
    /// guessed at.
    BadVersion,
    /// `offset_known` is neither 0 nor 1.
    BadOffsetFlag,
    /// The header declares more than [`MAX_OUTPUTS`] outputs.
    TooManyOutputs,
}

/// How a decode ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every declared block was read and nothing is left over.
    Complete,
    /// A block broke a bound: that block and the rest were dropped, the
    /// header and the blocks before it were kept.
    BlockRejected { index: u8, why: BlockError },
    /// The whole bundle was dropped.
    Ignored(Ignored),
}

/// What [`decode`] kept, and how it ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// What to use: the default (black, no failures, no offset) when the
    /// whole bundle was ignored.
    pub bundle: Bundle,
    pub status: Status,
}

fn ignored(why: Ignored) -> Decoded {
    Decoded {
        bundle: Bundle::default(),
        status: Status::Ignored(why),
    }
}

/// A cursor over the bytes after the header.
struct Cursor<'a> {
    rest: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        if self.rest.len() < count {
            return None;
        }
        let (taken, rest) = self.rest.split_at(count);
        self.rest = rest;
        Some(taken)
    }
}

/// One block read, or why not.
enum Read {
    Block(OutputBlock),
    /// A field broke its bound.
    Rejected(BlockError),
    /// The file ends before the block does.
    Short,
}

fn read_block(cursor: &mut Cursor<'_>) -> Read {
    let Some(&[name_len]) = cursor.take(1) else {
        return Read::Short;
    };
    let name_len = usize::from(name_len);
    if name_len == 0 || name_len > NAME_MAX {
        return Read::Rejected(BlockError::NameLength(name_len));
    }
    let Some(head) = cursor.take(name_len + 4) else {
        return Read::Short;
    };
    let Some((name, &[w0, w1, h0, h1])) = head.split_last_chunk::<4>() else {
        return Read::Short;
    };
    let Ok(name) = std::str::from_utf8(name) else {
        return Read::Rejected(BlockError::NameNotUtf8);
    };
    let width = u16::from_le_bytes([w0, w1]);
    let height = u16::from_le_bytes([h0, h1]);
    for side in [width, height] {
        if side == 0 || side > SIDE_MAX {
            return Read::Rejected(BlockError::Side(side));
        }
    }
    // The pixels are copied only now, after the bytes they cover are known to
    // be there: the allocation is never larger than what the file holds.
    let Some(pixels) = cursor.take(pixel_bytes(width, height)) else {
        return Read::Short;
    };
    Read::Block(OutputBlock {
        name: name.to_owned(),
        width,
        height,
        pixels: pixels.to_vec(),
    })
}

/// Reads a bundle the shell handed over.
///
/// `bytes` is what was read from the file and `meta` what `fstat` and the
/// seal query said about it. Every bound of design §4.3 (a) is checked; a
/// violation drops the whole bundle, except a bad block, which drops itself
/// and the blocks after it and keeps the header.
#[must_use]
pub fn decode(bytes: &[u8], meta: FileMeta) -> Decoded {
    if meta.size > MAX_FILE_BYTES {
        return ignored(Ignored::TooLarge);
    }
    if !meta.seals_ok {
        return ignored(Ignored::MissingSeal);
    }
    if u64::try_from(bytes.len()) != Ok(meta.size) {
        return ignored(Ignored::SizeMismatch);
    }
    let Some((&header, body)) = bytes.split_first_chunk::<HEADER_LEN>() else {
        return ignored(Ignored::TooShort);
    };
    let [m0, m1, m2, m3, v0, v1, prior_failures, flag, o0, o1, o2, o3, count] = header;
    if [m0, m1, m2, m3] != MAGIC {
        return ignored(Ignored::BadMagic);
    }
    if u16::from_le_bytes([v0, v1]) != VERSION {
        return ignored(Ignored::BadVersion);
    }
    let offset_known = match flag {
        0 => false,
        1 => true,
        _ => return ignored(Ignored::BadOffsetFlag),
    };
    let offset = i32::from_le_bytes([o0, o1, o2, o3]);
    let count = usize::from(count);
    if count > MAX_OUTPUTS {
        return ignored(Ignored::TooManyOutputs);
    }

    let mut bundle = Bundle {
        prior_failures,
        offset_known,
        utc_offset_s: if offset_known { offset } else { 0 },
        outputs: Vec::with_capacity(count),
    };
    let mut cursor = Cursor { rest: body };
    for index in 0..count {
        match read_block(&mut cursor) {
            Read::Block(block) => bundle.outputs.push(block),
            Read::Rejected(why) => {
                return Decoded {
                    bundle,
                    status: Status::BlockRejected {
                        index: u8::try_from(index).unwrap_or(u8::MAX),
                        why,
                    },
                }
            }
            Read::Short => return ignored(Ignored::SizeMismatch),
        }
    }
    if !cursor.rest.is_empty() {
        return ignored(Ignored::SizeMismatch);
    }
    Decoded {
        bundle,
        status: Status::Complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str, width: u16, height: u16, seed: u8) -> OutputBlock {
        let len = usize::from(width) * usize::from(height) * 4;
        let pixels = (0..len).map(|i| seed.wrapping_add(i as u8)).collect();
        OutputBlock::new(name, width, height, pixels).expect("a valid block")
    }

    fn sample() -> Bundle {
        Bundle {
            prior_failures: 3,
            offset_known: true,
            utc_offset_s: -7200,
            outputs: vec![block("DP-1", 4, 3, 1), block("eDP-1", 2, 2, 9)],
        }
    }

    fn sealed(bytes: &[u8]) -> FileMeta {
        FileMeta {
            size: bytes.len() as u64,
            seals_ok: true,
        }
    }

    fn decode_sealed(bytes: &[u8]) -> Decoded {
        decode(bytes, sealed(bytes))
    }

    /// A header with the given fields and no blocks.
    fn header(version: u16, flag: u8, outputs: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"CLKB");
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.push(2); // prior_failures
        bytes.push(flag);
        bytes.extend_from_slice(&3600i32.to_le_bytes());
        bytes.push(outputs);
        bytes
    }

    /// A raw block with arbitrary fields, validated by nothing.
    fn raw_block(name: &[u8], width: u16, height: u16, pixels: usize) -> Vec<u8> {
        let mut bytes = vec![u8::try_from(name.len()).expect("a short name")];
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0xAB, pixels));
        bytes
    }

    #[test]
    fn t_b1_bundle_round_trip() {
        let bundle = sample();
        let bytes = encode(&bundle);
        assert_eq!(&bytes[..4], b"CLKB");
        assert_eq!(&bytes[4..6], &1u16.to_le_bytes());
        assert_eq!(bytes[6], 3);
        assert_eq!(bytes[7], 1);
        assert_eq!(&bytes[8..12], &(-7200i32).to_le_bytes());
        assert_eq!(bytes[12], 2);
        let decoded = decode_sealed(&bytes);
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle, bundle);
        assert_eq!(decoded.bundle.outputs[0].name(), "DP-1");
        assert_eq!(decoded.bundle.outputs[1].width(), 2);
    }

    #[test]
    fn a_header_only_bundle_has_zero_outputs() {
        let bundle = Bundle::header_only(5);
        assert_eq!(bundle.prior_failures, 5);
        assert!(!bundle.offset_known);
        assert!(bundle.outputs.is_empty());
        let bytes = encode(&bundle);
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(bytes[12], 0);
        let decoded = decode_sealed(&bytes);
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle, bundle);
    }

    #[test]
    fn an_unknown_offset_is_written_and_read_as_zero() {
        let bundle = Bundle {
            prior_failures: 0,
            offset_known: false,
            utc_offset_s: 12345,
            outputs: Vec::new(),
        };
        let bytes = encode(&bundle);
        assert_eq!(bytes[7], 0);
        assert_eq!(&bytes[8..12], &0i32.to_le_bytes());
        let decoded = decode_sealed(&header(1, 0, 0));
        assert!(!decoded.bundle.offset_known);
        assert_eq!(decoded.bundle.utc_offset_s, 0);
        assert_eq!(decoded.bundle.prior_failures, 2);
    }

    #[test]
    fn t_b2_each_bound_of_the_bundle() {
        let good = encode(&sample());

        // File size over 16 MiB (by fstat) and a size that is not the one read.
        let over = FileMeta {
            size: MAX_FILE_BYTES + 1,
            seals_ok: true,
        };
        assert_eq!(
            decode(&good, over).status,
            Status::Ignored(Ignored::TooLarge)
        );
        let at_limit = FileMeta {
            size: MAX_FILE_BYTES,
            seals_ok: true,
        };
        assert_eq!(
            decode(&good, at_limit).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
        let short = FileMeta {
            size: good.len() as u64 + 1,
            seals_ok: true,
        };
        assert_eq!(
            decode(&good, short).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
        let mut longer = good.clone();
        longer.push(0);
        assert_eq!(
            decode(&longer, sealed(&longer)).status,
            Status::Ignored(Ignored::SizeMismatch),
            "trailing bytes"
        );

        // A missing seal.
        let unsealed = FileMeta {
            size: good.len() as u64,
            seals_ok: false,
        };
        assert_eq!(
            decode(&good, unsealed).status,
            Status::Ignored(Ignored::MissingSeal)
        );

        // Bad magic, version 2, a short header.
        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            decode_sealed(&bad_magic).status,
            Status::Ignored(Ignored::BadMagic)
        );
        assert_eq!(
            decode_sealed(&header(2, 1, 0)).status,
            Status::Ignored(Ignored::BadVersion)
        );
        assert_eq!(
            decode_sealed(&good[..HEADER_LEN - 1]).status,
            Status::Ignored(Ignored::TooShort)
        );
        assert_eq!(
            decode_sealed(&[]).status,
            Status::Ignored(Ignored::TooShort)
        );

        // The offset flag is 0 or 1.
        assert_eq!(
            decode_sealed(&header(1, 2, 0)).status,
            Status::Ignored(Ignored::BadOffsetFlag)
        );

        // Nine outputs.
        assert_eq!(
            decode_sealed(&header(1, 1, 9)).status,
            Status::Ignored(Ignored::TooManyOutputs)
        );
        assert_eq!(
            decode_sealed(&header(1, 1, 255)).status,
            Status::Ignored(Ignored::TooManyOutputs)
        );

        // Every ignored bundle is black with no failures carried.
        for ignored in [
            decode(&good, over),
            decode(&good, unsealed),
            decode_sealed(&bad_magic),
            decode_sealed(&header(1, 1, 9)),
        ] {
            assert_eq!(ignored.bundle, Bundle::default());
            assert_eq!(ignored.bundle.prior_failures, 0);
            assert!(ignored.bundle.outputs.is_empty());
        }
    }

    #[test]
    fn each_bound_of_a_block_rejects_that_block_and_keeps_the_header() {
        let one_pixel = 4;
        let cases: [(Vec<u8>, BlockError); 7] = [
            (raw_block(b"", 1, 1, one_pixel), BlockError::NameLength(0)),
            (
                raw_block(&[b'n'; 65], 1, 1, one_pixel),
                BlockError::NameLength(65),
            ),
            (
                raw_block(&[0xff, 0xfe], 1, 1, one_pixel),
                BlockError::NameNotUtf8,
            ),
            (raw_block(b"DP-1", 0, 1, 0), BlockError::Side(0)),
            (raw_block(b"DP-1", 641, 1, 0), BlockError::Side(641)),
            (raw_block(b"DP-1", 1, 0, 0), BlockError::Side(0)),
            (raw_block(b"DP-1", 1, 641, 0), BlockError::Side(641)),
        ];
        for (block_bytes, why) in cases {
            let mut bytes = header(1, 1, 1);
            bytes.extend_from_slice(&block_bytes);
            let decoded = decode_sealed(&bytes);
            assert_eq!(
                decoded.status,
                Status::BlockRejected { index: 0, why },
                "{why:?}"
            );
            assert_eq!(decoded.bundle.prior_failures, 2);
            assert!(decoded.bundle.offset_known);
            assert_eq!(decoded.bundle.utc_offset_s, 3600);
            assert!(decoded.bundle.outputs.is_empty());
        }
    }

    #[test]
    fn the_limits_themselves_are_accepted() {
        for (name_len, side) in [(1usize, 1u16), (64, 640)] {
            let name = "n".repeat(name_len);
            let mut bytes = header(1, 1, 1);
            bytes.extend_from_slice(&raw_block(
                name.as_bytes(),
                side,
                side,
                usize::from(side) * usize::from(side) * 4,
            ));
            let decoded = decode_sealed(&bytes);
            assert_eq!(decoded.status, Status::Complete, "{name_len} {side}");
            assert_eq!(decoded.bundle.outputs.len(), 1);
            assert_eq!(decoded.bundle.outputs[0].name().len(), name_len);
        }
    }

    #[test]
    fn t_b3_header_is_kept_when_a_block_fails() {
        // Block 0 is good, block 1 has a side of 700, block 2 would be good.
        let mut bytes = header(1, 1, 3);
        bytes.extend_from_slice(&raw_block(b"DP-1", 2, 2, 16));
        bytes.extend_from_slice(&raw_block(b"DP-2", 700, 1, 0));
        bytes.extend_from_slice(&raw_block(b"DP-3", 1, 1, 4));
        let decoded = decode_sealed(&bytes);
        assert_eq!(
            decoded.status,
            Status::BlockRejected {
                index: 1,
                why: BlockError::Side(700)
            }
        );
        assert_eq!(decoded.bundle.prior_failures, 2);
        assert!(decoded.bundle.offset_known);
        assert_eq!(decoded.bundle.utc_offset_s, 3600);
        // The good block before it stays; the failing block and the rest go.
        assert_eq!(decoded.bundle.outputs.len(), 1);
        assert_eq!(decoded.bundle.outputs[0].name(), "DP-1");
    }

    #[test]
    fn a_header_that_promises_more_blocks_than_the_file_holds_is_ignored() {
        // Review focus 5: the output count disagrees with the file size.
        let mut one_of_two = header(1, 1, 2);
        one_of_two.extend_from_slice(&raw_block(b"DP-1", 1, 1, 4));
        assert_eq!(
            decode_sealed(&one_of_two).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
        let mut no_blocks = header(1, 1, 1);
        assert_eq!(
            decode_sealed(&no_blocks).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
        // Pixels cut short in the last block.
        no_blocks.extend_from_slice(&raw_block(b"DP-1", 2, 2, 15));
        assert_eq!(
            decode_sealed(&no_blocks).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
        // A header that promises none but is followed by bytes.
        let mut stray = header(1, 1, 0);
        stray.push(0);
        assert_eq!(
            decode_sealed(&stray).status,
            Status::Ignored(Ignored::SizeMismatch)
        );
    }

    #[test]
    fn eight_maximal_outputs_fit_16_mib() {
        let name = "n".repeat(NAME_MAX);
        let pixels = vec![0xFF; usize::from(SIDE_MAX) * usize::from(SIDE_MAX) * 4];
        let outputs: Vec<OutputBlock> = (0..MAX_OUTPUTS)
            .map(|i| {
                let mut name = name.clone();
                name.replace_range(0..1, &i.to_string());
                OutputBlock::new(name, SIDE_MAX, SIDE_MAX, pixels.clone()).expect("a maximal block")
            })
            .collect();
        let bundle = Bundle {
            prior_failures: 255,
            offset_known: true,
            utc_offset_s: i32::MAX,
            outputs,
        };
        let bytes = encode(&bundle);
        // 13 + 8 x (1 + 64 + 2 + 2 + 640 x 640 x 4) = 13 107 765 bytes.
        assert_eq!(bytes.len(), 13 + 8 * (69 + 640 * 640 * 4));
        assert!((bytes.len() as u64) <= MAX_FILE_BYTES);
        assert!(bytes.len() > 12 * 1024 * 1024);
        assert_eq!(MAX_FILE_BYTES, 16 * 1024 * 1024);
        let decoded = decode_sealed(&bytes);
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle, bundle);
    }

    #[test]
    fn a_bundle_exactly_at_the_size_bound_is_not_too_large() {
        // Review focus 5: a bundle exactly at 16 MiB. The header alone is
        // never that large, so the bound is exercised through the metadata:
        // at the limit the size check passes and the content check decides.
        let bytes = header(1, 1, 0);
        let at = FileMeta {
            size: MAX_FILE_BYTES,
            seals_ok: true,
        };
        assert_ne!(
            decode(&bytes, at).status,
            Status::Ignored(Ignored::TooLarge)
        );
        let over = FileMeta {
            size: MAX_FILE_BYTES + 1,
            seals_ok: true,
        };
        assert_eq!(
            decode(&bytes, over).status,
            Status::Ignored(Ignored::TooLarge)
        );
    }

    #[test]
    fn encode_sends_only_the_first_eight_outputs() {
        let outputs: Vec<OutputBlock> = (0..12)
            .map(|i| block(&format!("out-{i:02}"), 1, 1, i))
            .collect();
        let bundle = Bundle {
            prior_failures: 0,
            offset_known: false,
            utc_offset_s: 0,
            outputs,
        };
        let decoded = decode_sealed(&encode(&bundle));
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle.outputs.len(), MAX_OUTPUTS);
        assert_eq!(decoded.bundle.outputs[7].name(), "out-07");
    }

    #[test]
    fn a_block_is_built_only_from_valid_parts() {
        assert_eq!(
            OutputBlock::new("", 1, 1, vec![0; 4]),
            Err(BlockError::NameLength(0))
        );
        assert_eq!(
            OutputBlock::new("a".repeat(65), 1, 1, vec![0; 4]),
            Err(BlockError::NameLength(65))
        );
        // The length bound is on bytes, not characters.
        assert_eq!(
            OutputBlock::new("\u{e9}".repeat(33), 1, 1, vec![0; 4]),
            Err(BlockError::NameLength(66))
        );
        assert_eq!(
            OutputBlock::new("a", 0, 1, vec![]),
            Err(BlockError::Side(0))
        );
        assert_eq!(
            OutputBlock::new("a", 1, 641, vec![]),
            Err(BlockError::Side(641))
        );
        assert_eq!(
            OutputBlock::new("a", 2, 2, vec![0; 15]),
            Err(BlockError::PixelCount {
                expected: 16,
                found: 15
            })
        );
        assert!(OutputBlock::new("a", 2, 2, vec![0; 16]).is_ok());
    }

    /// A seeded xorshift: the tests need noise, not entropy.
    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    #[test]
    fn t_b4_seeded_random_bytes_never_panic_or_over_allocate() {
        let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
        let valid = encode(&sample());
        for round in 0..10_000u32 {
            let bytes: Vec<u8> = match round % 4 {
                // Pure noise of a random length.
                0 => (0..rng.below(200)).map(|_| rng.next() as u8).collect(),
                // A good header, then noise: the blocks decide.
                1 => {
                    let mut bytes = header(1, (rng.next() % 3) as u8, (rng.next() % 12) as u8);
                    bytes.extend((0..rng.below(300)).map(|_| rng.next() as u8));
                    bytes
                }
                // A valid bundle with a few bytes flipped.
                2 => {
                    let mut bytes = valid.clone();
                    for _ in 0..=rng.below(4) {
                        let at = rng.below(bytes.len() as u64) as usize;
                        bytes[at] = rng.next() as u8;
                    }
                    bytes
                }
                // A valid bundle cut at a random point.
                _ => valid[..rng.below(valid.len() as u64 + 1) as usize].to_vec(),
            };
            let metas = [
                sealed(&bytes),
                FileMeta {
                    size: rng.next() % (MAX_FILE_BYTES * 2),
                    seals_ok: rng.next() % 2 == 0,
                },
            ];
            for meta in metas {
                let decoded = decode(&bytes, meta);
                // Whatever came back is built from bytes that were present.
                let held: usize = decoded
                    .bundle
                    .outputs
                    .iter()
                    .map(|block| block.pixels().len() + block.name().len())
                    .sum();
                assert!(held <= bytes.len(), "round {round}");
                assert!(decoded.bundle.outputs.len() <= MAX_OUTPUTS, "round {round}");
                if let Status::Ignored(_) = decoded.status {
                    assert_eq!(decoded.bundle, Bundle::default(), "round {round}");
                }
            }
        }
    }

    #[test]
    fn a_huge_declared_size_allocates_nothing_before_it_is_checked() {
        // The largest side the format allows, declared by a file that holds
        // none of it: the decoder must not reserve the pixels first.
        let mut bytes = header(1, 1, 8);
        for _ in 0..8 {
            bytes.extend_from_slice(&raw_block(b"DP-1", 640, 640, 0));
        }
        let decoded = decode_sealed(&bytes);
        assert_eq!(decoded.status, Status::Ignored(Ignored::SizeMismatch));
    }
}
