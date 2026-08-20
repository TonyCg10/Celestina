//! What the imported document rests on: a container that gives back the file
//! it was handed, not an equivalent one.

use std::io::Write;

use grafita_core::container::{Container, ContainerError};

const DOCUMENT: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:body><w:p><w:r><w:t>Report</w:t></w:r></w:p></w:body></w:document>"#
);

const CONTENT_TYPES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
    r#"<Default Extension="xml" ContentType="application/xml"/></Types>"#
);

/// A container written by somebody else's writer, which is the point: parsing
/// what this crate produced would prove nothing about a real `.docx`.
fn sample_docx() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    writer
        .start_file("[Content_Types].xml", deflated)
        .expect("start");
    writer.write_all(CONTENT_TYPES.as_bytes()).expect("write");
    // Stored on purpose: a container may hold both, and a rewrite must keep
    // each member the way it found it.
    writer.start_file("_rels/.rels", stored).expect("start");
    writer.write_all(b"<Relationships/>").expect("write");
    writer
        .start_file("word/document.xml", deflated)
        .expect("start");
    writer.write_all(DOCUMENT.as_bytes()).expect("write");
    writer.finish().expect("finish").into_inner()
}

#[test]
fn a_container_gives_back_the_parts_it_was_given() {
    let bytes = sample_docx();
    let container = Container::parse(bytes).expect("a zip container");

    assert_eq!(
        container.names(),
        vec!["[Content_Types].xml", "_rels/.rels", "word/document.xml"]
    );
    assert_eq!(
        container.read("word/document.xml"),
        Ok(DOCUMENT.as_bytes().to_vec())
    );
    assert_eq!(
        container.read("_rels/.rels"),
        Ok(b"<Relationships/>".to_vec())
    );
    assert_eq!(
        container.read("word/settings.xml"),
        Err(ContainerError::NoSuchMember {
            name: "word/settings.xml".to_owned()
        })
    );
}

#[test]
fn rewriting_nothing_reproduces_the_file_byte_for_byte() {
    // The whole imported contract in one assertion. If this ever fails, no
    // container document may be saved, because "everything you did not edit is
    // untouched" would be a claim rather than a fact.
    let bytes = sample_docx();
    let container = Container::parse(bytes.clone()).expect("a zip container");

    assert_eq!(container.rewrite(&[]), Ok(bytes));
}

#[test]
fn replacing_one_part_leaves_every_other_part_exactly_as_it_was() {
    let bytes = sample_docx();
    let original = Container::parse(bytes.clone()).expect("a zip container");
    let edited = DOCUMENT.replace("Report", "Summary");

    let rewritten = original
        .rewrite(&[("word/document.xml", edited.as_bytes().to_vec())])
        .expect("the rewrite");
    let container = Container::parse(rewritten.clone()).expect("still a container");

    assert_eq!(container.names(), original.names());
    assert_eq!(container.read("word/document.xml"), Ok(edited.into_bytes()));
    // Untouched members come back identical, and so does the compression they
    // were stored with: the stored one was not deflated on the way out.
    assert_eq!(
        container.read("[Content_Types].xml"),
        Ok(CONTENT_TYPES.as_bytes().to_vec())
    );
    assert_eq!(
        container.read("_rels/.rels"),
        Ok(b"<Relationships/>".to_vec())
    );

    // And the file a word processor would open is a file its own library
    // accepts, not merely one this crate can read back.
    let mut reader =
        zip::ZipArchive::new(std::io::Cursor::new(rewritten)).expect("a readable archive");
    assert_eq!(reader.len(), 3);
    let member = reader.by_name("_rels/.rels").expect("the untouched member");
    assert_eq!(member.compression(), zip::CompressionMethod::Stored);
}

#[test]
fn what_is_not_a_container_this_crate_edits_is_refused_by_name() {
    assert_eq!(
        Container::parse(b"no soy un zip".to_vec()).err(),
        Some(ContainerError::NotAnArchive)
    );
    assert_eq!(
        Container::parse(Vec::new()).err(),
        Some(ContainerError::NotAnArchive)
    );

    // A member whose bytes disagree with the checksum its own directory
    // carries: reading it would hand the author something nobody wrote.
    let mut bytes = sample_docx();
    let record = bytes
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .expect("a central directory record");
    bytes[record + 16] ^= 0xFF;
    let container = Container::parse(bytes).expect("the structure still parses");
    assert!(matches!(
        container.read("[Content_Types].xml"),
        Err(ContainerError::Corrupt { .. })
    ));
}
