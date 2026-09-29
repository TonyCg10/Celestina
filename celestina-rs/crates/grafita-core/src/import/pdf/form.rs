//! The fields a PDF form carries.
//!
//! This is the part of "editing a PDF" that the format defines outright. A
//! field has a name and a value, and changing the value is changing one entry
//! of one dictionary — no fonts, no glyph coverage, no layout. It is also the
//! part people actually need: a form is made to be filled.
//!
//! What a viewer draws inside the field is a separate appearance stream, which
//! this crate does not write. Instead the document is asked to have them
//! rebuilt, which is what `NeedAppearances` is for and what every viewer
//! honours; otherwise a filled field would show its old text until something
//! else redrew it.

use std::collections::{BTreeMap, BTreeSet};

use celestina_core::CancellationToken;

use super::file::{ObjectKey, Pdf};
use super::object::{Dictionary, Object, PdfError};
use super::update;

/// One field of a form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    /// The object holding it, which is what a change rewrites.
    pub object: u32,
    /// Its full name, with the names of the fields it sits under.
    pub name: String,
    /// Its value as text. A checkbox reads as the name of its state.
    pub value: String,
}

/// Every field of the document's form, in the order it declares them.
///
/// One walk under [`Pdf::walk_budget`], stopped by `cancellation`.
pub fn fields(pdf: &Pdf, cancellation: &CancellationToken) -> Result<Vec<Field>, PdfError> {
    pdf.within(pdf.walk_budget(), cancellation, || read_fields(pdf))
}

fn read_fields(pdf: &Pdf) -> Result<Vec<Field>, PdfError> {
    let Some(form) = acroform(pdf)? else {
        return Ok(Vec::new());
    };
    let roots = pdf.entry(&form, "Fields")?;
    let mut walker = Walker {
        pdf,
        visited: BTreeSet::new(),
        facts: BTreeMap::new(),
        found: Vec::new(),
    };
    // Roots are told apart by the bytes they read before anything is read,
    // and nothing read is kept: each is read here for its facts and again
    // when its turn in the walk comes.
    let mut seen = BTreeSet::new();
    let mut tops = Vec::new();
    let mut children = Vec::new();
    for reference in roots.as_array().unwrap_or(&[]) {
        let Some(number) = reference.as_reference() else {
            continue;
        };
        let Some(key) = pdf.key(number)? else {
            continue;
        };
        if !seen.insert(key) {
            continue;
        }
        // A field is named by its path from the top. A writer that lists a
        // child in `/Fields` beside its parent would otherwise have it walked
        // first, and named without the parent's part, depending only on the
        // order; so the fields that have a parent are walked after every true
        // root, and only if their parent did not already reach them.
        match walker.facts(number)? {
            Some(facts) if facts.has_parent => children.push(number),
            _ => tops.push(number),
        }
    }
    for number in tops.into_iter().chain(children) {
        walker.walk(number, "", 0)?;
    }
    Ok(walker.found)
}

/// What the walk needs to know about a field before walking it.
#[derive(Clone, Copy, Debug)]
struct Facts {
    /// It has a name of its own, which makes the node above it a group.
    named: bool,
    /// It names a parent.
    has_parent: bool,
}

/// One walk over the field tree.
///
/// Like the page tree, the field tree gives each node one parent. A node met
/// a second time, under the same number or another one pointing at the same
/// bytes, is skipped; and what a node is (named, parented) is read once per
/// object however many parents list it, so a kid shared by thousands of
/// fields is read twice in all, not once per parent.
struct Walker<'a> {
    pdf: &'a Pdf,
    visited: BTreeSet<ObjectKey>,
    facts: BTreeMap<ObjectKey, Facts>,
    found: Vec<Field>,
}

impl Walker<'_> {
    /// The facts of object `number`, read once per object. `None` for an
    /// object the file does not locate, which reads as null.
    fn facts(&mut self, number: u32) -> Result<Option<Facts>, PdfError> {
        let Some(key) = self.pdf.key(number)? else {
            return Ok(None);
        };
        if let Some(facts) = self.facts.get(&key) {
            return Ok(Some(*facts));
        }
        let object = self.pdf.object(number)?;
        let dictionary = object.as_dictionary();
        let facts = Facts {
            named: dictionary.is_some_and(|field| field.contains_key("T")),
            has_parent: dictionary.is_some_and(|field| field.contains_key("Parent")),
        };
        self.facts.insert(key, facts);
        Ok(Some(facts))
    }

    /// Collects the fields under object `number`, reading it when its turn
    /// comes and not keeping it after.
    fn walk(&mut self, number: u32, prefix: &str, depth: usize) -> Result<(), PdfError> {
        if depth > 32 {
            return Ok(());
        }
        let Some(key) = self.pdf.key(number)? else {
            return Ok(());
        };
        if !self.visited.insert(key) {
            return Ok(());
        }
        let pdf = self.pdf;
        let resolved = pdf.object(number)?;
        let Some(dictionary) = resolved.as_dictionary() else {
            return Ok(());
        };
        let own = match pdf.entry(dictionary, "T")? {
            Object::String(bytes) => text_of(&bytes),
            _ => String::new(),
        };
        let name = if prefix.is_empty() {
            own.clone()
        } else if own.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}.{own}")
        };

        let kids = pdf.entry(dictionary, "Kids")?;
        let kids: Vec<u32> = kids
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(Object::as_reference)
            .collect();
        // A node with named children is a group; a node with widget children
        // is still one field, drawn in several places.
        let mut named_children = false;
        for kid in &kids {
            if self.facts(*kid)?.is_some_and(|facts| facts.named) {
                named_children = true;
                break;
            }
        }
        if named_children {
            for kid in kids {
                self.walk(kid, &name, depth + 1)?;
            }
            return Ok(());
        }

        // Only a node that has a field type is a field; the rest are
        // structure.
        if dictionary.contains_key("FT") || dictionary.contains_key("V") {
            self.found.push(Field {
                object: number,
                name,
                value: value_of(pdf, dictionary)?,
            });
        }
        Ok(())
    }
}

fn value_of(pdf: &Pdf, dictionary: &Dictionary) -> Result<String, PdfError> {
    Ok(match pdf.entry(dictionary, "V")? {
        Object::String(bytes) => text_of(&bytes),
        Object::Name(name) => name,
        Object::Number(value) => {
            if value.fract() == 0.0 {
                format!("{}", value as i64)
            } else {
                value.to_string()
            }
        }
        Object::Boolean(value) => value.to_string(),
        _ => String::new(),
    })
}

/// A PDF text string: UTF-16 when it carries the mark, otherwise the Latin
/// encoding the format calls `PDFDocEncoding`, which agrees with Latin-1 over
/// everything a form is likely to hold.
fn text_of(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

/// The objects a set of field changes rewrites.
///
/// Each field keeps every entry it had; only its value changes, and the form
/// is asked to have its appearances rebuilt.
pub fn replacements(pdf: &Pdf, changes: &[(u32, String)]) -> Result<Vec<(u32, Vec<u8>)>, PdfError> {
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(changes.len() + 1);
    for (object, value) in changes {
        let resolved = pdf.object(*object)?;
        let Some(dictionary) = resolved.as_dictionary() else {
            return Err(PdfError::Malformed {
                detail: format!("field {object} is not a dictionary"),
            });
        };
        let mut updated = dictionary.clone();
        let written = Object::String(write_text(value));
        // A button's value is a name, not a string, and its appearance state
        // must follow it or the tick stays where it was.
        if pdf.entry(dictionary, "FT")?.as_name() == Some("Btn") {
            updated.insert("V".to_owned(), Object::Name(value.clone()));
            updated.insert("AS".to_owned(), Object::Name(value.clone()));
        } else {
            updated.insert("V".to_owned(), written);
        }
        out.push((
            *object,
            update::write(&Object::Dictionary(updated)).into_bytes(),
        ));
    }

    // The form itself, so a viewer draws what was just written.
    if let Some((number, form)) = acroform_object(pdf)? {
        let mut updated = form;
        updated.insert("NeedAppearances".to_owned(), Object::Boolean(true));
        out.push((
            number,
            update::write(&Object::Dictionary(updated)).into_bytes(),
        ));
    }
    Ok(out)
}

/// A PDF text string, written the way a form expects to read it back.
fn write_text(value: &str) -> Vec<u8> {
    if value.is_ascii() {
        return value.as_bytes().to_vec();
    }
    let mut out = vec![0xFE, 0xFF];
    for unit in value.encode_utf16() {
        out.extend_from_slice(&unit.to_be_bytes());
    }
    out
}

fn acroform(pdf: &Pdf) -> Result<Option<Dictionary>, PdfError> {
    Ok(acroform_object(pdf)?.map(|(_number, form)| form))
}

/// The form dictionary and the object it lives in, when it lives in one of its
/// own. A form written straight into the catalogue cannot be rewritten without
/// rewriting the catalogue, which is why the number is carried here.
fn acroform_object(pdf: &Pdf) -> Result<Option<(u32, Dictionary)>, PdfError> {
    let root = pdf.entry(pdf.trailer(), "Root")?;
    let Some(catalogue) = root.as_dictionary() else {
        return Ok(None);
    };
    let Some(reference) = catalogue.get("AcroForm") else {
        return Ok(None);
    };
    let number = match reference.as_reference() {
        Some(number) => number,
        // Written inline: there is nothing to replace on its own, so the
        // appearances cannot be asked for. The fields still change.
        None => return Ok(None),
    };
    let form = pdf.object(number)?;
    Ok(form
        .as_dictionary()
        .cloned()
        .map(|dictionary| (number, dictionary)))
}
