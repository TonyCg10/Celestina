//! How the daemon ends: on `SIGTERM` (every `systemctl stop`, so every
//! deploy) or `SIGINT`, `main` returns instead of being killed, and the
//! daemon's owners drop in order: the own wire closes its sessions, which
//! unmount the phone's FUSE directory and release held input, and the
//! advertisement and the adb worker end. A killed daemon leaves a dead
//! mountpoint behind until its next start sweeps it.

use std::io;

use tokio::signal::unix::{signal, Signal, SignalKind};

/// The handlers, installed; a signal that arrives from here on is waited
/// for rather than killing the process.
pub(crate) struct Termination {
    runtime: tokio::runtime::Runtime,
    term: Signal,
    interrupt: Signal,
}

impl Termination {
    pub(crate) fn install() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?;
        let (term, interrupt) = {
            let _context = runtime.enter();
            (
                signal(SignalKind::terminate())?,
                signal(SignalKind::interrupt())?,
            )
        };
        Ok(Self {
            runtime,
            term,
            interrupt,
        })
    }

    /// Blocks until one of the signals arrives, and names it.
    pub(crate) fn wait(mut self) -> &'static str {
        self.runtime.block_on(async {
            tokio::select! {
                _ = self.term.recv() => "SIGTERM",
                _ = self.interrupt.recv() => "SIGINT",
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sigterm_ends_the_wait_instead_of_the_process() {
        let termination = Termination::install().unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TERM)
            .unwrap();
        assert_eq!(termination.wait(), "SIGTERM");
    }
}
