//! The art inside a package that is really a zip file.
//!
//! An Android package keeps its launcher icon under `res/`, and an EPUB keeps
//! its cover among its resources. Neither needs the format understood: the
//! largest plausible image under the right prefix is the one a person would
//! recognise, and picking by size beats parsing a manifest to find a name that
//! then has to be looked up anyway.

use std::path::Path;

/// The launcher icon of an Android package.
pub(crate) fn android_icon(path: &Path) -> Option<Vec<u8>> {
    largest_image(
        path,
        &["res/mipmap", "res/drawable"],
        &["ic_launcher", "icon"],
    )
}

/// The cover of an EPUB book.
pub(crate) fn epub_cover(path: &Path) -> Option<Vec<u8>> {
    largest_image(path, &[""], &["cover", "portada"])
}

/// The largest image whose name sits under one of `prefixes` and mentions one
/// of `hints`, falling back to the largest image anywhere in the package.
fn largest_image(path: &Path, prefixes: &[&str], hints: &[&str]) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;

    let mut best: Option<(bool, u64, usize)> = None;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).ok()?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_ascii_lowercase();
        if !(name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg")) {
            continue;
        }
        if !prefixes.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        let hinted = hints.iter().any(|hint| name.contains(hint));
        let size = entry.size();
        // A hinted name always beats an unhinted one, and among equals the
        // biggest wins: that is the one drawn at a readable size.
        let better = match best {
            None => true,
            Some((best_hinted, best_size, _)) => (hinted, size) > (best_hinted, best_size),
        };
        if better {
            best = Some((hinted, size, index));
        }
    }

    let (_hinted, size, index) = best?;
    if size > crate::MAX_IMAGE as u64 {
        return None;
    }
    let mut entry = zip.by_index(index).ok()?;
    let mut out = Vec::with_capacity(usize::try_from(size).ok()?);
    std::io::Read::read_to_end(&mut entry, &mut out).ok()?;
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    /// A package whose largest hinted image is the one that comes back.
    #[test]
    fn the_launcher_icon_is_read_out_of_an_android_package() {
        let dir = std::env::temp_dir().join(format!("siderita-apk-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("app.apk");

        let file = std::fs::File::create(&path).expect("create");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("res/mipmap-hdpi/ic_launcher.png", options)
            .expect("start");
        zip.write_all(&[1u8; 64]).expect("write");
        zip.start_file("res/drawable/fondo.png", options)
            .expect("start");
        zip.write_all(&[2u8; 4096]).expect("write");
        zip.finish().expect("finish");

        // The bigger file is not the launcher: the hinted name wins anyway.
        assert_eq!(super::android_icon(&path), Some(vec![1u8; 64]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_package_without_images_has_no_icon() {
        let dir = std::env::temp_dir().join(format!("siderita-apk-empty-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("vacio.apk");
        let file = std::fs::File::create(&path).expect("create");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("classes.dex", zip::write::SimpleFileOptions::default())
            .expect("start");
        zip.write_all(b"nada").expect("write");
        zip.finish().expect("finish");
        assert_eq!(super::android_icon(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
