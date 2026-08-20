//! Changing a PDF by appending to it.
//!
//! The format has its own answer to the imported contract: an incremental
//! update. Replaced objects are written after the existing file, followed by a
//! cross-reference naming them and pointing back at the previous one. Every
//! byte that was there before is still there, at the same offset, which is a
//! stronger promise than "nothing you did not edit changed" — it is the same
//! file with an appendix.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::file::Pdf;
use super::object::{Dictionary, Object, PdfError};

/// Appends `replacements` to `pdf` as an incremental update.
///
/// Each replacement is a whole object body, written as the object number it
/// replaces. The trailer keeps whatever the file already said and only its
/// size and previous-offset change.
pub fn append(pdf: &Pdf, replacements: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, PdfError> {
    if replacements.is_empty() {
        return Ok(pdf.bytes().to_vec());
    }
    let mut out = pdf.bytes().to_vec();
    // A file that does not end in a newline would run its first appended
    // object into the previous `%%EOF`.
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }

    let mut written: Vec<(u32, usize)> = Vec::with_capacity(replacements.len());
    for (number, body) in replacements {
        written.push((*number, out.len()));
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    written.sort_unstable();

    let previous = pdf.start_xref();
    let size = pdf
        .last_object()
        .max(written.last().map_or(0, |(number, _)| *number))
        + 1;
    let xref_offset = out.len();

    // The kind of cross-reference matches the file's own. Appending a table to
    // a file whose sections are streams is a shape some readers reject, and the
    // point of an update is that every reader still opens it.
    if pdf.uses_xref_streams() {
        write_xref_stream(&mut out, &written, size, previous, xref_offset, pdf)?;
    } else {
        write_xref_table(&mut out, &written, size, previous, pdf);
    }
    out.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());
    Ok(out)
}

fn write_xref_table(
    out: &mut Vec<u8>,
    written: &[(u32, usize)],
    size: u32,
    previous: usize,
    pdf: &Pdf,
) {
    out.extend_from_slice(b"xref\n");
    // Consecutive object numbers share one subsection header.
    let mut index = 0;
    while index < written.len() {
        let mut end = index + 1;
        while end < written.len() && written[end].0 == written[end - 1].0 + 1 {
            end += 1;
        }
        out.extend_from_slice(format!("{} {}\n", written[index].0, end - index).as_bytes());
        for (_number, offset) in &written[index..end] {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        index = end;
    }
    out.extend_from_slice(b"trailer\n");
    out.extend_from_slice(trailer(pdf, size, previous).as_bytes());
    out.push(b'\n');
}

fn write_xref_stream(
    out: &mut Vec<u8>,
    written: &[(u32, usize)],
    size: u32,
    previous: usize,
    xref_offset: usize,
    pdf: &Pdf,
) -> Result<(), PdfError> {
    // The stream is itself an object, and it needs a number of its own.
    let number = size;
    let mut index = Vec::new();
    let mut rows: Vec<u8> = Vec::new();
    let mut entries: Vec<(u32, usize)> = written.to_vec();
    entries.push((number, xref_offset));
    entries.sort_unstable();

    let mut cursor = 0;
    while cursor < entries.len() {
        let mut end = cursor + 1;
        while end < entries.len() && entries[end].0 == entries[end - 1].0 + 1 {
            end += 1;
        }
        index.push((entries[cursor].0, (end - cursor) as u32));
        for (_number, offset) in &entries[cursor..end] {
            rows.push(1);
            rows.extend_from_slice(&(*offset as u32).to_be_bytes());
            rows.extend_from_slice(&0u16.to_be_bytes());
        }
        cursor = end;
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&rows).map_err(|_| PdfError::Malformed {
        detail: "the new cross-reference could not be compressed".to_owned(),
    })?;
    let data = encoder.finish().map_err(|_| PdfError::Malformed {
        detail: "the new cross-reference could not be compressed".to_owned(),
    })?;

    let index_text = index
        .iter()
        .map(|(first, count)| format!("{first} {count}"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut dictionary = trailer_entries(pdf);
    dictionary.push(format!("/Size {}", number + 1));
    dictionary.push(format!("/Prev {previous}"));
    dictionary.push("/Type /XRef".to_owned());
    dictionary.push("/W [1 4 2]".to_owned());
    dictionary.push(format!("/Index [{index_text}]"));
    dictionary.push("/Filter /FlateDecode".to_owned());
    dictionary.push(format!("/Length {}", data.len()));

    out.extend_from_slice(
        format!("{number} 0 obj\n<< {} >>\nstream\n", dictionary.join(" ")).as_bytes(),
    );
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\nendstream\nendobj\n");
    Ok(())
}

fn trailer(pdf: &Pdf, size: u32, previous: usize) -> String {
    let mut entries = trailer_entries(pdf);
    entries.push(format!("/Size {size}"));
    entries.push(format!("/Prev {previous}"));
    format!("<< {} >>", entries.join(" "))
}

/// The trailer keys an update must carry over, written back as they were.
fn trailer_entries(pdf: &Pdf) -> Vec<String> {
    let mut entries = Vec::new();
    for key in ["Root", "Info", "ID"] {
        if let Some(value) = pdf.trailer().get(key) {
            entries.push(format!("/{key} {}", write(value)));
        }
    }
    entries
}

/// Writes an object back out in the syntax a PDF uses.
pub fn write(object: &Object) -> String {
    match object {
        Object::Null => "null".to_owned(),
        Object::Boolean(value) => value.to_string(),
        Object::Number(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        Object::Name(name) => format!("/{name}"),
        Object::String(bytes) => {
            let mut out = String::from("<");
            for byte in bytes {
                out.push_str(&format!("{byte:02X}"));
            }
            out.push('>');
            out
        }
        Object::Array(items) => {
            let inside = items.iter().map(write).collect::<Vec<_>>().join(" ");
            format!("[{inside}]")
        }
        Object::Dictionary(dictionary) | Object::Stream { dictionary, .. } => {
            format!("<< {} >>", dictionary_entries(dictionary))
        }
        Object::Reference { number, generation } => format!("{number} {generation} R"),
    }
}

fn dictionary_entries(dictionary: &Dictionary) -> String {
    dictionary
        .iter()
        .map(|(key, value)| format!("/{key} {}", write(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds the body of a replaced stream object: its dictionary with the new
/// length and filter, then the data.
#[must_use]
pub fn stream_object(dictionary: &Dictionary, content: &[u8]) -> Vec<u8> {
    let mut entries: Dictionary = dictionary.clone();
    // The new content is written compressed, which is what every writer does
    // and what keeps a page-sized stream from doubling the file.
    entries.insert("Filter".to_owned(), Object::Name("FlateDecode".to_owned()));
    entries.remove("DecodeParms");
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let data = match encoder.write_all(content).and_then(|()| encoder.finish()) {
        Ok(data) => data,
        Err(_) => {
            entries.remove("Filter");
            content.to_vec()
        }
    };
    entries.insert("Length".to_owned(), Object::Number(data.len() as f64));

    let mut out = format!("<< {} >>\nstream\n", dictionary_entries(&entries)).into_bytes();
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\nendstream");
    out
}
