//! The lines `celestina-lock` prints on stdout (design §4.3 (b)).
//!
//! ASCII, one line per event, `\n`-terminated, at most [`MAX_LINE`] bytes
//! before the terminator. The shell reads them through a [`LineReader`] that
//! never holds more than one line, drops an overlong one with a report, and
//! never acts on a line whose terminator has not arrived. An unknown line is
//! reported and ignored, so a newer lock can say more than an older shell
//! understands.

use std::error::Error;
use std::fmt;

/// The longest line, in bytes, not counting the `\n`.
pub const MAX_LINE: usize = 64;

/// Why an attempt ended without unlocking, as the lock reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptKind {
    /// The password was wrong (counts for the delay).
    Refused,
    /// The password was right but the account was refused (counts).
    Account,
    /// PAM could not decide or the verifier failed.
    Unavailable,
    /// The verifier did not answer in time.
    Timeout,
    /// The stack asked a prompt the lock cannot answer.
    Unsupported,
    /// The PAM service file is not installed.
    ServiceMissing,
}

impl AttemptKind {
    fn token(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Account => "account",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Unsupported => "unsupported",
            Self::ServiceMissing => "service-missing",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        Some(match token {
            "refused" => Self::Refused,
            "account" => Self::Account,
            "unavailable" => Self::Unavailable,
            "timeout" => Self::Timeout,
            "unsupported" => Self::Unsupported,
            "service-missing" => Self::ServiceMissing,
            _ => return None,
        })
    }
}

/// One event the lock reports to the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusLine {
    /// `hello <version>`: connected, the session-lock manager is bound.
    Hello(u16),
    /// `requested`: `lock()` was sent and a surface exists for every output.
    Requested,
    /// `locked`: the compositor presented a locked frame everywhere.
    Locked,
    /// `attempt <kind>`: an attempt ended without unlocking.
    Attempt(AttemptKind),
    /// `unlocked`: the unlock was sent and a sync round trip returned.
    Unlocked,
}

/// Why a line was not understood. None of these ever changes a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineError {
    /// More than [`MAX_LINE`] bytes.
    TooLong,
    /// A byte outside printable ASCII (a NUL, a control byte, a terminator,
    /// anything above 0x7e).
    NotPrintable,
    /// Well formed bytes that are not a line of this protocol.
    Unknown,
}

impl fmt::Display for LineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLong => "a status line longer than the bound",
            Self::NotPrintable => "a status line with a byte outside printable ASCII",
            Self::Unknown => "a status line this protocol does not know",
        })
    }
}

impl Error for LineError {}

/// Reads one line, without its terminator.
///
/// # Errors
///
/// [`LineError::TooLong`] past [`MAX_LINE`] bytes, checked first;
/// [`LineError::NotPrintable`] for a byte outside `0x20..=0x7e`;
/// [`LineError::Unknown`] for anything that is not exactly one of the words.
pub fn parse_line(line: &[u8]) -> Result<StatusLine, LineError> {
    if line.len() > MAX_LINE {
        return Err(LineError::TooLong);
    }
    if line.iter().any(|byte| !(0x20..=0x7e).contains(byte)) {
        return Err(LineError::NotPrintable);
    }
    // Printable ASCII is valid UTF-8; the fallback is unreachable.
    let text = std::str::from_utf8(line).map_err(|_| LineError::NotPrintable)?;
    match text {
        "requested" => return Ok(StatusLine::Requested),
        "locked" => return Ok(StatusLine::Locked),
        "unlocked" => return Ok(StatusLine::Unlocked),
        _ => {}
    }
    match text.split_once(' ') {
        Some(("hello", version)) => parse_version(version)
            .map(StatusLine::Hello)
            .ok_or(LineError::Unknown),
        Some(("attempt", kind)) => AttemptKind::from_token(kind)
            .map(StatusLine::Attempt)
            .ok_or(LineError::Unknown),
        _ => Err(LineError::Unknown),
    }
}

/// A canonical decimal `u16`: digits only, no sign, no leading zero.
fn parse_version(text: &str) -> Option<u16> {
    let canonical = !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text.len() == 1 || !text.starts_with('0'));
    if canonical {
        text.parse().ok()
    } else {
        None
    }
}

/// The text of a line, without the `\n` the writer appends.
#[must_use]
pub fn render(line: &StatusLine) -> String {
    match line {
        StatusLine::Hello(version) => format!("hello {version}"),
        StatusLine::Requested => "requested".to_owned(),
        StatusLine::Locked => "locked".to_owned(),
        StatusLine::Attempt(kind) => format!("attempt {}", kind.token()),
        StatusLine::Unlocked => "unlocked".to_owned(),
    }
}

/// Splits a byte stream into lines, keeping at most one line of it.
///
/// A line is acted on only once its `\n` has arrived: a partial line is held,
/// and discarded with the reader if the stream ends. A line that grows past
/// [`MAX_LINE`] is reported once, then skipped up to its terminator, after
/// which reading resumes cleanly.
#[derive(Debug, Default)]
pub struct LineReader {
    pending: Vec<u8>,
    discarding: bool,
}

impl LineReader {
    /// How much to read from the pipe at once.
    pub const CHUNK: usize = 4096;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many bytes of an unfinished line are held; never more than
    /// [`MAX_LINE`].
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.pending.len()
    }

    /// Takes the next bytes of the stream and returns what each completed
    /// line made of them, in order.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Result<StatusLine, LineError>> {
        let mut lines = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                if !self.discarding {
                    lines.push(parse_line(&self.pending));
                }
                self.pending.clear();
                self.discarding = false;
            } else if self.discarding {
                // The rest of an overlong line: already reported.
            } else if self.pending.len() == MAX_LINE {
                self.pending.clear();
                self.discarding = true;
                lines.push(Err(LineError::TooLong));
            } else {
                self.pending.push(byte);
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_KIND: [AttemptKind; 6] = [
        AttemptKind::Refused,
        AttemptKind::Account,
        AttemptKind::Unavailable,
        AttemptKind::Timeout,
        AttemptKind::Unsupported,
        AttemptKind::ServiceMissing,
    ];

    fn every_line() -> Vec<StatusLine> {
        let mut lines = vec![
            StatusLine::Hello(1),
            StatusLine::Hello(0),
            StatusLine::Hello(u16::MAX),
            StatusLine::Requested,
            StatusLine::Locked,
            StatusLine::Unlocked,
        ];
        lines.extend(EVERY_KIND.map(StatusLine::Attempt));
        lines
    }

    #[test]
    fn status_line_round_trip_for_every_line() {
        for line in every_line() {
            let text = render(&line);
            assert!(text.len() <= MAX_LINE, "{text}");
            assert!(!text.contains('\n'), "the writer appends the terminator");
            assert_eq!(parse_line(text.as_bytes()), Ok(line.clone()), "{text}");
        }
        assert_eq!(
            render(&StatusLine::Hello(crate::PROTOCOL_VERSION)),
            "hello 1"
        );
        assert_eq!(
            render(&StatusLine::Attempt(AttemptKind::ServiceMissing)),
            "attempt service-missing"
        );
    }

    #[test]
    fn the_wire_words_are_the_specs() {
        let words = [
            ("hello 1", StatusLine::Hello(1)),
            ("requested", StatusLine::Requested),
            ("locked", StatusLine::Locked),
            ("unlocked", StatusLine::Unlocked),
            ("attempt refused", StatusLine::Attempt(AttemptKind::Refused)),
            ("attempt account", StatusLine::Attempt(AttemptKind::Account)),
            (
                "attempt unavailable",
                StatusLine::Attempt(AttemptKind::Unavailable),
            ),
            ("attempt timeout", StatusLine::Attempt(AttemptKind::Timeout)),
            (
                "attempt unsupported",
                StatusLine::Attempt(AttemptKind::Unsupported),
            ),
            (
                "attempt service-missing",
                StatusLine::Attempt(AttemptKind::ServiceMissing),
            ),
        ];
        for (text, line) in words {
            assert_eq!(parse_line(text.as_bytes()), Ok(line), "{text}");
        }
    }

    #[test]
    fn status_line_of_64_bytes_ok_65_refused_nul_refused() {
        // No valid line is 64 bytes long, so "ok" means the length bound lets
        // it through to the grammar, which does not know it.
        let sixty_four = vec![b'a'; 64];
        assert_eq!(parse_line(&sixty_four), Err(LineError::Unknown));
        let sixty_five = vec![b'a'; 65];
        assert_eq!(parse_line(&sixty_five), Err(LineError::TooLong));
        let mut with_nul = b"locked".to_vec();
        with_nul.insert(3, 0);
        assert_eq!(parse_line(&with_nul), Err(LineError::NotPrintable));
        assert_eq!(parse_line(b"locked\0"), Err(LineError::NotPrintable));
        // A valid word padded to 64 and 65 bytes by trailing spaces.
        let mut padded = b"locked".to_vec();
        padded.resize(64, b' ');
        assert_eq!(parse_line(&padded), Err(LineError::Unknown));
        padded.push(b' ');
        assert_eq!(parse_line(&padded), Err(LineError::TooLong));
    }

    #[test]
    fn a_line_that_is_not_exactly_a_word_is_unknown() {
        for text in [
            "",
            " ",
            "Locked",
            "locked ",
            " locked",
            "locked now",
            "hello",
            "hello ",
            "hello  1",
            "hello 01",
            "hello 1 ",
            "hello +1",
            "hello -1",
            "hello 65536",
            "hello 999999",
            "hello one",
            "attempt",
            "attempt ",
            "attempt Refused",
            "attempt refused ",
            "attempt granted",
            "attempt refused refused",
            "unlock",
        ] {
            assert_eq!(
                parse_line(text.as_bytes()),
                Err(LineError::Unknown),
                "{text:?}"
            );
        }
    }

    #[test]
    fn bytes_outside_printable_ascii_are_refused() {
        for byte in (0u8..0x20).chain(0x7f..=0xff) {
            let line = [b'l', b'o', b'c', byte, b'e', b'd'];
            assert_eq!(parse_line(&line), Err(LineError::NotPrintable), "{byte}");
        }
        // A terminator is not part of a line.
        assert_eq!(parse_line(b"locked\n"), Err(LineError::NotPrintable));
        assert_eq!(
            parse_line("locked\u{e9}".as_bytes()),
            Err(LineError::NotPrintable)
        );
    }

    #[test]
    fn the_length_is_checked_before_anything_else() {
        let mut line = vec![0u8; 65];
        assert_eq!(parse_line(&line), Err(LineError::TooLong));
        line.truncate(64);
        assert_eq!(parse_line(&line), Err(LineError::NotPrintable));
    }

    fn feed_all(reader: &mut LineReader, chunks: &[&[u8]]) -> Vec<Result<StatusLine, LineError>> {
        chunks.iter().flat_map(|chunk| reader.feed(chunk)).collect()
    }

    #[test]
    fn the_reader_splits_lines_across_chunks_and_never_acts_on_a_partial_one() {
        let mut reader = LineReader::new();
        assert!(reader.feed(b"hello").is_empty());
        assert_eq!(reader.feed(b" 1\nreq"), [Ok(StatusLine::Hello(1))]);
        assert!(reader.feed(b"uested").is_empty());
        assert_eq!(
            reader.feed(b"\nlocked\n"),
            [Ok(StatusLine::Requested), Ok(StatusLine::Locked)]
        );
        // A line with no newline is not a line, however complete it looks.
        assert!(reader.feed(b"unlocked").is_empty());
        assert!(reader.feed(b"").is_empty());
    }

    #[test]
    fn line_reader_drops_an_overlong_line_and_resynchronises() {
        let mut reader = LineReader::new();
        let mut long = vec![b'x'; 64];
        long.push(b'x');
        long.push(b'\n');
        long.extend_from_slice(b"locked\n");
        // The overlong line is reported once, then the next line reads clean.
        assert_eq!(
            reader.feed(&long),
            [Err(LineError::TooLong), Ok(StatusLine::Locked)]
        );

        // The same across chunks, with the tail of the long line arriving in
        // pieces and never reported twice.
        let results = feed_all(
            &mut reader,
            &[&[b'y'; 100], &[b'y'; 100], b"zz\nunlocked\n"],
        );
        assert_eq!(results, [Err(LineError::TooLong), Ok(StatusLine::Unlocked)]);

        // A line of exactly 64 bytes is not overlong.
        let mut sixty_four = vec![b'q'; 64];
        sixty_four.push(b'\n');
        assert_eq!(reader.feed(&sixty_four), [Err(LineError::Unknown)]);
    }

    #[test]
    fn an_overlong_line_without_any_newline_is_reported_once_and_buffers_nothing() {
        let mut reader = LineReader::new();
        let chunk = vec![b'a'; LineReader::CHUNK];
        let mut reported = 0;
        for _ in 0..64 {
            reported += reader.feed(&chunk).len();
        }
        assert_eq!(reported, 1);
        assert!(reader.buffered() <= MAX_LINE);
        assert_eq!(reader.feed(b"\nlocked\n"), [Ok(StatusLine::Locked)]);
    }

    #[test]
    fn a_nul_in_a_line_is_reported_and_the_next_line_is_clean() {
        let mut reader = LineReader::new();
        assert_eq!(
            reader.feed(b"lo\0cked\nlocked\n"),
            [Err(LineError::NotPrintable), Ok(StatusLine::Locked)]
        );
    }

    #[test]
    fn an_empty_line_is_unknown_and_changes_nothing_after_it() {
        let mut reader = LineReader::new();
        assert_eq!(
            reader.feed(b"\n\nrequested\n"),
            [
                Err(LineError::Unknown),
                Err(LineError::Unknown),
                Ok(StatusLine::Requested)
            ]
        );
    }

    #[test]
    fn the_reader_buffer_never_exceeds_one_line() {
        let mut reader = LineReader::new();
        for byte in 0..=255u8 {
            let _ = reader.feed(&[byte]);
            assert!(reader.buffered() <= MAX_LINE);
        }
    }
}
