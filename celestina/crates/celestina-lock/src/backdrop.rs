//! The backdrop bundle the shell hands over on stdin (design §4.3 (a)).
//!
//! CEL-2-H-a reads the **header only**: this module validates the whole
//! bundle with the shared codec and keeps what the lock acts on now, the
//! failures the previous lock counted (`prior_failures`) and the UTC offset
//! facts. It draws no pixel; the shell sends `outputs = 0` in this unit and the
//! pixel blocks arrive with CEL-2-H-b over the frozen format, where the
//! reading moves to a `backdrop` worker.
//!
//! The reader never trusts its input and never blocks on it: stdin must be a
//! memory file sealed against shrinking, growing, writing and sealing (a
//! `/dev/null`, a terminal or a pipe is not, and the lock then has no backdrop
//! and waits for nothing); the size is checked against the 16 MiB bound before
//! anything is allocated; the read is positioned (`pread` from offset 0), so
//! the descriptor's file offset is irrelevant, and bounded by a 2 s deadline.
//! Anything that fails a bound is ignored as a whole: black, no failures
//! carried. A bad block keeps the header (the codec's rule).
#![forbid(unsafe_code)]

use std::os::fd::BorrowedFd;
use std::time::{Duration, Instant};

use celestina_lock_proto::{decode, FileMeta, Ignored, Status, MAX_FILE_BYTES};
use rustix::fs::{fcntl_get_seals, fstat, FileType, SealFlags};

/// How long the read of the bundle may take.
pub const READ_DEADLINE: Duration = Duration::from_secs(2);

/// What the lock keeps of the bundle in this unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// Counted failures since the last unlock; seeds the backoff.
    pub prior_failures: u8,
    /// Whether `utc_offset_s` was read for this spawn.
    pub offset_known: bool,
    /// The local UTC offset in seconds (0 when not known).
    pub utc_offset_s: i32,
}

/// How the read ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// stdin is not a sealed memory file: no backdrop, nothing waited for.
    NoBundle,
    /// The file could not be read within the bounds.
    Unreadable,
    /// The codec's verdict on the bytes, and what was kept.
    Decoded { header: Header, status: Status },
}

impl Loaded {
    /// The header to use: the default (no failures carried) unless a valid
    /// header was read.
    #[must_use]
    pub fn header(&self) -> Header {
        match self {
            Self::Decoded { header, status } if !matches!(status, Status::Ignored(_)) => *header,
            _ => Header::default(),
        }
    }
}

const ALL_SEALS: SealFlags = SealFlags::from_bits_retain(
    SealFlags::SHRINK.bits()
        | SealFlags::GROW.bits()
        | SealFlags::WRITE.bits()
        | SealFlags::SEAL.bits(),
);

/// Reads and validates the bundle on `fd`.
#[must_use]
pub fn read_header(fd: BorrowedFd<'_>, deadline: Duration) -> Loaded {
    // Only a memory file can carry seals: this is also what keeps a pipe, a
    // terminal or `/dev/null` from ever being read.
    let Ok(seals) = fcntl_get_seals(fd) else {
        return Loaded::NoBundle;
    };
    let Ok(stat) = fstat(fd) else {
        return Loaded::Unreadable;
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Loaded::NoBundle;
    }
    let Ok(size) = u64::try_from(stat.st_size) else {
        return Loaded::Unreadable;
    };
    let seals_ok = seals.contains(ALL_SEALS);
    if size > MAX_FILE_BYTES {
        // Never allocate for what the bound refuses.
        return Loaded::Decoded {
            header: Header::default(),
            status: Status::Ignored(Ignored::TooLarge),
        };
    }
    let Ok(len) = usize::try_from(size) else {
        return Loaded::Unreadable;
    };
    let end = Instant::now() + deadline;
    let mut bytes = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        if Instant::now() >= end {
            return Loaded::Unreadable;
        }
        let offset = u64::try_from(filled).unwrap_or(u64::MAX);
        match rustix::io::pread(fd, &mut bytes[filled..], offset) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return Loaded::Unreadable,
        }
    }
    bytes.truncate(filled);
    let decoded = decode(&bytes, FileMeta { size, seals_ok });
    Loaded::Decoded {
        header: Header {
            prior_failures: decoded.bundle.prior_failures,
            offset_known: decoded.bundle.offset_known,
            utc_offset_s: decoded.bundle.utc_offset_s,
        },
        status: decoded.status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_lock_proto::{encode, Bundle};
    use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, MemfdFlags};
    use std::os::fd::{AsFd, OwnedFd};

    fn memfd(bytes: &[u8], seal: bool) -> OwnedFd {
        let fd =
            memfd_create("bundle", MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC).expect("memfd");
        ftruncate(&fd, u64::try_from(bytes.len()).expect("len")).expect("truncate");
        let mut written = 0;
        while written < bytes.len() {
            written +=
                rustix::io::pwrite(&fd, &bytes[written..], u64::try_from(written).expect("o"))
                    .expect("pwrite");
        }
        if seal {
            fcntl_add_seals(&fd, ALL_SEALS).expect("seal");
        }
        fd
    }

    fn header_bundle(prior_failures: u8) -> Vec<u8> {
        encode(&Bundle::header_only(prior_failures))
    }

    #[test]
    fn a_sealed_header_only_bundle_yields_its_prior_failures() {
        let fd = memfd(&header_bundle(3), true);
        let loaded = read_header(fd.as_fd(), READ_DEADLINE);
        assert_eq!(loaded.header().prior_failures, 3);
        assert!(matches!(
            loaded,
            Loaded::Decoded {
                status: Status::Complete,
                ..
            }
        ));
    }

    #[test]
    fn the_file_offset_does_not_matter() {
        let fd = memfd(&header_bundle(5), true);
        // A reader that had already consumed the file still finds the magic.
        let mut sink = [0u8; 64];
        let _ = rustix::io::read(&fd, &mut sink);
        assert_eq!(
            read_header(fd.as_fd(), READ_DEADLINE)
                .header()
                .prior_failures,
            5
        );
    }

    #[test]
    fn an_unsealed_memfd_is_ignored_whole() {
        let fd = memfd(&header_bundle(4), false);
        let loaded = read_header(fd.as_fd(), READ_DEADLINE);
        assert_eq!(loaded.header(), Header::default());
        assert!(matches!(
            loaded,
            Loaded::Decoded {
                status: Status::Ignored(Ignored::MissingSeal),
                ..
            }
        ));
    }

    #[test]
    fn stdin_that_is_not_a_memfd_means_no_bundle() {
        let (reader, _writer) = std::io::pipe().expect("pipe");
        assert_eq!(read_header(reader.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        let null = std::fs::File::open("/dev/null").expect("null");
        assert_eq!(read_header(null.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        let plain = std::fs::File::open("/proc/self/status").expect("status");
        assert_eq!(read_header(plain.as_fd(), READ_DEADLINE), Loaded::NoBundle);
        assert_eq!(Loaded::NoBundle.header(), Header::default());
    }

    #[test]
    fn a_bundle_past_16_mib_is_ignored_without_reading_it() {
        let fd = memfd_create("big", MemfdFlags::ALLOW_SEALING).expect("memfd");
        ftruncate(&fd, MAX_FILE_BYTES + 1).expect("truncate");
        fcntl_add_seals(&fd, ALL_SEALS).expect("seal");
        let loaded = read_header(fd.as_fd(), READ_DEADLINE);
        assert_eq!(
            loaded,
            Loaded::Decoded {
                header: Header::default(),
                status: Status::Ignored(Ignored::TooLarge)
            }
        );
    }

    #[test]
    fn bad_magic_a_short_file_and_a_size_mismatch_are_ignored() {
        let mut bad = header_bundle(2);
        bad[0] = b'X';
        for bytes in [bad, vec![1, 2, 3], {
            let mut long = header_bundle(2);
            long.push(0);
            long
        }] {
            let fd = memfd(&bytes, true);
            assert_eq!(
                read_header(fd.as_fd(), READ_DEADLINE).header(),
                Header::default()
            );
        }
    }

    #[test]
    fn the_offset_facts_are_kept() {
        let bundle = Bundle {
            prior_failures: 1,
            offset_known: true,
            utc_offset_s: 7200,
            outputs: Vec::new(),
        };
        let fd = memfd(&encode(&bundle), true);
        let header = read_header(fd.as_fd(), READ_DEADLINE).header();
        assert_eq!(
            header,
            Header {
                prior_failures: 1,
                offset_known: true,
                utc_offset_s: 7200
            }
        );
    }
}
