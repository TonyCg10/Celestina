//! The loop's message channel, shared by both profiles: workers clone a
//! [`Sender`] and publish typed messages into the loop from any thread.
#![forbid(unsafe_code)]

use calloop::channel;

/// Publishes messages into the loop from any thread; workers clone it.
pub struct Sender<M>(channel::Sender<M>);

impl<M> Sender<M> {
    /// Wraps the sending half of the loop's channel.
    pub(crate) fn new(inner: channel::Sender<M>) -> Self {
        Self(inner)
    }
}

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> std::fmt::Debug for Sender<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sender")
    }
}

impl<M> Sender<M> {
    /// Queues `msg` for the shell's `message`; fails once the loop is gone.
    pub fn send(&self, msg: M) -> Result<(), RuntimeGone> {
        self.0.send(msg).map_err(|_| RuntimeGone)
    }
}

/// The loop stopped receiving messages: the sender's worker should exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the runtime stopped receiving messages")]
pub struct RuntimeGone;
