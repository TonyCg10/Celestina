//! Text inside a gzip wrapper.
//!
//! A `.txt.gz` is the one imported document whose inside is an ordinary text
//! file, so everything the native document knows — the encoding, the line
//! terminators, the byte-for-byte save — applies to what is in there. What
//! makes it imported rather than native is the wrapper: compression is not
//! reproducible. Two runs of the same compressor at different settings produce
//! different bytes for the same text, so this crate can promise the text back
//! exactly and the container only approximately, which is precisely the line
//! between the two contracts.

use std::fmt;
use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::encoding::Encoding;
use crate::probe::{classify, Classification};

/// Why a compressed file did not become text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GzipError {
    /// The bytes are not gzip.
    NotGzip,
    /// The wrapper opened but its contents could not be read.
    Corrupt,
    /// What is inside is not text: an image, an archive, a database.
    NotText,
    /// The text inside is in an encoding this crate cannot write back.
    Unsupported { detail: String },
}

impl fmt::Display for GzipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotGzip => formatter.write_str("this file is not gzip"),
            Self::Corrupt => formatter.write_str("this compressed file could not be read"),
            Self::NotText => formatter.write_str("what is inside this compressed file is not text"),
            Self::Unsupported { detail } => write!(
                formatter,
                "the text inside cannot be written back: {detail}"
            ),
        }
    }
}

impl std::error::Error for GzipError {}

/// A text file inside a gzip wrapper.
#[derive(Clone, Debug)]
pub struct Compressed {
    text: String,
    encoding: Encoding,
}

impl Compressed {
    /// Whether these bytes carry the gzip mark.
    #[must_use]
    pub fn looks_like_gzip(bytes: &[u8]) -> bool {
        bytes.starts_with(&[0x1F, 0x8B])
    }

    /// Reads the text inside.
    pub fn open(bytes: &[u8]) -> Result<Self, GzipError> {
        if !Self::looks_like_gzip(bytes) {
            return Err(GzipError::NotGzip);
        }
        let mut inside = Vec::new();
        GzDecoder::new(bytes)
            .read_to_end(&mut inside)
            .map_err(|_| GzipError::Corrupt)?;

        // The same question the editor asks of any file, asked of what came
        // out: a `.gz` holding a photograph is not a document.
        let encoding = match classify(&inside, true) {
            Classification::EditableText { encoding } => encoding,
            // A `.gz` holding another container is not opened as one: this
            // editor unwraps one layer, and nesting them is an archive tool's
            // job rather than a document editor's.
            Classification::Binary { .. } | Classification::ImportedDocument => {
                return Err(GzipError::NotText)
            }
            Classification::UnsupportedEncoding { reason } => {
                return Err(GzipError::Unsupported {
                    detail: reason.to_string(),
                })
            }
        };
        let text = encoding
            .decode(&inside)
            .map_err(|reason| GzipError::Unsupported {
                detail: reason.to_string(),
            })?;
        Ok(Self { text, encoding })
    }

    /// The text an author edits.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text compressed again.
    ///
    /// The text inside is reproduced exactly, in the encoding it was read with;
    /// the compression is this crate's own, which is why a `.gz` is an imported
    /// document and not a native one.
    pub fn to_bytes(&self, text: &str) -> Result<Vec<u8>, GzipError> {
        let inside = self
            .encoding
            .encode(text)
            .map_err(|source| GzipError::Unsupported {
                detail: source.to_string(),
            })?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&inside).map_err(|_| GzipError::Corrupt)?;
        encoder.finish().map_err(|_| GzipError::Corrupt)
    }
}
