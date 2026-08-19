//! language-contract: product-copy
//!
//! Which glyph an entry wears, decided by what its name says it is.
//!
//! Every non-folder used to arrive at the same generic page: a `.zip`, a `.rs`
//! and a font all looked alike, so the view carried no information the name did
//! not already carry. The table below is the cheap half of fixing that — a file
//! that carries its own picture is a separate question, answered by the
//! thumbnail provider.
//!
//! It lives in Rust rather than in the delegates because three of them ask
//! (list, grid and picker) and a rule copied three times is a rule that drifts.
//! The names it answers are catalogue names, not paths, so a missing entry
//! degrades to the generic page instead of to nothing.

use cxx_qt_lib::QString;

use super::qobject;

impl qobject::SideritaController {
    /// The catalogue icon for an entry's name, by extension.
    pub fn glyph_for_name(&self, name: &QString) -> QString {
        QString::from(icon_for_extension(&extension_of(&name.to_string())))
    }

    /// The accent an entry's name earns, as one of the theme's sealed icon
    /// accent keys — or empty for the entries that take the plain tone.
    ///
    /// This is the other half of telling files apart. The icon family draws a
    /// page for some languages and not for others, and borrowing a glyph from a
    /// second family to fill the gap would put a foreign shape in a set that
    /// reads as one. Colour carries the difference instead: same page, own
    /// tint, and the tints are the six the theme already seals.
    pub fn glyph_accent_for_name(&self, name: &QString) -> QString {
        QString::from(accent_for_extension(&extension_of(&name.to_string())))
    }

    /// The image an entry carries as its own icon, as a `file://` URL, or an
    /// empty string when it carries none. Answered from the key so the bytes of
    /// the path survive the round trip through QML.
    pub fn own_icon_url(&self, key: &QString) -> QString {
        let Ok(path) = crate::pathkey::decode(key) else {
            return QString::default();
        };
        // A launcher names a file on disk; a program, a song or a package holds
        // the picture inside itself, and that one goes through the thumbnail
        // provider — which caches it, decodes it off this thread, and already
        // knows how to reach the file from a key.
        if let Some(icon) = crate::ownicon::own_icon(&path) {
            return QString::from(format!("file://{}", icon.display()).as_str());
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if siderita_embedded::may_carry_image(&name) {
            return QString::from(format!("image://thumb/{}", key).as_str());
        }
        QString::default()
    }
}

/// The lowercase extension of a file name, without the dot.
///
/// A compound extension is read from its last part (`archivo.tar.gz` is `gz`)
/// except where the pair means something the last part alone does not, which is
/// why `tar.gz` is checked before it.
fn extension_of(name: &str) -> String {
    let lower = name.to_lowercase();
    for compound in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"] {
        if lower.ends_with(compound) {
            return "tar.gz".to_owned();
        }
    }
    lower
        .rsplit_once('.')
        .map(|(_stem, extension)| extension.to_owned())
        .unwrap_or_default()
}

/// The catalogue name for an extension, or the generic page for one this table
/// says nothing about.
fn icon_for_extension(extension: &str) -> &'static str {
    match extension {
        // Text a person reads.
        "txt" | "md" | "markdown" | "rst" | "org" | "log" | "nfo" | "readme" | "tex" => "file-text",
        // Documents, each with its own page now that the family has one.
        "pdf" => "file-pdf",
        "doc" | "docx" | "odt" | "rtf" => "file-doc",
        "xls" | "xlsx" | "ods" => "file-xls",
        "ppt" | "pptx" | "odp" => "file-ppt",
        "csv" | "tsv" => "file-csv",
        "epub" | "mobi" | "djvu" => "file-text",
        // Languages the icon family draws a page for: one glyph each, from the
        // same family and the same grid.
        "rs" => "file-rs",
        "js" | "mjs" | "cjs" => "file-js",
        "ts" | "mts" | "cts" => "file-ts",
        "jsx" => "file-jsx",
        "tsx" => "file-tsx",
        "vue" => "file-vue",
        "html" | "htm" => "file-html",
        "css" | "scss" | "sass" | "less" => "file-css",
        "sql" => "file-sql",
        "svg" => "file-svg",
        // Everything else that is code keeps the generic page and is told apart
        // by its tint (see `tint_for_extension`): the family has no page for
        // Python, C, Go or Java, and borrowing one from another family would
        // put a foreign glyph in a set that reads as one.
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "go" | "java" | "kt" | "swift" | "py" | "rb"
        | "php" | "pl" | "lua" | "svelte" | "cs" | "scala" | "hs" | "ml" | "ex" | "exs"
        | "dart" | "zig" | "nim" | "qml" | "xml" => "file-code",
        // Shells and build files: the things that run other things.
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" | "mk" | "cmake" | "ninja"
        | "just" | "makefile" | "dockerfile" => "terminal",
        // Structured data.
        "json" | "yaml" | "yml" | "toml" | "ini" | "conf" | "cfg" | "kdl" | "plist" | "lock" => {
            "file-braces"
        }
        // Pictures, moving pictures, sound.
        "png" => "file-png",
        "jpg" | "jpeg" => "file-jpg",
        "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "avif" | "jxl" | "heic" | "heif"
        | "raw" | "psd" | "xcf" => "file-image",
        // `ts` is absent on purpose: in a folder a person browses it is
        // TypeScript far more often than a transport stream, and the view's
        // media rule claims the video case before this table is asked.
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" | "mpg" | "mpeg" | "wmv" | "flv" | "3gp"
        | "ogv" | "m2ts" => "file-video-camera",
        "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wav" | "wma" | "aif"
        | "aiff" | "mka" | "mid" | "midi" => "file-music",
        // Containers.
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4" | "lzma" | "cab"
        | "tar.gz" | "jar" | "war" | "apk" | "deb" | "rpm" | "pkg" | "xz2" => "file-archive",
        // Things that run.
        "exe" | "msi" | "dll" | "so" | "dylib" | "bin" | "appimage" | "elf" | "com" | "app"
        | "run" | "flatpakref" => "binary",
        "desktop" => "app-window",
        // Whole disks and machines.
        "iso" | "img" | "dmg" | "vhd" | "vdi" | "qcow2" | "vmdk" | "squashfs" => "hard-drive",
        // Typefaces.
        "ttf" | "otf" | "woff" | "woff2" | "eot" | "pfb" | "bdf" => "type",
        // Keys and secrets.
        "pem" | "key" | "crt" | "cer" | "pfx" | "gpg" | "asc" | "kdbx" => "file-lock",
        // Anything else keeps the page it always had.
        _ => "text-x-generic",
    }
}

/// The accent key for an extension, or nothing for the default tone.
///
/// Deliberately sparse: a folder where every row is a different colour is a
/// folder where colour has stopped meaning anything. Only the languages and
/// kinds that share a page with others are tinted, so the colour is the thing
/// that separates them.
fn accent_for_extension(extension: &str) -> &'static str {
    match extension {
        // Languages with no page of their own, in the colour each is known by.
        "py" | "pyw" | "pyi" => "amber",
        "go" => "cyan",
        "c" | "h" => "blue",
        "cc" | "cpp" | "cxx" | "hpp" => "violet",
        "java" | "kt" => "coral",
        "rb" => "coral",
        "php" => "violet",
        "cs" => "green",
        "swift" => "coral",
        "lua" | "zig" | "nim" | "dart" | "hs" | "ml" | "ex" | "exs" | "scala" => "violet",
        "qml" => "cyan",
        "svelte" => "coral",
        // Data and configuration share one page too.
        "json" => "amber",
        "yaml" | "yml" => "green",
        "toml" | "ini" | "conf" | "cfg" | "kdl" => "cyan",
        // Shells share the terminal glyph.
        "sh" | "bash" | "zsh" | "fish" => "green",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::{accent_for_extension, extension_of, icon_for_extension};

    #[test]
    fn an_extension_is_read_from_the_end_of_the_name() {
        assert_eq!(extension_of("notas.TXT"), "txt");
        assert_eq!(extension_of("archivo.tar.gz"), "tar.gz");
        assert_eq!(extension_of("sin-extension"), "");
        // A dot inside a version is not an extension of its own.
        assert_eq!(extension_of("web-2.1.2"), "2");
    }

    #[test]
    fn each_family_gets_its_own_glyph() {
        // A language the family draws a page for gets that page…
        assert_eq!(icon_for_extension("rs"), "file-rs");
        assert_eq!(icon_for_extension("tsx"), "file-tsx");
        // …and one it does not keeps the generic page, told apart by its tint.
        assert_eq!(icon_for_extension("py"), "file-code");
        assert_eq!(icon_for_extension("go"), "file-code");
        assert_eq!(icon_for_extension("zip"), "file-archive");
        assert_eq!(icon_for_extension("tar.gz"), "file-archive");
        assert_eq!(icon_for_extension("exe"), "binary");
        assert_eq!(icon_for_extension("ttf"), "type");
        assert_eq!(icon_for_extension("toml"), "file-braces");
        assert_eq!(icon_for_extension("iso"), "hard-drive");
        assert_eq!(icon_for_extension("sh"), "terminal");
        // Unknown keeps the generic page rather than inventing a family.
        assert_eq!(icon_for_extension("qwerty"), "text-x-generic");
        assert_eq!(icon_for_extension(""), "text-x-generic");
    }

    /// Colour separates what shares a page, and only that: a file with a page
    /// of its own is left alone, or the view would turn into a colour chart.
    #[test]
    fn colour_tells_apart_what_shares_a_glyph() {
        assert_eq!(accent_for_extension("py"), "amber");
        assert_eq!(accent_for_extension("go"), "cyan");
        assert_eq!(accent_for_extension("cpp"), "violet");
        // Rust has its own page, so it needs no tint.
        assert_eq!(accent_for_extension("rs"), "");
        assert_eq!(accent_for_extension("png"), "");
        assert_eq!(accent_for_extension(""), "");
    }
}
