//! The status lines on stdout (design §4.3 (b), §4.10).
//!
//! Lines are written by a thread of their own so the loop never blocks on a
//! pipe the shell is not reading. The queue is bounded: a line that finds it
//! full is dropped with a log (a hung shell is not the lock's problem). A
//! write error, `EPIPE` above all, is ignored for good: the shell died, the
//! lock goes on (design §4.1, TM-6). Nothing but [`StatusLine`]s is ever
//! written, and a line holds no secret.
#![forbid(unsafe_code)]

use std::io::Write;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use celestina_lock_proto::{render, StatusLine};

/// How many lines may wait for the writer.
const QUEUE: usize = 64;
/// How long a dropped reporter waits for its queued lines.
const DROP_WAIT: Duration = Duration::from_millis(500);

/// Writes status lines in order, off the loop.
///
/// The writer thread is **not** started by [`Reporter::new`]: the process
/// blocks its ignored signals in its loop thread before it
/// creates any other thread (a thread created earlier inherits no mask and a
/// signal would kill the lock, finding C1), so the thread is started by
/// [`Reporter::start`] once the loop exists. Lines queued meanwhile wait in
/// the bounded queue.
pub struct Reporter {
    tx: Option<SyncSender<StatusLine>>,
    done: Receiver<()>,
    /// The writer and the queue's far end, until [`Reporter::start`].
    idle: Option<Idle>,
    thread: Option<JoinHandle<()>>,
}

struct Idle {
    out: Box<dyn Write + Send>,
    rx: Receiver<StatusLine>,
    done: SyncSender<()>,
}

impl std::fmt::Debug for Reporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("started", &self.thread.is_some())
            .finish_non_exhaustive()
    }
}

impl Reporter {
    /// A reporter over `out` whose thread is not started yet.
    #[must_use]
    pub fn new(out: impl Write + Send + 'static) -> Self {
        let (tx, rx) = sync_channel::<StatusLine>(QUEUE);
        let (done_tx, done) = sync_channel::<()>(1);
        Self {
            tx: Some(tx),
            done,
            idle: Some(Idle {
                out: Box::new(out),
                rx,
                done: done_tx,
            }),
            thread: None,
        }
    }

    /// Starts the writer over `out` at once.
    ///
    /// # Errors
    ///
    /// When the thread cannot be created.
    pub fn spawn(out: impl Write + Send + 'static) -> std::io::Result<Self> {
        let mut reporter = Self::new(out);
        reporter.start()?;
        Ok(reporter)
    }

    /// Starts the writer thread; a second call does nothing.
    ///
    /// # Errors
    ///
    /// When the thread cannot be created; the lines are then dropped.
    pub fn start(&mut self) -> std::io::Result<()> {
        self.start_with(|builder, body| builder.spawn(body))
    }

    /// [`Reporter::start`] with the thread creation injected, so a test can
    /// make it fail.
    ///
    /// # Errors
    ///
    /// What `spawn` returns when it cannot create the thread.
    pub(crate) fn start_with(
        &mut self,
        spawn: impl FnOnce(thread::Builder, Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> std::io::Result<()> {
        let Some(Idle { mut out, rx, done }) = self.idle.take() else {
            return Ok(());
        };
        let body = Box::new(move || {
            let mut broken = false;
            for line in rx {
                if broken {
                    continue;
                }
                let mut text = render(&line);
                text.push('\n');
                if out
                    .write_all(text.as_bytes())
                    .and_then(|()| out.flush())
                    .is_err()
                {
                    broken = true;
                    tracing::warn!("the status pipe is closed: the shell is gone");
                }
            }
            let _ = done.send(());
        });
        let thread = spawn(thread::Builder::new().name("lock-report".to_owned()), body)?;
        self.thread = Some(thread);
        Ok(())
    }

    /// Queues one line; never blocks.
    pub fn send(&self, line: StatusLine) {
        let Some(tx) = &self.tx else { return };
        match tx.try_send(line) {
            Ok(()) => {}
            Err(TrySendError::Full(line)) => {
                tracing::warn!(?line, "the status queue is full: line dropped");
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    /// Closes the queue and waits up to `wait` for every queued line to be
    /// written.
    pub fn finish(mut self, wait: Duration) {
        self.close(wait);
    }

    fn close(&mut self, wait: Duration) {
        self.tx = None;
        if self.thread.is_none() {
            // Never started: nothing was written and nothing waits.
            return;
        }
        if self.done.recv_timeout(wait).is_ok() {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        } else {
            tracing::warn!("the status writer did not finish in time");
        }
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        if self.tx.is_some() {
            self.close(DROP_WAIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn lines_are_written_in_order_and_flushed_on_finish() {
        let sink = Sink::default();
        let reporter = Reporter::spawn(sink.clone()).expect("spawn");
        reporter.send(StatusLine::Hello(1));
        reporter.send(StatusLine::Requested);
        reporter.send(StatusLine::Locked);
        reporter.send(StatusLine::Unlocked);
        reporter.finish(Duration::from_secs(2));
        let written = sink.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(
            String::from_utf8(written).expect("ascii"),
            "hello 1\nrequested\nlocked\nunlocked\n"
        );
    }

    #[test]
    fn a_deferred_reporter_starts_no_thread_until_asked_and_loses_no_line() {
        let sink = Sink::default();
        let mut reporter = Reporter::new(sink.clone());
        reporter.send(StatusLine::Requested);
        assert!(reporter.thread.is_none(), "no thread before start");
        reporter.start().expect("start");
        reporter.start().expect("a second start does nothing");
        reporter.send(StatusLine::Locked);
        reporter.finish(Duration::from_secs(2));
        let written = sink.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(
            String::from_utf8(written).expect("ascii"),
            "requested\nlocked\n"
        );
    }

    #[test]
    fn a_writer_thread_that_cannot_be_created_is_an_error_not_a_silent_loss() {
        // Finding M-b: the caller must learn it, to refuse to lock.
        let mut reporter = Reporter::new(Sink::default());
        let err = reporter
            .start_with(|_, _| Err(std::io::Error::other("no threads")))
            .expect_err("the spawn failed");
        assert_eq!(err.to_string(), "no threads");
    }

    #[test]
    fn dropping_a_reporter_that_never_started_does_not_wait() {
        let started = std::time::Instant::now();
        drop(Reporter::new(Sink::default()));
        assert!(started.elapsed() < DROP_WAIT);
    }

    #[test]
    fn a_closed_pipe_is_ignored_and_the_reporter_still_finishes() {
        let reporter = Reporter::spawn(Broken).expect("spawn");
        for _ in 0..200 {
            reporter.send(StatusLine::Requested);
        }
        reporter.finish(Duration::from_secs(2));
    }
}
