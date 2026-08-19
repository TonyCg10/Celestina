#![forbid(unsafe_code)]

//! The picture a file carries inside itself.
//!
//! A folder of songs, of Windows programs or of Android packages shows the same
//! page over and over, while every one of those files already contains the image
//! a person would recognise it by: the album cover, the program's icon, the
//! app's launcher art. This crate reads that image out, and nothing else.
//!
//! It parses only as far as it must and never trusts what it reads: every offset
//! is bounds-checked against the bytes actually present, every length is capped,
//! and a malformed file answers `None` rather than panicking or allocating what
//! its headers claim. The formats here are exactly the ones people meet in a
//! file manager, not a general media library:
//!
//! - Windows executables (`.exe`, `.dll`), from their resource section;
//! - MP3, FLAC, MP4/M4A, Ogg — the cover art in their tags;
//! - Android packages and EPUB books, which are zip files with the art inside.
//!
//! What comes back is the image's own bytes, in whatever format the file stored
//! (PNG, JPEG, or an ICO assembled around a device-independent bitmap), for a
//! caller that already knows how to decode an image.

mod audio;
mod packaged;
mod pe;

use std::path::Path;

/// The largest image this crate will hand back. A cover or an icon is small;
/// anything claiming to be bigger is either not what it says or not worth
/// putting behind a 64-pixel tile.
const MAX_IMAGE: usize = 8 * 1024 * 1024;

/// The image `path` carries inside it, if it carries one.
///
/// Chosen by extension rather than by sniffing, because the parsers below are
/// format-specific and running all of them over every file in a folder would
/// cost far more than the picture is worth.
pub fn embedded_image(path: &Path) -> Option<Vec<u8>> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "exe" | "dll" | "scr" | "mun" | "cpl" => pe::icon(path),
        "mp3" => audio::id3_picture(path),
        "flac" => audio::flac_picture(path),
        "m4a" | "mp4" | "m4v" | "m4b" | "aac" => audio::mp4_cover(path),
        "ogg" | "oga" | "opus" => audio::ogg_picture(path),
        "apk" => packaged::android_icon(path),
        "epub" => packaged::epub_cover(path),
        _ => None,
    }
}

/// Whether this crate would even look inside a file with this name.
///
/// A host asks first so it can decide whether to spend a worker on the file at
/// all; answering from the name alone costs nothing.
pub fn may_carry_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some((_stem, extension)) = lower.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension,
        "exe"
            | "dll"
            | "scr"
            | "mun"
            | "cpl"
            | "mp3"
            | "flac"
            | "m4a"
            | "mp4"
            | "m4v"
            | "m4b"
            | "aac"
            | "ogg"
            | "oga"
            | "opus"
            | "apk"
            | "epub"
    )
}

/// Reads `len` bytes at `at`, or `None` when the slice does not hold them.
///
/// Every parser here goes through this: a file that says its cover is four
/// gigabytes long, or that it starts past the end, is a file this crate simply
/// does not have an image for.
pub(crate) fn slice_at(bytes: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    if len > MAX_IMAGE {
        return None;
    }
    bytes.get(at..at.checked_add(len)?)
}

pub(crate) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(slice_at(bytes, at, 2)?.try_into().ok()?))
}

pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(slice_at(bytes, at, 4)?.try_into().ok()?))
}

pub(crate) fn u32_be_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(slice_at(bytes, at, 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::{may_carry_image, slice_at};

    #[test]
    fn only_the_formats_that_can_carry_one_are_opened() {
        assert!(may_carry_image("juego.exe"));
        // Upper case and an accented name still name an MP3.
        assert!(may_carry_image("CANCION.MP3"));
        assert!(may_carry_image("app.apk"));
        assert!(!may_carry_image("notas.txt"));
        assert!(!may_carry_image("sin-extension"));
        assert!(!may_carry_image(""));
    }

    #[test]
    fn a_slice_past_the_end_is_not_a_slice() {
        let bytes = [1u8, 2, 3, 4];
        assert_eq!(slice_at(&bytes, 0, 4), Some(&bytes[..]));
        assert_eq!(slice_at(&bytes, 2, 4), None);
        assert_eq!(slice_at(&bytes, 0, usize::MAX), None);
        // A length no honest cover would have is refused before it is read.
        assert_eq!(slice_at(&bytes, 0, 64 * 1024 * 1024), None);
    }
}
