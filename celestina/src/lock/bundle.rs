//! The backdrop bundle as the lock receives it: a sealed memory file passed as
//! its stdin (CEL-2-H design §4.3 (a), M4).
//!
//! CEL-2-H-a sent the header only; CEL-2-H-b adds the pixel blocks in the
//! same frozen format: [`with_thumbnails`] copies the wallpaper state's
//! blurred thumbnails (the first eight outputs by connector name; a ninth
//! gets no block, F40). The header carries the local UTC offset the
//! supervisor read for this spawn (`offset_known = 1`), or `offset_known = 0`
//! when it could not be read and the lock then hides its clock (design §4.8).
//! The bytes come from the codec of `celestina-lock-proto`, the one owner of
//! the layout.
//!
//! The file is written with positioned writes, so the descriptor's offset
//! stays 0 and the child's first `read` starts at the magic; the writer
//! checks that before it hands the descriptor out. It is then sealed against
//! growing, shrinking, writing and sealing, so the child (which reads what
//! it is given and nothing else) can trust the size it measures.
#![forbid(unsafe_code)]

use std::io;
use std::os::fd::{AsFd, OwnedFd};

use celestina_lock_proto::{encode, Bundle, MAX_FILE_BYTES};
use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, seek, MemfdFlags, SealFlags, SeekFrom};

use crate::wallpaper::thumbnail::ThumbnailStore;

/// Why the bundle could not be made.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    /// The memory file could not be created, sized, written or sealed.
    #[error("cannot build the bundle's memory file")]
    File(#[source] io::Error),
    /// The encoded bundle is larger than the format allows.
    #[error("the bundle is {0} bytes, over the format's limit")]
    TooLarge(usize),
    /// The descriptor's offset is not 0: the child would read from the middle.
    #[error("the bundle file's offset is {0}, not 0")]
    Offset(u64),
}

/// The four seals: no shrinking, growing, writing or adding seals.
const ALL_SEALS: SealFlags = SealFlags::from_bits_retain(
    SealFlags::SHRINK.bits()
        | SealFlags::GROW.bits()
        | SealFlags::WRITE.bits()
        | SealFlags::SEAL.bits(),
);

/// The bundle for a spawn: the failures carried, the UTC offset read for this
/// spawn (`None` sets `offset_known = 0` and writes 0), and one block per
/// thumbnail the wallpaper state holds now, the first
/// [`celestina_lock_proto::MAX_OUTPUTS`] by connector name. With the wallpaper
/// duty off the store is empty and the bundle has no blocks: the lock is
/// black.
#[must_use]
pub fn with_thumbnails(
    prior_failures: u8,
    utc_offset_s: Option<i32>,
    thumbnails: &ThumbnailStore,
) -> Bundle {
    Bundle {
        offset_known: utc_offset_s.is_some(),
        utc_offset_s: utc_offset_s.unwrap_or(0),
        outputs: thumbnails.blocks(),
        ..Bundle::header_only(prior_failures)
    }
}

fn file_error(errno: rustix::io::Errno) -> BundleError {
    BundleError::File(io::Error::from(errno))
}

/// A sealed memory file holding `bundle`, its offset at 0, close-on-exec (the
/// standard library's `dup2` onto the child's stdin clears that for the child
/// only).
///
/// # Errors
///
/// [`BundleError`] when the file cannot be made, is over the format's limit,
/// or its offset is not 0 afterwards.
pub fn sealed(bundle: &Bundle) -> Result<OwnedFd, BundleError> {
    let bytes = encode(bundle);
    let len = u64::try_from(bytes.len()).map_err(|_| BundleError::TooLarge(bytes.len()))?;
    if len > MAX_FILE_BYTES {
        return Err(BundleError::TooLarge(bytes.len()));
    }
    let fd = memfd_create(
        "celestina-lock-bundle",
        MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
    )
    .map_err(file_error)?;
    ftruncate(&fd, len).map_err(file_error)?;
    let mut written = 0usize;
    while written < bytes.len() {
        let offset = u64::try_from(written).map_err(|_| BundleError::TooLarge(bytes.len()))?;
        match rustix::io::pwrite(&fd, &bytes[written..], offset) {
            Ok(0) => return Err(BundleError::File(io::Error::from(io::ErrorKind::WriteZero))),
            Ok(count) => written += count,
            Err(rustix::io::Errno::INTR) => {}
            Err(errno) => return Err(file_error(errno)),
        }
    }
    fcntl_add_seals(&fd, ALL_SEALS).map_err(file_error)?;
    let offset = seek(fd.as_fd(), SeekFrom::Current(0)).map_err(file_error)?;
    if offset != 0 {
        return Err(BundleError::Offset(offset));
    }
    Ok(fd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_lock_proto::{
        decode, FileMeta, OutputBlock, Status, MAGIC, MAX_OUTPUTS, SIDE_MAX,
    };
    use rustix::fs::{fcntl_get_seals, fstat};

    fn size(fd: &OwnedFd) -> u64 {
        u64::try_from(fstat(fd).expect("fstat").st_size).expect("size")
    }

    fn contents(fd: &OwnedFd) -> Vec<u8> {
        let mut bytes = vec![0u8; usize::try_from(size(fd)).expect("len")];
        let mut filled = 0;
        while filled < bytes.len() {
            let read =
                rustix::io::pread(fd, &mut bytes[filled..], u64::try_from(filled).expect("o"))
                    .expect("pread");
            assert_ne!(read, 0, "the file is shorter than its size");
            filled += read;
        }
        bytes
    }

    #[test]
    fn t_z1_the_memfd_offset_is_0_and_the_first_read_returns_the_magic() {
        let fd = sealed(&Bundle::header_only(2)).expect("a bundle");
        // The offset the child inherits: 0, so its first read is the magic.
        assert_eq!(seek(&fd, SeekFrom::Current(0)).expect("seek"), 0);
        let mut first = [0u8; 4];
        let read = rustix::io::read(&fd, &mut first).expect("read");
        assert_eq!(read, 4);
        assert_eq!(first, MAGIC);
    }

    #[test]
    fn a_missing_offset_sets_offset_known_0() {
        let fd = sealed(&Bundle::header_only(5)).expect("a bundle");
        let bytes = contents(&fd);
        let decoded = decode(
            &bytes,
            FileMeta {
                size: size(&fd),
                seals_ok: true,
            },
        );
        assert_eq!(decoded.status, Status::Complete);
        assert!(
            !decoded.bundle.offset_known,
            "a header-only bundle has no offset"
        );
        assert_eq!(decoded.bundle.prior_failures, 5);
        assert!(decoded.bundle.outputs.is_empty());
    }

    #[test]
    fn a_bundle_with_8_maximal_outputs_fits_16_mib() {
        let side = usize::from(SIDE_MAX);
        let outputs: Vec<OutputBlock> = (0..MAX_OUTPUTS)
            .map(|index| {
                OutputBlock::new(
                    format!("{index:0>64}"),
                    SIDE_MAX,
                    SIDE_MAX,
                    vec![0xff; side * side * 4],
                )
                .expect("a maximal block")
            })
            .collect();
        let bundle = Bundle {
            prior_failures: 9,
            offset_known: true,
            utc_offset_s: 3600,
            outputs,
        };
        let fd = sealed(&bundle).expect("fits");
        assert!(size(&fd) <= MAX_FILE_BYTES, "{} bytes", size(&fd));
        let decoded = decode(
            &contents(&fd),
            FileMeta {
                size: size(&fd),
                seals_ok: true,
            },
        );
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle.outputs.len(), MAX_OUTPUTS);
    }

    use crate::wallpaper::decode::Decoded;
    use crate::wallpaper::thumbnail::derive;

    fn store_with(names: &[&str], side: u32) -> ThumbnailStore {
        let source = Decoded {
            rgba: vec![0x80; (side * side * 4) as usize],
            width: side,
            height: side,
            source_px: (side, side),
        };
        let thumb = derive(&source).expect("a thumbnail");
        let store = ThumbnailStore::default();
        for name in names {
            store.set(name, thumb.clone());
        }
        store
    }

    fn decoded_from(fd: &OwnedFd) -> celestina_lock_proto::Decoded {
        decode(
            &contents(fd),
            FileMeta {
                size: size(fd),
                seals_ok: true,
            },
        )
    }

    #[test]
    fn the_bundle_carries_one_block_per_output_up_to_8_by_connector_name() {
        let store = store_with(&["HDMI-A-1", "DP-2", "eDP-1", "DP-1"], 64);
        let fd = sealed(&with_thumbnails(2, None, &store)).expect("a bundle");
        let decoded = decoded_from(&fd);
        assert_eq!(decoded.status, Status::Complete);
        assert_eq!(decoded.bundle.prior_failures, 2);
        assert!(!decoded.bundle.offset_known, "no offset was read");
        let names: Vec<&str> = decoded.bundle.outputs.iter().map(|b| b.name()).collect();
        assert_eq!(names, ["DP-1", "DP-2", "HDMI-A-1", "eDP-1"]);
        for block in &decoded.bundle.outputs {
            assert_eq!((block.width(), block.height()), (64, 64));
            assert_eq!(block.pixels().len(), 64 * 64 * 4);
            assert!(block
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [0x80, 0x80, 0x80, 255]));
        }
    }

    #[test]
    fn a_read_offset_is_written_known_and_a_failed_one_is_not() {
        let store = ThumbnailStore::default();
        for (read, known, seconds) in [
            (Some(7200), true, 7200),
            (Some(-16_200), true, -16_200),
            (Some(0), true, 0),
            (None, false, 0),
        ] {
            let fd = sealed(&with_thumbnails(1, read, &store)).expect("a bundle");
            let decoded = decoded_from(&fd);
            assert_eq!(decoded.status, Status::Complete);
            assert_eq!(decoded.bundle.offset_known, known, "{read:?}");
            assert_eq!(decoded.bundle.utc_offset_s, seconds, "{read:?}");
        }
    }

    #[test]
    fn a_ninth_output_gets_no_block() {
        let names: Vec<String> = (1..=9).map(|n| format!("DP-{n}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let fd = sealed(&with_thumbnails(0, None, &store_with(&names, 16))).expect("a bundle");
        let decoded = decoded_from(&fd);
        assert_eq!(decoded.status, Status::Complete);
        let kept: Vec<&str> = decoded.bundle.outputs.iter().map(|b| b.name()).collect();
        assert_eq!(kept.len(), MAX_OUTPUTS);
        assert!(!kept.contains(&"DP-9"), "{kept:?}");
    }

    #[test]
    fn an_empty_store_is_the_h_a_header_only_bundle() {
        let with = encode(&with_thumbnails(4, None, &ThumbnailStore::default()));
        assert_eq!(with, encode(&Bundle::header_only(4)));
    }

    #[test]
    fn the_bundle_still_validates_with_the_h_a_decoder_unchanged() {
        // The decoder is the proto crate's, which this unit does not touch;
        // a bundle with blocks is Complete under its unchanged rules, and so
        // is every truncation refused as before.
        let fd = sealed(&with_thumbnails(
            1,
            None,
            &store_with(&["DP-1", "DP-2"], 640),
        ))
        .expect("bundle");
        let bytes = contents(&fd);
        let good = decode(
            &bytes,
            FileMeta {
                size: size(&fd),
                seals_ok: true,
            },
        );
        assert_eq!(good.status, Status::Complete);
        assert_eq!(good.bundle.outputs.len(), 2);
        let short = &bytes[..bytes.len() - 1];
        let bad = decode(
            short,
            FileMeta {
                size: u64::try_from(short.len()).expect("len"),
                seals_ok: true,
            },
        );
        assert_ne!(bad.status, Status::Complete, "a short file is not accepted");
    }

    #[test]
    fn the_memfd_is_sealed_against_grow_shrink_write_and_seal() {
        let fd = sealed(&Bundle::header_only(0)).expect("a bundle");
        let seals = fcntl_get_seals(&fd).expect("seals");
        assert!(seals.contains(ALL_SEALS), "{seals:?}");
        let len = size(&fd);
        assert!(ftruncate(&fd, len + 1).is_err(), "it grew");
        assert!(ftruncate(&fd, len - 1).is_err(), "it shrank");
        assert!(rustix::io::pwrite(&fd, b"x", 0).is_err(), "it was written");
        assert!(
            fcntl_add_seals(&fd, SealFlags::SEAL).is_err(),
            "a seal was added"
        );
        assert_eq!(size(&fd), len);
    }

    #[test]
    fn the_descriptor_is_close_on_exec() {
        let fd = sealed(&Bundle::header_only(0)).expect("a bundle");
        let flags = rustix::io::fcntl_getfd(&fd).expect("fcntl");
        assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
    }
}
