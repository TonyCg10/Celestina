//! The text a PDF shows, and where each piece of it is written.
//!
//! A PDF does not store text. It stores instructions to draw glyphs, numbered
//! by whatever the font decided, so "what does this say" is a question only the
//! font can answer. Where a font carries a `ToUnicode` map, that map is the
//! answer and this module uses it; where it does not, the bytes are read as the
//! standard Latin encoding, which is right for the simple fonts that omit it
//! and honest about being a fallback.
//!
//! Nothing here lays anything out. Text is collected in the order the page
//! draws it, which is the order it was written in.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use celestina_core::CancellationToken;

use super::file::{ObjectKey, Pdf};
use super::glyphs;
use super::object::{Dictionary, Lexer, Object, PdfError};
use crate::encoding::{Encoding, SingleByte};

/// What one font map entry costs on the work budget: about what it occupies
/// in memory, so the maps a document builds are bounded like its bytes.
const ENTRY_COST: usize = 48;

/// How many entries one `ToUnicode` map may write, overwrites included.
///
/// A code is at most two bytes wide in the fonts this reader decodes, so a
/// map holds at most 65 536 distinct entries; twice that allows every code to
/// be restated once. A `bfrange` line writes up to 65 536 entries from a
/// dozen bytes, and a file repeating one was the fastest way to spend minutes
/// or gigabytes on one font.
const MAX_MAP_WRITES: usize = 2 * 65_536;

/// Where one drawn string lives, so an edit can find its way back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    /// The object holding the content stream this string is drawn by.
    pub stream: u32,
    /// Which of the extraction's fonts drew it, and therefore what its bytes
    /// must be written back as.
    pub font: usize,
    /// The string's byte range inside the *decoded* stream.
    pub span: (usize, usize),
    /// Where the text it produced sits in the flat text.
    pub text: (usize, usize),
}

/// What a page says, and where each piece of it is written.
#[derive(Clone, Debug, Default)]
pub struct Extraction {
    pub text: String,
    pub placements: Vec<Placement>,
    /// Every font any placement was drawn with, in the order they were met.
    /// Shared rather than copied: a content stream may select the same font
    /// thousands of times, and its map is read once.
    pub fonts: Vec<Arc<Font>>,
    /// A line break the page asked for, held until text follows it. Moving the
    /// pen before drawing anything is not a blank line, and a document that
    /// began with one would grow a line nobody wrote.
    pending_break: bool,
}

/// Reads every page's text, in page order, until `cancellation` says stop.
///
/// The whole reading is one walk under [`Pdf::walk_budget`]: every lookup,
/// decode and font map counts against it, and the token is checked at each.
pub fn extract(pdf: &Pdf, cancellation: &CancellationToken) -> Result<Extraction, PdfError> {
    pdf.within(pdf.walk_budget(), cancellation, || read_pages(pdf))
}

fn read_pages(pdf: &Pdf) -> Result<Extraction, PdfError> {
    let mut out = Extraction::default();
    // Index zero is always the fallback font, so a string drawn before any
    // `Tf`, or with a font the page does not name, still names one.
    out.fonts.push(Arc::new(Font::default()));
    let mut reader = Reader {
        pdf,
        fonts: BTreeMap::new(),
        font_sets: BTreeMap::new(),
    };
    for page in pages(pdf)? {
        // A page is read when its turn comes and dropped after it, so what
        // extraction holds is one page at a time, not the whole tree.
        pdf.checkpoint()?;
        let object;
        let dictionary = match &page.source {
            Source::Inline(dictionary) => dictionary,
            Source::Object(number) => {
                object = pdf.object(*number)?;
                match object.as_dictionary() {
                    Some(dictionary) => dictionary,
                    None => continue,
                }
            }
        };
        let fonts = reader.page_fonts(dictionary, page.inherited.as_deref())?;
        for stream in page_streams(pdf, dictionary)? {
            let content = pdf.stream_data(&pdf.object(stream)?)?;
            read_stream(&content, stream, &fonts, &mut out);
        }
        out.pending_break = true;
    }
    if out.placements.is_empty() {
        return Err(PdfError::NoText);
    }
    Ok(out)
}

/// Where a page's fonts were found, so a set shared by every page is read
/// once: the page's `/Resources` by reference, or its `/Font` dictionary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FontSet {
    Resources(ObjectKey),
    Font(ObjectKey),
}

type Fonts = Arc<BTreeMap<String, Arc<Font>>>;

/// Extraction's memory of what it has already read. Keyed by the bytes an
/// object reads rather than by its number, so a file that gives one font a
/// thousand numbers still has it read once.
struct Reader<'a> {
    pdf: &'a Pdf,
    /// Each font object, read once however many pages name it.
    fonts: BTreeMap<ObjectKey, Arc<Font>>,
    /// Each shared set of fonts, read once however many pages share it.
    font_sets: BTreeMap<FontSet, Fonts>,
}

impl Reader<'_> {
    fn key(&self, object: &Object) -> Result<Option<ObjectKey>, PdfError> {
        match object.as_reference() {
            Some(number) => self.pdf.key(number),
            None => Ok(None),
        }
    }

    /// One entry per font name the page uses: how its bytes become characters.
    fn page_fonts(
        &mut self,
        page: &Dictionary,
        inherited: Option<&Object>,
    ) -> Result<Fonts, PdfError> {
        let pdf = self.pdf;
        let Some(resources_entry) = page.get("Resources").or(inherited) else {
            return Ok(Fonts::default());
        };
        let resources_key = self.key(resources_entry)?.map(FontSet::Resources);
        if let Some(known) = resources_key.and_then(|key| self.font_sets.get(&key)) {
            return Ok(Arc::clone(known));
        }
        // Inherited resources written inline are read where they are: a copy
        // per page would cost pages × resources before anything was counted.
        let resolved;
        let resources = if resources_entry.as_reference().is_some() {
            resolved = pdf.resolve(resources_entry)?;
            &resolved
        } else {
            resources_entry
        };
        let Some(resources_dictionary) = resources.as_dictionary() else {
            return Ok(Fonts::default());
        };
        let font_key = match resources_dictionary.get("Font") {
            Some(entry) => self.key(entry)?.map(FontSet::Font),
            None => None,
        };
        let set = match font_key.and_then(|key| self.font_sets.get(&key)) {
            Some(known) => Arc::clone(known),
            None => {
                let fonts = pdf.entry(resources_dictionary, "Font")?;
                let mut out = BTreeMap::new();
                if let Some(fonts) = fonts.as_dictionary() {
                    // Listing a font costs something even when it is known,
                    // or a page could name the same font a million times.
                    pdf.spend(entry_cost(fonts.len()))?;
                    for (name, reference) in fonts {
                        if let Some(font) = self.font(reference)? {
                            out.insert(name.clone(), font);
                        }
                    }
                }
                let set = Arc::new(out);
                if let Some(key) = font_key {
                    self.font_sets.insert(key, Arc::clone(&set));
                }
                set
            }
        };
        if let Some(key) = resources_key {
            self.font_sets.insert(key, Arc::clone(&set));
        }
        Ok(set)
    }

    /// The font `reference` names, read once per font object.
    fn font(&mut self, reference: &Object) -> Result<Option<Arc<Font>>, PdfError> {
        let key = self.key(reference)?;
        if let Some(known) = key.and_then(|key| self.fonts.get(&key)) {
            return Ok(Some(Arc::clone(known)));
        }
        let font = self.pdf.resolve(reference)?;
        let Some(dictionary) = font.as_dictionary() else {
            return Ok(None);
        };
        let font = Arc::new(Font::read(self.pdf, dictionary)?);
        if let Some(key) = key {
            self.fonts.insert(key, Arc::clone(&font));
        }
        Ok(Some(font))
    }
}

/// What `entries` font map entries cost on the work counter.
fn entry_cost(entries: usize) -> usize {
    entries.saturating_mul(ENTRY_COST)
}

/// Where a page's dictionary is: an object of its own, read when the page's
/// turn comes, or written inline in its parent's `/Kids`.
enum Source {
    Object(u32),
    Inline(Dictionary),
}

/// A page, and the resources it inherits from the nodes above it when it has
/// none of its own. Shared, because every page under a node inherits the same
/// ones and copying them per page would cost pages × resources.
struct Page {
    source: Source,
    inherited: Option<Arc<Object>>,
}

/// The objects a page-tree walk has met, by number and by the bytes they
/// read.
#[derive(Default)]
struct Visited {
    numbers: BTreeSet<u32>,
    keys: BTreeSet<ObjectKey>,
}

impl Visited {
    /// Whether the walk should read object `number`: not when it has already
    /// met that number. Meeting the same object under another number is not
    /// a tree at all, and refused.
    fn first_visit(&mut self, pdf: &Pdf, number: u32) -> Result<bool, PdfError> {
        if !self.numbers.insert(number) {
            return Ok(false);
        }
        if let Some(key) = pdf.key(number)? {
            if !self.keys.insert(key) {
                return Err(PdfError::Malformed {
                    detail: format!(
                        "the page tree names one object under two numbers, {number} among them"
                    ),
                });
            }
        }
        Ok(true)
    }
}

/// Every page, in reading order. A page that is an object of its own is kept
/// as its number; only inline pages are kept as dictionaries.
fn pages(pdf: &Pdf) -> Result<Vec<Page>, PdfError> {
    let root = pdf.entry(pdf.trailer(), "Root")?;
    let catalogue = root.as_dictionary().cloned().ok_or(PdfError::Malformed {
        detail: "the file has no catalogue".to_owned(),
    })?;
    let mut visited = Visited::default();
    let reference = catalogue.get("Pages").and_then(Object::as_reference);
    if let Some(number) = reference {
        visited.first_visit(pdf, number)?;
    }
    let tree = pdf.entry(&catalogue, "Pages")?;
    let mut found = Vec::new();
    if let Some(node) = tree.as_dictionary() {
        if pdf.entry(node, "Type")?.as_name() == Some("Page") {
            let source = match reference {
                Some(number) => Source::Object(number),
                None => Source::Inline(node.clone()),
            };
            found.push(Page {
                source,
                inherited: None,
            });
        } else {
            walk(pdf, node, None, &mut found, 0, &mut visited)?;
        }
    }
    Ok(found)
}

/// Collects the pages under the page-tree node `node`.
///
/// A page tree is a tree: every node has one parent. A file that names a node
/// twice would otherwise be walked once per path to it, and forty levels of
/// "both kids are the next node" is 2^40 pages from a few hundred bytes. A
/// node already visited is therefore not visited again.
fn walk(
    pdf: &Pdf,
    node: &Dictionary,
    inherited: Option<Arc<Object>>,
    found: &mut Vec<Page>,
    depth: usize,
    visited: &mut Visited,
) -> Result<(), PdfError> {
    if depth > 64 {
        return Err(PdfError::Malformed {
            detail: "the page tree is deeper than any document".to_owned(),
        });
    }
    // A page inherits its resources from the nearest node above it that has
    // them, which is where a shared font usually lives.
    let inherited = match node.get("Resources") {
        Some(own) => Some(Arc::new(own.clone())),
        None => inherited,
    };
    let kids = pdf.entry(node, "Kids")?;
    for kid in kids.as_array().unwrap_or(&[]) {
        let number = kid.as_reference();
        if let Some(number) = number {
            if !visited.first_visit(pdf, number)? {
                continue;
            }
        }
        let resolved = pdf.resolve(kid)?;
        let Some(child) = resolved.as_dictionary() else {
            continue;
        };
        if pdf.entry(child, "Type")?.as_name() == Some("Page") {
            let source = match number {
                Some(number) => Source::Object(number),
                None => Source::Inline(child.clone()),
            };
            found.push(Page {
                source,
                inherited: inherited.clone(),
            });
        } else {
            walk(pdf, child, inherited.clone(), found, depth + 1, visited)?;
        }
    }
    Ok(())
}

/// The content streams of one page, in drawing order.
fn page_streams(pdf: &Pdf, page: &Dictionary) -> Result<Vec<u32>, PdfError> {
    let contents = page.get("Contents").cloned().unwrap_or(Object::Null);
    let mut streams = Vec::new();
    match contents {
        Object::Reference { number, .. } => match pdf.object(number)? {
            Object::Array(items) => {
                streams.extend(items.iter().filter_map(Object::as_reference));
            }
            _ => streams.push(number),
        },
        Object::Array(items) => {
            streams.extend(items.iter().filter_map(Object::as_reference));
        }
        _ => {}
    }
    Ok(streams)
}

/// How one font's bytes become characters, and back.
#[derive(Clone, Debug, Default)]
pub struct Font {
    /// The `ToUnicode` map, when the font carries one. This is the only thing
    /// in a PDF that states outright what a code means, so it wins.
    map: BTreeMap<u32, String>,
    /// What the font's own `/Encoding` says each code draws, read from its
    /// base encoding and its `/Differences`. Consulted when there is no
    /// `ToUnicode`, which is the common case for the standard fonts.
    encoded: BTreeMap<u32, char>,
    /// Whether its codes are two bytes wide rather than one.
    wide: bool,
}

impl Font {
    fn read(pdf: &Pdf, dictionary: &Dictionary) -> Result<Self, PdfError> {
        let wide = matches!(pdf.entry(dictionary, "Subtype")?.as_name(), Some("Type0"));
        let mut map = BTreeMap::new();
        if let Ok(stream @ Object::Stream { .. }) = pdf.entry(dictionary, "ToUnicode") {
            let content = pdf.stream_data(&stream)?;
            // The widest code this font's strings are read in; a map entry
            // past it can never be looked up.
            let widest = if wide { 0xFFFF } else { 0xFF };
            let (read, written) = read_to_unicode(&content, widest)?;
            pdf.spend(entry_cost(written))?;
            map = read;
        }
        let encoded = read_encoding(pdf, dictionary)?;
        pdf.spend(entry_cost(encoded.len()))?;
        Ok(Self { map, encoded, wide })
    }

    /// Writes text back the way this font draws it.
    ///
    /// A font that carries a `ToUnicode` map is read backwards: the code whose
    /// map is this character. A subset font numbers its glyphs however it
    /// pleased, so writing the ASCII byte instead would draw a different letter
    /// — silently, which is the one outcome this crate never accepts. A
    /// character the font has no code for is refused to the caller.
    pub fn encode(&self, text: &str) -> Result<Vec<u8>, char> {
        let mut out = Vec::with_capacity(text.len());
        for character in text.chars() {
            if self.map.is_empty() {
                // No map: the read took each byte as itself, so the write does
                // too, and only what fits in a byte can be written.
                let point = u32::from(character);
                if point > 0xFF {
                    return Err(character);
                }
                out.push(point as u8);
                continue;
            }
            let needle = character.to_string();
            let code = self
                .map
                .iter()
                .find_map(|(code, value)| (*value == needle).then_some(*code))
                .or_else(|| {
                    self.encoded
                        .iter()
                        .find_map(|(code, value)| (*value == character).then_some(*code))
                })
                .ok_or(character)?;
            if self.wide {
                out.extend_from_slice(&(code as u16).to_be_bytes());
            } else if code <= 0xFF {
                out.push(code as u8);
            } else {
                return Err(character);
            }
        }
        Ok(out)
    }

    /// What a drawn string says, as far as this font can tell.
    fn decode(&self, bytes: &[u8]) -> String {
        let mut out = String::new();
        if self.wide {
            for pair in bytes.chunks(2) {
                let code = pair
                    .iter()
                    .fold(0u32, |value, byte| (value << 8) | u32::from(*byte));
                match self.map.get(&code) {
                    Some(text) => out.push_str(text),
                    // A wide code with no map is a glyph number and nothing
                    // else; showing a wrong letter would be worse than a mark.
                    None => out.push(char::REPLACEMENT_CHARACTER),
                }
            }
            return out;
        }
        for byte in bytes {
            let code = u32::from(*byte);
            // `ToUnicode` first because it says outright what the code means;
            // then the font's own encoding, which is what an older document
            // has instead; and only then the byte itself.
            if let Some(text) = self.map.get(&code) {
                out.push_str(text);
            } else if let Some(character) = self.encoded.get(&code) {
                out.push(*character);
            } else {
                out.push(char::from(*byte));
            }
        }
        out
    }
}

/// What a simple font's `/Encoding` says each of its codes draws.
///
/// A base encoding gives the whole range, and `/Differences` overrides
/// individual codes by naming the glyph they draw. A name this crate does not
/// know is left out rather than guessed at, so the byte falls through as it
/// did before.
fn read_encoding(pdf: &Pdf, dictionary: &Dictionary) -> Result<BTreeMap<u32, char>, PdfError> {
    let encoding = pdf.entry(dictionary, "Encoding")?;
    let mut out = BTreeMap::new();

    let base = match &encoding {
        Object::Name(name) => Some(name.clone()),
        other => other
            .as_dictionary()
            .and_then(|inner| inner.get("BaseEncoding"))
            .and_then(Object::as_name)
            .map(str::to_owned),
    };
    // The two named base encodings this crate already carries as tables. The
    // third, `StandardEncoding`, is Adobe's own and differs above 0x7F; it is
    // left to `/Differences`, which such a font almost always supplies.
    let table = match base.as_deref() {
        Some("WinAnsiEncoding") => Some(SingleByte::Windows1252),
        Some("MacRomanEncoding") => Some(SingleByte::MacRoman),
        _ => None,
    };
    if let Some(table) = table {
        for byte in 0x20..=0xFFu16 {
            let raw = [u8::try_from(byte).unwrap_or(b' ')];
            if let Ok(text) = Encoding::SingleByte(table).decode(&raw) {
                if let Some(character) = text.chars().next() {
                    out.insert(u32::from(byte), character);
                }
            }
        }
    }

    // `/Differences` is a flat list: a number starts a run, and each name after
    // it takes the next code.
    if let Some(inner) = encoding.as_dictionary() {
        let differences = pdf.entry(inner, "Differences")?;
        let mut code = 0u32;
        for item in differences.as_array().unwrap_or(&[]) {
            match item {
                Object::Number(value) => {
                    code = if *value >= 0.0 {
                        // A code is a byte; anything else is not this array.
                        value.min(f64::from(u16::MAX)) as u32
                    } else {
                        0
                    };
                }
                Object::Name(name) => {
                    if let Some(character) = glyphs::name_to_char(name) {
                        out.insert(code, character);
                    } else {
                        out.remove(&code);
                    }
                    code += 1;
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

/// Reads the `bfchar` and `bfrange` sections of a `ToUnicode` map, and how
/// many entries it wrote.
///
/// Codes past `widest` are never looked up by the font that owns the map, so
/// they are not stored; and the writes, overwrites included, stop at
/// [`MAX_MAP_WRITES`] with a refusal.
fn read_to_unicode(
    content: &[u8],
    widest: u32,
) -> Result<(BTreeMap<u32, String>, usize), PdfError> {
    let mut map = BTreeMap::new();
    let mut writes = 0usize;
    let mut write = |map: &mut BTreeMap<u32, String>, code: u32, text: String| {
        writes += 1;
        if writes > MAX_MAP_WRITES {
            return Err(PdfError::Malformed {
                detail: "a font's ToUnicode map is larger than any font".to_owned(),
            });
        }
        if code <= widest {
            map.insert(code, text);
        }
        Ok(())
    };
    let mut lexer = Lexer::new(content, 0);
    let mut pending: Vec<Object> = Vec::new();
    while lexer.cursor < content.len() {
        lexer.skip_space();
        if lexer.cursor >= content.len() {
            break;
        }
        let byte = content[lexer.cursor];
        if byte == b'<' || byte == b'[' || byte.is_ascii_digit() {
            match lexer.object() {
                Ok(object) => {
                    pending.push(object);
                    continue;
                }
                Err(_) => break,
            }
        }
        // A keyword: read it and act on what came before.
        let start = lexer.cursor;
        while lexer
            .bytes
            .get(lexer.cursor)
            .is_some_and(|byte| byte.is_ascii_alphabetic())
        {
            lexer.cursor += 1;
        }
        if lexer.cursor == start {
            lexer.cursor += 1;
            continue;
        }
        let keyword = std::str::from_utf8(&content[start..lexer.cursor]).unwrap_or("");
        match keyword {
            "endbfchar" => {
                for pair in pending.chunks(2) {
                    if let [Object::String(code), Object::String(value)] = pair {
                        write(&mut map, number(code), utf16_be(value))?;
                    }
                }
                pending.clear();
            }
            "endbfrange" => {
                for triple in pending.chunks(3) {
                    match triple {
                        [Object::String(low), Object::String(high), Object::String(value)] => {
                            let (low, high) = (number(low), number(high));
                            let text = utf16_be(value);
                            let first = text.chars().next().unwrap_or('\u{FFFD}') as u32;
                            // One line is at most 65 536 codes, and none past
                            // what the font can draw.
                            let last = high.min(low.saturating_add(0xFFFF)).min(widest);
                            for (offset, code) in (low..=last).enumerate() {
                                let offset = u32::try_from(offset).unwrap_or(u32::MAX);
                                let point = first.saturating_add(offset);
                                if let Some(character) = char::from_u32(point) {
                                    write(&mut map, code, character.to_string())?;
                                }
                            }
                        }
                        [Object::String(low), Object::String(_high), Object::Array(values)] => {
                            let low = number(low);
                            for (offset, value) in values.iter().enumerate() {
                                let code = u32::try_from(offset)
                                    .ok()
                                    .and_then(|offset| low.checked_add(offset));
                                let Some(code) = code else {
                                    break;
                                };
                                if let Object::String(value) = value {
                                    write(&mut map, code, utf16_be(value))?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                pending.clear();
            }
            "beginbfchar" | "beginbfrange" => pending.clear(),
            _ => pending.clear(),
        }
    }
    Ok((map, writes))
}

fn number(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |value, byte| (value << 8) | u32::from(*byte))
}

/// A `ToUnicode` value is UTF-16 big endian, which is the one place a PDF says
/// what it means without asking a font.
fn utf16_be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// The gap wide enough to be a word break, in the thousandths of an em that
/// the `TJ` operator counts in.
///
/// A PDF does not write a space between words that are merely moved apart, so
/// without this every line comes out as one long word. The number is not a
/// convention: it was measured against every PDF on the author's machine,
/// comparing the text this crate reads with the text `pdftotext` reads from
/// the same file. Agreement peaks here at 83%, against 73% at a quarter of an
/// em; below about 120 the kerning inside ordinary words starts being read as
/// spaces and it falls away again.
const WORD_GAP: f64 = -150.0;

/// Walks one content stream, collecting what it draws.
///
/// `out.fonts` starts with the fallback font at index zero. A font the stream
/// selects is added once per stream and selected again by index, however many
/// `Tf` operators name it.
fn read_stream(
    content: &[u8],
    stream: u32,
    fonts: &BTreeMap<String, Arc<Font>>,
    out: &mut Extraction,
) {
    let mut lexer = Lexer::new(content, 0);
    let mut operands: Vec<(Object, (usize, usize))> = Vec::new();
    if out.fonts.is_empty() {
        out.fonts.push(Arc::new(Font::default()));
    }
    let mut selected: BTreeMap<&str, usize> = BTreeMap::new();
    let mut font = out.fonts.first().map(Arc::clone).unwrap_or_default();
    let mut current = 0usize;

    while lexer.cursor < content.len() {
        lexer.skip_space();
        if lexer.cursor >= content.len() {
            break;
        }
        let byte = content[lexer.cursor];
        if byte == b'/'
            || byte == b'('
            || byte == b'<'
            || byte == b'['
            || byte == b'+'
            || byte == b'-'
            || byte == b'.'
            || byte.is_ascii_digit()
        {
            let start = lexer.cursor;
            match lexer.object() {
                Ok(object) => operands.push((object, (start, lexer.cursor))),
                Err(_) => break,
            }
            continue;
        }
        let start = lexer.cursor;
        while lexer.bytes.get(lexer.cursor).is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || *byte == b'*' || *byte == b'\'' || *byte == b'"'
        }) {
            lexer.cursor += 1;
        }
        if lexer.cursor == start {
            lexer.cursor += 1;
            operands.clear();
            continue;
        }
        let operator = std::str::from_utf8(&content[start..lexer.cursor]).unwrap_or("");
        match operator {
            "Tf" => {
                if let Some((Object::Name(name), _)) = operands.first() {
                    current = match (selected.get(name.as_str()), fonts.get_key_value(name)) {
                        (Some(index), _) => *index,
                        (None, Some((key, chosen))) => {
                            out.fonts.push(Arc::clone(chosen));
                            let index = out.fonts.len() - 1;
                            selected.insert(key.as_str(), index);
                            index
                        }
                        // A name the page does not list draws with the
                        // fallback, as it always did.
                        (None, None) => 0,
                    };
                    font = out.fonts.get(current).map(Arc::clone).unwrap_or_default();
                }
            }
            "Tj" | "'" | "\"" => {
                if let Some((Object::String(bytes), span)) = operands
                    .iter()
                    .rev()
                    .find(|(object, _)| matches!(object, Object::String(_)))
                {
                    if operator != "Tj" {
                        out.pending_break = true;
                    }
                    let decoded = font.decode(bytes);
                    push(out, &decoded, stream, current, *span);
                }
            }
            "TJ" => {
                if let Some((Object::Array(items), _)) = operands.first() {
                    // The array alternates strings and the gaps between them.
                    // Each string keeps its own span so an edit lands in one.
                    // The walk starts *inside* the bracket: starting on it
                    // would read the whole array as one object and leave every
                    // span empty.
                    let mut cursor = operands[0].1 .0 + 1;
                    for item in items {
                        match item {
                            Object::String(bytes) => {
                                let found = find_string(content, cursor, bytes);
                                let decoded = font.decode(bytes);
                                push(out, &decoded, stream, current, found);
                                cursor = found.1;
                            }
                            // A gap wide enough to be a word break; see
                            // `WORD_GAP` for how the number was arrived at.
                            Object::Number(gap)
                                if *gap <= WORD_GAP
                                    && !out.text.is_empty()
                                    && !out.pending_break
                                    && !out.text.ends_with(' ') =>
                            {
                                out.text.push(' ');
                            }
                            _ => {}
                        }
                    }
                }
            }
            "Td" | "TD" | "T*" | "ET" => out.pending_break = true,
            _ => {}
        }
        operands.clear();
    }
}

fn push(out: &mut Extraction, decoded: &str, stream: u32, font: usize, span: (usize, usize)) {
    if decoded.is_empty() {
        return;
    }
    if out.pending_break {
        out.pending_break = false;
        if !out.text.is_empty() {
            out.text.push('\n');
        }
    }
    let start = out.text.len();
    out.text.push_str(decoded);
    out.placements.push(Placement {
        stream,
        font,
        span,
        text: (start, out.text.len()),
    });
}

/// Where a given string of a `TJ` array begins and ends, searching from `from`.
///
/// The bytes are compared rather than trusted to be the next string, so a walk
/// that falls behind is caught here instead of writing an edit into the wrong
/// word.
fn find_string(content: &[u8], from: usize, bytes: &[u8]) -> (usize, usize) {
    let mut lexer = Lexer::new(content, from);
    while lexer.cursor < content.len() {
        lexer.skip_space();
        match content.get(lexer.cursor) {
            Some(b'(' | b'<') => {
                let start = lexer.cursor;
                match lexer.object() {
                    Ok(Object::String(found)) if found == bytes => {
                        return (start, lexer.cursor);
                    }
                    Ok(_) => {}
                    Err(_) => return (from, from),
                }
            }
            Some(b']') | None => break,
            _ => {
                if lexer.object().is_err() {
                    break;
                }
            }
        }
    }
    (from, from)
}
