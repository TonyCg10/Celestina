//! Rich Text Format: brace markup around plain text, and no container at all.
//!
//! The imported contract holds here for the same reason it holds for a `.docx`:
//! the text is located as byte spans and everything else — control words,
//! groups, the font and colour tables, the pictures — is copied as the bytes it
//! was. What differs is that there is no archive to rebuild, so a saved file is
//! the original with the edited spans spliced into it.
//!
//! Two of RTF's own rules matter to a reader. Text is written in a code page
//! the document names with `\ansicpg`, which is one of the single-byte
//! encodings this crate already carries, so `\'hh` needs no table of its own.
//! And a Unicode character is written `\uN` followed by `\uc` fallback bytes
//! for readers that predate it; both the number and the skip are the
//! document's, not this module's.

use std::fmt;

use crate::encoding::{Encoding, SingleByte};

use super::part::Anchor;

/// A destination whose content is never the document's text. Everything inside
/// one of these groups is markup: the fonts it declares, the colours, the
/// styles, the metadata, the bytes of a picture.
const SKIPPED_DESTINATIONS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "listtable",
    "listoverridetable",
    "info",
    "pict",
    "object",
    "themedata",
    "colorschememapping",
    "latentstyles",
    "datastore",
    "generator",
];

/// Why an RTF file did not become text, or text did not go back into one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RtfError {
    /// The file does not start the way every RTF file does.
    NotRtf,
    /// The file carries no text at all: there is nowhere to put an edit.
    NoText,
    /// The edited text has a different number of paragraphs. Adding or removing
    /// one is structure, and structure is not something this editor writes.
    ParagraphCountChanged { had: usize, now: usize },
}

impl fmt::Display for RtfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRtf => formatter.write_str("this file is not rich text"),
            Self::NoText => formatter.write_str("this document carries no editable text"),
            Self::ParagraphCountChanged { had, now } => write!(
                formatter,
                "this document has {had} paragraphs and the text has {now}; \
                 Grafita edits words, not structure"
            ),
        }
    }
}

impl std::error::Error for RtfError {}

/// An RTF document: its bytes, the text inside them, and the way back.
#[derive(Clone, Debug)]
pub struct Document {
    bytes: Vec<u8>,
    text: String,
    anchors: Vec<Anchor>,
    /// How many bytes follow a `\uN` as its pre-Unicode fallback.
    fallback: usize,
}

impl Document {
    /// Whether these bytes announce themselves as rich text.
    #[must_use]
    pub fn looks_like_rtf(bytes: &[u8]) -> bool {
        bytes.starts_with(br"{\rtf")
    }

    /// Reads the text out of an RTF file.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, RtfError> {
        if !Self::looks_like_rtf(&bytes) {
            return Err(RtfError::NotRtf);
        }
        let mut text = String::new();
        let mut anchors = Vec::new();
        let mut cursor = 0;
        let mut depth = 0usize;
        // The depth a skipped destination started at, if the scan is inside one.
        let mut skipping: Option<usize> = None;
        let mut fallback = 1usize;
        // A paragraph mark ends a paragraph rather than starting one, so its
        // line break is held until text actually follows. Without that, the
        // final `\par` every document ends with would add a line nobody wrote,
        // and writing back would then demand that empty line forever.
        let mut pending_break = false;
        let mut codepage = SingleByte::Windows1252;
        // Where the current stretch of literal text began, in both spaces.
        let mut run: Option<(usize, usize)> = None;

        while cursor < bytes.len() {
            let byte = bytes[cursor];
            match byte {
                b'{' | b'}' => {
                    close_run(&mut run, &mut anchors, &text, cursor);
                    if byte == b'{' {
                        depth += 1;
                    } else {
                        if skipping == Some(depth) {
                            skipping = None;
                        }
                        depth = depth.saturating_sub(1);
                    }
                    cursor += 1;
                }
                b'\\' => {
                    let (word, argument, next) = control(&bytes, cursor);
                    // An escaped brace or backslash is text, not markup.
                    if word.is_empty() {
                        let literal = bytes.get(cursor + 1).copied();
                        if matches!(literal, Some(b'\\' | b'{' | b'}')) {
                            if skipping.is_none() {
                                flush_break(&mut pending_break, &mut text);
                                open_run(&mut run, &text, cursor);
                                text.push(char::from(literal.unwrap_or(b'\\')));
                            }
                            cursor += 2;
                            continue;
                        }
                    }
                    close_run(&mut run, &mut anchors, &text, cursor);
                    match word {
                        // A hex byte in the document's own code page.
                        "'" => {
                            if skipping.is_none() {
                                if let Some(value) = hex(&bytes, cursor + 2) {
                                    if let Ok(decoded) =
                                        Encoding::SingleByte(codepage).decode(&[value])
                                    {
                                        flush_break(&mut pending_break, &mut text);
                                        open_run(&mut run, &text, cursor);
                                        text.push_str(&decoded);
                                    }
                                }
                            }
                            cursor += 4;
                            continue;
                        }
                        "u" => {
                            if skipping.is_none() {
                                if let Some(point) = argument
                                    .and_then(|value| {
                                        u32::try_from(value.rem_euclid(0x1_0000)).ok()
                                    })
                                    .and_then(char::from_u32)
                                {
                                    flush_break(&mut pending_break, &mut text);
                                    open_run(&mut run, &text, cursor);
                                    text.push(point);
                                }
                            }
                            cursor = skip_fallback(&bytes, next, fallback);
                            continue;
                        }
                        "uc" => fallback = argument.unwrap_or(1).max(0) as usize,
                        "ansicpg" => {
                            if let Some(page) = argument.and_then(code_page) {
                                codepage = page;
                            }
                        }
                        "par" | "line" => {
                            if skipping.is_none() && !text.is_empty() {
                                pending_break = true;
                            }
                        }
                        "tab" => {
                            if skipping.is_none() {
                                flush_break(&mut pending_break, &mut text);
                                open_run(&mut run, &text, cursor);
                                text.push('\t');
                            }
                        }
                        // `\*` marks a destination a reader may ignore whole.
                        "*" => skipping = skipping.or(Some(depth)),
                        other if SKIPPED_DESTINATIONS.contains(&other) => {
                            skipping = skipping.or(Some(depth));
                        }
                        _ => {}
                    }
                    cursor = next;
                }
                b'\r' | b'\n' => {
                    // A line break in the file is formatting of the file, not
                    // of the document: RTF says a paragraph with `\par`.
                    close_run(&mut run, &mut anchors, &text, cursor);
                    cursor += 1;
                }
                _ => {
                    if skipping.is_none() {
                        flush_break(&mut pending_break, &mut text);
                        open_run(&mut run, &text, cursor);
                        // A byte of its own is written in the document's code
                        // page, exactly like a `\'hh` escape. Reading it as
                        // anything else would show a wrong letter for every
                        // accented word a Windows writer left literal.
                        match Encoding::SingleByte(codepage).decode(&[byte]) {
                            Ok(decoded) => text.push_str(&decoded),
                            Err(_) => text.push(char::REPLACEMENT_CHARACTER),
                        }
                    }
                    cursor += 1;
                }
            }
        }
        close_run(&mut run, &mut anchors, &text, bytes.len());

        if anchors.is_empty() {
            return Err(RtfError::NoText);
        }
        Ok(Self {
            bytes,
            text,
            anchors,
            fallback,
        })
    }

    /// The flat text an author edits.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The file with `text` written back into the spans it came from.
    pub fn write(&self, text: &str) -> Result<Vec<u8>, RtfError> {
        let lines: Vec<&str> = text.split('\n').collect();
        let paragraphs = self.paragraphs();
        if lines.len() != paragraphs.len() {
            return Err(RtfError::ParagraphCountChanged {
                had: paragraphs.len(),
                now: lines.len(),
            });
        }

        let mut out = Vec::with_capacity(self.bytes.len());
        let mut written = 0;
        for (line, runs) in lines.iter().zip(paragraphs.iter()) {
            for (index, anchor) in runs.iter().enumerate() {
                out.extend_from_slice(&self.bytes[written..anchor.xml.0]);
                if index == 0 {
                    out.extend_from_slice(self.escape(line).as_bytes());
                }
                written = anchor.xml.1;
            }
        }
        out.extend_from_slice(&self.bytes[written..]);
        Ok(out)
    }

    /// Writes one line the way RTF spells text.
    ///
    /// ASCII goes as itself with the three characters that mean something
    /// escaped. Anything else goes as `\uN` with the document's own number of
    /// fallback bytes, which is what a reader that predates Unicode will show
    /// instead — a question mark, and never a wrong letter.
    fn escape(&self, line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        for character in line.chars() {
            match character {
                '\\' => out.push_str(r"\\"),
                '{' => out.push_str(r"\{"),
                '}' => out.push_str(r"\}"),
                '\t' => out.push_str(r"\tab "),
                character if character.is_ascii() => out.push(character),
                character => {
                    let point = u32::from(character);
                    // Beyond the basic plane RTF wants a surrogate pair, which
                    // is what `encode_utf16` produces.
                    let mut units = [0u16; 2];
                    for unit in character.encode_utf16(&mut units) {
                        out.push_str(&format!("\\u{}", i32::from(*unit as i16)));
                        for _ in 0..self.fallback {
                            out.push('?');
                        }
                    }
                    let _ = point;
                }
            }
        }
        out
    }

    /// The runs of each paragraph. A paragraph break is a newline in the flat
    /// text, so the anchors split on exactly those.
    fn paragraphs(&self) -> Vec<Vec<Anchor>> {
        let mut paragraphs: Vec<Vec<Anchor>> = vec![Vec::new()];
        let mut previous_end = 0;
        for anchor in &self.anchors {
            if self.text[previous_end..anchor.text.0].contains('\n') {
                paragraphs.push(Vec::new());
            }
            if let Some(last) = paragraphs.last_mut() {
                last.push(*anchor);
            }
            previous_end = anchor.text.1;
        }
        paragraphs
    }
}

/// Puts off the line break a paragraph mark asked for until text arrives.
fn flush_break(pending: &mut bool, text: &mut String) {
    if *pending {
        text.push('\n');
        *pending = false;
    }
}

fn open_run(run: &mut Option<(usize, usize)>, text: &str, at: usize) {
    if run.is_none() {
        *run = Some((text.len(), at));
    }
}

fn close_run(run: &mut Option<(usize, usize)>, anchors: &mut Vec<Anchor>, text: &str, at: usize) {
    if let Some((text_start, byte_start)) = run.take() {
        if text.len() > text_start {
            anchors.push(Anchor {
                text: (text_start, text.len()),
                xml: (byte_start, at),
            });
        }
    }
}

/// The control word at `cursor`, its numeric argument, and where the word ends.
fn control(bytes: &[u8], cursor: usize) -> (&str, Option<i32>, usize) {
    let mut end = cursor + 1;
    if bytes.get(end) == Some(&b'\'') {
        return ("'", None, end + 1);
    }
    if bytes.get(end) == Some(&b'*') {
        return ("*", None, end + 1);
    }
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphabetic())
    {
        end += 1;
    }
    let word = std::str::from_utf8(&bytes[cursor + 1..end]).unwrap_or("");
    let mut number_end = end;
    if bytes.get(number_end) == Some(&b'-') {
        number_end += 1;
    }
    while bytes.get(number_end).is_some_and(u8::is_ascii_digit) {
        number_end += 1;
    }
    let argument = std::str::from_utf8(&bytes[end..number_end])
        .ok()
        .and_then(|digits| digits.parse::<i32>().ok());
    // One space after a control word is the delimiter and belongs to it.
    let mut next = number_end;
    if bytes.get(next) == Some(&b' ') {
        next += 1;
    }
    (word, argument, next)
}

/// The bytes a `\uN` is followed by for readers that do not know Unicode.
fn skip_fallback(bytes: &[u8], from: usize, fallback: usize) -> usize {
    let mut cursor = from;
    let mut remaining = fallback;
    while remaining > 0 && cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            // An escape counts as one fallback character however long it is.
            let (_word, _argument, next) = control(bytes, cursor);
            cursor = if bytes.get(cursor + 1) == Some(&b'\'') {
                cursor + 4
            } else {
                next
            };
        } else {
            cursor += 1;
        }
        remaining -= 1;
    }
    cursor
}

fn hex(bytes: &[u8], at: usize) -> Option<u8> {
    let text = std::str::from_utf8(bytes.get(at..at + 2)?).ok()?;
    u8::from_str_radix(text, 16).ok()
}

/// The single-byte encoding an `\ansicpg` number names, when this crate has it.
fn code_page(number: i32) -> Option<SingleByte> {
    Some(match number {
        1250 => SingleByte::Windows1250,
        1251 => SingleByte::Windows1251,
        1252 => SingleByte::Windows1252,
        1253 => SingleByte::Windows1253,
        1254 => SingleByte::Windows1254,
        1255 => SingleByte::Windows1255,
        1256 => SingleByte::Windows1256,
        1257 => SingleByte::Windows1257,
        1258 => SingleByte::Windows1258,
        437 => SingleByte::Cp437,
        850 => SingleByte::Cp850,
        866 => SingleByte::Cp866,
        10000 => SingleByte::MacRoman,
        _ => return None,
    })
}
