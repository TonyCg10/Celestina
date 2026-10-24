// Written for the shell's energy duties (CEL-2-B) from the two copies that
// `idle.rs` and `nightlight.rs` used to carry: one bounded Wayland pump for
// both workers, so the flood cap, the sync deadline and the read rules have
// one owner. CEL-2-G widened its items from the energy module to the crate so
// the clipboard worker shares this pump instead of carrying a copy.

//! The shared bounded Wayland pump of the energy workers and the clipboard
//! worker.
//!
//! Each such worker owns its own Wayland connection. What is the same for
//! all of them lives here:
//!
//! - [`Link`] holds the connection, its event queue and its registry, and
//!   pumps it: dispatch, flush, one bounded `poll` and read, dispatch again;
//! - [`Link::sync`] waits for one display sync, at most until a deadline;
//! - [`Wire`] is the dispatch state: the worker's own state, the events it
//!   collects (at most a fixed number per dispatch, beyond which the
//!   compositor is treated as hostile) and the sync callback's progress;
//! - a full socket (`WouldBlock` on flush) or a wake without a whole message
//!   (`WouldBlock` on read) is retried on the next pump, bounded by the
//!   caller's deadline, never treated as a lost connection;
//! - [`offer`] is the bounded command queue's one refusal rule: full is
//!   busy, disconnected is gone.
//!
//! What differs per worker stays with the worker: its globals, its own
//! reasons to stop (passed to [`Link::pump`] and [`Link::sync`] as a check
//! over its state), and what a confirmed sync releases.
//!
//! The capture worker (`crate::capture`, CEL-2-L) opens its connection
//! through this same pump, so its flood cap and bounded waits are these.

use std::io::ErrorKind;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use wayland_client::backend::WaylandError;
use wayland_client::protocol::{wl_callback, wl_registry};
use wayland_client::{ConnectError, Connection, Dispatch, DispatchError, EventQueue, QueueHandle};

/// How many commands may wait for a worker. A full queue is a stuck worker,
/// and the command is refused rather than queued behind it.
pub const COMMAND_QUEUE: usize = 4;
/// The reason reported when the session offers no connection or not the
/// globals a duty needs.
pub const UNAVAILABLE: &str = "unavailable";
/// How often a worker looks at its queue, its connection and the shutdown
/// flag.
pub(crate) const POLL: Duration = Duration::from_millis(25);
/// How long the compositor has to answer one display sync.
pub(crate) const SYNC_TIMEOUT: Duration = Duration::from_secs(1);

/// The connection itself is gone or cannot be trusted.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Lost {
    #[error("cannot connect to Wayland")]
    Connect(#[source] ConnectError),
    #[error("cannot flush the Wayland connection")]
    Flush(#[source] WaylandError),
    #[error("cannot read Wayland events")]
    Read(#[source] WaylandError),
    #[error("cannot wait for Wayland events")]
    Poll(#[source] Errno),
    #[error("cannot dispatch Wayland events")]
    Dispatch(#[source] DispatchError),
    #[error("the compositor did not answer a sync within {} ms", SYNC_TIMEOUT.as_millis())]
    Timeout,
    #[error("the compositor sent more than {0} events at once")]
    Flood(usize),
}

/// A worker's two refusals of a command, named in its own words.
pub(crate) trait QueueRefusal {
    /// [`COMMAND_QUEUE`] commands are already waiting.
    const BUSY: Self;
    /// The worker has stopped.
    const GONE: Self;
}

/// Offers `command` to a worker's bounded queue without waiting.
pub(crate) fn offer<T, R: QueueRefusal>(queue: &SyncSender<T>, command: T) -> Result<(), R> {
    queue.try_send(command).map_err(|error| match error {
        TrySendError::Full(_) => R::BUSY,
        TrySendError::Disconnected(_) => R::GONE,
    })
}

/// Whether `error` only says "not now": the socket is full, or it woke
/// without a whole message.
fn retry_later(error: &WaylandError) -> bool {
    matches!(error, WaylandError::Io(io) if io.kind() == ErrorKind::WouldBlock)
}

/// What a flush means: a full socket keeps the rest for the next pump.
fn flushed(result: Result<(), WaylandError>) -> Result<(), Lost> {
    match result {
        Err(error) if !retry_later(&error) => Err(Lost::Flush(error)),
        Ok(()) | Err(_) => Ok(()),
    }
}

/// What a read means: a wake without a whole message (libwayland reports
/// `EAGAIN` when the socket turns out empty) is read on a later pump.
fn was_read(result: Result<usize, WaylandError>) -> Result<(), Lost> {
    match result {
        Err(error) if !retry_later(&error) => Err(Lost::Read(error)),
        Ok(_) | Err(_) => Ok(()),
    }
}

/// The dispatch state of one worker's connection.
pub(crate) struct Wire<S, E> {
    /// The worker's own state.
    pub(crate) state: S,
    events: Vec<E>,
    limit: usize,
    flooded: bool,
    completed_sync: u64,
}

impl<S, E> Wire<S, E> {
    fn new(state: S, limit: usize) -> Self {
        Self {
            state,
            events: Vec::new(),
            limit,
            flooded: false,
            completed_sync: 0,
        }
    }

    /// Keeps `event`, or marks the connection flooded once `limit` events
    /// are already waiting.
    pub(crate) fn push(&mut self, event: E) {
        if self.events.len() < self.limit {
            self.events.push(event);
        } else {
            self.flooded = true;
        }
    }

    /// The events collected since the last call.
    pub(crate) fn take_events(&mut self) -> Vec<E> {
        std::mem::take(&mut self.events)
    }

    /// Forgets the events collected so far.
    pub(crate) fn clear_events(&mut self) {
        self.events.clear();
    }
}

impl<S: 'static, E: 'static> Dispatch<wl_callback::WlCallback, u64> for Wire<S, E> {
    fn event(
        wire: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        token: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            wire.completed_sync = wire.completed_sync.max(*token);
        }
    }
}

/// One worker's own connection, pumped with bounded waits.
pub(crate) struct Link<S: 'static, E: 'static> {
    connection: Connection,
    queue: EventQueue<Wire<S, E>>,
    pub(crate) qh: QueueHandle<Wire<S, E>>,
    _registry: wl_registry::WlRegistry,
    pub(crate) wire: Wire<S, E>,
    next_sync: u64,
}

impl<S: 'static, E: 'static> Link<S, E>
where
    Wire<S, E>: Dispatch<wl_registry::WlRegistry, ()>,
{
    /// Opens a connection to the session's compositor and asks for its
    /// registry; the globals arrive with the first [`Link::sync`]. At most
    /// `limit` events are kept per dispatch.
    pub(crate) fn connect(state: S, limit: usize) -> Result<Self, Lost> {
        let connection = Connection::connect_to_env().map_err(Lost::Connect)?;
        Ok(Self::over(connection, state, limit))
    }

    fn over(connection: Connection, state: S, limit: usize) -> Self {
        let queue = connection.new_event_queue();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        Self {
            connection,
            queue,
            qh,
            _registry: registry,
            wire: Wire::new(state, limit),
            next_sync: 0,
        }
    }

    /// Sends what is queued. A full socket keeps the rest for the next pump.
    pub(crate) fn flush(&mut self) -> Result<(), Lost> {
        flushed(self.queue.flush())
    }

    /// Dispatches what is waiting, waits at most `timeout` for more, and
    /// dispatches that. `check` is the worker's own reason to stop, looked at
    /// before the flood cap.
    pub(crate) fn pump<L: From<Lost>>(
        &mut self,
        timeout: Duration,
        check: impl Fn(&S) -> Result<(), L>,
    ) -> Result<(), L> {
        self.queue
            .dispatch_pending(&mut self.wire)
            .map_err(Lost::Dispatch)?;
        self.flush()?;
        if let Some(guard) = self.queue.prepare_read() {
            let fd = guard.connection_fd();
            let mut fds = [PollFd::new(&fd, PollFlags::IN)];
            let timeout = Timespec::try_from(timeout).unwrap_or(Timespec {
                tv_sec: 0,
                tv_nsec: 25_000_000,
            });
            match poll(&mut fds, Some(&timeout)) {
                Ok(count) if count > 0 => was_read(guard.read())?,
                Ok(_) | Err(Errno::INTR) => drop(guard),
                Err(error) => return Err(Lost::Poll(error).into()),
            }
        }
        self.queue
            .dispatch_pending(&mut self.wire)
            .map_err(Lost::Dispatch)?;
        check(&self.wire.state)?;
        if self.wire.flooded {
            return Err(Lost::Flood(self.wire.limit).into());
        }
        Ok(())
    }

    /// Waits until the compositor has handled everything sent so far, at
    /// most until `deadline`, stopping early when `check` does.
    pub(crate) fn sync<L: From<Lost>>(
        &mut self,
        deadline: Instant,
        check: impl Fn(&S) -> Result<(), L>,
    ) -> Result<(), L> {
        self.next_sync = self.next_sync.wrapping_add(1).max(1);
        let token = self.next_sync;
        self.connection.display().sync(&self.qh, token);
        self.flush()?;
        while self.wire.completed_sync < token {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or(Lost::Timeout)?;
            self.pump(remaining.min(POLL), &check)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;
    use std::thread;

    struct Bare;

    wayland_client::delegate_noop!(Wire<Bare, ()>: ignore wl_registry::WlRegistry);

    /// `wl_callback.done` for the first callback a fresh connection creates:
    /// object 1 is the display, 2 the registry, 3 the sync callback.
    fn done_for_first_sync() -> Vec<u8> {
        [3_u32, 12 << 16, 0]
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect()
    }

    fn no_check(_: &Bare) -> Result<(), Lost> {
        Ok(())
    }

    /// A connection whose other end is the test, standing in for the
    /// compositor.
    fn pair() -> (Link<Bare, ()>, UnixStream) {
        let (client, peer) = UnixStream::pair().expect("a socket pair");
        // The stand-in compositor threads read with `read_exact`: a bounded
        // read turns a request that never comes into a failure instead of a
        // test that never ends (round 10).
        peer.set_read_timeout(Some(Duration::from_secs(20)))
            .expect("a read deadline");
        let connection = Connection::from_socket(client).expect("a connection over it");
        (Link::over(connection, Bare, 8), peer)
    }

    fn io(kind: ErrorKind) -> WaylandError {
        WaylandError::Io(io::Error::from(kind))
    }

    #[test]
    fn would_block_is_retried_on_flush_and_read_and_anything_else_is_lost() {
        assert!(flushed(Ok(())).is_ok());
        assert!(flushed(Err(io(ErrorKind::WouldBlock))).is_ok());
        assert!(matches!(
            flushed(Err(io(ErrorKind::BrokenPipe))),
            Err(Lost::Flush(_))
        ));

        assert!(was_read(Ok(3)).is_ok());
        assert!(was_read(Err(io(ErrorKind::WouldBlock))).is_ok());
        assert!(matches!(
            was_read(Err(io(ErrorKind::ConnectionReset))),
            Err(Lost::Read(_))
        ));
    }

    #[test]
    fn a_full_socket_is_retried_and_only_the_sync_deadline_ends_it() {
        let (client, peer) = UnixStream::pair().expect("a socket pair");
        // A compositor that reads nothing: the socket is full before the
        // connection writes its first request.
        client.set_nonblocking(true).expect("a non-blocking socket");
        let junk = [0_u8; 4096];
        let mut filler = &client;
        loop {
            match filler.write(&junk) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("cannot fill the socket: {error}"),
            }
        }
        let connection = Connection::from_socket(client).expect("a connection over it");
        let mut link: Link<Bare, ()> = Link::over(connection, Bare, 8);

        assert!(link.flush().is_ok(), "a full socket is not a lost one");
        let outcome = link.sync(Instant::now() + Duration::from_millis(100), no_check);
        assert!(matches!(outcome, Err(Lost::Timeout)), "{outcome:?}");
        drop(peer);
    }

    /// Half a message wakes the poll; the pump must keep the half and finish
    /// the sync once the rest arrives, not give the connection up.
    #[test]
    fn a_wake_without_a_whole_message_is_read_on_a_later_pump() {
        let (mut link, mut peer) = pair();
        let compositor = thread::spawn(move || {
            // get_registry and sync, twelve bytes each.
            let mut requests = [0_u8; 24];
            peer.read_exact(&mut requests).expect("both requests");
            let done = done_for_first_sync();
            peer.write_all(&done[..4]).expect("half a header");
            thread::sleep(Duration::from_millis(60));
            peer.write_all(&done[4..]).expect("the rest");
            peer
        });

        let outcome = link.sync(Instant::now() + Duration::from_secs(5), no_check);
        assert!(outcome.is_ok(), "{outcome:?}");
        assert!(compositor.join().is_ok());
    }

    #[test]
    fn a_silent_compositor_times_out_the_sync() {
        let (mut link, peer) = pair();
        let outcome = link.sync(Instant::now() + Duration::from_millis(60), no_check);
        assert!(matches!(outcome, Err(Lost::Timeout)), "{outcome:?}");
        drop(peer);
    }

    #[derive(Debug, thiserror::Error)]
    enum Hooked {
        #[error("withdrawn")]
        Withdrawn,
        #[error(transparent)]
        Wire(#[from] Lost),
    }

    #[test]
    fn the_worker_check_stops_the_sync() {
        let (mut link, mut peer) = pair();
        let compositor = thread::spawn(move || {
            let mut requests = [0_u8; 24];
            peer.read_exact(&mut requests).expect("both requests");
            peer.write_all(&done_for_first_sync()).expect("the answer");
            peer
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let outcome = link.sync(deadline, |_: &Bare| Err::<(), _>(Hooked::Withdrawn));
        assert!(matches!(outcome, Err(Hooked::Withdrawn)), "{outcome:?}");
        assert!(compositor.join().is_ok());
    }

    #[test]
    fn events_beyond_the_limit_flood_the_wire() {
        let mut wire: Wire<Bare, u32> = Wire::new(Bare, 2);
        wire.push(1);
        wire.push(2);
        assert!(!wire.flooded);
        wire.push(3);
        assert!(wire.flooded);
        assert_eq!(wire.take_events(), [1, 2]);
        wire.push(4);
        wire.clear_events();
        assert!(wire.take_events().is_empty());
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Said {
        Busy,
        Gone,
    }

    impl QueueRefusal for Said {
        const BUSY: Self = Self::Busy;
        const GONE: Self = Self::Gone;
    }

    #[test]
    fn a_full_queue_is_busy_and_a_dropped_one_is_gone() {
        let (queue, worker) = mpsc::sync_channel(1);
        assert_eq!(offer::<_, Said>(&queue, 1), Ok(()));
        assert_eq!(offer::<_, Said>(&queue, 2), Err(Said::Busy));
        drop(worker);
        assert_eq!(offer::<_, Said>(&queue, 3), Err(Said::Gone));
    }
}
