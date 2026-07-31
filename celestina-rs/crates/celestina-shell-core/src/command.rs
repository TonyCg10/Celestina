//! The host's requests, and the refusals that answer the ones a helper cannot
//! serve.
//!
//! A command names the provider it is for, so one helper can carry many
//! providers without a verb table that has to be unique across all of them. A
//! request the helper cannot read still gets an answer whenever its id can be
//! recovered: a host that hears nothing can only wait out its own timeout,
//! which reads to the user as a hang rather than a refusal.

use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::bounded;
use crate::snapshot::{Payload, ProviderId, MAX_ID_CHARS, MAX_PAYLOAD_KEYS};

pub const MAX_REASON_CHARS: usize = 200;
pub const MAX_VERB_CHARS: usize = 32;

/// A request the helper accepted for execution. Acceptance is not arrival: what
/// a command did is reported by the provider's next snapshot, never by the
/// result frame alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub provider: ProviderId,
    pub verb: String,
    pub options: Payload,
}

#[derive(Debug, Deserialize)]
struct CommandLine {
    id: Option<String>,
    provider: Option<String>,
    verb: Option<String>,
    #[serde(default)]
    options: Payload,
}

/// Why a request was refused, and whom to tell. `id` is `None` only when the
/// request carried none that could be echoed safely — answering an unbounded
/// id would put hostile input into a downstream frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    pub id: Option<String>,
    pub reason: String,
}

impl Rejection {
    fn anonymous(reason: &str) -> Self {
        Self {
            id: None,
            reason: reason.to_owned(),
        }
    }
}

/// The answer to one request.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ResultFrame<'a> {
    pub kind: &'static str,
    pub id: &'a str,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl<'a> ResultFrame<'a> {
    #[must_use]
    pub fn accepted(id: &'a str) -> Self {
        Self {
            kind: "result",
            id,
            state: "accepted",
            reason: None,
        }
    }

    #[must_use]
    pub fn failed(id: &'a str, reason: &str) -> Self {
        Self {
            kind: "result",
            id,
            state: "failed",
            reason: Some(bounded(reason, MAX_REASON_CHARS)),
        }
    }
}

fn usable_id(raw: Option<&String>) -> Option<String> {
    raw.filter(|id| !id.is_empty() && id.chars().count() <= MAX_ID_CHARS)
        .cloned()
}

/// Reads one host line into a command.
///
/// # Errors
///
/// Returns a [`Rejection`] carrying the request id whenever one can be
/// recovered, so the host learns immediately instead of waiting for a timeout.
pub fn parse_command(line: &[u8]) -> Result<Command, Rejection> {
    let Ok(parsed) = serde_json::from_slice::<CommandLine>(line) else {
        return Err(Rejection::anonymous("the command is not readable JSON"));
    };

    let Some(id) = usable_id(parsed.id.as_ref()) else {
        return Err(Rejection::anonymous(
            "the command carries no usable request id",
        ));
    };
    let reject = |reason: &str| Rejection {
        id: Some(id.clone()),
        reason: reason.to_owned(),
    };

    let Some(provider) = parsed.provider.as_deref() else {
        return Err(reject("the command names no provider"));
    };
    let Ok(provider) = ProviderId::new(provider) else {
        return Err(reject("the command names no usable provider"));
    };

    let verb = parsed.verb.unwrap_or_default();
    if verb.is_empty() || verb.chars().count() > MAX_VERB_CHARS {
        return Err(reject("the command carries no usable verb"));
    }
    if parsed.options.len() > MAX_PAYLOAD_KEYS {
        return Err(reject("the command carries too many options"));
    }

    Ok(Command {
        id,
        provider,
        verb,
        options: parsed.options,
    })
}

/// The refusal a helper owes a well-formed command for a provider it does not
/// carry. Every helper answers this the same way, so an unknown provider is
/// never silently dropped.
#[must_use]
pub fn unknown_provider(command: &Command) -> Rejection {
    Rejection {
        id: Some(command.id.clone()),
        reason: format!(
            "no provider named '{}' is running in this helper",
            command.provider.as_str()
        ),
    }
}

/// An empty option map, for helpers building commands in tests and fixtures.
#[must_use]
pub fn no_options() -> Payload {
    Map::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_command_names_its_provider_and_verb() {
        let command =
            parse_command(br#"{"id":"7","provider":"volume","verb":"set","options":{"level":40}}"#)
                .expect("a well-formed command");

        assert_eq!(command.id, "7");
        assert_eq!(command.provider.as_str(), "volume");
        assert_eq!(command.verb, "set");
        assert_eq!(
            command
                .options
                .get("level")
                .and_then(serde_json::Value::as_i64),
            Some(40)
        );
    }

    #[test]
    fn a_command_without_options_is_still_a_command() {
        let command = parse_command(br#"{"id":"7","provider":"sysmon","verb":"refresh"}"#)
            .expect("a well-formed command");

        assert!(command.options.is_empty());
    }

    #[test]
    fn a_refusal_carries_the_request_id_whenever_one_can_be_recovered() {
        let rejection = parse_command(br#"{"id":"9","provider":"volume"}"#)
            .expect_err("a verbless command is refused");

        assert_eq!(rejection.id.as_deref(), Some("9"));
        assert!(rejection.reason.contains("verb"));
    }

    #[test]
    fn an_unusable_id_is_never_echoed_back() {
        for line in [
            br#"{"provider":"volume","verb":"set"}"#.to_vec(),
            format!(
                r#"{{"id":"{}","provider":"volume","verb":"set"}}"#,
                "x".repeat(MAX_ID_CHARS + 1)
            )
            .into_bytes(),
            b"not json at all".to_vec(),
        ] {
            let rejection = parse_command(&line).expect_err("refused");
            assert_eq!(rejection.id, None);
        }
    }

    #[test]
    fn a_provider_the_helper_does_not_carry_is_refused_by_name() {
        let command = parse_command(br#"{"id":"7","provider":"sysmon","verb":"refresh"}"#)
            .expect("a well-formed command");

        let rejection = unknown_provider(&command);
        assert_eq!(rejection.id.as_deref(), Some("7"));
        assert!(rejection.reason.contains("sysmon"));
    }

    #[test]
    fn a_failed_result_carries_a_bounded_reason() {
        let frame = ResultFrame::failed("7", &"z".repeat(MAX_REASON_CHARS + 40));

        assert_eq!(frame.state, "failed");
        assert_eq!(
            frame.reason.as_ref().map(|reason| reason.chars().count()),
            Some(MAX_REASON_CHARS)
        );
    }
}
