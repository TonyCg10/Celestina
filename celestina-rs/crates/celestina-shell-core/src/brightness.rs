//! Monitor brightness, as `ddcutil` reports it over DDC/CI.
//!
//! DDC is a slow, physical conversation with a monitor: a single read on this
//! author's machine takes about a second warm and nine cold, and not every
//! monitor answers at all. That shapes everything here — the panel never polls
//! it, never guesses a value it has not read, and distinguishes three states a
//! caller must keep apart:
//!
//! - a monitor that does not speak DDC at all, which has no brightness to show;
//! - one that does but has not answered yet, which is *unknown*, not zero;
//! - a value that was actually read back.

/// One monitor `ddcutil detect` found, tying its display number to the
/// connector the compositor calls the output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DdcDisplay {
    pub number: u8,
    pub connector: String,
}

/// Reads `ddcutil detect --brief`.
///
/// Blocks headed `Invalid display` are monitors that do not answer DDC; they
/// are left out entirely rather than reported as unknown, because there is
/// nothing there to know.
#[must_use]
pub fn parse_detect(listing: &str) -> Vec<DdcDisplay> {
    let mut displays = Vec::new();
    let mut number: Option<u8> = None;

    for line in listing.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Display ") {
            number = rest.trim().parse().ok();
            continue;
        }
        // Any other unindented heading — `Invalid display`, `Display detection`
        // — ends the block a connector could have belonged to.
        if !line.starts_with(char::is_whitespace) && !trimmed.is_empty() {
            number = None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("DRM connector:") {
            let Some(number) = number.take() else {
                continue;
            };
            // `card1-DP-1` is the compositor's `DP-1` with its card prefix.
            let connector = rest.trim();
            let connector = connector
                .split_once('-')
                .filter(|(card, _)| card.starts_with("card"))
                .map_or(connector, |(_, name)| name);
            if !connector.is_empty() {
                displays.push(DdcDisplay {
                    number,
                    connector: connector.to_owned(),
                });
            }
        }
    }

    displays
}

/// Reads `ddcutil getvcp 10 --brief`, whose line is
/// `VCP <feature> <type> <current> <max>`.
///
/// Returns whole percent of the monitor's own maximum, since monitors do not
/// agree on what that maximum is.
#[must_use]
pub fn parse_brightness(reading: &str) -> Option<u8> {
    let fields: Vec<&str> = reading
        .lines()
        .find(|line| line.trim_start().starts_with("VCP "))?
        .split_whitespace()
        .collect();

    // VCP, feature, type, current, max
    if fields.len() < 5 || fields[1] != "10" {
        return None;
    }
    let current: u32 = fields[3].parse().ok()?;
    let max: u32 = fields[4].parse().ok()?;
    if max == 0 {
        return None;
    }

    u8::try_from(current.min(max) * 100 / max).ok()
}

use core::time::Duration;

/// Nothing but the panel and the monitor's own buttons change brightness, so a
/// re-read exists only to notice the buttons — rarely, because it is expensive.
pub const REFRESH: Duration = Duration::from_secs(300);
/// DDC comes and goes on real hardware: the same `detect` answers with every
/// monitor one minute and none the next, and a sleeping monitor answers nothing
/// at all. Finding none is therefore not a verdict, so the search is retried on
/// its own shorter interval instead of waiting out a full refresh.
pub const REDETECT: Duration = Duration::from_secs(30);

/// When the single DDC worker should run `detect` again.
///
/// The live failure this answers: Celestina started with `DP-1` disabled, the
/// output was enabled sixteen minutes later, its panel mapped correctly, and
/// its brightness control did not appear for the rest of the five-minute
/// refresh. The startup detection was non-empty, so the worker was on the long
/// interval, and nothing woke it when an output arrived.
///
/// The correction is not a shorter interval — DDC is expensive and the retained
/// GPU evidence is a reason to run it less, not more — but a request the worker
/// consumes. A request is a single flag rather than a queue precisely so a
/// burst of outputs appearing at once is one detection, and so a request that
/// arrives while `ddcutil` is mid-conversation is answered by the next turn of
/// the loop rather than by a second child.
#[must_use]
pub fn detection_is_due(any_display_known: bool, requested: bool, since_last: Duration) -> bool {
    if requested {
        return true;
    }

    since_last >= if any_display_known { REFRESH } else { REDETECT }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_output_appearing_is_answered_before_the_next_refresh() {
        // The live case: a known monitor, so the worker is on the long
        // interval, and an output arrives seconds into it.
        assert!(detection_is_due(true, true, Duration::from_secs(4)));
        assert!(!detection_is_due(true, false, Duration::from_secs(4)));
    }

    #[test]
    fn an_unasked_worker_keeps_its_own_expensive_clock() {
        assert!(!detection_is_due(
            true,
            false,
            REFRESH - Duration::from_secs(1)
        ));
        assert!(detection_is_due(true, false, REFRESH));
        // Nothing found yet is not a verdict, so that search retries sooner.
        assert!(detection_is_due(false, false, REDETECT));
        assert!(!detection_is_due(
            false,
            false,
            REDETECT - Duration::from_secs(1)
        ));
        // And the short interval never applies to a worker that found monitors.
        assert!(!detection_is_due(true, false, REDETECT));
    }

    const DETECT: &str = "Invalid display\n   I2C bus:          /dev/i2c-7\n\
                          \x20  DRM connector:    card1-HDMI-A-1\n\
                          \x20  Monitor:          HPN:HP M27h:3CM3020XDF\n\
                          \nDisplay 1\n   I2C bus:          /dev/i2c-8\n\
                          \x20  DRM connector:    card1-DP-1\n\
                          \nDisplay 2\n   I2C bus:          /dev/i2c-9\n\
                          \x20  DRM connector:    card1-DP-2\n";

    #[test]
    fn a_monitor_that_does_not_answer_ddc_is_not_a_display() {
        let displays = parse_detect(DETECT);

        // HDMI-A-1 is there, and invalid: it has no brightness to show at all.
        assert_eq!(
            displays,
            [
                DdcDisplay {
                    number: 1,
                    connector: "DP-1".to_owned()
                },
                DdcDisplay {
                    number: 2,
                    connector: "DP-2".to_owned()
                }
            ]
        );
    }

    #[test]
    fn a_connector_is_named_the_way_the_compositor_names_its_output() {
        let displays = parse_detect("Display 3\n   DRM connector:    card0-DP-3\n");

        assert_eq!(displays[0].connector, "DP-3");
    }

    #[test]
    fn nothing_detected_is_an_empty_list() {
        assert!(parse_detect("").is_empty());
        assert!(parse_detect("ddcutil: no displays found\n").is_empty());
    }

    #[test]
    fn brightness_is_percent_of_the_monitors_own_maximum() {
        assert_eq!(parse_brightness("VCP 10 C 50 100\n"), Some(50));
        // A monitor whose range is not 0-100 still reports a percentage.
        assert_eq!(parse_brightness("VCP 10 C 40 80\n"), Some(50));
        assert_eq!(parse_brightness("VCP 10 C 0 100\n"), Some(0));
    }

    #[test]
    fn an_answer_that_is_not_a_brightness_reading_is_no_reading() {
        // Another feature, a short line, a monitor reporting a zero range, and
        // an error where a reading should be.
        assert_eq!(parse_brightness("VCP 12 C 50 100\n"), None);
        assert_eq!(parse_brightness("VCP 10 C 50\n"), None);
        assert_eq!(parse_brightness("VCP 10 C 50 0\n"), None);
        assert_eq!(parse_brightness("DDC communication failed\n"), None);
        assert_eq!(parse_brightness(""), None);
    }
}
