//! How often a changing provider is allowed to reach the GUI thread.
//!
//! A provider that samples every 500 ms and one that reports every keypress
//! share a pipe and a Qt host that must parse and marshal each frame. Coalescing
//! keeps a burst to one frame per window without ever dropping the *last* state:
//! what is delayed is the notification, never the value.

/// The window a burst is collapsed into. Fast enough that a panel still reads
/// as live, slow enough that a chatty provider cannot drive the GUI thread.
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

    /// Whether a frame is owed now. The first one is always due: a host that
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

    /// How long a helper may sleep before it must look again: `None` when
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
    fn a_quiet_helper_owes_nothing_and_may_sleep_on_its_sources() {
        let mut coalescer = Coalescer::new(100);
        coalescer.emitted(0);

        assert!(!coalescer.due(10_000));
        assert_eq!(coalescer.wait_ms(10_000), None);
    }
}
