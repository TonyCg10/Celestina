//! Bounded, disposable cache for album art received from a trusted phone.
//!
//! The remote `albumArtUrl` is only an opaque request key. Bytes arrive over
//! KDE Connect's separate TLS payload socket and land under the user's runtime
//! directory with a generated name, never a peer-provided path.

use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use magnetita_core::IncomingAlbumArt;
use magnetita_net::{PayloadPermit, TlsConfigs};

use crate::devices::{install_artwork_entry, DeviceEntry};

/// Covers normal embedded covers while refusing file-sized declarations before
/// opening a payload socket.
pub const MAX_ARTWORK_BYTES: i64 = 8 * 1024 * 1024;

/// Remove artwork left by a killed previous run. The cache is disposable and
/// rebuilt from the phone's current player state.
pub fn sweep() -> io::Result<()> {
    let root = root_dir()?;
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    Ok(())
}

/// Receive one cover losslessly: bounded declaration, exact byte count, known
/// image signature, then atomic `.part` -> final rename.
pub fn receive(
    device_id: &str,
    host: &str,
    tls: &TlsConfigs,
    expected_peer_fingerprint: &str,
    permit: PayloadPermit,
    incoming: &IncomingAlbumArt,
    cancellation: CancellationToken,
) -> io::Result<PathBuf> {
    validate_size(incoming.size)?;
    let directory = device_dir(device_id)?;
    fs::create_dir_all(&directory)?;

    let destination = directory.join(cache_name(incoming));
    let partial = destination.with_extension("part");
    let partial_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)?;
    let result = magnetita_net::receive_to_file(
        magnetita_net::PayloadSource {
            host,
            port: incoming.port,
            size: incoming.size,
            expected_peer_fingerprint,
        },
        tls,
        &cancellation,
        permit,
        partial_file,
    );
    let written = match result {
        Ok(written) => written,
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
    };
    if written != incoming.size as u64 {
        let _ = fs::remove_file(&partial);
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "album art declared {} bytes but sent {written}",
                incoming.size
            ),
        ));
    }
    let supported = match has_supported_signature(&partial) {
        Ok(supported) => supported,
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
    };
    if !supported {
        let _ = fs::remove_file(&partial);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "album art payload is not PNG, JPEG, or WebP",
        ));
    }
    if let Err(error) = fs::rename(&partial, &destination) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    Ok(destination)
}

/// Atomically publish one already-received cover. `false` means the
/// player/source changed while the transfer was in flight; the generated file
/// is discarded and the current snapshot remains untouched.
pub fn publish_received(entry: &mut DeviceEntry, path: &Path, incoming: &IncomingAlbumArt) -> bool {
    let url = file_url(path);
    match install_artwork_entry(
        entry,
        &incoming.player,
        &incoming.source_url,
        path.to_path_buf(),
        url,
    ) {
        Some(previous) => {
            if let Some(previous) = previous {
                discard(&previous);
            }
            true
        }
        None => {
            discard(path);
            false
        }
    }
}

/// Delete one generated cache file, but only when it still belongs to our
/// runtime cache root.
pub fn discard(path: &Path) {
    if root_dir().ok().is_some_and(|root| path.starts_with(root)) {
        let _ = fs::remove_file(path);
    }
}

/// Delete every cover for a disconnected device.
pub fn clear_device(device_id: &str) {
    if let Ok(directory) = device_dir(device_id) {
        let _ = fs::remove_dir_all(directory);
    }
}

/// A percent-encoded local URL suitable for QML's `Image.source`.
pub fn file_url(path: &Path) -> String {
    format!(
        "file://{}",
        celestina_core::percent::encode(&celestina_core::percent::path_bytes(path))
    )
}

fn root_dir() -> io::Result<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|path| path.join("magnetita").join("artwork"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "XDG_RUNTIME_DIR is unavailable for the album-art cache",
            )
        })
}

fn device_dir(device_id: &str) -> io::Result<PathBuf> {
    Ok(root_dir()?.join(format!("{:016x}", hash(device_id))))
}

fn cache_name(incoming: &IncomingAlbumArt) -> String {
    let identity = (
        incoming.player.as_str(),
        incoming.source_url.as_str(),
        incoming.transfer_id,
    );
    format!("{:016x}.img", hash(identity))
}

fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn validate_size(size: i64) -> io::Result<()> {
    if (1..=MAX_ARTWORK_BYTES).contains(&size) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing album art with declared size {size}"),
        ))
    }
}

fn has_supported_signature(path: &Path) -> io::Result<bool> {
    let mut header = [0_u8; 12];
    let read = fs::File::open(path)?.read(&mut header)?;
    Ok(is_supported_header(&header[..read]))
}

fn is_supported_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

#[cfg(test)]
mod tests {
    use super::{cache_name, is_supported_header, validate_size, MAX_ARTWORK_BYTES};
    use magnetita_core::IncomingAlbumArt;

    fn artwork(transfer_id: i64) -> IncomingAlbumArt {
        IncomingAlbumArt {
            player: "Spotify".to_owned(),
            source_url: "file:///phone/cover".to_owned(),
            size: 1024,
            port: 1740,
            transfer_id,
        }
    }

    #[test]
    fn artwork_has_a_tighter_bound_than_general_file_shares() {
        assert!(validate_size(1).is_ok());
        assert!(validate_size(MAX_ARTWORK_BYTES).is_ok());
        for size in [0, -1, MAX_ARTWORK_BYTES + 1] {
            assert_eq!(
                validate_size(size)
                    .expect_err("invalid size must fail")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn only_common_image_signatures_are_accepted() {
        assert!(is_supported_header(b"\x89PNG\r\n\x1a\nrest"));
        assert!(is_supported_header(&[0xff, 0xd8, 0xff, 0xe0]));
        assert!(is_supported_header(b"RIFF1234WEBP"));
        assert!(!is_supported_header(b"<svg onload='x'>"));
    }

    #[test]
    fn each_transfer_gets_a_distinct_cache_url() {
        assert_ne!(cache_name(&artwork(1)), cache_name(&artwork(2)));
        assert!(cache_name(&artwork(1)).ends_with(".img"));
    }
}
