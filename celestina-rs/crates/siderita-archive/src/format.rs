use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// An archive container this domain can read, and — for [`Format::Zip`] and
/// [`Format::TarGz`] — write.
///
/// The first three are implemented here, in Rust: opening them depends on
/// nothing being installed. The last two are proprietary or have no mature Rust
/// reader, so they are delegated to a tool the machine already has, and are only
/// offered when it is present (see [`can_read`](crate::can_read)).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// PKZIP container, deflate members.
    Zip,
    /// Uncompressed POSIX tar stream.
    Tar,
    /// A tar stream inside a gzip wrapper (`.tar.gz` / `.tgz`).
    TarGz,
    /// RAR, versions 4 and 5. Read through `unrar` or `7z`.
    Rar,
    /// 7z. Read through `7z`.
    SevenZip,
}

impl Format {
    /// Whether this domain can *create* the format (all three are readable).
    pub fn is_writable(self) -> bool {
        matches!(self, Self::Zip | Self::TarGz)
    }

    /// The extension a newly created archive of this format takes, without the
    /// leading dot. `Tar` answers too, so a caller naming a file never has to
    /// know the mapping, even though it does not create one.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::Rar => "rar",
            Self::SevenZip => "7z",
        }
    }

    /// Whether this domain decodes the format itself, rather than through an
    /// installed tool. The difference is visible to a person only when the tool
    /// is missing, so it is stated rather than implied.
    pub fn is_native(self) -> bool {
        matches!(self, Self::Zip | Self::Tar | Self::TarGz)
    }

    /// The stable identifier the Qt/QML side sends back when a person picks a
    /// format. Parsed by [`Format::from_token`]; never shown to a person.
    pub fn token(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::Rar => "rar",
            Self::SevenZip => "7z",
        }
    }

    /// The inverse of [`Format::token`].
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "zip" => Some(Self::Zip),
            "tar" => Some(Self::Tar),
            "tar.gz" | "tgz" => Some(Self::TarGz),
            "rar" => Some(Self::Rar),
            "7z" => Some(Self::SevenZip),
            _ => None,
        }
    }
}

/// Identifies `path` **by its bytes**, exactly like the suite's content
/// activation: a `.zip` that is really a gzip stream is a gzip stream, and an
/// archive whose name was lost still opens.
///
/// The name is consulted for one question the magic number cannot answer — a
/// gzip member holds no hint of what it wraps — and only to separate a `.tar.gz`
/// from a plain `.gz`, which this domain does not handle. Returns `None` when
/// the bytes are not one of the containers.
///
/// A container is named here whether or not this machine can open it; whether
/// it can is [`can_read`](crate::can_read)'s question, and keeping the two apart
/// is what lets a host say "install `unrar`" instead of "unknown file".
pub fn sniff(path: &Path) -> Option<Format> {
    let mut head = [0u8; 512];
    let read = read_head(path, &mut head)?;
    let head = &head[..read];

    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        return Some(Format::Zip);
    }
    // `Rar!\x1a\x07\x00` is RAR 4 and earlier; the same signature with a
    // trailing `\x01\x00` is RAR 5. Both start with the seven bytes checked
    // here, and neither is claimed unless a decoder for it exists.
    if head.starts_with(b"Rar!\x1a\x07") {
        return Some(Format::Rar);
    }
    if head.starts_with(b"7z\xbc\xaf\x27\x1c") {
        return Some(Format::SevenZip);
    }
    if head.starts_with(&[0x1f, 0x8b]) {
        // A gzip member. Only a tarball inside it is in scope; a lone `.gz`
        // (one compressed file) is a different verb and is not claimed here.
        return names_a_tarball(path).then_some(Format::TarGz);
    }
    // POSIX ustar/GNU tar: the magic sits at offset 257 of the first header.
    if read >= 262 && (&head[257..262] == b"ustar") {
        return Some(Format::Tar);
    }
    None
}

/// Whether the *name* claims a tarball inside the gzip member (`.tar.gz`,
/// `.tgz`). Byte-wise and case-insensitive on ASCII, so a non-UTF-8 name is
/// still answered rather than rejected.
fn names_a_tarball(path: &Path) -> bool {
    let Some(name) = path.file_name().map(OsStr::as_encoded_bytes) else {
        return false;
    };
    let lower: Vec<u8> = name.to_ascii_lowercase();
    lower.ends_with(b".tar.gz") || lower.ends_with(b".tgz") || lower.ends_with(b".tar.gzip")
}

/// Reads at most `buffer.len()` bytes from the head of `path`, answering `None`
/// when it cannot be opened or is not a regular file.
fn read_head(path: &Path, buffer: &mut [u8]) -> Option<usize> {
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    Some(filled)
}

/// The name an archive of `sources` takes by default, without an extension: the
/// single source's own stem, or the folder-shaped fallback the caller supplies
/// when several entries are compressed at once.
pub fn default_stem<'a>(sources: &'a [std::path::PathBuf], fallback: &'a OsStr) -> &'a OsStr {
    match sources {
        [only] => only
            .file_stem()
            .or_else(|| only.file_name())
            .unwrap_or(fallback),
        _ => fallback,
    }
}
