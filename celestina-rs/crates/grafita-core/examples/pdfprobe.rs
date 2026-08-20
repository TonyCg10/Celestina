//! A bench for reading and correcting real PDFs, used while G12 was written.

use grafita_core::import::pdf::{edit, file::Pdf, text};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().unwrap_or_default();
    for path in arguments {
        let bytes = std::fs::read(&path).expect("read");
        let size = bytes.len();
        let pdf = match Pdf::parse(bytes) {
            Ok(pdf) => pdf,
            Err(error) => {
                println!("{path} ({size} B): REFUSED {error}");
                continue;
            }
        };
        let extraction = match text::extract(&pdf) {
            Ok(extraction) => extraction,
            Err(error) => {
                println!("{path} ({size} B): parsed, no text: {error}");
                continue;
            }
        };
        if mode == "check" {
            // The invariant an edit depends on: the bytes a placement points at
            // must be the string that produced its text.
            let mut wrong = 0;
            let mut checked = 0;
            let mut first_wrong = String::new();
            let mut streams: std::collections::BTreeMap<u32, Vec<u8>> = Default::default();
            for placement in &extraction.placements {
                let content = streams.entry(placement.stream).or_insert_with(|| {
                    pdf.stream_data(&pdf.object(placement.stream).unwrap())
                        .unwrap_or_default()
                });
                let slice = content
                    .get(placement.span.0..placement.span.1)
                    .unwrap_or_default();
                let said = &extraction.text[placement.text.0..placement.text.1];
                checked += 1;
                let looks_like_string = slice.first().is_some_and(|b| *b == b'(' || *b == b'<');
                if !looks_like_string {
                    wrong += 1;
                    if first_wrong.is_empty() {
                        first_wrong = format!(
                            "said {said:?} but span holds {:?}",
                            String::from_utf8_lossy(&slice[..slice.len().min(30)])
                        );
                    }
                }
            }
            println!("{path}: {checked} placements, {wrong} with a span that is not a string | {first_wrong}");
            continue;
        }
        if mode == "read" {
            let head: String = extraction
                .text
                .replace('\n', " ⏎ ")
                .chars()
                .take(80)
                .collect();
            println!(
                "{path} ({size} B): {} chars, {} placements | {head}",
                extraction.text.chars().count(),
                extraction.placements.len()
            );
            continue;
        }

        // Correct the first word that is long enough to be worth correcting.
        let Some(word) = extraction
            .text
            .split_whitespace()
            .find(|word| word.len() > 5 && word.chars().all(|c| c.is_ascii_alphabetic()))
            .map(str::to_owned)
        else {
            println!("{path}: no plain word to correct");
            continue;
        };
        let corrected = extraction.text.replacen(&word, "REDACTED", 1);
        match edit::apply(&pdf, &extraction, &corrected) {
            Err(error) => println!("{path}: edit refused: {error}"),
            Ok(written) => {
                if let Ok(out) = std::env::var("WRITE_TO") {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    std::fs::write(std::path::Path::new(&out).join(name), &written).expect("write");
                }
                let grew = written.len() - size;
                let prefix_intact = written.starts_with(pdf.bytes());
                let reread = Pdf::parse(written).ok().and_then(|pdf| {
                    text::extract(&pdf)
                        .ok()
                        .map(|again| (again.text.contains("REDACTED"), again.text.contains(&word)))
                });
                println!(
                    "{path}: '{word}' → REDACTED | +{grew} B, prefix intact {prefix_intact}, reread {reread:?}"
                );
            }
        }
    }
}
