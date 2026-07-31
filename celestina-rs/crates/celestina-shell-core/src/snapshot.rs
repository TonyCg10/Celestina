//! The provider envelope: who published what, and in which generation.
//!
//! One helper carries every bar provider, so the host needs to know which
//! provider a value came from, when a provider has nothing to say any more,
//! and when the whole set belongs to a previous helper process. Payloads stay
//! opaque here on purpose: each provider owns the shape of its own value, and
//! this layer owns identity, bounds and generation — the things that keep a
//! stale or unbounded value from reaching the panel.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

/// The panel's inventory is a handful of widgets; anything past this is a bug
/// in a helper, not a session with many providers.
pub const MAX_PROVIDERS: usize = 32;
/// One provider's value is a flat handful of fields, not a document.
pub const MAX_PAYLOAD_KEYS: usize = 32;
pub const MAX_ID_CHARS: usize = 32;
/// Titles and names arrive from other processes and are shown; they are capped
/// before they are ever published.
pub const MAX_TEXT_CHARS: usize = 512;

pub type Payload = Map<String, Value>;

/// A provider's name on the wire: lowercase, kebab, bounded — the same shape
/// the shell's command verbs use, so one rule covers both.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ProviderId(String);

impl ProviderId {
    /// # Errors
    ///
    /// Returns [`SnapshotError::InvalidId`] when the name is empty, too long or
    /// carries anything but `a-z`, `0-9` and `-`.
    pub fn new(raw: &str) -> Result<Self, SnapshotError> {
        let valid = !raw.is_empty()
            && raw.chars().count() <= MAX_ID_CHARS
            && raw.starts_with(|first: char| first.is_ascii_lowercase())
            && raw.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            });

        if !valid {
            return Err(SnapshotError::InvalidId);
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidId,
    TooManyProviders,
    TooManyFields,
    TextTooLong,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId => write!(formatter, "not a provider name"),
            Self::TooManyProviders => write!(formatter, "too many providers"),
            Self::TooManyFields => write!(formatter, "too many fields in one provider"),
            Self::TextTooLong => write!(formatter, "a provider field is too long"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// What the host receives: every live provider, stamped with the generation
/// that produced it.
#[derive(Debug, PartialEq, Serialize)]
pub struct SnapshotFrame<'a> {
    pub kind: &'static str,
    pub version: u32,
    pub generation: u64,
    pub providers: &'a BTreeMap<ProviderId, Payload>,
}

/// The version a host reads before it reads anything else. New keys never bump
/// it; a changed meaning would.
pub const PROTOCOL_VERSION: u32 = 1;

/// Every live provider's latest value, and whether the host still owes a frame.
#[derive(Debug, Default)]
pub struct ProviderSnapshots {
    generation: u64,
    providers: BTreeMap<ProviderId, Payload>,
    dirty: bool,
}

impl ProviderSnapshots {
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            providers: BTreeMap::new(),
            dirty: false,
        }
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Publishes a provider's latest value. An identical value is not news and
    /// leaves the set clean, which is what keeps an idle panel idle.
    ///
    /// # Errors
    ///
    /// Refuses a set past [`MAX_PROVIDERS`], a payload past [`MAX_PAYLOAD_KEYS`]
    /// and any string field past [`MAX_TEXT_CHARS`], so a broken provider
    /// cannot grow the frame without bound.
    pub fn publish(&mut self, id: ProviderId, payload: Payload) -> Result<bool, SnapshotError> {
        if payload.len() > MAX_PAYLOAD_KEYS {
            return Err(SnapshotError::TooManyFields);
        }
        if payload.values().any(|value| {
            value
                .as_str()
                .is_some_and(|text| text.chars().count() > MAX_TEXT_CHARS)
        }) {
            return Err(SnapshotError::TextTooLong);
        }
        if !self.providers.contains_key(&id) && self.providers.len() >= MAX_PROVIDERS {
            return Err(SnapshotError::TooManyProviders);
        }

        if self.providers.get(&id) == Some(&payload) {
            return Ok(false);
        }

        self.providers.insert(id, payload);
        self.dirty = true;
        Ok(true)
    }

    /// A provider that has gone away leaves no value behind. Silence is never
    /// the last thing it said.
    pub fn withdraw(&mut self, id: &ProviderId) -> bool {
        let removed = self.providers.remove(id).is_some();
        self.dirty |= removed;
        removed
    }

    /// Starts a new generation with nothing in it. The host clears whatever the
    /// previous helper published rather than blending two processes' state.
    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.providers.clear();
        self.dirty = true;
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The frame to send, after which the set owes nothing until it changes.
    pub fn take_frame(&mut self) -> SnapshotFrame<'_> {
        self.dirty = false;
        SnapshotFrame {
            kind: "providers",
            version: PROTOCOL_VERSION,
            generation: self.generation,
            providers: &self.providers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(pairs: &[(&str, Value)]) -> Payload {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    fn id(raw: &str) -> ProviderId {
        ProviderId::new(raw).expect("a valid provider name")
    }

    #[test]
    fn a_provider_name_is_lowercase_kebab_and_bounded() {
        assert!(ProviderId::new("sysmon").is_ok());
        assert!(ProviderId::new("power-profile").is_ok());
        assert_eq!(ProviderId::new(""), Err(SnapshotError::InvalidId));
        assert_eq!(ProviderId::new("SysMon"), Err(SnapshotError::InvalidId));
        assert_eq!(ProviderId::new("sys mon"), Err(SnapshotError::InvalidId));
        assert_eq!(ProviderId::new("9lives"), Err(SnapshotError::InvalidId));
        assert_eq!(
            ProviderId::new(&"a".repeat(MAX_ID_CHARS + 1)),
            Err(SnapshotError::InvalidId)
        );
    }

    #[test]
    fn republishing_the_same_value_is_not_news() {
        let mut snapshots = ProviderSnapshots::new(1);
        let value = payload(&[("cpu", Value::from(12))]);

        assert_eq!(snapshots.publish(id("sysmon"), value.clone()), Ok(true));
        assert!(snapshots.is_dirty());
        snapshots.take_frame();

        assert_eq!(snapshots.publish(id("sysmon"), value), Ok(false));
        assert!(!snapshots.is_dirty());
    }

    #[test]
    fn a_withdrawn_provider_leaves_no_value_behind() {
        let mut snapshots = ProviderSnapshots::new(1);
        snapshots
            .publish(id("sysmon"), payload(&[("cpu", Value::from(12))]))
            .expect("published");
        snapshots.take_frame();

        assert!(snapshots.withdraw(&id("sysmon")));
        assert!(snapshots.is_dirty());
        assert!(snapshots.is_empty());
        // Withdrawing what is already gone is not a change.
        assert!(!snapshots.withdraw(&id("sysmon")));
    }

    #[test]
    fn a_new_generation_starts_empty() {
        let mut snapshots = ProviderSnapshots::new(1);
        snapshots
            .publish(id("sysmon"), payload(&[("cpu", Value::from(12))]))
            .expect("published");

        snapshots.reset(2);
        assert_eq!(snapshots.generation(), 2);
        assert!(snapshots.is_empty());
        assert!(snapshots.is_dirty());
    }

    #[test]
    fn a_broken_provider_cannot_grow_the_frame_without_bound() {
        let mut snapshots = ProviderSnapshots::new(1);

        let wide: Payload = (0..=MAX_PAYLOAD_KEYS)
            .map(|index| (format!("k{index}"), Value::from(index)))
            .collect();
        assert_eq!(
            snapshots.publish(id("sysmon"), wide),
            Err(SnapshotError::TooManyFields)
        );

        let long = payload(&[("title", Value::from("x".repeat(MAX_TEXT_CHARS + 1)))]);
        assert_eq!(
            snapshots.publish(id("media"), long),
            Err(SnapshotError::TextTooLong)
        );

        for index in 0..MAX_PROVIDERS {
            snapshots
                .publish(id(&format!("p{index}")), Payload::new())
                .expect("published");
        }
        assert_eq!(
            snapshots.publish(id("one-too-many"), Payload::new()),
            Err(SnapshotError::TooManyProviders)
        );
    }

    #[test]
    fn a_frame_carries_its_version_and_generation() {
        let mut snapshots = ProviderSnapshots::new(7);
        snapshots
            .publish(id("sysmon"), payload(&[("cpu", Value::from(12))]))
            .expect("published");

        let frame = snapshots.take_frame();
        assert_eq!(frame.kind, "providers");
        assert_eq!(frame.version, PROTOCOL_VERSION);
        assert_eq!(frame.generation, 7);
        let encoded = serde_json::to_string(&frame).expect("serializes");
        assert!(encoded.contains(r#""generation":7"#));
        assert!(encoded.contains(r#""sysmon":{"cpu":12}"#));
    }
}
