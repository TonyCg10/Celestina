//! Melibea's shell-facing protocol and its ordered minimized-window projection.
//!
//! Melibea and Niri remain authoritative. This module validates one versioned
//! line, applies a complete snapshot or one gapless incremental revision, and
//! builds requests. It owns no socket, thread, Qt type or visual policy.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::snapshot::MAX_TEXT_UNITS;

/// The frozen v1 contract. Every request that needs nothing from v2 still uses it, so a
/// Melibea that speaks only v1 keeps serving this shell unchanged.
pub const PROTOCOL_VERSION: u32 = 1;
/// Adds action-scoped presentation hints. Used only for a request that carries one.
pub const PROTOCOL_VERSION_V2: u32 = 2;
/// Every version this client can read a reply under, ascending.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[u32] = &[PROTOCOL_VERSION, PROTOCOL_VERSION_V2];
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_WINDOWS: usize = 64;
pub const MAX_CHANGES: usize = 128;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServerEnvelope {
    pub version: u32,
    pub message: Message,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Snapshot {
        revision: u64,
        windows: Vec<Window>,
    },
    Changes {
        revision: u64,
        changes: Vec<WindowChange>,
    },
    ActionResult(ActionResult),
    Unavailable {
        revision: u64,
        reason: String,
    },
    Error(ProtocolError),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub icon_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowChange {
    Added {
        index: usize,
        window: Window,
    },
    Updated {
        index: usize,
        window: Window,
    },
    Moved {
        window_id: u64,
        from_index: usize,
        to_index: usize,
    },
    Removed {
        index: usize,
        window_id: u64,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ActionResult {
    pub operation: Operation,
    pub requested_id: Option<u64>,
    pub window_id: Option<u64>,
    pub status: ActionStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Minimize,
    Restore,
    Close,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Applied,
    AlreadyInRequestedState,
    CloseRequested,
    WindowNotFound,
    Blocked,
    LegacyHandled,
}

impl ActionStatus {
    #[must_use]
    pub const fn accepts_state_confirmation(self) -> bool {
        matches!(
            self,
            Self::Applied
                | Self::AlreadyInRequestedState
                | Self::CloseRequested
                | Self::LegacyHandled
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub supported_versions: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    IncompatibleVersion,
    InvalidRequest,
    Unavailable,
    ActionFailed,
}

/// How one minimize or restore should be presented.
///
/// This is a hint attached to a single action. Melibea validates and forwards it but never
/// stores it, and Niri degrades to its ordinary motion whenever it cannot be honoured, so a
/// shell that crashes mid-animation leaves nothing behind to clean up.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowTransition {
    /// Travel between the window and a rectangle this shell draws.
    Anchored { anchor: BubbleAnchor },
    /// Change state with no spatial, scale, or opacity animation.
    Disabled,
}

/// An output-local logical rectangle this shell owns because it draws it.
///
/// Niri keeps output topology, transforms, scale, and clipping. Constructing one is fallible so
/// an unusable rectangle is refused here rather than travelling to the compositor to be ignored.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BubbleAnchor {
    pub output: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BubbleAnchor {
    /// Builds an anchor, refusing anything that cannot describe a real rectangle.
    #[must_use]
    pub fn new(output: &str, x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        if output.is_empty() || output.len() > MAX_TEXT_UNITS {
            return None;
        }
        if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
            return None;
        }
        if width <= 0. || height <= 0. {
            return None;
        }
        Some(Self {
            output: output.to_owned(),
            x,
            y,
            width,
            height,
        })
    }
}

// `Eq` is deliberately absent from here down: an anchor carries `f64`, which has no total
// equality. Nothing in this crate needs more than `PartialEq`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClientEnvelope {
    pub version: u32,
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Subscribe,
    Minimize {
        /// A null asks Niri to resolve the focused window itself. That is not a convenience:
        /// reading focus here and acting on it afterwards would race, and Niri is the only
        /// authority on what is focused at the instant the action lands.
        window_id: Option<u64>,
        /// Omitted entirely when absent. An explicit `null` is malformed at both versions, so
        /// this must never serialize as one.
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<WindowTransition>,
    },
    Restore {
        window_id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<WindowTransition>,
    },
    Close {
        window_id: u64,
    },
}

impl ClientEnvelope {
    #[must_use]
    pub const fn subscribe() -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request: Request::Subscribe,
        }
    }

    /// An action with no presentation hint, sent under the frozen v1 contract.
    #[must_use]
    pub fn action(operation: Operation, window_id: u64) -> Option<Self> {
        Self::action_with_transition(operation, window_id, None)
    }

    /// Minimize whichever window Niri finds focused, rather than one this shell named.
    ///
    /// Only minimize can ask this. Restoring or closing names a window that is already a
    /// bubble, so there is nothing for Niri to resolve.
    #[must_use]
    pub fn minimize_focused(transition: Option<WindowTransition>) -> Self {
        let version = if transition.is_some() {
            PROTOCOL_VERSION_V2
        } else {
            PROTOCOL_VERSION
        };
        Self {
            version,
            request: Request::Minimize {
                window_id: None,
                transition,
            },
        }
    }

    /// An action that may carry one presentation hint.
    ///
    /// The version is chosen by the request rather than by the shell: without a transition this
    /// stays on v1, so a Melibea that speaks only v1 still serves every ordinary action.
    #[must_use]
    pub fn action_with_transition(
        operation: Operation,
        window_id: u64,
        transition: Option<WindowTransition>,
    ) -> Option<Self> {
        let version = if transition.is_some() {
            PROTOCOL_VERSION_V2
        } else {
            PROTOCOL_VERSION
        };
        let request = match operation {
            Operation::Minimize => Request::Minimize {
                window_id: Some(window_id),
                transition,
            },
            Operation::Restore => Request::Restore {
                window_id,
                transition,
            },
            // Closing a window has no destination to travel to, so it takes no hint.
            Operation::Close => match transition {
                None => Request::Close { window_id },
                Some(_) => return None,
            },
        };
        Some(Self { version, request })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateEffect {
    Changed,
    Unchanged,
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Projection {
    revision: Option<u64>,
    windows: Vec<Window>,
    ready: bool,
}

impl Projection {
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.ready
    }

    #[must_use]
    pub const fn revision(&self) -> Option<u64> {
        self.revision
    }

    #[must_use]
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    #[must_use]
    pub fn contains(&self, id: u64) -> bool {
        self.windows.iter().any(|window| window.id == id)
    }

    /// Applies only authoritative state messages.
    ///
    /// # Errors
    ///
    /// Refuses invalid revisions, identities, indexes, bounds and text.
    pub fn apply(&mut self, message: Message) -> Result<StateEffect, ContractError> {
        match message {
            Message::Snapshot { revision, windows } => {
                validate_windows(&windows)?;
                if let Some(previous) = self.revision {
                    if revision < previous {
                        return Err(ContractError::StaleRevision {
                            current: previous,
                            received: revision,
                        });
                    }
                    if revision == previous && self.ready && self.windows != windows {
                        return Err(ContractError::RevisionConflict(revision));
                    }
                }
                let changed =
                    !self.ready || self.revision != Some(revision) || self.windows != windows;
                self.revision = Some(revision);
                self.windows = windows;
                self.ready = true;
                Ok(if changed {
                    StateEffect::Changed
                } else {
                    StateEffect::Unchanged
                })
            }
            Message::Changes { revision, changes } => {
                let Some(current) = self.revision.filter(|_| self.ready) else {
                    return Err(ContractError::ChangesBeforeSnapshot);
                };
                if revision != current.saturating_add(1) {
                    return Err(ContractError::RevisionGap {
                        current,
                        received: revision,
                    });
                }
                if changes.len() > MAX_CHANGES {
                    return Err(ContractError::TooManyChanges);
                }

                let mut next = self.windows.clone();
                for change in changes {
                    apply_change(&mut next, change)?;
                    if next.len() > MAX_WINDOWS {
                        return Err(ContractError::TooManyWindows);
                    }
                }
                validate_windows(&next)?;
                let changed = next != self.windows;
                self.windows = next;
                self.revision = Some(revision);
                Ok(if changed {
                    StateEffect::Changed
                } else {
                    StateEffect::Unchanged
                })
            }
            Message::Unavailable { revision, reason } => {
                validate_text(&reason)?;
                if let Some(previous) = self.revision {
                    if revision < previous {
                        return Err(ContractError::StaleRevision {
                            current: previous,
                            received: revision,
                        });
                    }
                }
                self.revision = Some(revision);
                self.windows.clear();
                self.ready = false;
                Ok(StateEffect::Unavailable)
            }
            Message::ActionResult(_) | Message::Error(_) => Err(ContractError::NotStateMessage),
        }
    }
}

fn apply_change(windows: &mut Vec<Window>, change: WindowChange) -> Result<(), ContractError> {
    match change {
        WindowChange::Added { index, window } => {
            validate_window(&window)?;
            if index > windows.len() {
                return Err(ContractError::InvalidIndex);
            }
            if windows.iter().any(|current| current.id == window.id) {
                return Err(ContractError::DuplicateWindow(window.id));
            }
            windows.insert(index, window);
        }
        WindowChange::Updated { index, window } => {
            validate_window(&window)?;
            let Some(current) = windows.get_mut(index) else {
                return Err(ContractError::InvalidIndex);
            };
            if current.id != window.id {
                return Err(ContractError::WrongWindow {
                    expected: current.id,
                    received: window.id,
                });
            }
            *current = window;
        }
        WindowChange::Moved {
            window_id,
            from_index,
            to_index,
        } => {
            if from_index >= windows.len() || to_index >= windows.len() {
                return Err(ContractError::InvalidIndex);
            }
            if windows[from_index].id != window_id {
                return Err(ContractError::WrongWindow {
                    expected: windows[from_index].id,
                    received: window_id,
                });
            }
            let window = windows.remove(from_index);
            windows.insert(to_index, window);
        }
        WindowChange::Removed { index, window_id } => {
            let Some(current) = windows.get(index) else {
                return Err(ContractError::InvalidIndex);
            };
            if current.id != window_id {
                return Err(ContractError::WrongWindow {
                    expected: current.id,
                    received: window_id,
                });
            }
            windows.remove(index);
        }
    }
    Ok(())
}

fn validate_windows(windows: &[Window]) -> Result<(), ContractError> {
    if windows.len() > MAX_WINDOWS {
        return Err(ContractError::TooManyWindows);
    }
    let mut ids = BTreeSet::new();
    for window in windows {
        validate_window(window)?;
        if !ids.insert(window.id) {
            return Err(ContractError::DuplicateWindow(window.id));
        }
    }
    Ok(())
}

fn validate_window(window: &Window) -> Result<(), ContractError> {
    for text in [
        window.app_id.as_deref(),
        window.title.as_deref(),
        window.icon_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_text(text)?;
    }
    Ok(())
}

fn validate_text(text: &str) -> Result<(), ContractError> {
    if text.encode_utf16().count() > MAX_TEXT_UNITS {
        Err(ContractError::TextTooLong)
    } else {
        Ok(())
    }
}

/// Decodes one complete newline-free server value.
///
/// # Errors
///
/// Refuses oversized bytes, invalid JSON, a mismatched version and oversized
/// protocol diagnostics.
pub fn decode_message(line: &[u8]) -> Result<Message, ContractError> {
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(ContractError::MessageTooLarge);
    }
    let envelope: ServerEnvelope =
        serde_json::from_slice(line).map_err(|_| ContractError::InvalidJson)?;
    if !SUPPORTED_PROTOCOL_VERSIONS.contains(&envelope.version) {
        return Err(ContractError::IncompatibleVersion(envelope.version));
    }
    match &envelope.message {
        Message::Error(error) => validate_text(&error.message)?,
        Message::Unavailable { reason, .. } => validate_text(reason)?,
        _ => {}
    }
    Ok(envelope.message)
}

/// Encodes one request as the newline-delimited frame the service expects.
///
/// # Errors
///
/// Returns a serialization error only if the fixed protocol representation
/// itself becomes invalid.
pub fn encode_request(request: &ClientEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    let mut line = serde_json::to_vec(request)?;
    line.push(b'\n');
    Ok(line)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    MessageTooLarge,
    InvalidJson,
    IncompatibleVersion(u32),
    ChangesBeforeSnapshot,
    RevisionGap { current: u64, received: u64 },
    StaleRevision { current: u64, received: u64 },
    RevisionConflict(u64),
    TooManyWindows,
    TooManyChanges,
    DuplicateWindow(u64),
    InvalidIndex,
    WrongWindow { expected: u64, received: u64 },
    TextTooLong,
    NotStateMessage,
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MessageTooLarge => write!(formatter, "Melibea message exceeds 64 KiB"),
            Self::InvalidJson => write!(formatter, "Melibea message is not valid protocol JSON"),
            Self::IncompatibleVersion(version) => {
                write!(formatter, "unsupported Melibea protocol version {version}")
            }
            Self::ChangesBeforeSnapshot => {
                write!(formatter, "Melibea changes arrived before a snapshot")
            }
            Self::RevisionGap { current, received } => write!(
                formatter,
                "Melibea revision gap after {current}: received {received}"
            ),
            Self::StaleRevision { current, received } => write!(
                formatter,
                "stale Melibea revision {received} after {current}"
            ),
            Self::RevisionConflict(revision) => write!(
                formatter,
                "Melibea revision {revision} names two different snapshots"
            ),
            Self::TooManyWindows => write!(formatter, "Melibea published too many windows"),
            Self::TooManyChanges => write!(formatter, "Melibea published too many changes"),
            Self::DuplicateWindow(id) => write!(formatter, "Melibea repeated window {id}"),
            Self::InvalidIndex => write!(formatter, "Melibea change carries an invalid index"),
            Self::WrongWindow { expected, received } => write!(
                formatter,
                "Melibea change names window {received} where {expected} is stored"
            ),
            Self::TextTooLong => write!(formatter, "Melibea published oversized text"),
            Self::NotStateMessage => write!(formatter, "Melibea message carries no state"),
        }
    }
}

impl std::error::Error for ContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, title: &str) -> Window {
        Window {
            id,
            app_id: Some("org.example.App".to_owned()),
            title: Some(title.to_owned()),
            icon_name: None,
        }
    }

    #[test]
    fn requests_are_versioned_and_newline_delimited() {
        assert_eq!(
            String::from_utf8(encode_request(&ClientEnvelope::subscribe()).expect("encoded"))
                .expect("UTF-8"),
            "{\"version\":1,\"request\":{\"type\":\"subscribe\"}}\n"
        );
        let restore = ClientEnvelope::action(Operation::Restore, 42).expect("supported");
        assert_eq!(
            String::from_utf8(encode_request(&restore).expect("encoded")).expect("UTF-8"),
            "{\"version\":1,\"request\":{\"type\":\"restore\",\"window_id\":42}}\n"
        );
        // Minimize is part of the frozen v1 contract too; it simply had no shell caller before
        // this shell could originate one.
        let minimize = ClientEnvelope::action(Operation::Minimize, 42).expect("supported");
        assert_eq!(
            String::from_utf8(encode_request(&minimize).expect("encoded")).expect("UTF-8"),
            "{\"version\":1,\"request\":{\"type\":\"minimize\",\"window_id\":42}}\n"
        );
    }

    #[test]
    fn an_action_without_a_hint_never_leaves_the_frozen_v1_contract() {
        // A transition is the only reason to ask for v2. Everything else must keep working
        // against a Melibea that speaks v1 alone.
        for operation in [Operation::Minimize, Operation::Restore, Operation::Close] {
            let envelope = ClientEnvelope::action(operation, 42).expect("supported");
            assert_eq!(envelope.version, PROTOCOL_VERSION);
            let line =
                String::from_utf8(encode_request(&envelope).expect("encoded")).expect("UTF-8");
            assert!(
                !line.contains("transition"),
                "an omitted hint must not appear on the wire at all, got {line}"
            );
        }
    }

    #[test]
    fn a_hint_moves_the_request_to_v2_in_the_shape_melibea_froze() {
        let anchor = BubbleAnchor::new("DP-1", 1874., 9., 22., 22.).expect("valid anchor");
        let anchored = ClientEnvelope::action_with_transition(
            Operation::Minimize,
            42,
            Some(WindowTransition::Anchored { anchor }),
        )
        .expect("supported");
        assert_eq!(anchored.version, PROTOCOL_VERSION_V2);
        assert_eq!(
            String::from_utf8(encode_request(&anchored).expect("encoded")).expect("UTF-8"),
            "{\"version\":2,\"request\":{\"type\":\"minimize\",\"window_id\":42,\
\"transition\":{\"type\":\"anchored\",\"anchor\":{\"output\":\"DP-1\",\"x\":1874.0,\
\"y\":9.0,\"width\":22.0,\"height\":22.0}}}}\n"
        );

        let reduced = ClientEnvelope::action_with_transition(
            Operation::Restore,
            42,
            Some(WindowTransition::Disabled),
        )
        .expect("supported");
        assert_eq!(
            String::from_utf8(encode_request(&reduced).expect("encoded")).expect("UTF-8"),
            "{\"version\":2,\"request\":{\"type\":\"restore\",\"window_id\":42,\
\"transition\":{\"type\":\"disabled\"}}}\n"
        );

        // Closing has no destination to travel to, so it refuses a hint rather than sending one
        // Melibea would have to ignore.
        assert_eq!(
            ClientEnvelope::action_with_transition(
                Operation::Close,
                42,
                Some(WindowTransition::Disabled)
            ),
            None
        );
    }

    #[test]
    fn a_focused_minimize_sends_an_explicit_null_rather_than_omitting_the_window() {
        // Melibea reads a null as "resolve the focused window" and an omitted field as
        // malformed, so this is one place where the two are not interchangeable.
        let focused = ClientEnvelope::minimize_focused(None);
        assert_eq!(focused.version, PROTOCOL_VERSION);
        assert_eq!(
            String::from_utf8(encode_request(&focused).expect("encoded")).expect("UTF-8"),
            "{\"version\":1,\"request\":{\"type\":\"minimize\",\"window_id\":null}}\n"
        );

        let anchor = BubbleAnchor::new("DP-1", 1874., 9., 22., 22.).expect("valid anchor");
        let travelling =
            ClientEnvelope::minimize_focused(Some(WindowTransition::Anchored { anchor }));
        assert_eq!(travelling.version, PROTOCOL_VERSION_V2);
        assert_eq!(
            String::from_utf8(encode_request(&travelling).expect("encoded")).expect("UTF-8"),
            "{\"version\":2,\"request\":{\"type\":\"minimize\",\"window_id\":null,\
\"transition\":{\"type\":\"anchored\",\"anchor\":{\"output\":\"DP-1\",\"x\":1874.0,\
\"y\":9.0,\"width\":22.0,\"height\":22.0}}}}\n"
        );
    }

    #[test]
    fn an_unusable_anchor_is_refused_before_it_can_be_sent() {
        assert!(BubbleAnchor::new("DP-1", 1874., 9., 22., 22.).is_some());
        // A partly offscreen bubble is ordinary shell layout, not a broken anchor.
        assert!(BubbleAnchor::new("DP-1", -4., -4., 22., 22.).is_some());

        assert!(BubbleAnchor::new("", 0., 0., 22., 22.).is_none());
        assert!(BubbleAnchor::new("DP-1", 0., 0., 0., 22.).is_none());
        assert!(BubbleAnchor::new("DP-1", 0., 0., 22., -1.).is_none());
        assert!(BubbleAnchor::new("DP-1", f64::NAN, 0., 22., 22.).is_none());
        assert!(BubbleAnchor::new("DP-1", 0., 0., f64::INFINITY, 22.).is_none());
        assert!(BubbleAnchor::new(&"x".repeat(MAX_TEXT_UNITS + 1), 0., 0., 22., 22.).is_none());
    }

    #[test]
    fn a_v2_reply_is_readable_and_an_unknown_version_still_is_not() {
        // Melibea answers a v2 request under version 2.
        assert!(decode_message(
            br#"{"version":2,"message":{"type":"snapshot","revision":1,"windows":[]}}"#
        )
        .is_ok());
        assert!(decode_message(
            br#"{"version":1,"message":{"type":"snapshot","revision":1,"windows":[]}}"#
        )
        .is_ok());
        assert_eq!(
            decode_message(
                br#"{"version":3,"message":{"type":"snapshot","revision":1,"windows":[]}}"#
            ),
            Err(ContractError::IncompatibleVersion(3))
        );
    }

    #[test]
    fn snapshot_then_sequential_changes_reproduce_the_order() {
        let mut state = Projection::default();
        assert_eq!(
            state.apply(Message::Snapshot {
                revision: 7,
                windows: vec![window(1, "one"), window(2, "old")],
            }),
            Ok(StateEffect::Changed)
        );
        assert_eq!(
            state.apply(Message::Changes {
                revision: 8,
                changes: vec![
                    WindowChange::Moved {
                        window_id: 2,
                        from_index: 1,
                        to_index: 0,
                    },
                    WindowChange::Updated {
                        index: 0,
                        window: window(2, "new"),
                    },
                    WindowChange::Added {
                        index: 2,
                        window: window(3, "three"),
                    },
                    WindowChange::Removed {
                        index: 1,
                        window_id: 1,
                    },
                ],
            }),
            Ok(StateEffect::Changed)
        );
        assert_eq!(state.windows(), &[window(2, "new"), window(3, "three")]);
    }

    #[test]
    fn invalid_change_is_atomic_and_a_gap_requires_resynchronization() {
        let mut state = Projection::default();
        state
            .apply(Message::Snapshot {
                revision: 1,
                windows: vec![window(1, "one")],
            })
            .expect("snapshot");
        let before = state.clone();
        assert_eq!(
            state.apply(Message::Changes {
                revision: 2,
                changes: vec![WindowChange::Removed {
                    index: 0,
                    window_id: 9,
                }],
            }),
            Err(ContractError::WrongWindow {
                expected: 1,
                received: 9,
            })
        );
        assert_eq!(state, before);
        assert_eq!(
            state.apply(Message::Changes {
                revision: 4,
                changes: Vec::new(),
            }),
            Err(ContractError::RevisionGap {
                current: 1,
                received: 4,
            })
        );
    }

    #[test]
    fn one_revision_cannot_describe_two_different_snapshots() {
        let mut state = Projection::default();
        state
            .apply(Message::Snapshot {
                revision: 5,
                windows: vec![window(1, "one")],
            })
            .expect("first snapshot");
        let before = state.clone();
        assert_eq!(
            state.apply(Message::Snapshot {
                revision: 5,
                windows: vec![window(2, "two")],
            }),
            Err(ContractError::RevisionConflict(5))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn unavailable_clears_state_and_only_a_snapshot_reopens_it() {
        let mut state = Projection::default();
        state
            .apply(Message::Snapshot {
                revision: 3,
                windows: vec![window(1, "one")],
            })
            .expect("snapshot");
        assert_eq!(
            state.apply(Message::Unavailable {
                revision: 3,
                reason: "niri connection lost".to_owned(),
            }),
            Ok(StateEffect::Unavailable)
        );
        assert!(!state.ready());
        assert!(state.windows().is_empty());
        assert_eq!(
            state.apply(Message::Changes {
                revision: 4,
                changes: Vec::new(),
            }),
            Err(ContractError::ChangesBeforeSnapshot)
        );
        assert_eq!(
            state.apply(Message::Snapshot {
                revision: 4,
                windows: vec![window(2, "two")],
            }),
            Ok(StateEffect::Changed)
        );
        assert!(state.ready());
    }

    #[test]
    fn incompatible_oversized_and_duplicate_input_is_refused() {
        // Version 2 became readable when this client learned to send transitions;
        // `a_v2_reply_is_readable_and_an_unknown_version_still_is_not` owns that boundary now.
        assert_eq!(
            decode_message(
                br#"{"version":9,"message":{"type":"snapshot","revision":1,"windows":[]}}"#
            ),
            Err(ContractError::IncompatibleVersion(9))
        );
        assert_eq!(
            decode_message(&vec![b'x'; MAX_MESSAGE_BYTES + 1]),
            Err(ContractError::MessageTooLarge)
        );
        let mut state = Projection::default();
        assert_eq!(
            state.apply(Message::Snapshot {
                revision: 1,
                windows: vec![window(7, "first"), window(7, "second")],
            }),
            Err(ContractError::DuplicateWindow(7))
        );
    }

    #[test]
    fn action_statuses_preserve_the_confirmation_boundary() {
        for status in [
            ActionStatus::Applied,
            ActionStatus::AlreadyInRequestedState,
            ActionStatus::CloseRequested,
            ActionStatus::LegacyHandled,
        ] {
            assert!(status.accepts_state_confirmation());
        }
        for status in [ActionStatus::WindowNotFound, ActionStatus::Blocked] {
            assert!(!status.accepts_state_confirmation());
        }
    }
}
