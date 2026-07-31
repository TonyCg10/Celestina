//! The aggregate: which providers a helper carries, what they last said, and
//! when the host is told.
//!
//! One helper carries every provider that needs long-lived, non-Qt IO, so the
//! decisions that used to be per-widget become one runtime's rules: a value
//! identical to the last one is not news, a provider that stops takes its value
//! with it, a command for a provider that is not registered is refused by name,
//! and a burst of changes becomes one frame per window.
//!
//! It owns no threads, no clock and no pipe: time arrives as a millisecond
//! stamp and the frame is handed back to the caller to write. That is what
//! makes every rule here testable without a process.

use std::collections::BTreeSet;

use crate::coalesce::Coalescer;
use crate::command::{unknown_provider, Command, Rejection};
use crate::snapshot::{Payload, ProviderId, ProviderSnapshots, SnapshotError, SnapshotFrame};

#[derive(Debug)]
pub struct ProviderRuntime {
    snapshots: ProviderSnapshots,
    coalescer: Coalescer,
    /// Registered sources, which is not the same as sources that have spoken:
    /// a provider that is starting up carries no value yet but is already the
    /// right recipient for a command.
    sources: BTreeSet<ProviderId>,
}

impl ProviderRuntime {
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self {
            snapshots: ProviderSnapshots::new(generation),
            coalescer: Coalescer::default(),
            sources: BTreeSet::new(),
        }
    }

    /// Announces a provider this helper carries.
    pub fn register(&mut self, id: ProviderId) {
        self.sources.insert(id);
    }

    /// Retires a provider and everything it published. Silence is never the
    /// last thing a provider said.
    pub fn unregister(&mut self, id: &ProviderId) {
        self.sources.remove(id);
        self.withdraw(id);
    }

    /// Drops what a provider published while it keeps carrying it. A media
    /// provider with no player running has nothing to show and everything to
    /// answer: the panel loses the widget, not the ability to be told why a
    /// command cannot be served.
    pub fn withdraw(&mut self, id: &ProviderId) {
        if self.snapshots.withdraw(id) {
            self.coalescer.mark();
        }
    }

    #[must_use]
    pub fn carries(&self, id: &ProviderId) -> bool {
        self.sources.contains(id)
    }

    /// A provider's latest value.
    ///
    /// # Errors
    ///
    /// Refuses an unregistered provider and any value past the snapshot bounds,
    /// so one broken provider degrades itself alone instead of the frame.
    pub fn publish(&mut self, id: &ProviderId, payload: Payload) -> Result<bool, SnapshotError> {
        if !self.carries(id) {
            return Err(SnapshotError::InvalidId);
        }

        let changed = self.snapshots.publish(id.clone(), payload)?;
        if changed {
            self.coalescer.mark();
        }
        Ok(changed)
    }

    /// The refusal owed to a command for a provider this helper does not carry.
    /// `None` means the command may proceed to that provider.
    #[must_use]
    pub fn refuse_unknown(&self, command: &Command) -> Option<Rejection> {
        if self.carries(&command.provider) {
            return None;
        }
        Some(unknown_provider(command))
    }

    /// Starts a new generation with nothing in it, keeping the registered
    /// sources: the helper is the same, its published state is not.
    pub fn reset(&mut self, generation: u64) {
        self.snapshots.reset(generation);
        self.coalescer.mark();
    }

    #[must_use]
    pub fn due(&self, now_ms: u64) -> bool {
        self.coalescer.due(now_ms)
    }

    #[must_use]
    pub fn wait_ms(&self, now_ms: u64) -> Option<u64> {
        self.coalescer.wait_ms(now_ms)
    }

    /// The frame owed to the host. The caller writes it and says when.
    pub fn take_frame(&mut self, now_ms: u64) -> SnapshotFrame<'_> {
        self.coalescer.emitted(now_ms);
        self.snapshots.take_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::command::parse_command;
    use serde_json::Value;

    fn id(raw: &str) -> ProviderId {
        ProviderId::new(raw).expect("a valid provider name")
    }

    fn payload(value: i64) -> Payload {
        let mut payload = Payload::new();
        payload.insert("value".to_owned(), Value::from(value));
        payload
    }

    #[test]
    fn a_registered_provider_reaches_the_frame_once_per_change() {
        let mut runtime = ProviderRuntime::new(1);
        runtime.register(id("fixture"));

        assert_eq!(runtime.publish(&id("fixture"), payload(1)), Ok(true));
        assert!(runtime.due(0));
        runtime.take_frame(0);

        // The same value again is not news, so the host is not woken for it.
        assert_eq!(runtime.publish(&id("fixture"), payload(1)), Ok(false));
        assert!(!runtime.due(1_000));

        assert_eq!(runtime.publish(&id("fixture"), payload(2)), Ok(true));
        assert!(runtime.due(1_000));
    }

    #[test]
    fn an_unregistered_provider_cannot_publish() {
        let mut runtime = ProviderRuntime::new(1);

        assert_eq!(
            runtime.publish(&id("stranger"), payload(1)),
            Err(SnapshotError::InvalidId)
        );
        let frame = runtime.take_frame(0);
        assert!(frame.providers.is_empty());
    }

    #[test]
    fn a_provider_that_disappears_takes_its_value_with_it() {
        let mut runtime = ProviderRuntime::new(1);
        runtime.register(id("fixture"));
        runtime
            .publish(&id("fixture"), payload(1))
            .expect("published");
        runtime.take_frame(0);

        runtime.unregister(&id("fixture"));
        assert!(!runtime.carries(&id("fixture")));
        // The withdrawal is owed, but it is still a change like any other: it
        // waits for its window rather than jumping the queue.
        assert!(!runtime.due(0));
        assert!(runtime.due(100));
        let frame = runtime.take_frame(100);
        assert!(frame.providers.is_empty());
    }

    #[test]
    fn a_provider_can_lose_its_value_and_still_answer_for_itself() {
        let mut runtime = ProviderRuntime::new(1);
        runtime.register(id("media"));
        runtime
            .publish(&id("media"), payload(1))
            .expect("published");
        runtime.take_frame(0);

        runtime.withdraw(&id("media"));
        let frame = runtime.take_frame(100);
        assert!(frame.providers.is_empty());
        // Still carried: a command for it is answered by the provider, not
        // refused as if the helper had never heard of it.
        assert!(runtime.carries(&id("media")));
    }

    #[test]
    fn a_new_generation_publishes_nothing_of_the_previous_one() {
        let mut runtime = ProviderRuntime::new(1);
        runtime.register(id("fixture"));
        runtime
            .publish(&id("fixture"), payload(1))
            .expect("published");

        runtime.reset(2);
        let frame = runtime.take_frame(0);
        assert_eq!(frame.generation, 2);
        assert!(frame.providers.is_empty());
        // The helper still carries the provider; only its state was cleared.
        assert!(runtime.carries(&id("fixture")));
    }

    #[test]
    fn a_command_for_a_provider_the_helper_does_not_carry_is_refused_by_name() {
        let mut runtime = ProviderRuntime::new(1);
        let command = parse_command(br#"{"id":"7","provider":"sysmon","verb":"refresh"}"#)
            .expect("a well-formed command");

        let rejection = runtime
            .refuse_unknown(&command)
            .expect("an unknown provider is refused");
        assert_eq!(rejection.id.as_deref(), Some("7"));
        assert!(rejection.reason.contains("sysmon"));

        runtime.register(id("sysmon"));
        assert!(runtime.refuse_unknown(&command).is_none());
    }

    #[test]
    fn the_first_frame_is_owed_immediately_and_then_coalesced() {
        let mut runtime = ProviderRuntime::new(1);
        runtime.register(id("fixture"));

        // A host that just started deserves the current state at once, even
        // when that state is "carrying nothing yet".
        assert!(runtime.due(0));
        runtime.take_frame(0);
        assert!(!runtime.due(0));

        runtime
            .publish(&id("fixture"), payload(1))
            .expect("published");
        assert!(!runtime.due(10));
        assert_eq!(runtime.wait_ms(10), Some(90));
        assert!(runtime.due(100));
    }
}
