//! language-contract: product-copy
//!
//! Display text: the words a person reads about an entry.
//!
//! ADR 0008 splits what crosses the Qt seam in two. The byte-exact identity of
//! a file is a path key (`crate::pathkey`); everything here is the other half —
//! the lossy conversion, with U+FFFD standing in for bytes no font can show.
//! It is published under its own property names, it is never an argument to
//! anything, and it never returns to Rust as a path.
//!
//! The marker above declares what this module is for: `kind_label` and
//! `row_subtitle` produce the Spanish a person reads. `kind_key`'s values are
//! the English tokens the QML maps to a glyph — a vocabulary, not prose — and
//! the comments and identifiers here stay English like everywhere else.

use std::path::Path;

use siderita_qt::{EntryRow, RowKind};

/// The final path component, for a compact per-entry line in a batch error.
/// Falls back to the full lossy path when there is no file name (e.g. `/`).
pub(crate) fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// The stable token a row's kind is published under. A vocabulary the QML maps
/// to a glyph, not prose, so it stays English on both sides of the seam.
pub(crate) const fn kind_key(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Directory => "directory",
        RowKind::File => "file",
        RowKind::Symlink => "symlink",
        RowKind::Other => "other",
    }
}

pub(crate) const fn kind_label(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Directory => "Carpeta",
        RowKind::File => "Archivo",
        RowKind::Symlink => "Enlace simbólico",
        RowKind::Other => "Otro",
    }
}

pub(crate) fn row_subtitle(row: &EntryRow) -> String {
    if row.kind() == RowKind::Directory {
        return "Carpeta".to_owned();
    }

    format!(
        "{} · {}",
        kind_label(row.kind()),
        crate::format::size(row.size())
    )
}

/// The containing folder of a search hit, shown as its subtitle so a result
/// carries where it lives (the one thing a flat folder row doesn't need).
pub(crate) fn search_hit_parent(path: &Path) -> String {
    path.parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn display_name_uses_the_final_component() {
        assert_eq!(
            super::display_name(Path::new("/home/toni/nota.txt")),
            "nota.txt"
        );
        assert_eq!(
            super::display_name(Path::new("/home/toni/carpeta")),
            "carpeta"
        );
        // No file name (root) falls back to the whole path.
        assert_eq!(super::display_name(Path::new("/")), "/");
    }

    #[test]
    fn a_non_utf8_name_is_shown_with_a_replacement_character() {
        let path = PathBuf::from(OsStr::from_bytes(b"/tmp/na\xffme"));
        assert_eq!(super::display_name(&path), "na\u{fffd}me");
    }
}
