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
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use lopdf::{
    decode_text_string, DecompressError, Dictionary, Document, LoadOptions, Object, ObjectId,
    StringFormat,
};

use super::coords::{PageBox, Rotation};
use super::model::{AnnotId, Annotation, Field, FieldKind, Kind, Point, Quad, Rect, Rgb};

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
const MAX_DEPTH: usize = 32;

/// The size of a text box's font when its `/DA` names none.
const DEFAULT_TEXT_SIZE: f64 = 12.0;

/// `/Ff` bits of a button field (ISO 32000-1, table 226).
const FLAG_RADIO: i64 = 1 << 15;
const FLAG_PUSHBUTTON: i64 = 1 << 16;

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
}

impl AnnotError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> &'static str {
        match self {
            Self::Io(_) => "No se pudo leer el documento.",
            Self::TooLarge { .. } | Self::InflatesTooMuch { .. } => {
                "El documento es demasiado grande para editarlo."
            }
            Self::Encrypted => "Este documento está cifrado y no se puede editar.",
            Self::Unparseable(_) => "Calcita no puede editar este documento.",
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
        }
    }
}

impl std::error::Error for AnnotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Unparseable(error) => Some(error),
            Self::TooLarge { .. } | Self::InflatesTooMuch { .. } | Self::Encrypted => None,
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
    let document = load(path)?;
    let mut annotations = Vec::new();
    for (page, page_id) in pages(&document) {
        for (index, id, dictionary) in page_annotations(&document, page_id) {
            if let Some(annotation) = annotation(&document, page, index, id, dictionary) {
                annotations.push(annotation);
            }
        }
    }
    Ok(annotations)
}

/// Every terminal field of the document's form, in the form's order; empty
/// for a document without one.
///
/// # Errors
///
/// As [`read_annotations`].
pub fn read_fields(path: &Path) -> Result<Vec<Field>, AnnotError> {
    let document = load(path)?;
    let Some(form) = document
        .catalog()
        .ok()
        .and_then(|catalog| catalog.get(b"AcroForm").ok())
        .and_then(|form| resolve_dictionary(&document, form))
    else {
        return Ok(Vec::new());
    };
    let Some(roots) = form
        .get(b"Fields")
        .ok()
        .and_then(|fields| resolve(&document, fields).as_array().ok())
    else {
        return Ok(Vec::new());
    };
    let widget_pages = widget_pages(&document);
    let mut walk = FieldWalk {
        document: &document,
        widget_pages: &widget_pages,
        visited: HashSet::new(),
        fields: Vec::new(),
    };
    for root in roots {
        walk.visit(root, &Inherited::default(), 0);
    }
    Ok(walk.fields)
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
    let document = load(path)?;
    Ok(pages(&document)
        .into_iter()
        .map(|(_, page_id)| page_box(&document, page_id))
        .collect())
}

fn load(path: &Path) -> Result<Document, AnnotError> {
    load_within(path, MAX_INFLATED_BYTES)
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
fn load_within(path: &Path, limit: u64) -> Result<Document, AnnotError> {
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
        Ok(document) => Ok(document),
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
fn pages(document: &Document) -> Vec<(u32, ObjectId)> {
    document
        .get_pages()
        .into_values()
        .enumerate()
        .map(|(index, id)| (u32::try_from(index).unwrap_or(u32::MAX), id))
        .collect()
}

/// `object` with references followed (a bounded chain).
fn resolve<'a>(document: &'a Document, object: &'a Object) -> &'a Object {
    document
        .dereference(object)
        .map_or(object, |(_, target)| target)
}

fn resolve_dictionary<'a>(document: &'a Document, object: &'a Object) -> Option<&'a Dictionary> {
    resolve(document, object).as_dict().ok()
}

fn number(document: &Document, object: &Object) -> Option<f64> {
    resolve(document, object).as_float().ok().map(f64::from)
}

fn numbers(document: &Document, object: &Object) -> Vec<f64> {
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

fn rect_of(document: &Document, object: &Object) -> Option<Rect> {
    match numbers(document, object).as_slice() {
        [ax, ay, bx, by] => Some(Rect::new(*ax, *ay, *bx, *by)),
        _ => None,
    }
}

fn text(document: &Document, dictionary: &Dictionary, key: &[u8]) -> Option<String> {
    let object = resolve(document, dictionary.get(key).ok()?);
    object.as_str().ok().map(decode_text)
}

/// A PDF text string (ISO 32000-1, 7.9.2.2): UTF-16 after a byte-order mark
/// (big-endian as the standard writes it, little-endian as some programs
/// do), UTF-8 after its mark, PDFDocEncoding otherwise. The mark is never
/// part of the text, and a malformed string is decoded as far as it goes
/// with U+FFFD for what does not decode (an odd last byte, a lone
/// surrogate, a byte PDFDocEncoding leaves undefined), never dropped.
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
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        utf16(rest, u16::from_be_bytes)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        String::from_utf8_lossy(rest).into_owned()
    } else {
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
}

fn name(document: &Document, dictionary: &Dictionary, key: &[u8]) -> Option<String> {
    let object = resolve(document, dictionary.get(key).ok()?);
    object
        .as_name()
        .ok()
        .map(|name| String::from_utf8_lossy(name).into_owned())
}

/// An attribute a page inherits from the page tree when it lacks it.
fn inherited<'a>(document: &'a Document, page: ObjectId, key: &[u8]) -> Option<&'a Object> {
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

fn page_box(document: &Document, page: ObjectId) -> PageBox {
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
fn page_annotations(
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

fn annotation(
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
                    points: [
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
                    ],
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
            (Kind::Ink { strokes }, declared.unwrap_or(Rgb::BLACK))
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

/// A colour array of 1 (grey), 3 (RGB) or 4 (CMYK) components in `0..=1`;
/// none for an empty array (transparent) or any other length.
fn color_of(components: &[f64]) -> Option<Rgb> {
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
struct DefaultAppearance {
    size: Option<f64>,
    color: Option<Rgb>,
}

impl DefaultAppearance {
    fn parse(da: &str) -> Self {
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

/// The page of each widget a page lists in `/Annots`.
fn widget_pages(document: &Document) -> HashMap<ObjectId, u32> {
    let mut found = HashMap::new();
    for (page, page_id) in pages(document) {
        for (_, id, _) in page_annotations(document, page_id) {
            if let Some(id) = id {
                found.entry(id).or_insert(page);
            }
        }
    }
    found
}

/// What a field inherits from its ancestors (ISO 32000-1, 12.7.3.1).
#[derive(Clone, Default)]
struct Inherited {
    name: String,
    field_type: Option<String>,
    flags: i64,
    value: Option<Object>,
}

struct FieldWalk<'a> {
    document: &'a Document,
    widget_pages: &'a HashMap<ObjectId, u32>,
    visited: HashSet<ObjectId>,
    fields: Vec<Field>,
}

impl FieldWalk<'_> {
    fn visit(&mut self, node: &Object, parent: &Inherited, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let id = node.as_reference().ok();
        if let Some(id) = id {
            if !self.visited.insert(id) {
                return;
            }
        }
        let Some(dictionary) = resolve_dictionary(self.document, node) else {
            return;
        };
        let partial = text(self.document, dictionary, b"T");
        let mut own = parent.clone();
        if let Some(partial) = partial {
            own.name = if own.name.is_empty() {
                partial
            } else {
                format!("{}.{partial}", own.name)
            };
        }
        if let Some(field_type) = name(self.document, dictionary, b"FT") {
            own.field_type = Some(field_type);
        }
        if let Some(flags) = dictionary
            .get(b"Ff")
            .ok()
            .and_then(|flags| resolve(self.document, flags).as_i64().ok())
        {
            own.flags = flags;
        }
        if let Ok(value) = dictionary.get(b"V") {
            own.value = Some(resolve(self.document, value).clone());
        }
        let kids: Vec<&Object> = dictionary
            .get(b"Kids")
            .ok()
            .and_then(|kids| resolve(self.document, kids).as_array().ok())
            .map(|kids| kids.iter().collect())
            .unwrap_or_default();
        // A kid with a `/T` is a field of its own; one without is a widget of
        // this field.
        let (fields, widgets): (Vec<&Object>, Vec<&Object>) = kids.into_iter().partition(|kid| {
            resolve_dictionary(self.document, kid).is_some_and(|kid| kid.has(b"T"))
        });
        for kid in fields {
            self.visit(kid, &own, depth + 1);
        }
        let first_widget = if widgets.is_empty() {
            dictionary.has(b"Rect").then_some((id, dictionary))
        } else {
            widgets.first().and_then(|widget| {
                resolve_dictionary(self.document, widget)
                    .map(|dict| (widget.as_reference().ok(), dict))
            })
        };
        let Some((widget_id, widget)) = first_widget else {
            return;
        };
        self.fields.push(self.field(&own, widget_id, widget));
    }

    fn field(&self, own: &Inherited, widget_id: Option<ObjectId>, widget: &Dictionary) -> Field {
        let kind = match own.field_type.as_deref() {
            Some("Tx") => FieldKind::Text,
            Some("Btn") if own.flags & (FLAG_RADIO | FLAG_PUSHBUTTON) == 0 => FieldKind::Checkbox,
            _ => FieldKind::Other,
        };
        let value = own
            .value
            .as_ref()
            .and_then(|value| match value {
                Object::Name(state) => Some(String::from_utf8_lossy(state).into_owned()),
                other => other.as_str().ok().map(decode_text),
            })
            .or_else(|| {
                (kind == FieldKind::Checkbox)
                    .then(|| name(self.document, widget, b"AS"))
                    .flatten()
            });
        let page = widget_id
            .and_then(|id| self.widget_pages.get(&id).copied())
            .or_else(|| {
                let target = widget.get(b"P").ok()?.as_reference().ok()?;
                pages(self.document)
                    .into_iter()
                    .find_map(|(page, id)| (id == target).then_some(page))
            });
        Field {
            name: own.name.clone(),
            kind,
            value,
            rect: widget
                .get(b"Rect")
                .ok()
                .and_then(|rect| rect_of(self.document, rect))
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
            page,
        }
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
