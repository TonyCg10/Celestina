//! Bounded line framing in, one serialized writer out.
//!
//! Both directions of a helper's pipe are hostile until proven otherwise: the
//! host may send a line that never ends, and a helper with more than one
//! producing thread may interleave two frames into an unparseable one. The
//! reader here never buffers more than its limit and recovers on the next
//! line; the writer serializes whole frames, flushing each one, so a request
//! result can never land inside a snapshot.

use std::io::{self, BufRead, Read, Write};
use std::sync::{Mutex, MutexGuard};

/// A host line longer than this is hostile or corrupt. The framing recovers by
/// discarding it through its newline rather than desynchronizing for good.
pub const MAX_LINE_BYTES: usize = 4 * 1024;

/// The most a helper may put on one line going the other way.
///
/// This is the host's own line limit. The host discards a longer line and drops
/// every provider's reading with it, and a helper that built such a line would
/// build it again on the next change — an empty bar with a healthy helper. Every
/// field is bounded long before this, so reaching it means a frame is wrong
/// rather than large, and refusing it here keeps the channel synchronized.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum HostLine {
    Complete(Vec<u8>),
    /// A line past the limit was discarded whole; framing continues.
    Oversized,
    /// The host closed the pipe.
    End,
}

/// Reads one newline-terminated line, never buffering more than `MAX_LINE_BYTES`.
///
/// # Errors
///
/// Returns the reader's own error; a caller that cannot read its host has
/// nothing left to do but leave.
pub fn read_bounded_line<R: BufRead>(reader: &mut R) -> io::Result<HostLine> {
    let mut buffer = Vec::new();
    let limit = MAX_LINE_BYTES as u64;
    if reader
        .by_ref()
        .take(limit + 1)
        .read_until(b'\n', &mut buffer)?
        == 0
    {
        return Ok(HostLine::End);
    }

    if buffer.last() == Some(&b'\n') {
        return Ok(HostLine::Complete(buffer));
    }

    loop {
        let mut discarded = Vec::new();
        if reader
            .by_ref()
            .take(limit)
            .read_until(b'\n', &mut discarded)?
            == 0
        {
            return Ok(HostLine::End);
        }
        if discarded.last() == Some(&b'\n') {
            return Ok(HostLine::Oversized);
        }
    }
}

/// The single exit every frame of a helper passes through.
pub struct SharedWriter<W: Write> {
    inner: Mutex<W>,
}

impl<W: Write> SharedWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: Mutex::new(writer),
        }
    }

    /// A poisoned writer lock still owns a usable writer. Recovering it keeps
    /// the helper's frames flowing instead of taking it down with the thread
    /// that panicked — the documented mutex pattern of this suite.
    fn lock(&self) -> MutexGuard<'_, W> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Writes one JSON frame and its newline, then flushes, holding the lock
    /// for the whole frame.
    ///
    /// The frame is encoded before the lock is taken and measured before any of
    /// it is written, so an oversized frame is refused whole. Writing part of
    /// one and then failing would leave the host mid-line.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Encode`] or [`WriteError::TooLong`] for a frame
    /// that was never written, and [`WriteError::Write`] when the pipe itself
    /// failed. Only the last means the host is gone; see
    /// [`WriteError::is_fatal`].
    pub fn emit<T: serde::Serialize>(&self, value: &T) -> Result<(), WriteError> {
        let mut line = serde_json::to_vec(value).map_err(WriteError::Encode)?;
        if line.len() >= MAX_FRAME_BYTES {
            return Err(WriteError::TooLong(line.len()));
        }
        line.push(b'\n');

        let mut writer = self.lock();
        writer.write_all(&line).map_err(WriteError::Write)?;
        writer.flush().map_err(WriteError::Write)
    }
}

#[derive(Debug)]
pub enum WriteError {
    Encode(serde_json::Error),
    Write(io::Error),
    /// A frame the host would have discarded, carrying its size. Nothing was
    /// written.
    TooLong(usize),
}

impl WriteError {
    /// Whether this ends the helper.
    ///
    /// Losing the pipe does: there is nobody left to answer. A frame that could
    /// not be built or was too long does not — the helper still has a host, and
    /// leaving over one bad frame would take every working provider with it.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Write(_))
    }
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "cannot encode a frame: {error}"),
            Self::Write(error) => write!(formatter, "cannot write a frame: {error}"),
            Self::TooLong(bytes) => write!(
                formatter,
                "refused a {bytes}-byte frame the host would discard whole"
            ),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::TooLong(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn reads_lines_in_order() {
        let mut reader = Cursor::new(b"one\ntwo\n".to_vec());

        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::Complete(b"one\n".to_vec())
        );
        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::Complete(b"two\n".to_vec())
        );
        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::End
        );
    }

    #[test]
    fn an_oversized_line_is_discarded_and_the_next_one_survives() {
        let mut input = Vec::new();
        input.extend(std::iter::repeat_n(b'x', MAX_LINE_BYTES + 64));
        input.push(b'\n');
        input.extend_from_slice(b"after\n");
        let mut reader = Cursor::new(input);

        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::Oversized
        );
        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::Complete(b"after\n".to_vec())
        );
    }

    #[test]
    fn an_unterminated_oversized_line_ends_the_stream() {
        let input = vec![b'x'; MAX_LINE_BYTES + 8];
        let mut reader = Cursor::new(input);

        assert_eq!(
            read_bounded_line(&mut reader).expect("readable"),
            HostLine::End
        );
    }

    #[test]
    fn every_frame_leaves_whole_and_newline_terminated() {
        let writer = SharedWriter::new(Vec::new());
        writer
            .emit(&serde_json::json!({"kind": "one"}))
            .expect("emitted");
        writer
            .emit(&serde_json::json!({"kind": "two"}))
            .expect("emitted");

        let written = writer.lock().clone();
        assert_eq!(
            String::from_utf8(written).expect("utf-8"),
            "{\"kind\":\"one\"}\n{\"kind\":\"two\"}\n"
        );
    }
}
