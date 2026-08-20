//! Documents that live inside a container somebody else wrote.
//!
//! A native document is bytes Grafita can reproduce exactly. An imported one is
//! the text inside a structure it cannot: what it promises instead is that
//! every part the author did not edit is written back as the bytes it already
//! was. The two never share a save path and one is never presented as the
//! other.

pub mod epub;
pub mod gzip;
pub mod part;
pub mod pdf;
pub mod rtf;

/// What a form's fields are listed under, when a document has any.
const FIELD_HEADING: &str = "\n\n--- Campos del formulario ---";

/// Splits the page's text from the field values a form document shows after it.
fn split_fields(text: &str, count: usize) -> Result<(&str, Vec<String>), ImportError> {
    if count == 0 {
        return Ok((text, Vec::new()));
    }
    let (page, listed) = text
        .split_once(FIELD_HEADING)
        .ok_or_else(|| ImportError::Incomplete {
            detail: "the list of form fields is gone from the text".to_owned(),
        })?;
    let values: Vec<String> = listed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_once(": ")
                .map_or_else(|| line.trim().to_owned(), |(_name, value)| value.to_owned())
        })
        .collect();
    if values.len() != count {
        return Err(ImportError::Part(PartError::ParagraphCountChanged {
            had: count,
            now: values.len(),
        }));
    }
    Ok((page, values))
}

/// What separates one part's text from the next in the flat projection. A blank
/// line, because a chapter break is not a paragraph break and an author needs
/// to see which is which before typing into either.
const PART_BREAK: &str = "\n\n";

use std::fmt;

use crate::container::{Container, ContainerError};
use part::{Part, PartError, Rules};

/// The container formats an imported document can be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// WordprocessingML, the `.docx` package.
    Docx,
    /// OpenDocument text, the `.odt` package.
    Odt,
    /// EPUB, whose text is several XHTML documents in reading order.
    Epub,
    /// Rich Text Format, which is markup around text and no container at all.
    Rtf,
    /// PDF, whose text is drawn rather than stored.
    Pdf,
    /// Text inside a gzip wrapper.
    Gzip,
}

impl Format {
    /// A stable identifier for logs and host-visible state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Docx => "Word (docx)",
            Self::Odt => "OpenDocument (odt)",
            Self::Epub => "EPUB",
            Self::Rtf => "Rich text (rtf)",
            Self::Pdf => "PDF",
            Self::Gzip => "Texto comprimido (gz)",
        }
    }

    /// The member whose presence identifies this format, by content rather
    /// than by the name the file was given.
    const fn marker(self) -> &'static str {
        match self {
            Self::Docx => "word/document.xml",
            Self::Odt => "content.xml",
            Self::Epub => "META-INF/container.xml",
            // Neither of these is an archive, so neither has a member to look
            // for.
            Self::Rtf | Self::Pdf | Self::Gzip => "",
        }
    }

    /// Which elements carry this format's text and which begin a line.
    #[must_use]
    pub const fn rules(self) -> Rules {
        match self {
            // Every scrap of text in WordprocessingML is inside `<w:t>`.
            Self::Docx => Rules {
                carriers: &["w:t"],
                skipped: &[],
                paragraphs: &["w:p"],
            },
            // OpenDocument puts text straight inside the paragraph, so the rule
            // is the other way round: everything counts except what these hold.
            Self::Odt => Rules {
                carriers: &[],
                skipped: &["office:annotation", "office:binary-data", "text:note-body"],
                paragraphs: &["text:p", "text:h"],
            },
            Self::Epub => Rules {
                carriers: &[],
                skipped: &["script", "style", "head", "title"],
                paragraphs: &["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "blockquote"],
            },
            // Neither has elements at all; each owns its own scan.
            Self::Rtf | Self::Pdf | Self::Gzip => Rules {
                carriers: &[],
                skipped: &[],
                paragraphs: &[],
            },
        }
    }
}

/// A document held inside a container somebody else wrote.
///
/// One part for a `.docx` or an `.odt`, several for an `.epub`: its chapters
/// are one document to the reader, which is the only way a person can edit a
/// book without being asked which file each sentence lives in.
#[derive(Clone, Debug)]
pub struct Imported {
    format: Format,
    body: Body,
}

/// What the document is actually made of. A package holds its text in members;
/// rich text holds it in one stream of its own bytes.
#[derive(Clone, Debug)]
enum Body {
    Package {
        container: Container,
        parts: Vec<(String, Part)>,
    },
    Rtf(Box<rtf::Document>),
    Gzip(Box<gzip::Compressed>),
    Pdf {
        file: Box<pdf::file::Pdf>,
        extraction: Box<pdf::text::Extraction>,
        fields: Vec<pdf::form::Field>,
    },
}

/// Why a container did not become a document, or a document did not go back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportError {
    /// The bytes are not a container this crate opens.
    Container(ContainerError),
    /// The container is one, but its text part is not readable.
    Part(PartError),
    /// A rich text file that could not be read or written.
    Rtf(rtf::RtfError),
    /// A PDF that could not be read.
    Pdf(pdf::object::PdfError),
    /// A compressed file that could not be read or written.
    Gzip(gzip::GzipError),
    /// A correction a PDF cannot take.
    PdfEdit(pdf::edit::EditError),
    /// The container declares parts it does not hold, or none at all.
    Incomplete { detail: String },
    /// A container with no part this crate knows how to read — a `.xlsx`, a
    /// `.jar`, an ordinary zip.
    UnknownFormat,
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(source) => source.fmt(formatter),
            Self::Part(source) => source.fmt(formatter),
            Self::Rtf(source) => source.fmt(formatter),
            Self::Pdf(source) => source.fmt(formatter),
            Self::Gzip(source) => source.fmt(formatter),
            Self::PdfEdit(source) => source.fmt(formatter),
            Self::Incomplete { detail } => {
                write!(formatter, "this container is incomplete: {detail}")
            }
            Self::UnknownFormat => {
                formatter.write_str("this container holds no document Grafita reads")
            }
        }
    }
}

impl std::error::Error for ImportError {}

impl Imported {
    /// Whether these bytes could be a container at all, by their first bytes.
    ///
    /// Cheap enough for the probe, and never an answer on its own: opening is
    /// what decides, as it does for text.
    #[must_use]
    pub fn looks_importable(bytes: &[u8]) -> bool {
        bytes.starts_with(b"PK\x03\x04")
            || rtf::Document::looks_like_rtf(bytes)
            || bytes.starts_with(b"%PDF-")
            || gzip::Compressed::looks_like_gzip(bytes)
    }

    /// Reads a container into a document, deciding the format by what is in it
    /// rather than by the name it was given.
    pub fn open(bytes: Vec<u8>) -> Result<Self, ImportError> {
        if gzip::Compressed::looks_like_gzip(&bytes) {
            let compressed = gzip::Compressed::open(&bytes).map_err(ImportError::Gzip)?;
            return Ok(Self {
                format: Format::Gzip,
                body: Body::Gzip(Box::new(compressed)),
            });
        }
        if bytes.starts_with(b"%PDF-") {
            let file = pdf::file::Pdf::parse(bytes).map_err(ImportError::Pdf)?;
            let extraction = pdf::text::extract(&file).map_err(ImportError::Pdf)?;
            let fields = pdf::form::fields(&file).map_err(ImportError::Pdf)?;
            return Ok(Self {
                format: Format::Pdf,
                body: Body::Pdf {
                    file: Box::new(file),
                    extraction: Box::new(extraction),
                    fields,
                },
            });
        }
        if rtf::Document::looks_like_rtf(&bytes) {
            let document = rtf::Document::parse(bytes).map_err(ImportError::Rtf)?;
            return Ok(Self {
                format: Format::Rtf,
                body: Body::Rtf(Box::new(document)),
            });
        }
        let container = Container::parse(bytes).map_err(ImportError::Container)?;
        let format = [Format::Docx, Format::Odt, Format::Epub]
            .into_iter()
            .find(|format| container.names().contains(&format.marker()))
            .ok_or(ImportError::UnknownFormat)?;
        let names = match format {
            Format::Docx | Format::Odt => vec![format.marker().to_owned()],
            Format::Epub => epub::reading_order(&container)?,
            Format::Rtf | Format::Pdf | Format::Gzip => return Err(ImportError::UnknownFormat),
        };
        let mut parts = Vec::with_capacity(names.len());
        for name in names {
            let bytes = container.read(&name).map_err(ImportError::Container)?;
            match Part::parse(bytes, format.rules()) {
                Ok(part) => parts.push((name, part)),
                // A part with no text is not a broken document. A book's cover
                // is one image and its own file, and refusing the whole book
                // over it — which is what happened — is the wrong answer. It
                // stays in the container untouched, because nothing here can
                // edit what it does not show.
                Err(PartError::NoRuns) => {}
                Err(source) => return Err(ImportError::Part(source)),
            }
        }
        if parts.is_empty() {
            return Err(ImportError::Part(PartError::NoRuns));
        }
        Ok(Self {
            format,
            body: Body::Package { container, parts },
        })
    }

    /// What kind of container this is.
    #[must_use]
    pub const fn format(&self) -> Format {
        self.format
    }

    /// The flat text an author edits. Several parts are one text, joined by a
    /// blank line so a chapter break is visible and countable.
    #[must_use]
    pub fn text(&self) -> String {
        match &self.body {
            Body::Package { parts, .. } => parts
                .iter()
                .map(|(_name, part)| part.text())
                .collect::<Vec<_>>()
                .join(PART_BREAK),
            Body::Rtf(document) => document.text().to_owned(),
            Body::Gzip(compressed) => compressed.text().to_owned(),
            Body::Pdf {
                extraction, fields, ..
            } => {
                let mut text = extraction.text.clone();
                // A form's fields are not drawn on the page, so they would be
                // invisible in the page's text. They are shown after it, under
                // a heading, because a form is made to be filled and a filler
                // needs to see the boxes.
                if !fields.is_empty() {
                    text.push_str(FIELD_HEADING);
                    for field in fields {
                        text.push_str(&format!("\n{}: {}", field.name, field.value));
                    }
                }
                text
            }
        }
    }

    /// The whole container, with `text` written back into the parts it came
    /// from and every other part copied as the bytes it already was.
    pub fn to_bytes(&self, text: &str) -> Result<Vec<u8>, ImportError> {
        let (container, parts) = match &self.body {
            Body::Rtf(document) => return document.write(text).map_err(ImportError::Rtf),
            Body::Gzip(compressed) => return compressed.to_bytes(text).map_err(ImportError::Gzip),
            Body::Pdf {
                file,
                extraction,
                fields,
            } => {
                let (page, filled) = split_fields(text, fields.len())?;
                let mut replacements = pdf::edit::replacements(file, extraction, page)
                    .map_err(ImportError::PdfEdit)?;
                let changed: Vec<(u32, String)> = fields
                    .iter()
                    .zip(filled)
                    .filter(|(field, value)| field.value != *value)
                    .map(|(field, value)| (field.object, value))
                    .collect();
                replacements
                    .extend(pdf::form::replacements(file, &changed).map_err(ImportError::Pdf)?);
                return pdf::update::append(file, &replacements).map_err(ImportError::Pdf);
            }
            Body::Package { container, parts } => (container, parts),
        };
        let pieces: Vec<&str> = text.split(PART_BREAK).collect();
        if pieces.len() != parts.len() {
            return Err(ImportError::Part(PartError::ParagraphCountChanged {
                had: parts.len(),
                now: pieces.len(),
            }));
        }
        let mut written = Vec::with_capacity(parts.len());
        for ((name, part), piece) in parts.iter().zip(pieces) {
            written.push((name.clone(), part.write(piece).map_err(ImportError::Part)?));
        }
        let replacements: Vec<(&str, Vec<u8>)> = written
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.clone()))
            .collect();
        container
            .rewrite(&replacements)
            .map_err(ImportError::Container)
    }
}
