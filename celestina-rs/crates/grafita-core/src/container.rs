//! The ZIP container, read and rebuilt rather than repacked.
//!
//! An imported document lives inside a container somebody else wrote, and the
//! contract is that every part the author did not edit comes back as the bytes
//! it already was. An archive library cannot promise that: it promises a
//! correct archive, which is a different thing — its own header layout, its own
//! extra fields, its own compression level. So this module keeps the original
//! file and copies out of it.
//!
//! What that means concretely: rebuilding copies each untouched entry's local
//! header and compressed data as one verbatim byte range, in the order they
//! appear, and rewrites only the entries that were replaced. Nothing is
//! recompressed to be written back unchanged, because nothing that was not
//! edited is compressed at all.

use std::fmt;
use std::io::Write;

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::inflate::{Budget, InflateError};

const LOCAL_HEADER: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
const CENTRAL_HEADER: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
const END_OF_CENTRAL: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
const ZIP64_END_OF_CENTRAL: [u8; 4] = [0x50, 0x4B, 0x06, 0x06];

const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// The largest end-of-central-directory comment a scan will look behind.
const MAX_COMMENT: usize = u16::MAX as usize;

/// The fixed part of a central directory record, and so the least room each
/// member the end record counts must take up.
const CENTRAL_RECORD: usize = 46;

/// Why a byte stream is not a container this crate will touch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainerError {
    /// No end-of-central-directory record: not a ZIP at all.
    NotAnArchive,
    /// A structure this crate refuses to guess at, named so a host can say why.
    Unsupported { detail: String },
    /// The archive contradicts itself — an offset or length outside the file.
    Malformed { detail: String },
    /// A member's compressed data could not be decoded.
    Corrupt { name: String },
    /// The archive has no member by that name.
    NoSuchMember { name: String },
    /// A member unpacks past what the document may hold, alone or together
    /// with the members read before it.
    TooLarge { name: String, limit: u64 },
}

impl fmt::Display for ContainerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnArchive => formatter.write_str("this file is not a zip container"),
            Self::Unsupported { detail } => {
                write!(
                    formatter,
                    "this container is not one Grafita edits: {detail}"
                )
            }
            Self::Malformed { detail } => write!(formatter, "this container is damaged: {detail}"),
            Self::Corrupt { name } => write!(formatter, "'{name}' could not be decompressed"),
            Self::NoSuchMember { name } => write!(formatter, "this container has no '{name}'"),
            Self::TooLarge { name, limit } => write!(
                formatter,
                "'{name}' unpacks past the {limit}-byte ceiling on a document"
            ),
        }
    }
}

impl std::error::Error for ContainerError {}

/// One member of the container, described by where it sits in the file.
#[derive(Clone, Debug)]
struct Member {
    name: String,
    method: u16,
    /// The whole local header and its data, which is what a copy moves.
    local_span: (usize, usize),
    /// Just the compressed data inside that span.
    data_span: (usize, usize),
    /// The member's record in the central directory, copied the same way.
    central_span: (usize, usize),
    crc: u32,
    /// What the directory says the member unpacks to. A bound on the read and
    /// a check on its result, never a size to allocate.
    uncompressed_size: u32,
}

/// A container held together with the bytes it was read from.
#[derive(Clone, Debug)]
pub struct Container {
    bytes: Vec<u8>,
    members: Vec<Member>,
    /// Everything after the last central directory record, copied as it is
    /// except for the two counts and the offset a rebuild changes.
    comment: Vec<u8>,
}

impl Container {
    /// Reads the container's structure without decompressing anything.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ContainerError> {
        let end = find_end_of_central_directory(&bytes)?;
        if find_signature(&bytes, &ZIP64_END_OF_CENTRAL).is_some() {
            return Err(ContainerError::Unsupported {
                detail: "it uses the zip64 extensions".to_owned(),
            });
        }
        let count = read_u16(&bytes, position(end, 10)?)? as usize;
        let directory_offset = read_u32(&bytes, position(end, 16)?)? as usize;
        let comment_length = read_u16(&bytes, position(end, 20)?)? as usize;
        let comment_start = position(end, 22)?;
        if position(comment_start, comment_length)? != bytes.len() {
            return Err(ContainerError::Malformed {
                detail: "trailing bytes after the end record".to_owned(),
            });
        }

        // The count is the file's claim; the file's length bounds it.
        let mut members = Vec::with_capacity(count.min(bytes.len() / CENTRAL_RECORD));
        let mut cursor = directory_offset;
        for _ in 0..count {
            let member = read_central_record(&bytes, &mut cursor)?;
            members.push(member);
        }
        let comment = bytes
            .get(comment_start..)
            .ok_or_else(|| ContainerError::Malformed {
                detail: "the end record runs past the end of the file".to_owned(),
            })?
            .to_vec();
        Ok(Self {
            bytes,
            members,
            comment,
        })
    }

    /// Every member's name, in the order the directory lists them.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.members
            .iter()
            .map(|member| member.name.as_str())
            .collect()
    }

    /// The decompressed content of one member, charged to `budget`.
    ///
    /// Every member a document reads draws on the same budget, so the ceiling
    /// is on the document and not on each of its parts. The size the
    /// directory declares bounds the read but sizes nothing: content that
    /// disagrees with it is refused as corrupt.
    pub fn read(&self, name: &str, budget: &mut Budget) -> Result<Vec<u8>, ContainerError> {
        let member = self
            .members
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| ContainerError::NoSuchMember {
                name: name.to_owned(),
            })?;
        let data = self
            .bytes
            .get(member.data_span.0..member.data_span.1)
            .ok_or_else(|| ContainerError::Malformed {
                detail: format!("'{name}' lies outside the file"),
            })?;
        let refused = |error: InflateError| match error {
            InflateError::TooLarge { limit } => ContainerError::TooLarge {
                name: name.to_owned(),
                limit,
            },
            InflateError::Corrupt => ContainerError::Corrupt {
                name: name.to_owned(),
            },
        };
        let declared = u64::from(member.uncompressed_size);
        let content = match member.method {
            STORED => {
                budget.charge(data.len()).map_err(refused)?;
                data.to_vec()
            }
            DEFLATED => budget
                .inflate_at_most(DeflateDecoder::new(data), declared)
                .map_err(refused)?,
            other => {
                return Err(ContainerError::Unsupported {
                    detail: format!("'{name}' uses compression method {other}"),
                })
            }
        };
        // The directory says what this member should check out as. Reading a
        // document whose bytes disagree with its own archive is the one case
        // where continuing would edit something nobody wrote.
        if u64::try_from(content.len()).ok() != Some(declared) || crc32(&content) != member.crc {
            return Err(ContainerError::Corrupt {
                name: name.to_owned(),
            });
        }
        Ok(content)
    }

    /// Rebuilds the container with `replacements` applied.
    ///
    /// Every member not named in `replacements` is copied as the exact bytes it
    /// occupies — local header, extra field and compressed data alike — so a
    /// rebuild with no replacement reproduces the input file.
    pub fn rewrite(&self, replacements: &[(&str, Vec<u8>)]) -> Result<Vec<u8>, ContainerError> {
        for (name, _) in replacements {
            if !self.members.iter().any(|member| member.name == *name) {
                return Err(ContainerError::NoSuchMember {
                    name: (*name).to_owned(),
                });
            }
        }

        let mut out = Vec::with_capacity(self.bytes.len());
        let mut directory = Vec::with_capacity(self.members.len() * 64);
        for member in &self.members {
            let offset = u32::try_from(out.len()).map_err(|_| ContainerError::Unsupported {
                detail: "the rebuilt container would pass four gigabytes".to_owned(),
            })?;
            let replacement = replacements
                .iter()
                .find(|(name, _)| *name == member.name)
                .map(|(_, content)| content);
            let central = span(&self.bytes, member.central_span)?;
            match replacement {
                None => {
                    out.extend_from_slice(span(&self.bytes, member.local_span)?);
                    let start = directory.len();
                    directory.extend_from_slice(central);
                    write_u32_at(&mut directory, start + 42, offset);
                }
                Some(content) => {
                    let (method, data) = compress(content, member.method);
                    let crc = crc32(content);
                    let sizes = (u32::try_from(data.len()), u32::try_from(content.len()));
                    let (Ok(compressed_size), Ok(uncompressed_size)) = sizes else {
                        return Err(ContainerError::Unsupported {
                            detail: "the replacement would pass four gigabytes".to_owned(),
                        });
                    };
                    let header_end = position(
                        member.local_span.0,
                        local_header_length(&self.bytes, member)?,
                    )?;
                    let mut header = span(&self.bytes, (member.local_span.0, header_end))?.to_vec();
                    // The header keeps this member's name, extra field and
                    // timestamp; only what the new content changes is written.
                    write_u16_at(&mut header, 8, method);
                    write_u32_at(&mut header, 14, crc);
                    write_u32_at(&mut header, 18, compressed_size);
                    write_u32_at(&mut header, 22, uncompressed_size);
                    // A data descriptor would put the sizes after the data
                    // instead of here; the sizes are known now, so the flag goes.
                    let flags = read_u16(&header, 6)? & !0x0008;
                    write_u16_at(&mut header, 6, flags);
                    out.extend_from_slice(&header);
                    out.extend_from_slice(&data);

                    let start = directory.len();
                    directory.extend_from_slice(central);
                    write_u16_at(&mut directory, start + 8, flags);
                    write_u16_at(&mut directory, start + 10, method);
                    write_u32_at(&mut directory, start + 16, crc);
                    write_u32_at(&mut directory, start + 20, compressed_size);
                    write_u32_at(&mut directory, start + 24, uncompressed_size);
                    write_u32_at(&mut directory, start + 42, offset);
                }
            }
        }

        let directory_offset =
            u32::try_from(out.len()).map_err(|_| ContainerError::Unsupported {
                detail: "the rebuilt container would pass four gigabytes".to_owned(),
            })?;
        let directory_size =
            u32::try_from(directory.len()).map_err(|_| ContainerError::Unsupported {
                detail: "the rebuilt directory would pass four gigabytes".to_owned(),
            })?;
        let count = u16::try_from(self.members.len()).map_err(|_| ContainerError::Unsupported {
            detail: "the container has more than 65535 members".to_owned(),
        })?;
        out.extend_from_slice(&directory);
        out.extend_from_slice(&END_OF_CENTRAL);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&directory_size.to_le_bytes());
        out.extend_from_slice(&directory_offset.to_le_bytes());
        let comment_length =
            u16::try_from(self.comment.len()).map_err(|_| ContainerError::Unsupported {
                detail: "the container comment is longer than 65535 bytes".to_owned(),
            })?;
        out.extend_from_slice(&comment_length.to_le_bytes());
        out.extend_from_slice(&self.comment);
        Ok(out)
    }
}

/// Compresses a replacement the way the member it replaces was stored.
///
/// A stored member stays stored, so a container that carries its parts
/// uncompressed keeps doing so; anything else is deflated.
fn compress(content: &[u8], method: u16) -> (u16, Vec<u8>) {
    if method == STORED {
        return (STORED, content.to_vec());
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(content).is_err() {
        return (STORED, content.to_vec());
    }
    match encoder.finish() {
        Ok(data) => (DEFLATED, data),
        Err(_) => (STORED, content.to_vec()),
    }
}

fn local_header_length(bytes: &[u8], member: &Member) -> Result<usize, ContainerError> {
    let start = member.local_span.0;
    let name_length = read_u16(bytes, position(start, 26)?)? as usize;
    let extra_length = read_u16(bytes, position(start, 28)?)? as usize;
    position(position(30, name_length)?, extra_length)
}

fn read_central_record(bytes: &[u8], cursor: &mut usize) -> Result<Member, ContainerError> {
    let start = *cursor;
    if bytes.get(start..position(start, 4)?) != Some(&CENTRAL_HEADER) {
        return Err(ContainerError::Malformed {
            detail: "a central directory record is missing its signature".to_owned(),
        });
    }
    let flags = read_u16(bytes, position(start, 8)?)?;
    if flags & 0x0001 != 0 {
        return Err(ContainerError::Unsupported {
            detail: "it is encrypted".to_owned(),
        });
    }
    let method = read_u16(bytes, position(start, 10)?)?;
    let crc = read_u32(bytes, position(start, 16)?)?;
    let compressed_size = read_u32(bytes, position(start, 20)?)? as usize;
    let uncompressed_size = read_u32(bytes, position(start, 24)?)?;
    let name_length = read_u16(bytes, position(start, 28)?)? as usize;
    let extra_length = read_u16(bytes, position(start, 30)?)? as usize;
    let comment_length = read_u16(bytes, position(start, 32)?)? as usize;
    let local_offset = read_u32(bytes, position(start, 42)?)? as usize;
    let name_start = position(start, CENTRAL_RECORD)?;
    let name = bytes
        .get(name_start..position(name_start, name_length)?)
        .ok_or_else(|| ContainerError::Malformed {
            detail: "a member name runs past the end of the file".to_owned(),
        })?;
    let name = String::from_utf8(name.to_vec()).map_err(|_| ContainerError::Unsupported {
        detail: "a member name is not UTF-8".to_owned(),
    })?;
    let record_end = position(
        position(position(name_start, name_length)?, extra_length)?,
        comment_length,
    )?;
    if record_end > bytes.len() {
        return Err(ContainerError::Malformed {
            detail: "a directory record runs past the end of the file".to_owned(),
        });
    }

    if bytes.get(local_offset..position(local_offset, 4)?) != Some(&LOCAL_HEADER) {
        return Err(ContainerError::Malformed {
            detail: format!("'{name}' does not start with a local header"),
        });
    }
    let local_name_length = read_u16(bytes, position(local_offset, 26)?)? as usize;
    let local_extra_length = read_u16(bytes, position(local_offset, 28)?)? as usize;
    let data_start = position(
        position(position(local_offset, 30)?, local_name_length)?,
        local_extra_length,
    )?;
    let data_end = position(data_start, compressed_size)?;
    if data_end > bytes.len() {
        return Err(ContainerError::Malformed {
            detail: format!("'{name}' claims more data than the file holds"),
        });
    }
    // A data descriptor follows the data when the flag is set. It is part of
    // the member as far as a verbatim copy is concerned.
    let local_end = if flags & 0x0008 != 0 {
        descriptor_end(bytes, data_end)
    } else {
        data_end
    };

    *cursor = record_end;
    Ok(Member {
        name,
        method,
        local_span: (local_offset, local_end),
        data_span: (data_start, data_end),
        central_span: (start, record_end),
        crc,
        uncompressed_size,
    })
}

/// How far a data descriptor reaches. It is twelve bytes, or sixteen when it
/// carries the optional signature that most writers include.
fn descriptor_end(bytes: &[u8], data_end: usize) -> usize {
    const DESCRIPTOR: [u8; 4] = [0x50, 0x4B, 0x07, 0x08];
    let signed = data_end
        .checked_add(4)
        .and_then(|end| bytes.get(data_end..end))
        == Some(&DESCRIPTOR);
    let length = if signed { 16 } else { 12 };
    data_end.saturating_add(length).min(bytes.len())
}

fn find_end_of_central_directory(bytes: &[u8]) -> Result<usize, ContainerError> {
    if bytes.len() < 22 {
        return Err(ContainerError::NotAnArchive);
    }
    let earliest = bytes.len().saturating_sub(22 + MAX_COMMENT);
    let mut index = bytes.len() - 22;
    loop {
        if bytes.get(index..index + 4) == Some(&END_OF_CENTRAL) {
            return Ok(index);
        }
        if index == earliest {
            return Err(ContainerError::NotAnArchive);
        }
        index -= 1;
    }
}

fn find_signature(bytes: &[u8], signature: &[u8; 4]) -> Option<usize> {
    bytes.windows(4).position(|window| window == signature)
}

/// The bytes a parsed span covers. Every span was checked against the file
/// when it was read; this keeps a rebuild from trusting that check blindly.
fn span(bytes: &[u8], (start, end): (usize, usize)) -> Result<&[u8], ContainerError> {
    bytes
        .get(start..end)
        .ok_or_else(|| ContainerError::Malformed {
            detail: "a member lies outside the file".to_owned(),
        })
}

/// `base + length` as a position in the file, or a refusal when a header's
/// numbers do not add up to one.
fn position(base: usize, length: usize) -> Result<usize, ContainerError> {
    base.checked_add(length)
        .ok_or_else(|| ContainerError::Malformed {
            detail: "a header points past any file".to_owned(),
        })
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, ContainerError> {
    bytes
        .get(at..position(at, 2)?)
        .and_then(|slice| slice.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| ContainerError::Malformed {
            detail: format!("the file ends inside a header at byte {at}"),
        })
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, ContainerError> {
    bytes
        .get(at..position(at, 4)?)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| ContainerError::Malformed {
            detail: format!("the file ends inside a header at byte {at}"),
        })
}

fn write_u16_at(bytes: &mut [u8], at: usize, value: u16) {
    if let Some(slice) = bytes.get_mut(at..at + 2) {
        slice.copy_from_slice(&value.to_le_bytes());
    }
}

fn write_u32_at(bytes: &mut [u8], at: usize, value: u32) {
    if let Some(slice) = bytes.get_mut(at..at + 4) {
        slice.copy_from_slice(&value.to_le_bytes());
    }
}

/// The ZIP checksum. Written here rather than taken, because it is eight lines
/// and a dependency for eight lines is a dependency to explain forever.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
