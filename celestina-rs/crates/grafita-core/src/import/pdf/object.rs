//! What a PDF is made of, and how to read it out of bytes.
//!
//! A PDF is a set of numbered objects with a table saying where each one
//! starts. This module is the reader for that: the eight object kinds the
//! format defines, and a scanner that turns bytes into them. It knows nothing
//! about pages, text or forms — those are built on top, and keeping the layers
//! apart is what makes it possible to say honestly which part of a file a
//! refusal came from.

use std::collections::BTreeMap;
use std::fmt;

/// One value in a PDF file.
#[derive(Clone, Debug, PartialEq)]
pub enum Object {
    Null,
    Boolean(bool),
    /// Integers and reals alike: the format does not separate them everywhere,
    /// and a length written `12.0` is still a length.
    Number(f64),
    /// A literal or hexadecimal string, as the bytes it holds. Never decoded
    /// here: what those bytes mean depends on the font that shows them.
    String(Vec<u8>),
    /// A name such as `/Type`, without its slash.
    Name(String),
    Array(Vec<Object>),
    Dictionary(Dictionary),
    /// A dictionary followed by raw data. The data is kept as the byte range it
    /// occupies rather than copied, so a file can be walked without holding a
    /// second copy of every image it contains.
    Stream {
        dictionary: Dictionary,
        data: (usize, usize),
    },
    /// A reference to another object: `12 0 R`.
    Reference {
        number: u32,
        generation: u16,
    },
}

/// A PDF dictionary, ordered so that writing one back is reproducible.
pub type Dictionary = BTreeMap<String, Object>;

impl Object {
    /// The number this object is, when it is one.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// The name this object is, without its slash.
    #[must_use]
    pub fn as_name(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// The dictionary this object is, whether it stands alone or heads a
    /// stream.
    #[must_use]
    pub const fn as_dictionary(&self) -> Option<&Dictionary> {
        match self {
            Self::Dictionary(dictionary) | Self::Stream { dictionary, .. } => Some(dictionary),
            _ => None,
        }
    }

    /// The array this object is, or a single object read as a one-element one,
    /// which is what several PDF keys allow.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Object]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// What this object refers to, when it is a reference.
    #[must_use]
    pub const fn as_reference(&self) -> Option<u32> {
        match self {
            Self::Reference { number, .. } => Some(*number),
            _ => None,
        }
    }
}

/// Why a byte stream is not a PDF this crate reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PdfError {
    /// The file does not begin as a PDF does.
    NotPdf,
    /// A structure this crate refuses to guess at.
    Unsupported { detail: String },
    /// The file contradicts itself.
    Malformed { detail: String },
    /// The file is encrypted. Reading it would need its password, and this
    /// editor asks for none.
    Encrypted,
    /// The document holds no text a reader can extract — a scan, most likely.
    NoText,
}

impl fmt::Display for PdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPdf => formatter.write_str("this file is not a PDF"),
            Self::Unsupported { detail } => {
                write!(formatter, "this PDF uses something Grafita does not read: {detail}")
            }
            Self::Malformed { detail } => write!(formatter, "this PDF is damaged: {detail}"),
            Self::Encrypted => formatter.write_str("this PDF is encrypted"),
            Self::NoText => formatter.write_str(
                "this PDF holds no text to edit; it is probably a scan, and Grafita does not read images",
            ),
        }
    }
}

impl std::error::Error for PdfError {}

/// A reader over a PDF's bytes.
pub struct Lexer<'a> {
    pub bytes: &'a [u8],
    pub cursor: usize,
}

impl<'a> Lexer<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8], cursor: usize) -> Self {
        Self { bytes, cursor }
    }

    /// Steps over whitespace and comments, which may appear between any two
    /// tokens.
    pub fn skip_space(&mut self) {
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'\0' | b'\t' | b'\n' | b'\x0C' | b'\r' | b' ' => self.cursor += 1,
                b'%' => {
                    while self.cursor < self.bytes.len()
                        && !matches!(self.bytes[self.cursor], b'\n' | b'\r')
                    {
                        self.cursor += 1;
                    }
                }
                _ => break,
            }
        }
    }

    /// Whether the bytes at the cursor are this keyword, stepping over it if so.
    pub fn eat(&mut self, keyword: &[u8]) -> bool {
        self.skip_space();
        if self.bytes[self.cursor..].starts_with(keyword) {
            self.cursor += keyword.len();
            return true;
        }
        false
    }

    /// Reads the next object.
    pub fn object(&mut self) -> Result<Object, PdfError> {
        self.skip_space();
        let byte = *self.bytes.get(self.cursor).ok_or(PdfError::Malformed {
            detail: "the file ends where an object should start".to_owned(),
        })?;
        match byte {
            b'/' => self.name(),
            b'(' => self.literal_string(),
            b'<' => {
                if self.bytes.get(self.cursor + 1) == Some(&b'<') {
                    self.dictionary_or_stream()
                } else {
                    self.hex_string()
                }
            }
            b'[' => {
                self.cursor += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.bytes.get(self.cursor) == Some(&b']') {
                        self.cursor += 1;
                        break;
                    }
                    items.push(self.object()?);
                }
                Ok(Object::Array(items))
            }
            b't' if self.eat(b"true") => Ok(Object::Boolean(true)),
            b'f' if self.eat(b"false") => Ok(Object::Boolean(false)),
            b'n' if self.eat(b"null") => Ok(Object::Null),
            byte if byte == b'+' || byte == b'-' || byte == b'.' || byte.is_ascii_digit() => {
                self.number_or_reference()
            }
            byte => Err(PdfError::Malformed {
                detail: format!("byte {byte:#04X} begins no object"),
            }),
        }
    }

    fn name(&mut self) -> Result<Object, PdfError> {
        self.cursor += 1;
        let mut name = String::new();
        while let Some(byte) = self.bytes.get(self.cursor) {
            if is_delimiter(*byte) || byte.is_ascii_whitespace() {
                break;
            }
            // `#` introduces a two-digit hexadecimal byte inside a name.
            if *byte == b'#' {
                let digits = self
                    .bytes
                    .get(self.cursor + 1..self.cursor + 3)
                    .and_then(|slice| std::str::from_utf8(slice).ok())
                    .and_then(|text| u8::from_str_radix(text, 16).ok());
                if let Some(value) = digits {
                    name.push(char::from(value));
                    self.cursor += 3;
                    continue;
                }
            }
            name.push(char::from(*byte));
            self.cursor += 1;
        }
        Ok(Object::Name(name))
    }

    fn literal_string(&mut self) -> Result<Object, PdfError> {
        self.cursor += 1;
        let mut out = Vec::new();
        let mut depth = 1;
        while let Some(byte) = self.bytes.get(self.cursor).copied() {
            self.cursor += 1;
            match byte {
                b'\\' => {
                    let escaped = self.bytes.get(self.cursor).copied().unwrap_or(b'\\');
                    self.cursor += 1;
                    match escaped {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'\n' => {}
                        b'\r' => {
                            if self.bytes.get(self.cursor) == Some(&b'\n') {
                                self.cursor += 1;
                            }
                        }
                        digit if digit.is_ascii_digit() => {
                            let mut value = u16::from(digit - b'0');
                            for _ in 0..2 {
                                match self.bytes.get(self.cursor) {
                                    Some(next) if next.is_ascii_digit() => {
                                        value = value * 8 + u16::from(next - b'0');
                                        self.cursor += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((value & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(byte);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Object::String(out));
                    }
                    out.push(byte);
                }
                other => out.push(other),
            }
        }
        Err(PdfError::Malformed {
            detail: "a string never closes".to_owned(),
        })
    }

    fn hex_string(&mut self) -> Result<Object, PdfError> {
        self.cursor += 1;
        let mut digits = Vec::new();
        while let Some(byte) = self.bytes.get(self.cursor).copied() {
            self.cursor += 1;
            if byte == b'>' {
                if digits.len() % 2 == 1 {
                    digits.push(b'0');
                }
                let bytes = digits
                    .chunks(2)
                    .filter_map(|pair| {
                        std::str::from_utf8(pair)
                            .ok()
                            .and_then(|text| u8::from_str_radix(text, 16).ok())
                    })
                    .collect();
                return Ok(Object::String(bytes));
            }
            if byte.is_ascii_hexdigit() {
                digits.push(byte);
            }
        }
        Err(PdfError::Malformed {
            detail: "a hexadecimal string never closes".to_owned(),
        })
    }

    fn dictionary_or_stream(&mut self) -> Result<Object, PdfError> {
        self.cursor += 2;
        let mut dictionary = Dictionary::new();
        loop {
            self.skip_space();
            if self.bytes[self.cursor..].starts_with(b">>") {
                self.cursor += 2;
                break;
            }
            let Object::Name(key) = self.name()? else {
                return Err(PdfError::Malformed {
                    detail: "a dictionary key is not a name".to_owned(),
                });
            };
            let value = self.object()?;
            dictionary.insert(key, value);
        }
        self.skip_space();
        if self.bytes[self.cursor..].starts_with(b"stream") {
            self.cursor += b"stream".len();
            // The data begins after the end-of-line that follows the keyword.
            if self.bytes.get(self.cursor) == Some(&b'\r') {
                self.cursor += 1;
            }
            if self.bytes.get(self.cursor) == Some(&b'\n') {
                self.cursor += 1;
            }
            let start = self.cursor;
            return Ok(Object::Stream {
                dictionary,
                data: (start, start),
            });
        }
        Ok(Object::Dictionary(dictionary))
    }

    fn number_or_reference(&mut self) -> Result<Object, PdfError> {
        let start = self.cursor;
        if matches!(self.bytes.get(self.cursor), Some(b'+' | b'-')) {
            self.cursor += 1;
        }
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
        {
            self.cursor += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.cursor]).unwrap_or("");
        let value = text.parse::<f64>().map_err(|_| PdfError::Malformed {
            detail: format!("'{text}' is not a number"),
        })?;

        // `12 0 R` is a reference; anything else is just a number. Deciding
        // needs two more tokens, so the cursor is put back if they are not
        // there.
        let rewind = self.cursor;
        if value >= 0.0 && value.fract() == 0.0 {
            let mut probe = Lexer::new(self.bytes, self.cursor);
            probe.skip_space();
            let generation_start = probe.cursor;
            while probe
                .bytes
                .get(probe.cursor)
                .is_some_and(u8::is_ascii_digit)
            {
                probe.cursor += 1;
            }
            if probe.cursor > generation_start {
                let generation = std::str::from_utf8(&self.bytes[generation_start..probe.cursor])
                    .ok()
                    .and_then(|text| text.parse::<u16>().ok());
                probe.skip_space();
                if let (Some(generation), Some(b'R')) =
                    (generation, probe.bytes.get(probe.cursor).copied())
                {
                    // `R` must stand alone, not begin a longer keyword.
                    let after = probe.bytes.get(probe.cursor + 1).copied();
                    if after.is_none_or(|byte| is_delimiter(byte) || byte.is_ascii_whitespace()) {
                        self.cursor = probe.cursor + 1;
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let number = value as u32;
                        return Ok(Object::Reference { number, generation });
                    }
                }
            }
        }
        self.cursor = rewind;
        Ok(Object::Number(value))
    }
}

const fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}
