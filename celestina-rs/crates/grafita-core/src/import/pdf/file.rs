//! Where each object of a PDF lives, and how to get at it.
//!
//! A PDF says where its objects are in a cross-reference, which is either a
//! table of offsets or, since version 1.5, a stream of them — and objects
//! themselves may be packed inside other streams. All three shapes are read
//! here, because refusing the modern ones would mean refusing most files
//! anybody actually has.
//!
//! Nothing is rewritten. A file is read as it is, and an edit is appended as an
//! incremental update, which is the format's own way of changing a document
//! without touching a byte of what came before.

use std::collections::BTreeMap;
use std::io::Read;

use flate2::read::ZlibDecoder;

use super::object::{Dictionary, Lexer, Object, PdfError};

/// Where one object is: at an offset in the file, or inside another object.
#[derive(Clone, Copy, Debug)]
enum Location {
    Offset(usize),
    InStream { container: u32, index: usize },
}

/// A PDF file, with its objects located but not yet read.
#[derive(Clone)]
pub struct Pdf {
    bytes: Vec<u8>,
    locations: BTreeMap<u32, Location>,
    trailer: Dictionary,
    /// Where the newest cross-reference starts, which an update points back at.
    start_xref: usize,
    /// Whether that newest one is a stream. An update writes the same kind: a
    /// table appended to a stream-based file is a shape some readers reject.
    xref_streams: bool,
}

impl std::fmt::Debug for Pdf {
    /// Named without its bytes: a debug line for a document should not be a
    /// megabyte of compressed streams.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pdf")
            .field("bytes", &self.bytes.len())
            .field("objects", &self.locations.len())
            .finish_non_exhaustive()
    }
}

impl Pdf {
    /// Reads a file's structure.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, PdfError> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(PdfError::NotPdf);
        }
        let start = find_start_xref(&bytes)?;
        let mut locations = BTreeMap::new();
        let mut trailer = Dictionary::new();
        let mut next = Some(start);
        let mut seen = Vec::new();
        let mut xref_streams = false;

        // Each cross-reference points back at the one it updates, so a file
        // that has been edited before is read newest first and older entries
        // only fill what the newer ones left out.
        while let Some(offset) = next {
            if seen.contains(&offset) {
                break;
            }
            seen.push(offset);
            let section = read_section(&bytes, offset)?;
            if seen.len() == 1 {
                xref_streams = section.is_stream;
            }
            for (number, location) in section.locations {
                locations.entry(number).or_insert(location);
            }
            for (key, value) in section.trailer {
                trailer.entry(key).or_insert(value);
            }
            next = section.previous;
        }

        if trailer.contains_key("Encrypt") {
            return Err(PdfError::Encrypted);
        }
        Ok(Self {
            bytes,
            locations,
            trailer,
            start_xref: start,
            xref_streams,
        })
    }

    /// Where the newest cross-reference begins.
    #[must_use]
    pub const fn start_xref(&self) -> usize {
        self.start_xref
    }

    /// Whether this file's cross-reference is a stream rather than a table.
    #[must_use]
    pub const fn uses_xref_streams(&self) -> bool {
        self.xref_streams
    }

    /// The file's own bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The trailer, which names the catalogue and the rest of the roots.
    #[must_use]
    pub const fn trailer(&self) -> &Dictionary {
        &self.trailer
    }

    /// The highest object number the file uses.
    #[must_use]
    pub fn last_object(&self) -> u32 {
        self.locations.keys().copied().max().unwrap_or(0)
    }

    /// Reads one object by number.
    pub fn object(&self, number: u32) -> Result<Object, PdfError> {
        match self.locations.get(&number).copied() {
            None => Ok(Object::Null),
            Some(Location::Offset(offset)) => self.object_at(offset),
            Some(Location::InStream { container, index }) => {
                let contents = self.object_stream(container)?;
                contents
                    .get(index)
                    .cloned()
                    .ok_or_else(|| PdfError::Malformed {
                        detail: format!("object {number} is not in the stream that claims it"),
                    })
            }
        }
    }

    /// Follows a reference until it is not one.
    pub fn resolve(&self, object: &Object) -> Result<Object, PdfError> {
        match object {
            Object::Reference { number, .. } => self.object(*number),
            other => Ok(other.clone()),
        }
    }

    /// One entry of a dictionary, resolved.
    pub fn entry(&self, dictionary: &Dictionary, key: &str) -> Result<Object, PdfError> {
        match dictionary.get(key) {
            None => Ok(Object::Null),
            Some(object) => self.resolve(object),
        }
    }

    /// A stream's content, decompressed when this crate knows the filter.
    pub fn stream_data(&self, object: &Object) -> Result<Vec<u8>, PdfError> {
        let Object::Stream { dictionary, data } = object else {
            return Err(PdfError::Malformed {
                detail: "this object is not a stream".to_owned(),
            });
        };
        let length = self.length_of(dictionary, data.0);
        let start = data.0;
        let end = (start + length).min(self.bytes.len());
        let raw = &self.bytes[start..end];

        let filters = match self.entry(dictionary, "Filter")? {
            Object::Name(name) => vec![name],
            Object::Array(items) => items
                .iter()
                .filter_map(|item| item.as_name().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        let mut content = raw.to_vec();
        for filter in filters {
            content = match filter.as_str() {
                "FlateDecode" => {
                    let mut out = Vec::new();
                    ZlibDecoder::new(content.as_slice())
                        .read_to_end(&mut out)
                        .map_err(|_| PdfError::Malformed {
                            detail: "a compressed stream could not be read".to_owned(),
                        })?;
                    out
                }
                other => {
                    return Err(PdfError::Unsupported {
                        detail: format!("the {other} stream filter"),
                    })
                }
            };
        }
        Ok(content)
    }

    /// How long a stream's data is.
    ///
    /// The declared length is trusted when it is there and plausible; when it
    /// is a reference this file cannot resolve yet — which happens while the
    /// cross-reference itself is being read — the `endstream` keyword decides.
    fn length_of(&self, dictionary: &Dictionary, start: usize) -> usize {
        if let Ok(Object::Number(value)) = self.entry(dictionary, "Length") {
            let length = value.max(0.0) as usize;
            if start + length <= self.bytes.len() {
                return length;
            }
        }
        self.bytes[start..]
            .windows(9)
            .position(|window| window == b"endstream")
            .unwrap_or(self.bytes.len() - start)
    }

    fn object_at(&self, offset: usize) -> Result<Object, PdfError> {
        let mut lexer = Lexer::new(&self.bytes, offset);
        // `12 0 obj`
        lexer.skip_space();
        while lexer
            .bytes
            .get(lexer.cursor)
            .is_some_and(u8::is_ascii_digit)
        {
            lexer.cursor += 1;
        }
        lexer.skip_space();
        while lexer
            .bytes
            .get(lexer.cursor)
            .is_some_and(u8::is_ascii_digit)
        {
            lexer.cursor += 1;
        }
        if !lexer.eat(b"obj") {
            return Err(PdfError::Malformed {
                detail: format!("no object begins at byte {offset}"),
            });
        }
        lexer.object()
    }

    /// The objects packed inside an object stream, in its own order.
    fn object_stream(&self, number: u32) -> Result<Vec<Object>, PdfError> {
        let container = match self.locations.get(&number).copied() {
            Some(Location::Offset(offset)) => self.object_at(offset)?,
            _ => {
                return Err(PdfError::Malformed {
                    detail: format!("object stream {number} is not in the file"),
                })
            }
        };
        let Object::Stream { ref dictionary, .. } = container else {
            return Err(PdfError::Malformed {
                detail: format!("object {number} is not a stream"),
            });
        };
        let count = self.entry(dictionary, "N")?.as_number().unwrap_or(0.0) as usize;
        let first = self.entry(dictionary, "First")?.as_number().unwrap_or(0.0) as usize;
        let content = self.stream_data(&container)?;

        let mut header = Lexer::new(&content, 0);
        let mut offsets = Vec::with_capacity(count);
        for _ in 0..count {
            let _number = header.object()?;
            let offset = header.object()?.as_number().unwrap_or(0.0) as usize;
            offsets.push(offset);
        }
        let mut objects = Vec::with_capacity(count);
        for offset in offsets {
            let mut lexer = Lexer::new(&content, first + offset);
            objects.push(lexer.object()?);
        }
        Ok(objects)
    }
}

struct Section {
    locations: Vec<(u32, Location)>,
    trailer: Dictionary,
    previous: Option<usize>,
    is_stream: bool,
}

fn read_section(bytes: &[u8], offset: usize) -> Result<Section, PdfError> {
    let mut lexer = Lexer::new(bytes, offset);
    if lexer.eat(b"xref") {
        return read_table(&mut lexer);
    }
    read_stream_section(bytes, offset)
}

/// The classic cross-reference: `xref`, then runs of twenty-byte entries.
fn read_table(lexer: &mut Lexer<'_>) -> Result<Section, PdfError> {
    let mut locations = Vec::new();
    loop {
        lexer.skip_space();
        if lexer.bytes[lexer.cursor..].starts_with(b"trailer") {
            lexer.cursor += b"trailer".len();
            break;
        }
        let start = lexer.object()?.as_number().unwrap_or(0.0) as u32;
        let count = lexer.object()?.as_number().unwrap_or(0.0) as usize;
        for index in 0..count {
            lexer.skip_space();
            let entry = lexer
                .bytes
                .get(lexer.cursor..lexer.cursor + 18)
                .ok_or_else(|| PdfError::Malformed {
                    detail: "the cross-reference table is cut short".to_owned(),
                })?;
            let text = std::str::from_utf8(entry).unwrap_or("");
            let offset = text[..10].trim().parse::<usize>().unwrap_or(0);
            let kind = text.as_bytes().get(17).copied().unwrap_or(b'f');
            if kind == b'n' {
                locations.push((start + index as u32, Location::Offset(offset)));
            }
            lexer.cursor += 18;
        }
    }
    let trailer = match lexer.object()? {
        Object::Dictionary(dictionary) => dictionary,
        _ => Dictionary::new(),
    };
    let previous = trailer
        .get("Prev")
        .and_then(Object::as_number)
        .map(|value| value as usize);
    Ok(Section {
        locations,
        trailer,
        previous,
        is_stream: false,
    })
}

/// The cross-reference stream a modern file carries instead of a table.
fn read_stream_section(bytes: &[u8], offset: usize) -> Result<Section, PdfError> {
    // The stream is an ordinary object, so a file with nothing located but this
    // one is enough to read it.
    let probe = Pdf {
        bytes: bytes.to_vec(),
        locations: BTreeMap::new(),
        trailer: Dictionary::new(),
        start_xref: offset,
        xref_streams: true,
    };
    let object = probe.object_at(offset)?;
    let Object::Stream { ref dictionary, .. } = object else {
        return Err(PdfError::Malformed {
            detail: format!("byte {offset} holds no cross-reference"),
        });
    };
    let content = probe.stream_data(&object)?;
    let content = apply_predictor(&probe, dictionary, content)?;

    let widths: Vec<usize> = probe
        .entry(dictionary, "W")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(|item| item.as_number().unwrap_or(0.0) as usize)
        .collect();
    if widths.len() < 3 {
        return Err(PdfError::Malformed {
            detail: "a cross-reference stream has no field widths".to_owned(),
        });
    }
    let size = probe.entry(dictionary, "Size")?.as_number().unwrap_or(0.0) as u32;
    let index: Vec<u32> = match probe.entry(dictionary, "Index")? {
        Object::Array(items) => items
            .iter()
            .map(|item| item.as_number().unwrap_or(0.0) as u32)
            .collect(),
        _ => vec![0, size],
    };

    let row = widths.iter().sum::<usize>();
    let mut locations = Vec::new();
    let mut cursor = 0;
    for pair in index.chunks(2) {
        let (first, count) = (pair[0], *pair.get(1).unwrap_or(&0));
        for number in first..first + count {
            if cursor + row > content.len() {
                break;
            }
            let mut fields = [1u64, 0, 0];
            let mut at = cursor;
            for (slot, width) in widths.iter().enumerate().take(3) {
                if *width > 0 {
                    let mut value = 0u64;
                    for byte in &content[at..at + width] {
                        value = (value << 8) | u64::from(*byte);
                    }
                    fields[slot] = value;
                    at += width;
                }
            }
            cursor += row;
            match fields[0] {
                1 => locations.push((number, Location::Offset(fields[1] as usize))),
                2 => locations.push((
                    number,
                    Location::InStream {
                        container: fields[1] as u32,
                        index: fields[2] as usize,
                    },
                )),
                _ => {}
            }
        }
    }
    let previous = dictionary
        .get("Prev")
        .and_then(Object::as_number)
        .map(|value| value as usize);
    Ok(Section {
        locations,
        trailer: dictionary.clone(),
        previous,
        is_stream: true,
    })
}

/// Undoes the PNG predictor a cross-reference stream is usually written with.
fn apply_predictor(
    pdf: &Pdf,
    dictionary: &Dictionary,
    content: Vec<u8>,
) -> Result<Vec<u8>, PdfError> {
    let parameters = match pdf.entry(dictionary, "DecodeParms")? {
        Object::Dictionary(parameters) => parameters,
        _ => return Ok(content),
    };
    let predictor = pdf
        .entry(&parameters, "Predictor")?
        .as_number()
        .unwrap_or(1.0) as usize;
    if predictor < 10 {
        return Ok(content);
    }
    let columns = pdf
        .entry(&parameters, "Columns")?
        .as_number()
        .unwrap_or(1.0) as usize;
    let row = columns + 1;
    let mut out = Vec::with_capacity(content.len());
    let mut previous = vec![0u8; columns];
    for chunk in content.chunks(row) {
        if chunk.len() < 2 {
            break;
        }
        let tag = chunk[0];
        let mut line = chunk[1..].to_vec();
        line.resize(columns, 0);
        // "Up" is what every writer uses for this table; the others would need
        // a pixel width these rows do not have.
        if tag == 2 {
            for (index, byte) in line.iter_mut().enumerate() {
                *byte = byte.wrapping_add(previous[index]);
            }
        }
        out.extend_from_slice(&line);
        previous = line;
    }
    Ok(out)
}

fn find_start_xref(bytes: &[u8]) -> Result<usize, PdfError> {
    let tail_start = bytes.len().saturating_sub(2048);
    let tail = &bytes[tail_start..];
    let at = tail
        .windows(9)
        .rposition(|window| window == b"startxref")
        .ok_or(PdfError::Malformed {
            detail: "the file names no cross-reference".to_owned(),
        })?;
    let mut lexer = Lexer::new(bytes, tail_start + at + 9);
    let offset = lexer.object()?.as_number().ok_or(PdfError::Malformed {
        detail: "the cross-reference offset is not a number".to_owned(),
    })?;
    Ok(offset.max(0.0) as usize)
}
