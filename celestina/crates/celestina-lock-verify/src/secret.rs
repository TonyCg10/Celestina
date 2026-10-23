//! Reading the secret from stdin (design §4.3 (c), §4.11).
//!
//! 1 to 512 bytes, no NUL, exactly as typed, then EOF. The buffer is a
//! `Zeroizing<Vec<u8>>` allocated once at its final size, so no reallocation
//! leaves a copy behind, and every refusal drops (zeroes) it.
#![forbid(unsafe_code)]

use std::io::{self, Read};

use celestina_lock_proto::SECRET_MAX;
use zeroize::Zeroizing;

/// Why the secret was refused. Every variant is `BadRequest` (exit 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretError {
    /// No byte at all.
    Empty,
    /// More than [`SECRET_MAX`] bytes.
    TooLong,
    /// A NUL byte: it would end the string PAM sees.
    Nul,
    /// stdin could not be read.
    Unreadable,
}

/// Read the secret from `input` until EOF.
///
/// At most `SECRET_MAX + 1` bytes are ever read: the extra byte is the proof
/// that the input was too long, and nothing after it is consumed.
///
/// # Errors
///
/// [`SecretError`] for an empty, oversized or NUL-bearing input, or a read
/// error.
pub fn read_secret<R: Read>(mut input: R) -> Result<Zeroizing<Vec<u8>>, SecretError> {
    // One allocation at its final size: reads fill it in place.
    let mut buf = Zeroizing::new(vec![0_u8; SECRET_MAX + 1]);
    let mut filled = 0;
    while filled < buf.len() {
        match input.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(SecretError::Unreadable),
        }
    }
    if filled > SECRET_MAX {
        return Err(SecretError::TooLong);
    }
    if filled == 0 {
        return Err(SecretError::Empty);
    }
    buf.truncate(filled);
    if buf.contains(&0) {
        return Err(SecretError::Nul);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader that hands out at most `step` bytes per call.
    struct Trickle<'a> {
        data: &'a [u8],
        step: usize,
        reads: usize,
    }

    impl Read for Trickle<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            let n = self.step.min(self.data.len()).min(out.len());
            out[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    #[test]
    fn t_v4_secret_bounds() {
        assert_eq!(read_secret(&b""[..]).unwrap_err(), SecretError::Empty);
        assert_eq!(read_secret(&b"a"[..]).unwrap().as_slice(), b"a");
        let exact = vec![b'x'; 512];
        assert_eq!(read_secret(&exact[..]).unwrap().len(), 512);
        let over = vec![b'x'; 513];
        assert_eq!(read_secret(&over[..]).unwrap_err(), SecretError::TooLong);
        let far_over = vec![b'x'; 100_000];
        assert_eq!(
            read_secret(&far_over[..]).unwrap_err(),
            SecretError::TooLong
        );
        assert_eq!(read_secret(&b"ab\0cd"[..]).unwrap_err(), SecretError::Nul);
        assert_eq!(read_secret(&b"\0"[..]).unwrap_err(), SecretError::Nul);
        // The secret is exactly as typed: a newline is part of it.
        assert_eq!(read_secret(&b"pw\n"[..]).unwrap().as_slice(), b"pw\n");
    }

    #[test]
    fn a_secret_of_exactly_512_multibyte_bytes_is_accepted_and_one_more_is_not() {
        // U+00F1 is two bytes in UTF-8; spelled as an escape, not a literal.
        let word = "\u{f1}".repeat(256);
        assert_eq!(word.len(), 512);
        assert_eq!(read_secret(word.as_bytes()).unwrap().len(), 512);
        let one_more = format!("{word}a");
        assert_eq!(
            read_secret(one_more.as_bytes()).unwrap_err(),
            SecretError::TooLong
        );
    }

    #[test]
    fn short_reads_are_joined_and_nothing_past_the_bound_is_consumed() {
        let data = vec![b'k'; 600];
        let mut reader = Trickle {
            data: &data,
            step: 7,
            reads: 0,
        };
        assert_eq!(read_secret(&mut reader).unwrap_err(), SecretError::TooLong);
        // 513 bytes were consumed at most; the rest stays unread.
        assert!(reader.data.len() >= 600 - 513 - 7, "{}", reader.data.len());
        let mut reader = Trickle {
            data: b"hello",
            step: 1,
            reads: 0,
        };
        assert_eq!(read_secret(&mut reader).unwrap().as_slice(), b"hello");
    }

    #[test]
    fn a_read_error_is_a_refusal() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
        assert_eq!(read_secret(Broken).unwrap_err(), SecretError::Unreadable);
    }

    #[test]
    fn an_interrupted_read_is_retried() {
        struct Once(bool);
        impl Read for Once {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Ok(0);
                }
                self.0 = true;
                Err(io::Error::from(io::ErrorKind::Interrupted))
            }
        }
        // Interrupted, then EOF: empty, not an unreadable input.
        assert_eq!(read_secret(Once(false)).unwrap_err(), SecretError::Empty);
    }

    #[test]
    fn the_buffer_is_allocated_once_at_its_final_size() {
        let secret = read_secret(&b"abc"[..]).unwrap();
        assert!(secret.capacity() > SECRET_MAX);
        assert_eq!(secret.as_slice(), b"abc");
    }
}
