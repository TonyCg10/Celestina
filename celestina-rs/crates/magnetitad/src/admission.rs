//! Bounded admission for network links that have not earned trust yet.
//!
//! A permit accounts for one handshake or live untrusted link. It releases its
//! slot on every return path through `Drop`; callers that promote a link out of
//! this pool can release it explicitly. Dial throttling shares the same lock so
//! concurrent discovery paths cannot race past the per-device interval.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_UNTRUSTED_LINKS: usize = 42;
const MAX_UNTRUSTED_LINKS_PER_IP: usize = 4;
const DIAL_THROTTLE: Duration = Duration::from_millis(500);

#[derive(Default)]
struct State {
    active: usize,
    active_by_ip: HashMap<IpAddr, usize>,
    last_dial: HashMap<String, Instant>,
}

/// Shared admission state for handshakes and links that are not trusted yet.
pub(crate) struct Admission {
    state: Mutex<State>,
}

/// One accounted untrusted link. Releasing or dropping it frees one slot.
pub(crate) struct Permit {
    admission: Arc<Admission>,
    address: IpAddr,
    active: bool,
}

impl Admission {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }

    /// Reserve a slot without blocking. Both the global and source-IP limits
    /// must have capacity for the same atomic acquisition to succeed.
    pub(crate) fn try_acquire(self: &Arc<Self>, address: IpAddr) -> Option<Permit> {
        let mut state = self.lock_state();
        let address_active = state.active_by_ip.get(&address).copied().unwrap_or(0);
        if state.active >= MAX_UNTRUSTED_LINKS || address_active >= MAX_UNTRUSTED_LINKS_PER_IP {
            return None;
        }

        state.active += 1;
        state.active_by_ip.insert(address, address_active + 1);
        Some(Permit {
            admission: Arc::clone(self),
            address,
            active: true,
        })
    }

    /// Admit one dial for `device_id` per throttle interval. Entries outside
    /// the interval are discarded so transient discoveries do not accumulate.
    pub(crate) fn allow_dial(&self, device_id: &str, now: Instant) -> bool {
        let mut state = self.lock_state();
        state.last_dial.retain(|_, previous| {
            now.checked_duration_since(*previous)
                .is_none_or(|elapsed| elapsed < DIAL_THROTTLE)
        });

        if state.last_dial.contains_key(device_id) {
            return false;
        }
        state.last_dial.insert(device_id.to_owned(), now);
        true
    }

    fn release(&self, address: IpAddr) {
        let mut state = self.lock_state();
        if state.active > 0 {
            state.active -= 1;
        }

        let Some(address_active) = state.active_by_ip.get_mut(&address) else {
            return;
        };
        if *address_active > 1 {
            *address_active -= 1;
        } else {
            state.active_by_ip.remove(&address);
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Default for Admission {
    fn default() -> Self {
        Self::new()
    }
}

impl Permit {
    /// Release this permit early. Repeated calls are harmless and do not free
    /// capacity owned by another permit.
    pub(crate) fn release(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.admission.release(self.address);
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{Admission, MAX_UNTRUSTED_LINKS};

    fn address(last_octet: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last_octet))
    }

    #[test]
    fn limits_each_source_ip_to_four_links() {
        let admission = Arc::new(Admission::new());
        let ip = address(1);
        let permits: Vec<_> = (0..4).filter_map(|_| admission.try_acquire(ip)).collect();

        assert_eq!(permits.len(), 4);
        assert!(admission.try_acquire(ip).is_none());
        assert!(admission.try_acquire(address(2)).is_some());
    }

    #[test]
    fn limits_all_sources_to_forty_two_links() {
        let admission = Arc::new(Admission::new());
        let permits: Vec<_> = (1..=MAX_UNTRUSTED_LINKS)
            .filter_map(|index| admission.try_acquire(address(index as u8)))
            .collect();

        assert_eq!(permits.len(), MAX_UNTRUSTED_LINKS);
        assert!(admission.try_acquire(address(200)).is_none());
    }

    #[test]
    fn explicit_release_and_drop_each_free_exactly_one_slot() {
        let admission = Arc::new(Admission::new());
        let ip = address(1);
        let mut permits: Vec<_> = (0..4).filter_map(|_| admission.try_acquire(ip)).collect();
        assert!(admission.try_acquire(ip).is_none());

        permits[0].release();
        permits[0].release();
        let replacement = admission.try_acquire(ip);
        assert!(replacement.is_some());
        assert!(admission.try_acquire(ip).is_none());

        drop(replacement);
        assert!(admission.try_acquire(ip).is_some());
    }

    #[test]
    fn throttles_each_device_for_five_hundred_milliseconds() {
        let admission = Admission::new();
        let start = Instant::now();

        assert!(admission.allow_dial("phone-a", start));
        assert!(!admission.allow_dial("phone-a", start));
        assert!(!admission.allow_dial("phone-a", start + Duration::from_millis(499)));
        assert!(admission.allow_dial("phone-b", start + Duration::from_millis(499)));
        assert!(admission.allow_dial("phone-a", start + Duration::from_millis(500)));
    }

    #[test]
    fn a_clock_value_before_the_last_seen_does_not_bypass_throttling() {
        let admission = Admission::new();
        let start = Instant::now();
        let later = start + Duration::from_secs(1);

        assert!(admission.allow_dial("phone", later));
        assert!(!admission.allow_dial("phone", start));
    }
}
