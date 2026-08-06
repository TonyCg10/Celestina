//! One owned, cancellable provider thread.
//!
//! A provider that keeps a thread has to be able to stop it. Dropping a bare
//! `JoinHandle` detaches the thread instead, which then outlives the helper —
//! and a detached thread that owns an external child outlives whatever was
//! supposed to reap that child. Both long-lived provider threads use this one
//! shape so neither can be left running by an early return: the guard requests
//! shutdown and waits, whether it is joined deliberately or simply dropped.
//!
//! The flag is the helper's own shutdown flag, not a private one. A worker that
//! goes away during startup is telling the rest of the process the same thing
//! the signal handler would.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct Worker {
    what: &'static str,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts `body` on a named thread it will be held responsible for.
    ///
    /// # Errors
    ///
    /// Returns the failure to start the thread at all.
    pub fn spawn<F>(
        what: &'static str,
        shutdown: &Arc<AtomicBool>,
        body: F,
    ) -> std::io::Result<Self>
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = thread::Builder::new().name(what.to_owned()).spawn(body)?;
        Ok(Self {
            what,
            shutdown: Arc::clone(shutdown),
            handle: Some(handle),
        })
    }

    /// Asks the thread to stop and waits for it, reporting a panic by name.
    pub fn join(mut self) {
        if let Some(handle) = self.take_and_stop() {
            if handle.join().is_err() {
                eprintln!(
                    "celestina-provider-adapter: the {} worker panicked",
                    self.what
                );
            }
        }
    }

    fn take_and_stop(&mut self) -> Option<JoinHandle<()>> {
        self.shutdown.store(true, Ordering::Release);
        self.handle.take()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(handle) = self.take_and_stop() {
            let _ = handle.join();
        }
    }
}
