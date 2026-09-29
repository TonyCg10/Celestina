//! A session's control stream has one writer.
//!
//! Every envelope this side sends on a session's control stream goes through
//! an [`Outbox`]: a bounded queue that one task drains, each write under
//! [`SEND_DEADLINE`]. The session loop and its helpers only queue, so a peer
//! that stops reading, whose flow control then holds the write, never stalls
//! the loop that enforces Forget, stop and supersede. A write that outlives
//! its deadline closes the connection, and the closed connection ends the
//! session.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use magnetita_link::Session;
use magnetita_proto::Envelope;
use tokio::sync::mpsc::error::TrySendError;

use crate::runtime::log;

/// How long one control-stream write may wait for the peer's flow-control
/// credit before the peer counts as gone. Shorter than the link's 30 s idle
/// timeout, and long enough for a phone on a congested network. The tests
/// use a short one so a stalled peer is seen quickly.
#[cfg(not(test))]
pub(crate) const SEND_DEADLINE: Duration = Duration::from_secs(10);
#[cfg(test)]
pub(crate) const SEND_DEADLINE: Duration = Duration::from_secs(2);

/// How many envelopes may wait for the writer. A peer that reads keeps this
/// near empty; one that does not is closed at its deadline long before
/// anything else fills it.
const QUEUE: usize = 256;

/// Why an envelope was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refused {
    /// The writer is this far behind: the peer stopped reading.
    Full,
    /// The session has ended.
    Closed,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refused::Full => "the control stream is backed up",
            Refused::Closed => "the session has ended",
        })
    }
}

/// The queue in front of one session's control-stream writer. Cloned by
/// everything that answers the phone: the loop, share transfers, the
/// storage client and the mirror's input.
#[derive(Clone)]
pub(crate) struct Outbox {
    tx: tokio::sync::mpsc::Sender<Item>,
}

/// What waits for the writer: an envelope, or a mark the writer answers once
/// everything queued before it has been written.
pub(crate) enum Item {
    Envelope(Envelope),
    Flushed(tokio::sync::oneshot::Sender<()>),
}

impl Item {
    /// The envelope, for a test that reads a detached queue.
    #[cfg(test)]
    pub(crate) fn into_envelope(self) -> Option<Envelope> {
        match self {
            Item::Envelope(env) => Some(env),
            Item::Flushed(_) => None,
        }
    }
}

impl Outbox {
    /// Queues one envelope without waiting. The writer gives it the next
    /// envelope id; `env.id` is ignored.
    pub(crate) fn send(&self, env: Envelope) -> Result<(), Refused> {
        self.queue(Item::Envelope(env))
    }

    /// Waits, up to `limit`, for everything queued so far to be written; false
    /// when the writer did not get there (the queue was full, the session
    /// had ended, or the peer is not reading).
    pub(crate) async fn flush(&self, limit: Duration) -> bool {
        let (done, written) = tokio::sync::oneshot::channel();
        if self.queue(Item::Flushed(done)).is_err() {
            return false;
        }
        matches!(tokio::time::timeout(limit, written).await, Ok(Ok(())))
    }

    fn queue(&self, item: Item) -> Result<(), Refused> {
        self.tx.try_send(item).map_err(|e| match e {
            TrySendError::Full(_) => Refused::Full,
            TrySendError::Closed(_) => Refused::Closed,
        })
    }

    /// An outbox whose queue the test reads itself, with no session behind.
    #[cfg(test)]
    pub(crate) fn detached(capacity: usize) -> (Self, tokio::sync::mpsc::Receiver<Item>) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        (Self { tx }, rx)
    }
}

/// Starts the writer of `session`'s control stream on the current runtime.
/// The task ends when every [`Outbox`] is gone, when a write fails, or when
/// a write outlives [`SEND_DEADLINE`]; the last two close the connection.
pub(crate) fn spawn(session: Arc<Session>, name: String) -> (Outbox, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Item>(QUEUE);
    let task = tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            let env = match item {
                Item::Envelope(env) => env,
                Item::Flushed(done) => {
                    let _ = done.send(());
                    continue;
                }
            };
            let write = session.send_message(env.capability, env.kind, env.body);
            match tokio::time::timeout(SEND_DEADLINE, write).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    log("link", &format!("{name}: send: {e}"));
                    session.close("send failed");
                    break;
                }
                Err(_) => {
                    log(
                        "link",
                        &format!(
                            "{name}: a send waited past {} s; the phone stopped reading",
                            SEND_DEADLINE.as_secs()
                        ),
                    );
                    session.close("send deadline");
                    break;
                }
            }
        }
    });
    (Outbox { tx }, task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_or_closed_queue_refuses_at_once() {
        let (outbox, rx) = Outbox::detached(1);
        let env = || Envelope {
            capability: 1,
            kind: 1,
            id: 0,
            body: Vec::new(),
        };
        assert_eq!(outbox.send(env()), Ok(()));
        assert_eq!(outbox.send(env()), Err(Refused::Full));
        drop(rx);
        assert_eq!(outbox.send(env()), Err(Refused::Closed));
    }
}
