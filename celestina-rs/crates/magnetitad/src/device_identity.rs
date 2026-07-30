//! Durable local KDE Connect device identity.

use std::fs;
use std::io;
use std::path::Path;

/// Load a valid stable 32-hex id, or generate and atomically persist one.
pub(crate) fn ensure(dir: &Path) -> io::Result<String> {
    let path = dir.join("device_id");
    if let Ok(existing) = fs::read_to_string(&path) {
        let existing = existing.trim();
        if valid(existing) {
            return Ok(existing.to_ascii_lowercase());
        }
    }
    let uuid = fs::read_to_string("/proc/sys/kernel/random/uuid")?;
    let id: String = uuid
        .trim()
        .chars()
        .filter(|character| *character != '-')
        .collect();
    if !valid(&id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "kernel UUID did not produce a KDE Connect device id",
        ));
    }
    celestina_core::atomic_file::replace(&path, id.as_bytes())?;
    Ok(id)
}

fn valid(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::valid;

    #[test]
    fn device_ids_are_exactly_32_hex_characters() {
        assert!(valid("0123456789abcdef0123456789ABCDEF"));
        assert!(!valid("phone"));
        assert!(!valid("0123456789abcdef0123456789abcdeg"));
    }
}
