// Copied from `celestina-rs/crates/celestina-shell-core/src/coalesce.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: the module prose names
// the in-process worker and the event loop instead of the helper pipe and the
// Qt host, and one test is renamed for the worker; the code is unchanged.

//! How often a changing provider is allowed to reach the event loop.
//!
//! A provider that samples every 500 ms and one that reports every keypress
//! (a window title storm, a tray item changing its tooltip) publish into the
//! same loop that lays out and paints every surface. Coalescing keeps a burst
//! to one message per window without ever dropping the *last* state: what is
//! delayed is the notification, never the value.

/// The window a burst is collapsed into. Fast enough that a panel still reads
/// as live, slow enough that a chatty provider cannot drive the event loop.
pub const DEFAULT_INTERVAL_MS: u64 = 100;

#[derive(Debug)]
pub struct Coalescer {
    interval_ms: u64,
    last_emit_ms: Option<u64>,
    pending: bool,
}

impl Default for Coalescer {
    fn default() -> Self {
        Self::new(DEFAULT_INTERVAL_MS)
    }
}

impl Coalescer {
    #[must_use]
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_emit_ms: None,
            pending: true,
        }
    }

    /// Something changed. The frame it belongs to may already be owed.
    pub fn mark(&mut self) {
        self.pending = true;
    }

    /// Whether a frame is owed now. The first one is always due: a shell that
    /// just started deserves the current state immediately.
    #[must_use]
    pub fn due(&self, now_ms: u64) -> bool {
        if !self.pending {
            return false;
        }
        match self.last_emit_ms {
            None => true,
            Some(last) => now_ms.saturating_sub(last) >= self.interval_ms,
        }
    }

    /// How long a worker may sleep before it must look again: `None` when
    /// nothing is owed and it may wait for its own sources instead.
    #[must_use]
    pub fn wait_ms(&self, now_ms: u64) -> Option<u64> {
        if !self.pending {
            return None;
        }
        match self.last_emit_ms {
            None => Some(0),
            Some(last) => Some(self.interval_ms.saturating_sub(now_ms.saturating_sub(last))),
        }
    }

    pub fn emitted(&mut self, now_ms: u64) {
        self.pending = false;
        self.last_emit_ms = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_frame_is_due_immediately() {
        let coalescer = Coalescer::new(100);

        assert!(coalescer.due(0));
        assert_eq!(coalescer.wait_ms(0), Some(0));
    }

    #[test]
    fn a_burst_becomes_one_frame_per_window() {
        let mut coalescer = Coalescer::new(100);
        coalescer.emitted(0);

        coalescer.mark();
        coalescer.mark();
        coalescer.mark();
        assert!(!coalescer.due(50));
        assert_eq!(coalescer.wait_ms(50), Some(50));

        assert!(coalescer.due(100));
        coalescer.emitted(100);
        assert!(!coalescer.due(1000));
    }

    #[test]
    fn a_quiet_worker_owes_nothing_and_may_sleep_on_its_sources() {
        let mut coalescer = Coalescer::new(100);
        coalescer.emitted(0);

        assert!(!coalescer.due(10_000));
        assert_eq!(coalescer.wait_ms(10_000), None);
    }
}
