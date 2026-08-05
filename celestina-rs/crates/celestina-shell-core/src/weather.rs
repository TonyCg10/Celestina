//! What the sky is doing where the person said, and how little has to leave the
//! machine to find out.
//!
//! This is the only thing in the shell that talks to the internet, so the rules
//! are about restraint rather than features. One request carries a coordinate
//! pair and the fields being asked for — no identifier, no place name, no
//! history, no anything else. What comes back is bounded before it is believed,
//! and a reading that is too old is *absent* rather than shown as if it were
//! current: a temperature from four hours ago is not weather, it is a memory.
//!
//! Nothing here performs the request. It builds the URL, reads the answer, and
//! decides when the last answer stopped counting — all without a clock or a
//! socket, so every rule is testable.

use serde::Deserialize;

use crate::settings::Location;

/// The service. Free, no key, and no account to tie a request to a person.
const HOST: &str = "https://api.open-meteo.com/v1/forecast";
/// How long a reading counts as current. Weather does not move faster than
/// this, and asking more often would be spending somebody's network on nothing.
pub const FRESH_MS: u64 = 15 * 60 * 1000;
/// How long to wait before asking again after a failure. Long enough that a
/// service having a bad afternoon is not hammered, short enough that a
/// reconnected laptop catches up.
pub const RETRY_MS: u64 = 5 * 60 * 1000;
/// The largest answer worth reading. Open-Meteo's current-weather reply is a
/// few hundred bytes; anything past this is not that reply.
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024;

/// One reading, in whole degrees Celsius.
///
/// Whole degrees because that is what a panel shows and what a person says. The
/// service reports tenths; keeping them would put a precision on screen that
/// the reading does not deserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub celsius: i16,
    /// The service's own weather code, which the surface maps to words. Kept as
    /// the number rather than translated here: what a code should be called is
    /// product copy, not protocol.
    pub code: u8,
    /// Whether the service says it is daytime there — not here, and not now by
    /// this machine's clock.
    pub daylight: bool,
}

/// The exact URL to ask for one reading.
///
/// Coordinates are rounded to two decimals — about a kilometre. A panel's
/// weather does not improve with more precision, and more precision is a more
/// exact answer to "where is this person".
#[must_use]
pub fn request_url(place: &Location) -> String {
    format!(
        "{HOST}?latitude={:.2}&longitude={:.2}&current=temperature_2m,weather_code,is_day",
        place.latitude, place.longitude
    )
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: Option<f64>,
    weather_code: Option<i64>,
    is_day: Option<i64>,
}

#[derive(Deserialize)]
struct Response {
    current: Option<Current>,
}

/// Reads one answer.
///
/// Returns `None` for anything that is not a reading this shell can show: an
/// oversized body, unreadable JSON, a missing temperature, or a number that
/// cannot be a temperature on this planet. An absent reading is honest; a
/// zero is not.
#[must_use]
pub fn read(body: &[u8]) -> Option<Reading> {
    if body.len() > MAX_RESPONSE_BYTES {
        return None;
    }

    let parsed: Response = serde_json::from_slice(body).ok()?;
    let current = parsed.current?;
    let celsius = current.temperature_2m?;
    if !celsius.is_finite() || !(-100.0..=100.0).contains(&celsius) {
        return None;
    }

    Some(Reading {
        // Rounded rather than truncated: -0.4 °C is 0 °C, not -1 °C.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the range above makes this fit an i16 exactly"
        )]
        celsius: celsius.round() as i16,
        code: current
            .weather_code
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(0),
        // Anything but an explicit 1 is night: a missing field must not invent
        // daylight.
        daylight: current.is_day == Some(1),
    })
}

/// The last answer, and when it arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cached {
    pub reading: Reading,
    pub taken_ms: u64,
}

/// What the provider should do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// The cached reading still counts; show it and ask nothing.
    Keep,
    /// Ask again.
    Ask,
    /// Wait: the last attempt failed recently and the service deserves a pause.
    Wait,
}

/// Decides whether to ask, wait, or keep what is held.
///
/// `last_failure_ms` is when the most recent attempt failed, if one did. The
/// failure pause applies even with nothing cached: a service that just refused
/// will refuse again a second later.
#[must_use]
pub fn next_step(cached: Option<Cached>, last_failure_ms: Option<u64>, now_ms: u64) -> Next {
    if let Some(failed) = last_failure_ms {
        if now_ms.saturating_sub(failed) < RETRY_MS {
            return Next::Wait;
        }
    }
    match cached {
        Some(cached) if now_ms.saturating_sub(cached.taken_ms) < FRESH_MS => Next::Keep,
        _ => Next::Ask,
    }
}

/// Whether a held reading may still be shown.
///
/// Deliberately stricter than [`next_step`]: a reading stops being shown at the
/// same moment it stops being current, so a panel never carries a stale number
/// while a retry is pending.
#[must_use]
pub fn still_worth_showing(cached: Cached, now_ms: u64) -> bool {
    now_ms.saturating_sub(cached.taken_ms) < FRESH_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> Location {
        Location::new(53.349_805, -6.260_31, "Dublin").expect("a place")
    }

    fn body(celsius: &str) -> Vec<u8> {
        format!(r#"{{"current":{{"temperature_2m":{celsius},"weather_code":3,"is_day":1}}}}"#)
            .into_bytes()
    }

    #[test]
    fn the_request_carries_a_rounded_coordinate_and_nothing_else() {
        let url = request_url(&place());
        assert!(url.starts_with(HOST));
        assert!(url.contains("latitude=53.35"));
        assert!(url.contains("longitude=-6.26"));
        // No identifier, no place name, no anything that says who is asking.
        assert!(!url.contains("Dublin"));
        assert!(!url.to_lowercase().contains("key"));
        assert!(!url.contains("timezone"));
    }

    #[test]
    fn a_reading_is_whole_degrees() {
        assert_eq!(
            read(&body("12.4")),
            Some(Reading {
                celsius: 12,
                code: 3,
                daylight: true
            })
        );
        // Rounded, not truncated: just below freezing is 0, never -1.
        assert_eq!(read(&body("-0.4")).map(|reading| reading.celsius), Some(0));
        assert_eq!(read(&body("-3.6")).map(|reading| reading.celsius), Some(-4));
    }

    #[test]
    fn an_answer_that_is_not_a_reading_is_no_reading_at_all() {
        assert_eq!(read(b"not json"), None);
        assert_eq!(read(b"{}"), None);
        assert_eq!(read(br#"{"current":{}}"#), None);
        // Not a temperature on this planet.
        assert_eq!(read(&body("999")), None);
        assert_eq!(read(&vec![b'x'; MAX_RESPONSE_BYTES + 1]), None);
    }

    #[test]
    fn a_missing_daylight_field_never_invents_daylight() {
        let body = br#"{"current":{"temperature_2m":9.0,"weather_code":61}}"#;
        assert_eq!(read(body).map(|reading| reading.daylight), Some(false));
    }

    #[test]
    fn a_current_reading_is_kept_rather_than_asked_for_again() {
        let cached = Cached {
            reading: Reading {
                celsius: 12,
                code: 3,
                daylight: true,
            },
            taken_ms: 1_000,
        };

        assert_eq!(
            next_step(Some(cached), None, 1_000 + FRESH_MS - 1),
            Next::Keep
        );
        assert_eq!(next_step(Some(cached), None, 1_000 + FRESH_MS), Next::Ask);
        assert_eq!(next_step(None, None, 0), Next::Ask);
    }

    #[test]
    fn a_service_that_just_refused_is_left_alone() {
        assert_eq!(
            next_step(None, Some(1_000), 1_000 + RETRY_MS - 1),
            Next::Wait
        );
        assert_eq!(next_step(None, Some(1_000), 1_000 + RETRY_MS), Next::Ask);
    }

    #[test]
    fn a_reading_stops_being_shown_when_it_stops_being_current() {
        let cached = Cached {
            reading: Reading {
                celsius: 12,
                code: 3,
                daylight: true,
            },
            taken_ms: 0,
        };

        assert!(still_worth_showing(cached, FRESH_MS - 1));
        // A temperature from four hours ago is not weather; the panel shows
        // nothing rather than a number that stopped being true.
        assert!(!still_worth_showing(cached, FRESH_MS));
    }
}
