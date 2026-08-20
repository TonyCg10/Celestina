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

use super::file::Pdf;
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
pub fn fields(pdf: &Pdf) -> Result<Vec<Field>, PdfError> {
    let Some(form) = acroform(pdf)? else {
        return Ok(Vec::new());
    };
    let roots = pdf.entry(&form, "Fields")?;
    let mut found = Vec::new();
    for reference in roots.as_array().unwrap_or(&[]) {
        walk(pdf, reference, "", &mut found, 0)?;
    }
    Ok(found)
}

fn walk(
    pdf: &Pdf,
    reference: &Object,
    prefix: &str,
    found: &mut Vec<Field>,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > 32 {
        return Ok(());
    }
    let Some(object) = reference.as_reference() else {
        return Ok(());
    };
    let resolved = pdf.object(object)?;
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
    let children = kids.as_array().unwrap_or(&[]).to_vec();
    // A node with named children is a group; a node with widget children is
    // still one field, drawn in several places.
    let named_children = children.iter().any(|kid| {
        kid.as_reference()
            .and_then(|number| pdf.object(number).ok())
            .and_then(|object| object.as_dictionary().cloned())
            .is_some_and(|child| child.contains_key("T"))
    });
    if named_children {
        for kid in &children {
            walk(pdf, kid, &name, found, depth + 1)?;
        }
        return Ok(());
    }

    // Only a node that has a field type is a field; the rest are structure.
    if dictionary.contains_key("FT") || dictionary.contains_key("V") {
        found.push(Field {
            object,
            name,
            value: value_of(pdf, dictionary)?,
        });
    }
    Ok(())
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
