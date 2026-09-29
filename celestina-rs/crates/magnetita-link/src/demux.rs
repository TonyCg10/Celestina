//! The bulk streams a peer opens, told apart by the transfer id each one
//! starts with.
//!
//! One task per connection only accepts streams; each stream's 4-byte id is
//! read by a task of its own, at most [`HEADER_READS`] at a time and each
//! within [`STREAM_HEADER_BUDGET`], so a peer that opens silent streams
//! delays no other stream. A stream whose id someone [`Demux::expect`]s goes
//! to that waiter. Any other goes to the queue [`Demux::next`] reads (the
//! desktop's way), which holds at most [`QUEUE`] streams, well under the
//! connection's stream limit, and past it a stream nobody takes is stopped;
//! or, on a side that reads only reserved streams ([`Demux::reserve_only`],
//! the phone's way), it is stopped at once. A queued stream nobody reads
//! keeps one of the peer's stream slots, so neither way lets unread streams
//! fill them.
//!
//! The task starts on first use and ends with the connection or with the
//! last handle to the session's streams.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::error::LinkError;

/// How long a new bulk stream may take to name its transfer: the id is
/// written as the stream opens, so a stream silent past this is refused.
pub const STREAM_HEADER_BUDGET: Duration = Duration::from_secs(2);

/// Stream ids read at once; a further stream waits for a slot.
const HEADER_READS: usize = 8;

/// Streams that wait for [`Demux::next`] before new ones are stopped; under
/// the connection's 16 concurrent streams, so a reader that falls behind
/// still leaves room for reserved ones.
const QUEUE: usize = 8;

/// Transfers that may wait for their stream at once.
const MAX_EXPECTED: usize = 64;

/// One bulk stream the peer opened.
#[derive(Debug)]
pub enum Accepted {
    /// A stream and the transfer it names.
    Stream(u32, quinn::RecvStream),
    /// Reset, ended or silent before its transfer id; why, for a log. The
    /// connection goes on.
    Skipped(String),
}

type Waiters = Arc<Mutex<HashMap<u32, (u64, oneshot::Sender<quinn::RecvStream>)>>>;

/// The demultiplexer of one connection, shared by every handle to it.
pub(crate) struct Demux {
    conn: quinn::Connection,
    waiters: Waiters,
    reserve_only: Arc<AtomicBool>,
    next_waiter: std::sync::atomic::AtomicU64,
    started: OnceLock<Started>,
}

struct Started {
    queue: tokio::sync::Mutex<mpsc::Receiver<Result<Accepted, LinkError>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Demux {
    fn drop(&mut self) {
        if let Some(started) = self.started.get() {
            started.task.abort();
        }
        self.waiters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

impl Demux {
    pub(crate) fn new(conn: quinn::Connection) -> Self {
        Self {
            conn,
            waiters: Arc::default(),
            reserve_only: Arc::default(),
            next_waiter: std::sync::atomic::AtomicU64::new(1),
            started: OnceLock::new(),
        }
    }

    /// Starts the accept task once. Must run inside the connection's runtime.
    fn started(&self) -> &Started {
        self.started.get_or_init(|| {
            let (tx, rx) = mpsc::channel(QUEUE);
            let task = tokio::spawn(run(
                self.conn.clone(),
                tx,
                Arc::clone(&self.waiters),
                Arc::clone(&self.reserve_only),
            ));
            Started {
                queue: tokio::sync::Mutex::new(rx),
                task,
            }
        })
    }

    /// The next stream nobody expects, or a skipped one; an error once the
    /// connection has ended.
    pub(crate) async fn next(&self) -> Result<Accepted, LinkError> {
        let started = self.started();
        let mut queue = started.queue.lock().await;
        queue.recv().await.unwrap_or(Err(LinkError::Closed))
    }

    /// Stops unreserved streams at once instead of queueing them.
    pub(crate) fn reserve_only(&self) {
        self.reserve_only.store(true, Ordering::Relaxed);
    }

    /// Reserves the stream of `transfer` before the peer is told to send it,
    /// so it reaches this waiter whenever it arrives and in whatever order.
    /// Must run inside the connection's runtime.
    pub(crate) fn expect(&self, transfer: u32) -> Result<Expected, LinkError> {
        self.started();
        let (tx, rx) = oneshot::channel();
        let generation = self
            .next_waiter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut waiters = self.waiters.lock().unwrap_or_else(PoisonError::into_inner);
        if waiters.contains_key(&transfer) {
            return Err(LinkError::Refused(
                "this transfer's stream is already awaited",
            ));
        }
        if waiters.len() >= MAX_EXPECTED {
            return Err(LinkError::Refused(
                "too many transfers wait for their stream",
            ));
        }
        waiters.insert(transfer, (generation, tx));
        Ok(Expected {
            transfer,
            generation,
            stream: rx,
            waiters: Arc::clone(&self.waiters),
        })
    }
}

/// A transfer's stream, reserved by [`Demux::expect`]; dropping it gives up
/// the reservation.
#[derive(Debug)]
pub struct Expected {
    transfer: u32,
    generation: u64,
    stream: oneshot::Receiver<quinn::RecvStream>,
    waiters: Waiters,
}

impl Expected {
    /// Waits for the stream; an error when the connection ends first.
    pub async fn stream(mut self) -> Result<quinn::RecvStream, LinkError> {
        (&mut self.stream).await.map_err(|_| LinkError::Closed)
    }
}

impl Drop for Expected {
    fn drop(&mut self) {
        let mut waiters = self.waiters.lock().unwrap_or_else(PoisonError::into_inner);
        if waiters
            .get(&self.transfer)
            .is_some_and(|(generation, _)| *generation == self.generation)
        {
            waiters.remove(&self.transfer);
        }
    }
}

/// Accepts streams until the connection or the queue's reader ends; each
/// stream's id is read by a task of its own under a slot.
async fn run(
    conn: quinn::Connection,
    tx: mpsc::Sender<Result<Accepted, LinkError>>,
    waiters: Waiters,
    reserve_only: Arc<AtomicBool>,
) {
    let slots = Arc::new(Semaphore::new(HEADER_READS));
    loop {
        let Ok(slot) = Arc::clone(&slots).acquire_owned().await else {
            break;
        };
        let accepted = tokio::select! {
            accepted = conn.accept_uni() => accepted,
            () = tx.closed() => break,
        };
        match accepted {
            Ok(stream) => {
                let tx = tx.clone();
                let waiters = Arc::clone(&waiters);
                let reserve_only = reserve_only.load(Ordering::Relaxed);
                tokio::spawn(async move {
                    let item = header(stream).await;
                    drop(slot);
                    deliver(item, &tx, &waiters, reserve_only);
                });
            }
            Err(e) => {
                let _ = tx.send(Err(e.into())).await;
                break;
            }
        }
    }
    // Nothing more arrives: every waiter learns the connection ended.
    waiters
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// Reads one stream's transfer id within the budget.
async fn header(mut stream: quinn::RecvStream) -> Result<Accepted, LinkError> {
    let mut id = [0u8; 4];
    match tokio::time::timeout(STREAM_HEADER_BUDGET, stream.read_exact(&mut id)).await {
        Ok(Ok(())) => Ok(Accepted::Stream(u32::from_be_bytes(id), stream)),
        Ok(Err(quinn::ReadExactError::ReadError(quinn::ReadError::ConnectionLost(e)))) => {
            Err(e.into())
        }
        Ok(Err(e)) => Ok(Accepted::Skipped(e.to_string())),
        Err(_) => {
            let _ = stream.stop(0u32.into());
            Ok(Accepted::Skipped("no transfer id in time".into()))
        }
    }
}

/// Hands a stream to the waiter of its transfer, else to the queue unless
/// only reserved streams are taken; a stream neither can take is stopped.
fn deliver(
    item: Result<Accepted, LinkError>,
    tx: &mpsc::Sender<Result<Accepted, LinkError>>,
    waiters: &Waiters,
    reserve_only: bool,
) {
    let item = match item {
        Ok(Accepted::Stream(id, stream)) => {
            let waiter = waiters
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id);
            match waiter {
                Some((_, waiter)) => {
                    if let Err(mut stream) = waiter.send(stream) {
                        let _ = stream.stop(0u32.into());
                    }
                    return;
                }
                None if reserve_only => {
                    let mut stream = stream;
                    let _ = stream.stop(0u32.into());
                    return;
                }
                None => Ok(Accepted::Stream(id, stream)),
            }
        }
        other => other,
    };
    if let Err(
        mpsc::error::TrySendError::Full(Ok(Accepted::Stream(_, mut stream)))
        | mpsc::error::TrySendError::Closed(Ok(Accepted::Stream(_, mut stream))),
    ) = tx.try_send(item)
    {
        let _ = stream.stop(0u32.into());
    }
}
