// Copied from `celestina-rs/crates/celestina-shell-core/src/audio.rs` at
// c7fb0b5d, adapted: the logic changes only in `parse_wpctl_volume`, which
// now rejects a level that overflows u32 instead of wrapping, then extended by CEL-2-C with the
// `pactl subscribe` line parser and the step arithmetic at the end of the file.

//! What the session's audio device is set to, as `wpctl` reports it.
//!
//! One line — `Volume: 0.60`, or `Volume: 0.70 [MUTED]` — carries both facts
//! the panel shows. Reading it is the part worth testing, so it lives here as a
//! function over text; asking `wpctl` for the line is the caller's business.
//!
//! A level is whole percent, like every other number the panel shows: the
//! session sets volume in steps a person chose, not in fractions nobody reads.

/// A device's level and whether it is silenced. Muted is not zero: a muted
/// device remembers where it was, and the panel says both things rather than
/// pretending the level moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioLevel {
    pub percent: u8,
    pub muted: bool,
}

/// Reads one `wpctl get-volume` line.
///
/// Returns `None` for anything that is not that line — a missing device, an
/// error message, a future format — because a panel with no reading is
/// truthful and a panel showing 0 % is not.
#[must_use]
pub fn parse_wpctl_volume(line: &str) -> Option<AudioLevel> {
    let rest = line.trim().strip_prefix("Volume:")?;
    let mut fields = rest.split_whitespace();
    let level = fields.next()?;

    // wpctl prints a unit fraction in hundredths — `0.60`, and above `1.00`
    // when a session allows overdrive — which is already whole percent once the
    // point is read as text. No float ever enters: the reading is exact, and
    // rounding one would only be a way to lose it.
    let (whole, fraction) = level.split_once('.').unwrap_or((level, ""));
    let whole: u32 = whole.parse().ok()?;
    let mut hundredths = fraction.chars();
    let tens = hundredths
        .next()
        .map_or(Some(0), |digit| digit.to_digit(10))?;
    let units = hundredths
        .next()
        .map_or(Some(0), |digit| digit.to_digit(10))?;

    // Hostile input must not wrap (release) or panic (debug): too large is no reading.
    let hundredths = whole.checked_mul(100)?.checked_add(tens * 10 + units)?;
    Some(AudioLevel {
        percent: u8::try_from(hundredths).unwrap_or(u8::MAX),
        muted: fields.any(|field| field.eq_ignore_ascii_case("[MUTED]")),
    })
}

/// The longest `pactl subscribe` line read; anything longer is not an event.
pub const MAX_EVENT_LINE_BYTES: usize = 4096;

/// The step one wheel notch or key press moves a level, in whole percent.
pub const STEP_PERCENT: u8 = 5;

/// The highest level a step raises a device to. A session that already sits
/// above it (overdrive) is never lowered by a raise.
pub const CEILING_PERCENT: u8 = 100;

/// What a `pactl subscribe` line says changed. Only the three facilities that
/// move what the panel shows are typed; every other line (clients, streams,
/// cards, modules) is `None`, so a stream starting never wakes a reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioEvent {
    /// An output device: its level or mute changed, or it came or went.
    Sink,
    /// An input device.
    Source,
    /// The server itself, where the default device changes.
    Server,
}

/// Reads one `pactl subscribe` line such as `Event 'change' on sink #51`.
///
/// The child is another process, so the line is hostile input: anything over
/// [`MAX_EVENT_LINE_BYTES`], with an unknown action or facility, a malformed
/// index or trailing text is no event.
#[must_use]
pub fn parse_pactl_event(line: &str) -> Option<AudioEvent> {
    if line.len() > MAX_EVENT_LINE_BYTES {
        return None;
    }
    let rest = line.trim().strip_prefix("Event '")?;
    let (action, rest) = rest.split_once("' on ")?;
    if !matches!(action, "change" | "new" | "remove") {
        return None;
    }
    let mut fields = rest.split(' ');
    let facility = fields.next()?;
    let event = match facility {
        "sink" => AudioEvent::Sink,
        "source" => AudioEvent::Source,
        "server" => AudioEvent::Server,
        _ => return None,
    };
    match (fields.next(), fields.next()) {
        (None, None) if event == AudioEvent::Server => Some(event),
        (Some(index), None) => {
            let digits = index.strip_prefix('#')?;
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            digits.parse::<u32>().ok().map(|_| event)
        }
        _ => None,
    }
}

/// Which way a step moves a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// The level after one step of `step` percent: saturating at 0, raised to at
/// most [`CEILING_PERCENT`], and never lowered by a raise.
#[must_use]
pub fn stepped(current: u8, direction: Direction, step: u8) -> u8 {
    match direction {
        Direction::Down => current.saturating_sub(step),
        Direction::Up => current.max(current.saturating_add(step).min(CEILING_PERCENT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_whole_percent() {
        assert_eq!(
            parse_wpctl_volume("Volume: 0.60\n"),
            Some(AudioLevel {
                percent: 60,
                muted: false
            })
        );
        // wpctl prints hundredths; anything finer than the panel shows is not
        // rounded up into a level the device is not at.
        assert_eq!(
            parse_wpctl_volume("Volume: 0.005"),
            Some(AudioLevel {
                percent: 0,
                muted: false
            })
        );
        assert_eq!(
            parse_wpctl_volume("Volume: 1"),
            Some(AudioLevel {
                percent: 100,
                muted: false
            })
        );
        assert_eq!(
            parse_wpctl_volume("Volume: 0.00"),
            Some(AudioLevel {
                percent: 0,
                muted: false
            })
        );
    }

    #[test]
    fn a_muted_device_keeps_the_level_it_remembers() {
        assert_eq!(
            parse_wpctl_volume("Volume: 0.70 [MUTED]"),
            Some(AudioLevel {
                percent: 70,
                muted: true
            })
        );
    }

    #[test]
    fn a_session_that_allows_overdrive_is_reported_as_it_is() {
        assert_eq!(
            parse_wpctl_volume("Volume: 1.50"),
            Some(AudioLevel {
                percent: 150,
                muted: false
            })
        );
    }

    #[test]
    fn anything_that_is_not_a_reading_is_no_reading_at_all() {
        // A device that is not there, and a tool that answered something else.
        assert_eq!(parse_wpctl_volume("Node 51 not found"), None);
        assert_eq!(parse_wpctl_volume(""), None);
        assert_eq!(parse_wpctl_volume("Volume:"), None);
        assert_eq!(parse_wpctl_volume("Volume: loud"), None);
        assert_eq!(parse_wpctl_volume("Volume: -0.5"), None);
        assert_eq!(parse_wpctl_volume("Volume: 0.6x"), None);
        assert_eq!(parse_wpctl_volume("Volume: 99999999999"), None);
        assert_eq!(parse_wpctl_volume("Volume: 99999999.99"), None);
    }

    #[test]
    fn a_pactl_line_names_the_facility_that_changed() {
        assert_eq!(
            parse_pactl_event("Event 'change' on sink #51"),
            Some(AudioEvent::Sink)
        );
        assert_eq!(
            parse_pactl_event("Event 'change' on source #52\n"),
            Some(AudioEvent::Source)
        );
        assert_eq!(
            parse_pactl_event("Event 'change' on server #4294967295"),
            Some(AudioEvent::Server)
        );
        assert_eq!(
            parse_pactl_event("Event 'change' on server"),
            Some(AudioEvent::Server)
        );
        assert_eq!(
            parse_pactl_event("Event 'change' on server #5"),
            Some(AudioEvent::Server)
        );
        assert_eq!(
            parse_pactl_event("Event 'new' on sink #4294967295"),
            Some(AudioEvent::Sink)
        );
        assert_eq!(
            parse_pactl_event("Event 'remove' on sink #7"),
            Some(AudioEvent::Sink)
        );
    }

    #[test]
    fn a_line_that_is_not_a_device_event_is_none() {
        for line in [
            "",
            "Event 'change' on sink-input #5",
            "Event 'change' on client #5",
            "Event 'change' on card #5",
            "Event 'explode' on sink #5",
            "Event 'change' on sink",
            "Event 'change' on sink #",
            "Event 'change' on sink #-1",
            "Event 'change' on sink #5x",
            "Event 'change' on sink #99999999999",
            "Event 'change' on sink #5 extra",
            "Event 'change' on server #",
            "Event 'change' on server #x",
            "Event 'change' on server #99999999999",
            "Event 'change' on sink  #5",
            "change on sink #5",
        ] {
            assert_eq!(parse_pactl_event(line), None, "{line:?}");
        }
    }

    #[test]
    fn a_hostile_line_is_bounded() {
        let long = format!(
            "Event 'change' on sink #5{}",
            " ".repeat(MAX_EVENT_LINE_BYTES)
        );
        assert_eq!(parse_pactl_event(&long), None);
        assert_eq!(parse_pactl_event("Event 'change' on sink #\u{0}"), None);
    }

    #[test]
    fn a_step_saturates_at_both_ends() {
        assert_eq!(stepped(50, Direction::Up, STEP_PERCENT), 55);
        assert_eq!(stepped(50, Direction::Down, STEP_PERCENT), 45);
        assert_eq!(stepped(98, Direction::Up, STEP_PERCENT), CEILING_PERCENT);
        assert_eq!(stepped(100, Direction::Up, STEP_PERCENT), 100);
        assert_eq!(stepped(3, Direction::Down, STEP_PERCENT), 0);
        assert_eq!(stepped(0, Direction::Down, STEP_PERCENT), 0);
        assert_eq!(stepped(254, Direction::Up, 200), 254);
    }

    #[test]
    fn a_raise_never_lowers_an_overdriven_device() {
        assert_eq!(stepped(150, Direction::Up, STEP_PERCENT), 150);
        assert_eq!(stepped(150, Direction::Down, STEP_PERCENT), 145);
    }
}
