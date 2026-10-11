// language-contract: product-copy
//! Reading the annotation model with `lopdf`.
//!
//! The file is hostile input: it is read whole but bounded
//! ([`MAX_FILE_BYTES`]); every stream `lopdf` inflates while loading is
//! bounded on its own ([`MAX_STREAM_BYTES`]) and the object streams of one
//! read together ([`MAX_INFLATED_BYTES`]); a damaged annotation costs only
//! itself; a page's `/Parent` chain and the form's `/Kids` tree are followed
//! with a depth bound (the page tree itself is `lopdf`'s walk) and the form
//! tree with a visited set. An encrypted document is refused, even one that
//! opens with the empty password, because Calcita never edits one.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use lopdf::{
    decode_text_string, DecompressError, Dictionary, Document, LoadOptions, Object, ObjectId,
    StringFormat,
};

use super::coords::{PageBox, Rotation};
use super::form::{fields_of, ReadOnly};
use super::model::{AnnotId, Annotation, Field, Kind, Point, Quad, Rect, Rgb};

/// The largest file read: a PDF past this is not parsed into memory.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// The most any one object or cross-reference stream may inflate to while
/// the document loads (a decompression bomb stops here).
pub const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// The most the object streams of one read may inflate to together: many
/// streams each under [`MAX_STREAM_BYTES`] stop here.
pub const MAX_INFLATED_BYTES: u64 = 256 * 1024 * 1024;

/// How deep the `/Parent` chain of a page and the `/Kids` tree of a form
/// are followed.
pub(crate) const MAX_DEPTH: usize = 32;

/// The size of a text box's font when its `/DA` names none.
const DEFAULT_TEXT_SIZE: f64 = 12.0;

/// Why a document's annotations could not be read.
#[derive(Debug)]
pub enum AnnotError {
    /// The file could not be opened or read.
    Io(std::io::Error),
    /// The file is larger than [`MAX_FILE_BYTES`].
    TooLarge { bytes: u64 },
    /// Its object streams inflate past `limit` bytes together.
    InflatesTooMuch { limit: u64 },
    /// The document is encrypted (with or without a password to open it).
    Encrypted,
    /// `lopdf` cannot parse the document.
    Unparseable(lopdf::Error),
    /// The document's cross-reference table is broken and `lopdf` rebuilt
    /// it by scanning: shown, but an update could not point back at it
    /// (`/Prev`), so it is not edited.
    Damaged,
    /// A text box's text holds a character Helvetica in WinAnsi (the one
    /// font Calcita writes, nothing embedded) cannot show.
    Unencodable(char),
    /// The edit names no annotation Calcita may change that way: one that
    /// is not there (any more), one of a kind Calcita only shows, or the
    /// text or colour of a text box with rich text.
    NotEditable,
    /// The form field cannot be filled in Calcita, and why.
    ReadOnlyField(ReadOnly),
    /// The fill names no field there is, or a value the field cannot take
    /// (text for a checkbox, a check for a text field).
    NotFillable,
    /// The text is longer than the field's `/MaxLen` characters.
    TooLong(usize),
    /// `lopdf` could not write the update.
    Write(std::io::Error),
}

impl AnnotError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> String {
        match self {
            Self::Io(_) => "No se pudo leer el documento.".to_owned(),
            Self::TooLarge { .. } | Self::InflatesTooMuch { .. } => {
                "El documento es demasiado grande para editarlo.".to_owned()
            }
            Self::Encrypted => "Este documento está cifrado y no se puede editar.".to_owned(),
            Self::Unparseable(_) => "Calcita no puede editar este documento.".to_owned(),
            Self::Damaged => {
                "Este documento está dañado: se puede leer, pero no editar.".to_owned()
            }
            Self::Unencodable(character) => {
                format!("El carácter «{character}» no se puede escribir en un cuadro de texto.")
            }
            Self::NotEditable => "Esa anotación no se puede cambiar así.".to_owned(),
            Self::ReadOnlyField(why) => why.message_es().to_owned(),
            Self::NotFillable => "Ese campo no se puede rellenar así.".to_owned(),
            Self::TooLong(most) => {
                format!("Este campo admite como máximo {most} caracteres.")
            }
            Self::Write(_) => "No se pudo escribir el documento editado.".to_owned(),
        }
    }
}

impl fmt::Display for AnnotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "the document could not be read: {error}"),
            Self::TooLarge { bytes } => write!(
                formatter,
                "the document is {bytes} bytes, past the {MAX_FILE_BYTES}-byte bound"
            ),
            Self::InflatesTooMuch { limit } => write!(
                formatter,
                "the document's object streams inflate past {limit} bytes"
            ),
            Self::Encrypted => formatter.write_str("the document is encrypted"),
            Self::Unparseable(error) => {
                write!(formatter, "the document could not be parsed: {error}")
            }
            Self::Damaged => formatter.write_str(
                "the document's cross-reference table was rebuilt by scanning; an update could not point back at it",
            ),
            Self::Unencodable(character) => write!(
                formatter,
                "{character:?} has no code in WinAnsiEncoding, Helvetica's encoding"
            ),
            Self::NotEditable => {
                formatter.write_str("the edit names no annotation that can change that way")
            }
            Self::ReadOnlyField(why) => write!(formatter, "the field cannot be filled: {why:?}"),
            Self::NotFillable => formatter.write_str("the fill names no field that takes that value"),
            Self::TooLong(most) => write!(formatter, "the field holds at most {most} characters"),
            Self::Write(error) => write!(formatter, "the update could not be written: {error}"),
        }
    }
}

impl std::error::Error for AnnotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) | Self::Write(error) => Some(error),
            Self::Unparseable(error) => Some(error),
            Self::TooLarge { .. }
            | Self::InflatesTooMuch { .. }
            | Self::Encrypted
            | Self::Damaged
            | Self::Unencodable(_)
            | Self::NotEditable
            | Self::ReadOnlyField(_)
            | Self::NotFillable
            | Self::TooLong(_) => None,
        }
    }
}

impl From<std::io::Error> for AnnotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Every annotation of every page of the document at `path`, page by page,
/// each page's in its `/Annots` order.
///
/// # Errors
///
/// [`AnnotError`] when the file cannot be read, is too large, is encrypted
/// or cannot be parsed.
pub fn read_annotations(path: &Path) -> Result<Vec<Annotation>, AnnotError> {
    Ok(model_of(&load(path)?).0)
}

/// Every terminal field of the document's form, in the form's order; empty
/// for a document without one.
///
/// # Errors
///
/// As [`read_annotations`].
pub fn read_fields(path: &Path) -> Result<Vec<Field>, AnnotError> {
    let document = load(path)?;
    Ok(fields_of(&document)
        .into_iter()
        .map(|field| {
            let first = field.widgets.first();
            Field {
                name: field.name.0,
                kind: field.kind,
                value: field.value,
                rect: first.map_or(Rect::new(0.0, 0.0, 0.0, 0.0), |widget| widget.rect),
                page: first.and_then(|widget| widget.page),
            }
        })
        .collect())
}

/// Each page's box and rotation, in page order, as PDFium takes them: the
/// `/MediaBox` (US Letter when it has no area), the `/CropBox` clipped to it
/// when there is one with an area left, both inherited from the page tree
/// as ISO 32000-1 allows.
///
/// # Errors
///
/// As [`read_annotations`].
pub fn read_page_boxes(path: &Path) -> Result<Vec<PageBox>, AnnotError> {
    Ok(model_of(&load(path)?).1)
}

fn load(path: &Path) -> Result<Document, AnnotError> {
    load_within(path, MAX_INFLATED_BYTES).map(|loaded| loaded.document)
}

/// A document as read for an edit: the file's bytes, which an incremental
/// update keeps as its prefix, and what `lopdf` made of them.
pub(crate) struct Loaded {
    pub(crate) bytes: Vec<u8>,
    pub(crate) document: Document,
}

impl fmt::Debug for Loaded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Loaded")
            .field("bytes", &self.bytes.len())
            .field("objects", &self.document.objects.len())
            .finish()
    }
}

/// Every annotation of `document` and each page's box, as
/// [`read_annotations`] and [`read_page_boxes`] give them.
pub(crate) fn model_of(document: &Document) -> (Vec<Annotation>, Vec<PageBox>) {
    let mut annotations = Vec::new();
    let mut boxes = Vec::new();
    for (page, page_id) in pages(document) {
        boxes.push(page_box(document, page_id));
        for (index, id, dictionary) in page_annotations(document, page_id) {
            if let Some(annotation) = annotation(document, page, index, id, dictionary) {
                annotations.push(annotation);
            }
        }
    }
    (annotations, boxes)
}

/// Where an annotation is in its file: the page object listing it, its
/// place in that page's `/Annots`, and its own object when it has one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Location {
    pub(crate) page: ObjectId,
    pub(crate) index: u32,
    pub(crate) object: Option<ObjectId>,
}

/// The [`Location`] of each annotation of `document` the model names.
pub(crate) fn locations(document: &Document) -> HashMap<AnnotId, Location> {
    let mut found = HashMap::new();
    for (page, page_id) in pages(document) {
        for (index, object, dictionary) in page_annotations(document, page_id) {
            if let Some(annotation) = annotation(document, page, index, object, dictionary) {
                found.entry(annotation.id).or_insert(Location {
                    page: page_id,
                    index,
                    object,
                });
            }
        }
    }
    found
}

/// [`read_annotations`]'s load, keeping the bytes for an update, for a
/// document that can take one: one whose cross-reference table `lopdf`
/// found where the file says (a table rebuilt by scanning leaves no offset
/// for the update's `/Prev`, so it is [`AnnotError::Damaged`]; the reader
/// still shows its annotations).
pub(crate) fn load_for_edit(path: &Path) -> Result<Loaded, AnnotError> {
    let loaded = load_within(path, MAX_INFLATED_BYTES)?;
    if loaded.document.xref_start == 0 {
        return Err(AnnotError::Damaged);
    }
    Ok(loaded)
}

/// What one read may still inflate, for [`count_inflation`].
#[derive(Clone, Copy)]
struct Inflation {
    remaining: u64,
    exceeded: bool,
}

thread_local! {
    /// The budget of the read running on this thread. `lopdf` is built
    /// without its `rayon` feature, so a load and its object filter run on
    /// the thread that asked for it.
    static INFLATION: Cell<Option<Inflation>> = const { Cell::new(None) };
}

/// Clears this thread's budget when the load ends, however it ends.
struct InflationScope;

impl InflationScope {
    fn begin(limit: u64) -> Self {
        INFLATION.with(|budget| {
            budget.set(Some(Inflation {
                remaining: limit,
                exceeded: false,
            }));
        });
        Self
    }

    fn exceeded(&self) -> bool {
        INFLATION.with(|budget| budget.get().is_some_and(|budget| budget.exceeded))
    }
}

impl Drop for InflationScope {
    fn drop(&mut self) {
        INFLATION.with(|budget| budget.set(None));
    }
}

/// `lopdf`'s object filter: inflates each object stream here, within what
/// the read has left, before `lopdf` parses it (which then finds it already
/// plain and inflates nothing more). An object stream that would take the
/// read past its budget marks the read, which then fails, and is dropped, as
/// is every later one; one that is broken, or past [`MAX_STREAM_BYTES`] on
/// its own, is dropped as `lopdf` itself would skip it. Every other object
/// passes unchanged.
///
/// `lopdf` keeps a top-level object as it was handed in (and inflated here)
/// and takes the returned copy for an object-stream member, so the copy is
/// returned in both cases; members are never streams, so it is small.
fn count_inflation(id: ObjectId, object: &mut Object) -> Option<(ObjectId, Object)> {
    if let Object::Stream(stream) = object {
        if let Some(mut budget) = INFLATION
            .with(Cell::get)
            .filter(|_| stream.dict.has_type(b"ObjStm"))
        {
            if budget.exceeded {
                return None;
            }
            let cap = MAX_STREAM_BYTES as u64;
            let allowed = budget.remaining.min(cap);
            let inflated = if stream.is_compressed() {
                stream.decompress_with_limit(usize::try_from(allowed).unwrap_or(MAX_STREAM_BYTES))
            } else {
                Ok(())
            };
            let size = stream.content.len() as u64;
            let keep = match inflated {
                Ok(()) if size <= budget.remaining => {
                    budget.remaining -= size;
                    true
                }
                // Plain, but more than the read has left.
                Ok(()) => {
                    budget.exceeded = true;
                    false
                }
                // The budget, not the per-stream cap, was the bound.
                Err(lopdf::Error::Decompress(DecompressError::MemoryLimitExceeded { .. }))
                    if allowed < cap =>
                {
                    budget.exceeded = true;
                    false
                }
                Err(_) => false,
            };
            INFLATION.with(|cell| cell.set(Some(budget)));
            if !keep {
                return None;
            }
        }
    }
    Some((id, object.clone()))
}

/// [`load`] with `limit` bytes for the object streams to inflate to.
pub(crate) fn load_within(path: &Path, limit: u64) -> Result<Loaded, AnnotError> {
    let file = File::open(path)?;
    let bytes = file.metadata()?.len();
    if bytes > MAX_FILE_BYTES {
        return Err(AnnotError::TooLarge { bytes });
    }
    let mut buffer = Vec::with_capacity(usize::try_from(bytes).unwrap_or(0));
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut buffer)?;
    let read = u64::try_from(buffer.len()).unwrap_or(u64::MAX);
    if read > MAX_FILE_BYTES {
        return Err(AnnotError::TooLarge { bytes: read });
    }
    let options = LoadOptions {
        max_decompressed_size: Some(MAX_STREAM_BYTES),
        filter: Some(count_inflation),
        ..LoadOptions::default()
    };
    let scope = InflationScope::begin(limit);
    let loaded = Document::load_mem_with_options(&buffer, options);
    if scope.exceeded() {
        return Err(AnnotError::InflatesTooMuch { limit });
    }
    drop(scope);
    match loaded {
        // lopdf opens a document whose user password is empty and drops its
        // `/Encrypt`; `was_encrypted` still tells.
        Ok(document) if document.is_encrypted() || document.was_encrypted() => {
            Err(AnnotError::Encrypted)
        }
        Ok(document) => Ok(Loaded {
            bytes: buffer,
            document,
        }),
        Err(
            lopdf::Error::Decryption(_)
            | lopdf::Error::InvalidPassword
            | lopdf::Error::UnsupportedSecurityHandler(_)
            | lopdf::Error::AlreadyEncrypted,
        ) => Err(AnnotError::Encrypted),
        Err(error) => Err(AnnotError::Unparseable(error)),
    }
}

/// The pages, 0-based, in order.
pub(crate) fn pages(document: &Document) -> Vec<(u32, ObjectId)> {
    document
        .get_pages()
        .into_values()
        .enumerate()
        .map(|(index, id)| (u32::try_from(index).unwrap_or(u32::MAX), id))
        .collect()
}

/// `object` with references followed (a bounded chain).
pub(crate) fn resolve<'a>(document: &'a Document, object: &'a Object) -> &'a Object {
    document
        .dereference(object)
        .map_or(object, |(_, target)| target)
}

pub(crate) fn resolve_dictionary<'a>(
    document: &'a Document,
    object: &'a Object,
) -> Option<&'a Dictionary> {
    resolve(document, object).as_dict().ok()
}

pub(crate) fn number(document: &Document, object: &Object) -> Option<f64> {
    resolve(document, object).as_float().ok().map(f64::from)
}

pub(crate) fn numbers(document: &Document, object: &Object) -> Vec<f64> {
    resolve(document, object)
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| number(document, item))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn rect_of(document: &Document, object: &Object) -> Option<Rect> {
    match numbers(document, object).as_slice() {
        [ax, ay, bx, by] => Some(Rect::new(*ax, *ay, *bx, *by)),
        _ => None,
    }
}

pub(crate) fn text(document: &Document, dictionary: &Dictionary, key: &[u8]) -> Option<String> {
    let object = resolve(document, dictionary.get(key).ok()?);
    object.as_str().ok().map(decode_text)
}

/// A PDF text string (ISO 32000-1, 7.9.2.2): UTF-16 after a byte-order mark
/// (big-endian as the standard writes it, little-endian as some programs
/// do), UTF-8 after its mark, PDFDocEncoding otherwise. The mark is never
/// part of the text, and a malformed string is decoded as far as it goes
/// with U+FFFD for what does not decode (an odd last byte, a lone
/// surrogate, a byte PDFDocEncoding leaves undefined), never dropped. In
/// PDFDocEncoding the tab, line feed and carriage return are themselves
/// (ISO 32000-2, Annex D), which `lopdf`'s table leaves undefined. A line
/// break reads as a line feed whatever ends it (CR LF, CR or LF), so a
/// multiline value or a text box's text from any program reads with its
/// lines.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| {
        let pairs = rest.chunks_exact(2);
        let odd = !pairs.remainder().is_empty();
        let mut text: String = char::decode_utf16(pairs.map(|pair| unit([pair[0], pair[1]])))
            .map(|decoded| decoded.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        if odd {
            text.push(char::REPLACEMENT_CHARACTER);
        }
        text
    };
    let text = if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        utf16(rest, u16::from_be_bytes)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        String::from_utf8_lossy(rest).into_owned()
    } else {
        pdf_doc_text(bytes)
    };
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// `bytes` in PDFDocEncoding, its tabs and line breaks kept.
fn pdf_doc_text(bytes: &[u8]) -> String {
    let control = |byte: &u8| matches!(byte, b'\t' | b'\n' | b'\r');
    let mut text = String::new();
    for run in bytes.split_inclusive(control) {
        let (body, end) = match run.split_last() {
            Some((last, body)) if control(last) => (body, Some(char::from(*last))),
            _ => (run, None),
        };
        if !body.is_empty() {
            text.push_str(&pdf_doc_run(body));
        }
        text.extend(end);
    }
    text
}

/// A run of PDFDocEncoding without tabs or line breaks: whole, else byte
/// by byte with U+FFFD for an undefined one.
fn pdf_doc_run(bytes: &[u8]) -> String {
    let string = Object::String(bytes.to_vec(), StringFormat::Literal);
    decode_text_string(&string).unwrap_or_else(|_| {
        bytes
            .iter()
            .map(|&byte| {
                let decoded =
                    decode_text_string(&Object::String(vec![byte], StringFormat::Literal));
                decoded.unwrap_or_else(|_| char::REPLACEMENT_CHARACTER.to_string())
            })
            .collect()
    })
}

pub(crate) fn name(document: &Document, dictionary: &Dictionary, key: &[u8]) -> Option<String> {
    let object = resolve(document, dictionary.get(key).ok()?);
    object
        .as_name()
        .ok()
        .map(|name| String::from_utf8_lossy(name).into_owned())
}

/// An attribute a page inherits from the page tree when it lacks it.
pub(crate) fn inherited<'a>(
    document: &'a Document,
    page: ObjectId,
    key: &[u8],
) -> Option<&'a Object> {
    let mut node = document.get_dictionary(page).ok()?;
    for _ in 0..MAX_DEPTH {
        if let Ok(value) = node.get(key) {
            return Some(value);
        }
        let parent = node.get(b"Parent").ok()?.as_reference().ok()?;
        node = document.get_dictionary(parent).ok()?;
    }
    None
}

fn has_area(rect: &Rect) -> bool {
    rect.width() > 0.0 && rect.height() > 0.0
}

/// The part of `a` inside `b`, with no area when they do not meet.
fn intersection(a: Rect, b: Rect) -> Rect {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    Rect {
        x0,
        y0,
        x1: a.x1.min(b.x1).max(x0),
        y1: a.y1.min(b.y1).max(y0),
    }
}

pub(crate) fn page_box(document: &Document, page: ObjectId) -> PageBox {
    let usable = |key: &[u8]| {
        inherited(document, page, key)
            .and_then(|object| rect_of(document, object))
            .filter(has_area)
    };
    let media = usable(b"MediaBox").unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
    let rect = usable(b"CropBox")
        .map(|crop| intersection(crop, media))
        .filter(has_area)
        .unwrap_or(media);
    let rotate = inherited(document, page, b"Rotate")
        .map(|object| resolve(document, object))
        .map_or(Rotation::R0, |object| match object {
            Object::Integer(degrees) => Rotation::from_degrees(*degrees),
            Object::Real(degrees) => Rotation::from_number(f64::from(*degrees)),
            _ => Rotation::R0,
        });
    PageBox {
        x0: rect.x0,
        y0: rect.y0,
        w: rect.width(),
        h: rect.height(),
        rotate,
    }
}

/// A page's `/Annots` entries: each dictionary with its place in the array
/// (counting every entry, so a `null` or a dangling reference before it
/// still counts) and its object id when it is an indirect object. An entry
/// that is not a dictionary is skipped.
pub(crate) fn page_annotations(
    document: &Document,
    page: ObjectId,
) -> Vec<(u32, Option<ObjectId>, &Dictionary)> {
    let Some(entries) = document
        .get_dictionary(page)
        .ok()
        .and_then(|page| page.get(b"Annots").ok())
        .and_then(|annots| resolve(document, annots).as_array().ok())
    else {
        return Vec::new();
    };
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let index = u32::try_from(index).ok()?;
            match entry {
                Object::Reference(id) => document
                    .get_dictionary(*id)
                    .ok()
                    .map(|dict| (index, Some(*id), dict)),
                Object::Dictionary(dict) => Some((index, None, dict)),
                _ => None,
            }
        })
        .collect()
}

pub(crate) fn annotation(
    document: &Document,
    page: u32,
    index: u32,
    object: Option<ObjectId>,
    dictionary: &Dictionary,
) -> Option<Annotation> {
    let subtype = name(document, dictionary, b"Subtype")?;
    let rect = rect_of(document, dictionary.get(b"Rect").ok()?)?;
    let id = match (object, text(document, dictionary, b"NM")) {
        (Some((number, generation)), _) => AnnotId::Object { number, generation },
        (None, Some(nm)) => AnnotId::Name(nm),
        (None, None) => AnnotId::Position { page, index },
    };
    let declared = dictionary
        .get(b"C")
        .ok()
        .and_then(|color| color_of(&numbers(document, color)));
    let (kind, color) = match subtype.as_str() {
        "FreeText" => {
            let appearance = text(document, dictionary, b"DA")
                .map(|da| DefaultAppearance::parse(&da))
                .unwrap_or_default();
            let kind = Kind::Text {
                contents: text(document, dictionary, b"Contents").unwrap_or_default(),
                size_pt: appearance.size.unwrap_or(DEFAULT_TEXT_SIZE),
                rich: dictionary.has(b"RC"),
            };
            (kind, appearance.color.or(declared).unwrap_or(Rgb::BLACK))
        }
        "Highlight" => {
            let quads = dictionary
                .get(b"QuadPoints")
                .map(|quads| numbers(document, quads))
                .unwrap_or_default()
                .chunks_exact(8)
                .map(|corners| Quad {
                    points: outline_order([
                        Point {
                            x: corners[0],
                            y: corners[1],
                        },
                        Point {
                            x: corners[2],
                            y: corners[3],
                        },
                        Point {
                            x: corners[4],
                            y: corners[5],
                        },
                        Point {
                            x: corners[6],
                            y: corners[7],
                        },
                    ]),
                })
                .collect();
            (Kind::Highlight { quads }, declared.unwrap_or(Rgb::YELLOW))
        }
        "Ink" => {
            let strokes = dictionary
                .get(b"InkList")
                .ok()
                .and_then(|list| resolve(document, list).as_array().ok())
                .map(|strokes| {
                    strokes
                        .iter()
                        .map(|stroke| {
                            numbers(document, stroke)
                                .chunks_exact(2)
                                .map(|xy| Point { x: xy[0], y: xy[1] })
                                .collect()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let width = ink_width(document, dictionary);
            (Kind::Ink { strokes, width }, declared.unwrap_or(Rgb::BLACK))
        }
        _ => (Kind::Other { subtype }, declared.unwrap_or(Rgb::BLACK)),
    };
    Some(Annotation {
        id,
        page,
        kind,
        rect,
        color,
        author: text(document, dictionary, b"T"),
    })
}

/// Which side of the line `a`→`b` the point `c` lies on: positive, negative
/// or 0 when on it.
fn turn(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Whether the segments `a`–`b` and `c`–`d` cross at a point inside both.
fn cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let opposite = |one: f64, other: f64| (one > 0.0 && other < 0.0) || (one < 0.0 && other > 0.0);
    opposite(turn(a, b, c), turn(a, b, d)) && opposite(turn(c, d, a), turn(c, d, b))
}

/// A quadrilateral's corners in the order Calcita keeps them: the order
/// Acrobat writes (upper left, upper right, lower left, lower right), in
/// which the outline runs 0, 1, 3, 2. ISO 32000-1 asks for the corners
/// counter-clockwise, and some programs write them so; read as Acrobat's
/// order, that outline would cross itself (an hourglass), so the order is
/// the one of the three ways round four points whose outline does not.
fn outline_order(points: [Point; 4]) -> [Point; 4] {
    let simple = |[a, b, c, d]: [Point; 4]| !cross(a, b, d, c) && !cross(b, d, c, a);
    [[0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3]]
        .into_iter()
        .map(|order| order.map(|index| points[index]))
        .find(|candidate| simple(*candidate))
        .unwrap_or(points)
}

/// An ink's line width: `/BS /W`, else the third number of `/Border`, else
/// 1 (ISO 32000-1, 12.5.2 and 12.5.4).
fn ink_width(document: &Document, dictionary: &Dictionary) -> f64 {
    let border_style = dictionary
        .get(b"BS")
        .ok()
        .and_then(|style| resolve_dictionary(document, style))
        .and_then(|style| style.get(b"W").ok())
        .and_then(|width| number(document, width));
    let border = || {
        dictionary
            .get(b"Border")
            .ok()
            .and_then(|border| numbers(document, border).get(2).copied())
    };
    border_style
        .or_else(border)
        .filter(|width| width.is_finite() && *width >= 0.0)
        .unwrap_or(1.0)
}

/// A colour array of 1 (grey), 3 (RGB) or 4 (CMYK) components in `0..=1`;
/// none for an empty array (transparent) or any other length.
pub(crate) fn color_of(components: &[f64]) -> Option<Rgb> {
    // Clamped to 0..=255 first, so the cast cannot truncate or wrap.
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    match *components {
        [grey] => Some(Rgb {
            r: channel(grey),
            g: channel(grey),
            b: channel(grey),
        }),
        [r, g, b] => Some(Rgb {
            r: channel(r),
            g: channel(g),
            b: channel(b),
        }),
        [c, m, y, k] => Some(Rgb {
            r: channel((1.0 - c) * (1.0 - k)),
            g: channel((1.0 - m) * (1.0 - k)),
            b: channel((1.0 - y) * (1.0 - k)),
        }),
        _ => None,
    }
}

/// What a `/DA` string says: the font size of its `Tf` and the colour of its
/// last fill-colour operator (`g`, `rg` or `k`).
#[derive(Default)]
pub(crate) struct DefaultAppearance {
    pub(crate) size: Option<f64>,
    pub(crate) color: Option<Rgb>,
}

impl DefaultAppearance {
    pub(crate) fn parse(da: &str) -> Self {
        let mut parsed = Self::default();
        let mut operands: Vec<f64> = Vec::new();
        for token in da.split_ascii_whitespace() {
            if let Ok(value) = token.parse::<f64>() {
                operands.push(value);
                continue;
            }
            match token {
                "Tf" => {
                    if let Some(size) = operands.last().copied().filter(|size| *size > 0.0) {
                        parsed.size = Some(size);
                    }
                }
                "g" | "rg" | "k" => {
                    let wanted = match token {
                        "g" => 1,
                        "rg" => 3,
                        _ => 4,
                    };
                    if operands.len() >= wanted {
                        parsed.color = color_of(&operands[operands.len() - wanted..]);
                    }
                }
                // A font name (`/Helv`) is an operand that is not a number.
                name if name.starts_with('/') => continue,
                _ => {}
            }
            operands.clear();
        }
        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::{load_within, AnnotError, MAX_INFLATED_BYTES};
    use lopdf::{Document, SaveOptions};
    use std::path::{Path, PathBuf};

    /// The annotated fixture saved with object streams and a cross-reference
    /// stream into a scratch file, and the bytes its object streams inflate
    /// to.
    fn with_object_streams(label: &str) -> (PathBuf, u64) {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../calcita/tests/fixtures/annotated.pdf");
        let mut document = Document::load(fixture).expect("the fixture");
        let dir = std::env::temp_dir().join(format!(
            "calcita-core-inflation-{}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("modern.pdf");
        let mut file = std::fs::File::create(&path).expect("created");
        document
            .save_with_options(
                &mut file,
                SaveOptions::builder()
                    .use_object_streams(true)
                    .use_xref_streams(true)
                    .build(),
            )
            .expect("saved");
        let saved = Document::load(&path).expect("reloaded");
        let inflated = saved
            .objects
            .values()
            .filter_map(|object| object.as_stream().ok())
            .filter(|stream| stream.dict.has_type(b"ObjStm"))
            .map(|stream| {
                stream
                    .decompressed_content()
                    .map_or(0, |plain| plain.len() as u64)
            })
            .sum();
        (path, inflated)
    }

    #[test]
    fn the_object_streams_of_one_read_share_one_budget() {
        let (path, inflated) = with_object_streams("budget");
        assert!(inflated > 0, "the save wrote object streams");
        assert!(load_within(&path, MAX_INFLATED_BYTES).is_ok());
        assert!(load_within(&path, inflated).is_ok(), "exactly enough");
        let short = inflated - 1;
        let error = load_within(&path, short).expect_err("one byte short");
        assert!(
            matches!(error, AnnotError::InflatesTooMuch { limit } if limit == short),
            "{error}"
        );
        assert_eq!(
            error.message_es(),
            "El documento es demasiado grande para editarlo."
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("a directory"));
    }
}
