//! How many bulk transfers may run at once: one limiter the daemon shares
//! across sessions, each transfer holding a permit for its life.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The most a declared payload may claim. Generous — phones legitimately share
/// multi-gigabyte videos — while refusing an absurd or negative declaration
/// before a single byte lands on disk.
pub const MAX_PAYLOAD_SIZE: i64 = 64 * 1024 * 1024 * 1024;

/// A paired peer may announce several transfers, but may not create an
/// unbounded number of long-lived socket workers.
const MAX_CONCURRENT_PAYLOADS: usize = 4;

struct PayloadLimit {
    active: AtomicUsize,
}

/// Shared, non-blocking admission control for payload workers.
#[derive(Clone)]
pub struct PayloadLimiter {
    inner: Arc<PayloadLimit>,
}

/// One active payload slot. Dropping it releases the slot on every error path.
pub struct PayloadPermit {
    inner: Arc<PayloadLimit>,
}

impl PayloadLimiter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(PayloadLimit {
                active: AtomicUsize::new(0),
            }),
        }
    }

    pub fn try_acquire(&self) -> Option<PayloadPermit> {
        let mut active = self.inner.active.load(Ordering::Relaxed);
        loop {
            if active >= MAX_CONCURRENT_PAYLOADS {
                return None;
            }
            match self.inner.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(PayloadPermit {
                        inner: Arc::clone(&self.inner),
                    });
                }
                Err(current) => active = current,
            }
        }
    }
}

impl Default for PayloadLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PayloadPermit {
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_are_bounded_and_returned() {
        let limiter = PayloadLimiter::new();
        let held: Vec<_> = (0..MAX_CONCURRENT_PAYLOADS)
            .map(|_| limiter.try_acquire().unwrap())
            .collect();
        assert!(limiter.try_acquire().is_none());
        drop(held);
        assert!(limiter.try_acquire().is_some());
    }
}
