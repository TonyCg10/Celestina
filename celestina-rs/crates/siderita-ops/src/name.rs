use std::error::Error;
use std::ffi::OsStr;
use std::fmt;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

/// Why a proposed name is not a usable single path component.
///
/// The visible name is never identity, but a new or renamed entry must still be
/// exactly one component inside its parent — never empty, never a path that
/// climbs out of or descends below the directory it belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameError {
    /// The name is empty.
    Empty,
    /// The name contains a path separator (`/`).
    Separator,
    /// The name is `.` or `..`.
    Reserved,
    /// The name contains an interior NUL byte, which no path may hold.
    InteriorNul,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "a name cannot be empty",
            Self::Separator => "a name cannot contain '/'",
            Self::Reserved => "'.' and '..' are reserved",
            Self::InteriorNul => "a name cannot contain a NUL byte",
        })
    }
}

impl Error for NameError {}

/// Validates that `name` is a single, safe path component.
///
/// Non-UTF-8 names are accepted: the structural checks work on the raw bytes so
/// a valid but non-UTF-8 filename is never rejected merely for its encoding.
pub fn validate_name(name: &OsStr) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name == OsStr::new(".") || name == OsStr::new("..") {
        return Err(NameError::Reserved);
    }
    check_bytes(name)
}

#[cfg(unix)]
fn check_bytes(name: &OsStr) -> Result<(), NameError> {
    let bytes = name.as_bytes();
    if bytes.contains(&b'/') {
        return Err(NameError::Separator);
    }
    if bytes.contains(&0) {
        return Err(NameError::InteriorNul);
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_bytes(name: &OsStr) -> Result<(), NameError> {
    let text = name.to_string_lossy();
    if text.contains('/') || text.contains('\\') {
        return Err(NameError::Separator);
    }
    if text.contains('\0') {
        return Err(NameError::InteriorNul);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_name, NameError};
    use std::ffi::OsStr;

    #[test]
    fn plain_names_are_accepted() {
        assert!(validate_name(OsStr::new("archivo.txt")).is_ok());
        assert!(validate_name(OsStr::new("una carpeta")).is_ok());
        assert!(validate_name(OsStr::new("...leading-dots")).is_ok());
    }

    #[test]
    fn structural_names_are_rejected() {
        assert_eq!(validate_name(OsStr::new("")), Err(NameError::Empty));
        assert_eq!(validate_name(OsStr::new(".")), Err(NameError::Reserved));
        assert_eq!(validate_name(OsStr::new("..")), Err(NameError::Reserved));
        assert_eq!(validate_name(OsStr::new("a/b")), Err(NameError::Separator));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_names_are_accepted_by_bytes() {
        use std::os::unix::ffi::OsStrExt;
        assert!(validate_name(OsStr::from_bytes(b"na\xffme")).is_ok());
        assert_eq!(
            validate_name(OsStr::from_bytes(b"a\0b")),
            Err(NameError::InteriorNul)
        );
    }
}
