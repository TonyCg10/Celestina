//! The text inside one part of a container, and the way back into it.
//!
//! The part is not parsed into a tree. A tree would have to be written out
//! again, and no serialiser reproduces someone else's whitespace, attribute
//! order or namespace prefixes — the imported contract would be broken by the
//! act of saving. Instead the text-carrying character data is located as byte
//! spans and everything between those spans is left exactly where it is.
//!
//! The formats differ only in which elements those are, which is what [`Rules`]
//! says. WordprocessingML puts every scrap of text inside `<w:t>`; OpenDocument
//! and XHTML put it directly inside the paragraph, so there the rule is "all
//! character data except what these elements hold". Paragraphs become lines,
//! because a person editing a document expects paragraphs to be lines and has
//! no other handle on them.

use std::fmt;

/// Which elements of one format carry text, and which start a line.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// The only elements whose character data is text. Empty means every
    /// element's character data counts, except inside `skipped`.
    pub carriers: &'static [&'static str],
    /// Elements whose content is never the document's text: a script, a style
    /// sheet, a comment body.
    pub skipped: &'static [&'static str],
    /// Elements that begin a paragraph, and therefore a line.
    pub paragraphs: &'static [&'static str],
}

/// Where one piece of the flat text came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Anchor {
    /// Byte range in the flat text this run contributes.
    pub text: (usize, usize),
    /// Byte range in the XML holding that run's escaped content.
    pub xml: (usize, usize),
}

/// The text of a document part, with the map back into its bytes.
#[derive(Clone, Debug)]
pub struct Part {
    xml: Vec<u8>,
    text: String,
    anchors: Vec<Anchor>,
}

/// Why a part could not become text, or text could not become a part again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PartError {
    /// The XML is not valid UTF-8, which this format requires.
    NotUtf8,
    /// An element opens and never closes.
    Unterminated { at: usize },
    /// There is nowhere to put this text: the document carries no run, so
    /// writing would mean inventing structure.
    NoRuns,
    /// The edited text has fewer lines than the document has paragraphs, or
    /// more. Adding or removing a paragraph is structure, and structure is not
    /// something this editor creates.
    ParagraphCountChanged { had: usize, now: usize },
}

impl fmt::Display for PartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 => formatter.write_str("this document part is not UTF-8"),
            Self::Unterminated { at } => {
                write!(formatter, "an element at byte {at} never closes")
            }
            Self::NoRuns => formatter.write_str("this document carries no editable text"),
            Self::ParagraphCountChanged { had, now } => write!(
                formatter,
                "this document has {had} paragraphs and the text has {now}; \
                 Grafita edits words, not structure"
            ),
        }
    }
}

impl std::error::Error for PartError {}

impl Part {
    /// Reads a document part into flat text and its anchors.
    pub fn parse(xml: Vec<u8>, rules: Rules) -> Result<Self, PartError> {
        let source = std::str::from_utf8(&xml).map_err(|_| PartError::NotUtf8)?;
        let mut text = String::new();
        let mut anchors = Vec::new();
        let mut cursor = 0;
        // How deep inside a skipped element the scan is, and inside a carrier.
        let mut skipping: Vec<&str> = Vec::new();
        let mut carrying: Vec<&str> = Vec::new();

        while let Some(found) = source[cursor..].find('<') {
            let open = cursor + found;
            // Character data is whatever lies between the previous tag and this
            // one. Only a carrier's data is text, and only when it is not
            // merely the whitespace a pretty-printer left behind.
            if open > cursor
                && skipping.is_empty()
                && (rules.carriers.is_empty() || !carrying.is_empty())
            {
                let data = &source[cursor..open];
                if !data.trim().is_empty() {
                    let start_of_text = text.len();
                    text.push_str(&unescape(data));
                    anchors.push(Anchor {
                        text: (start_of_text, text.len()),
                        xml: (cursor, open),
                    });
                }
            }

            let close = source[open..]
                .find('>')
                .map(|offset| open + offset)
                .ok_or(PartError::Unterminated { at: open })?;
            let tag = &source[open + 1..close];
            let closing = tag.starts_with('/');
            let empty = tag.ends_with('/');
            let name = tag
                .trim_start_matches('/')
                .split([' ', '/', '\t', '\n', '\r'])
                .next()
                .unwrap_or("");

            if !closing && !empty && rules.paragraphs.contains(&name) && !text.is_empty() {
                // The newline is the reader's, not the document's: no byte of
                // the XML says it, and writing back does not put one there.
                text.push('\n');
            }

            if !empty {
                if rules.skipped.contains(&name) {
                    if closing {
                        skipping.pop();
                    } else {
                        skipping.push(name);
                    }
                }
                if rules.carriers.contains(&name) {
                    if closing {
                        carrying.pop();
                    } else {
                        carrying.push(name);
                    }
                }
            }

            cursor = close + 1;
        }

        if anchors.is_empty() {
            return Err(PartError::NoRuns);
        }
        Ok(Self { xml, text, anchors })
    }

    /// The flat text an author edits.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where each run's text sits, for a host that wants to show it.
    #[must_use]
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Writes `text` back into the part, returning its new bytes.
    ///
    /// Only the runs change. Everything between them — styles, properties,
    /// bookmarks, the parts this crate does not understand — is copied as the
    /// bytes it was. Text inserted inside a run inherits that run's formatting,
    /// which is the whole formatting rule and a deliberate limit.
    pub fn write(&self, text: &str) -> Result<Vec<u8>, PartError> {
        let lines: Vec<&str> = text.split('\n').collect();
        let paragraphs = self.paragraphs();
        if lines.len() != paragraphs.len() {
            return Err(PartError::ParagraphCountChanged {
                had: paragraphs.len(),
                now: lines.len(),
            });
        }

        let mut out = Vec::with_capacity(self.xml.len());
        let mut written = 0;
        for (line, runs) in lines.iter().zip(paragraphs.iter()) {
            // A paragraph's text goes into its first run, and the rest are
            // emptied. Distributing it any other way would need to know which
            // half of an edit belonged to which style, and nothing says that.
            for (index, anchor) in runs.iter().enumerate() {
                out.extend_from_slice(&self.xml[written..anchor.xml.0]);
                if index == 0 {
                    out.extend_from_slice(escape(line).as_bytes());
                }
                written = anchor.xml.1;
            }
        }
        out.extend_from_slice(&self.xml[written..]);
        Ok(out)
    }

    /// The runs of each paragraph, in order. A paragraph break is a newline in
    /// the flat text, so the anchors split on exactly those.
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

/// The five entities XML defines. A numeric reference is left alone rather than
/// resolved: resolving it would change the bytes of a run nobody edited.
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
