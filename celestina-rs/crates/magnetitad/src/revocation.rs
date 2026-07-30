//! Linearizable local pairing revocation shared by D-Bus and link threads.
//!
//! Forgetting trust is not just another queued link command: an already-read
//! `Paired` event must not be allowed to pin the certificate again. Each request
//! therefore leaves a generation-tagged tombstone until an explicit new Pair
//! action clears it, or until the old live link is fully gone after trust was
//! removed. The link acknowledges only the generation it actually applied.

use std::collections::HashMap;
use std::io;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use celestina_core::{Generation, GenerationClock, GenerationExhausted};
use magnetita_core::ConnectionEvent;

#[derive(Clone, Copy, Debug)]
struct Entry {
    generation: Generation,
    applied: bool,
}

#[derive(Default)]
struct State {
    clock: GenerationClock,
    devices: HashMap<String, Entry>,
}

/// Tombstones for devices whose durable trust has been (or is being) removed.
#[derive(Default)]
pub(crate) struct Revocations {
    state: Mutex<State>,
    changed: Condvar,
}

impl Revocations {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Start a new revocation and return the generation its caller must await.
    #[cfg(test)]
    pub(crate) fn request(&self, device_id: &str) -> Result<Generation, GenerationExhausted> {
        let mut state = self.lock();
        let generation = state.clock.issue()?;
        state.devices.insert(
            device_id.to_owned(),
            Entry {
                generation,
                applied: false,
            },
        );
        self.changed.notify_all();
        Ok(generation)
    }

    /// Atomically decide whether a target still exists, install its tombstone,
    /// and persist the trust removal while Pair/teardown are excluded by the
    /// same mutex. A failed write restores the exact previous tombstone.
    pub(crate) fn request_if_and_apply(
        &self,
        device_id: &str,
        should_request: impl FnOnce() -> bool,
        apply: impl FnOnce() -> io::Result<()>,
    ) -> Result<Option<Generation>, RequestError> {
        let mut state = self.lock();
        if !should_request() {
            if state.devices.remove(device_id).is_some() {
                self.changed.notify_all();
            }
            return Ok(None);
        }
        let generation = state.clock.issue().map_err(RequestError::Generation)?;
        let previous = state.devices.insert(
            device_id.to_owned(),
            Entry {
                generation,
                applied: false,
            },
        );
        if let Err(error) = apply() {
            match previous {
                Some(previous) => {
                    state.devices.insert(device_id.to_owned(), previous);
                }
                None => {
                    state.devices.remove(device_id);
                }
            }
            self.changed.notify_all();
            return Err(RequestError::Apply(error));
        }
        self.changed.notify_all();
        Ok(Some(generation))
    }

    /// Snapshot the ordering point carried by a future queued Pair command.
    /// A Forget issued after this observation receives a greater generation and
    /// therefore wins even if the older Pair reaches the link thread later.
    pub(crate) fn observe_pair(&self) -> Generation {
        self.lock().clock.current()
    }

    /// The revocation the link must apply, if this device has a tombstone.
    pub(crate) fn current(&self, device_id: &str) -> Option<Generation> {
        self.lock()
            .devices
            .get(device_id)
            .map(|entry| entry.generation)
    }

    /// The generation which still needs the live link to apply its cleanup.
    pub(crate) fn pending(&self, device_id: &str) -> Option<Generation> {
        self.lock()
            .devices
            .get(device_id)
            .filter(|entry| !entry.applied)
            .map(|entry| entry.generation)
    }

    pub(crate) fn suppresses(&self, device_id: &str, event: &ConnectionEvent) -> bool {
        self.current(device_id).is_some()
            && matches!(
                event,
                ConnectionEvent::Pairing | ConnectionEvent::Paired | ConnectionEvent::Pinged
            )
    }

    /// Remove stale state once no live session can re-establish trust.
    #[cfg(test)]
    pub(crate) fn clear(&self, device_id: &str) {
        self.lock().devices.remove(device_id);
        self.changed.notify_all();
    }

    /// Clear a tombstone only while a caller-provided teardown invariant holds.
    /// The condition runs under the revocation mutex, so a newly inserted
    /// session must observe either the tombstone or already-removed trust.
    pub(crate) fn clear_if(&self, device_id: &str, condition: impl FnOnce() -> bool) -> bool {
        let mut state = self.lock();
        if !condition() {
            return false;
        }
        let removed = state.devices.remove(device_id).is_some();
        if removed {
            self.changed.notify_all();
        }
        removed
    }

    /// Authorize a Pair only if no newer Forget crossed its D-Bus observation.
    /// A Pair observed at or after an existing tombstone is the explicit action
    /// which clears that tombstone; a stale queued Pair can never do so.
    pub(crate) fn authorize_pair(&self, device_id: &str, observed: Generation) -> bool {
        let mut state = self.lock();
        match state.devices.get(device_id) {
            Some(entry) if entry.generation > observed => false,
            Some(_) => {
                state.devices.remove(device_id);
                self.changed.notify_all();
                true
            }
            None => true,
        }
    }

    /// Run a trust-establishing write only while no revocation can cross it.
    /// `request` takes the same mutex, so either the pin wins first and Forget
    /// removes it afterwards, or the tombstone wins and the pin is skipped.
    pub(crate) fn if_pairing_allowed<T>(
        &self,
        device_id: &str,
        apply: impl FnOnce() -> T,
    ) -> Option<T> {
        let state = self.lock();
        if state.devices.contains_key(device_id) {
            return None;
        }
        let result = apply();
        drop(state);
        Some(result)
    }

    /// Mark exactly the generation applied by the link; stale turns are ignored.
    pub(crate) fn acknowledge(&self, device_id: &str, generation: Generation) {
        let mut state = self.lock();
        let Some(entry) = state.devices.get_mut(device_id) else {
            return;
        };
        if entry.generation != generation {
            return;
        }
        entry.applied = true;
        self.changed.notify_all();
    }

    /// Wait until the live link has cleared its paired state, with a hard bound.
    pub(crate) fn wait_applied(
        &self,
        device_id: &str,
        generation: Generation,
        timeout: Duration,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        loop {
            match state.devices.get(device_id) {
                Some(entry) if entry.generation >= generation && entry.applied => return true,
                Some(entry) if entry.generation >= generation => {}
                // Only a later explicit Pair or teardown with no live entry may
                // remove this tombstone. Both happen after durable trust was
                // forgotten, so they resolve this older barrier successfully.
                _ => return true,
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let waited = self.changed.wait_timeout(state, remaining);
            let (next, result) = match waited {
                Ok(pair) => pair,
                Err(poisoned) => poisoned.into_inner(),
            };
            state = next;
            if result.timed_out() {
                return state
                    .devices
                    .get(device_id)
                    .is_none_or(|entry| entry.generation < generation || entry.applied);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum RequestError {
    Generation(GenerationExhausted),
    Apply(io::Error),
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use super::Revocations;

    #[test]
    fn an_acknowledgement_applies_only_its_exact_generation() {
        let revocations = Revocations::new();
        let first = revocations.request("phone").unwrap();
        let second = revocations.request("phone").unwrap();

        revocations.acknowledge("phone", first);
        assert!(!revocations.wait_applied("phone", second, Duration::ZERO));
        revocations.acknowledge("phone", second);
        assert!(revocations.wait_applied("phone", second, Duration::ZERO));
    }

    #[test]
    fn only_an_explicit_pair_action_clears_the_tombstone() {
        let revocations = Revocations::new();
        let generation = revocations.request("phone").unwrap();
        revocations.acknowledge("phone", generation);
        assert_eq!(revocations.current("phone"), Some(generation));
        assert_eq!(revocations.if_pairing_allowed("phone", || 7), None);

        revocations.clear("phone");
        assert!(revocations.current("phone").is_none());
        assert_eq!(revocations.if_pairing_allowed("phone", || 7), Some(7));
    }

    #[test]
    fn stale_pair_cannot_clear_a_newer_forget() {
        let revocations = Revocations::new();
        let observed = revocations.observe_pair();
        let generation = revocations.request("phone").unwrap();

        assert!(!revocations.authorize_pair("phone", observed));
        assert_eq!(revocations.current("phone"), Some(generation));
    }

    #[test]
    fn pair_observed_after_forget_explicitly_clears_it() {
        let revocations = Revocations::new();
        let forgotten = revocations.request("phone").unwrap();
        let observed = revocations.observe_pair();

        assert!(revocations.authorize_pair("phone", observed));
        assert!(revocations.current("phone").is_none());
        assert!(revocations.wait_applied("phone", forgotten, Duration::ZERO));
    }

    #[test]
    fn newer_applied_forget_satisfies_an_older_waiter() {
        let revocations = Revocations::new();
        let older = revocations.request("phone").unwrap();
        let newer = revocations.request("phone").unwrap();
        revocations.acknowledge("phone", newer);

        assert!(revocations.wait_applied("phone", older, Duration::ZERO));
        assert!(revocations.pending("phone").is_none());
    }

    #[test]
    fn failed_persistence_restores_the_previous_generation() {
        let revocations = Revocations::new();
        let older = revocations.request("phone").unwrap();
        let error = revocations
            .request_if_and_apply("phone", || true, || Err(io::Error::other("disk full")))
            .unwrap_err();

        assert!(matches!(error, super::RequestError::Apply(_)));
        assert_eq!(revocations.current("phone"), Some(older));
    }

    #[test]
    fn unknown_offline_ids_leave_no_tombstone() {
        let revocations = Revocations::new();
        revocations.request("invented").unwrap();

        let generation = revocations
            .request_if_and_apply("invented", || false, || Ok(()))
            .unwrap();
        assert!(generation.is_none());
        assert!(revocations.current("invented").is_none());
    }
}
