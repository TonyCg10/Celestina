//! Reproducing a file's metadata onto the temporary that will replace it.
//!
//! A save that publishes the right bytes but drops the file's mode, owner,
//! extended attributes or ACL has still lost part of the user's file. Whatever
//! cannot be reproduced is reported, and the caller refuses the save rather
//! than shipping a diminished copy.

use std::fmt;
use std::fs::{self, Metadata, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::target::Ownership;

/// Copies mode, ownership and extended attributes from `source` onto
/// `temporary`.
///
/// Order matters and is not incidental: extended attributes go first because a
/// POSIX ACL is one of them and writing it changes the mode; ownership follows,
/// because changing it clears set-user-id and set-group-id; the permission bits
/// are written last so the file ends with exactly the source's mode.
pub fn reproduce(
    source: &Path,
    metadata: &Metadata,
    temporary: &Path,
) -> Result<(), MetadataError> {
    copy_extended_attributes(source, temporary)?;
    let ownership = Ownership::of(metadata);
    apply_ownership(temporary, ownership)?;
    fs::set_permissions(
        temporary,
        Permissions::from_mode(ownership.permission_bits()),
    )
    .map_err(|error| MetadataError::Permissions {
        mode: ownership.permission_bits(),
        message: error.to_string(),
    })
}

fn copy_extended_attributes(source: &Path, temporary: &Path) -> Result<(), MetadataError> {
    let names = match xattr::list(source) {
        Ok(names) => names,
        // A filesystem without extended-attribute support has none to lose.
        Err(error) if is_unsupported(&error) => return Ok(()),
        Err(error) => {
            return Err(MetadataError::ExtendedAttributesUnreadable {
                message: error.to_string(),
            })
        }
    };

    for name in names {
        // The contract covers *readable* attributes. One that cannot be read
        // back — it vanished, or this process may not see it — is not a value
        // being dropped, so it is skipped rather than turned into a refusal.
        let Ok(Some(value)) = xattr::get(source, &name) else {
            continue;
        };
        // The temporary inherits defaults from its directory, so a security
        // label may already be correct. Writing it again could need privileges
        // that reproducing it does not.
        if matches!(xattr::get(temporary, &name), Ok(Some(present)) if present == value) {
            continue;
        }
        xattr::set(temporary, &name, &value).map_err(|error| MetadataError::ExtendedAttribute {
            name: name.to_string_lossy().into_owned(),
            message: error.to_string(),
        })?;
    }
    Ok(())
}

fn apply_ownership(temporary: &Path, ownership: Ownership) -> Result<(), MetadataError> {
    let attempt = std::os::unix::fs::chown(temporary, Some(ownership.user), Some(ownership.group));
    let Err(error) = attempt else {
        return Ok(());
    };
    // Changing owner normally needs privileges. When the temporary already
    // belongs to the same user and group, nothing was lost and the refusal
    // would be pure ceremony.
    let current = temporary
        .metadata()
        .map(|metadata| Ownership::of(&metadata))
        .map_err(|error| MetadataError::Ownership {
            user: ownership.user,
            group: ownership.group,
            message: error.to_string(),
        })?;
    if current.user == ownership.user && current.group == ownership.group {
        return Ok(());
    }
    Err(MetadataError::Ownership {
        user: ownership.user,
        group: ownership.group,
        message: error.to_string(),
    })
}

/// `ENOTSUP`/`EOPNOTSUPP` and `ENOSYS`: the filesystem or kernel has no
/// extended attributes at all, which is different from failing to read them.
const ENOTSUP: i32 = 95;
const ENOSYS: i32 = 38;

fn is_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
        || matches!(error.raw_os_error(), Some(ENOTSUP | ENOSYS))
}

/// A piece of the original file this save could not reproduce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataError {
    /// The source's extended attributes could not even be listed, so it is
    /// unknown what would be lost.
    ExtendedAttributesUnreadable { message: String },
    /// A readable extended attribute — a POSIX ACL among them — could not be
    /// written to the replacement.
    ExtendedAttribute { name: String, message: String },
    /// The replacement could not be given the original's owner or group.
    Ownership {
        user: u32,
        group: u32,
        message: String,
    },
    /// The replacement could not be given the original's permission bits.
    Permissions { mode: u32, message: String },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtendedAttributesUnreadable { message } => write!(
                formatter,
                "the original's extended attributes could not be listed: {message}"
            ),
            Self::ExtendedAttribute { name, message } => write!(
                formatter,
                "the extended attribute '{name}' could not be reproduced: {message}"
            ),
            Self::Ownership {
                user,
                group,
                message,
            } => write!(
                formatter,
                "the replacement could not be owned by {user}:{group}: {message}"
            ),
            Self::Permissions { mode, message } => write!(
                formatter,
                "the replacement could not take mode {mode:o}: {message}"
            ),
        }
    }
}

impl std::error::Error for MetadataError {}

#[cfg(test)]
mod tests {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    use super::reproduce;
    use crate::target::Ownership;
    use crate::testing::scratch_directory;

    #[test]
    fn mode_and_readable_extended_attributes_reach_the_replacement() {
        let root = scratch_directory("metadata-reproduce");
        let source = root.join("original");
        let temporary = root.join("replacement");
        fs::write(&source, b"antes").expect("write source");
        fs::write(&temporary, b"despues").expect("write temporary");
        fs::set_permissions(&source, Permissions::from_mode(0o640)).expect("mode");
        let has_xattr = xattr::set(&source, "user.grafita", b"valor").is_ok();

        let metadata = source.metadata().expect("metadata");
        reproduce(&source, &metadata, &temporary).expect("reproduce");

        let copied = temporary.metadata().expect("metadata");
        assert_eq!(Ownership::of(&copied).permission_bits(), 0o640);
        if has_xattr {
            assert_eq!(
                xattr::get(&temporary, "user.grafita").expect("get"),
                Some(b"valor".to_vec())
            );
        }

        let _ = fs::remove_dir_all(root);
    }
}
