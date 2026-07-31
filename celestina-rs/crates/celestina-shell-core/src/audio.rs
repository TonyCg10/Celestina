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

    Some(AudioLevel {
        percent: u8::try_from(whole * 100 + tens * 10 + units).unwrap_or(u8::MAX),
        muted: fields.any(|field| field.eq_ignore_ascii_case("[MUTED]")),
    })
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
    }
}
