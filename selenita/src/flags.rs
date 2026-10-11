//! The command line niri's key bindings use: `--screenshot
//! <screen|window|region>` takes a capture through the running instance (or
//! this launch, when none runs); `--record` starts a recording or stops the
//! one under way (`ToggleRecording()`); `--record-focused` does the same
//! but records the monitor niri has focused without the portal's output
//! dialog (`RecordFocused()`, SEL-4-A); `--stop` only stops
//! (`StopRecording()`, or the stop file when no instance answers);
//! `--pick-output` is the portal's output chooser, answered before any Qt,
//! bus or settings work (`picker.rs`). Anything else on the line is
//! ignored: a path handed to Selenita never triggers anything.

use std::ffi::OsString;
use std::fmt;

use selenita_core::target::UnknownTarget;
use selenita_core::TargetKind;

pub const SCREENSHOT: &str = "--screenshot";
pub const RECORD: &str = "--record";
pub const STOP: &str = "--stop";
pub const RECORD_FOCUSED: &str = "--record-focused";
pub const PICK_OUTPUT: &str = "--pick-output";

/// What a launch was asked to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Launch {
    /// Open (or raise) the window.
    Window,
    /// Take a capture of this target.
    Screenshot(TargetKind),
    /// Start a recording, or stop the one under way.
    Record,
    /// Start a recording of the focused monitor, or stop the one under way.
    RecordFocused,
    /// Stop the recording under way, if any.
    Stop,
    /// Answer the portal's output chooser.
    PickOutput,
}

/// A flag used wrongly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlagError {
    /// `--screenshot` with nothing after it.
    MissingTarget,
    UnknownTarget(UnknownTarget),
}

impl fmt::Display for FlagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget => {
                write!(
                    formatter,
                    "{SCREENSHOT} needs a target: screen, window or region"
                )
            }
            Self::UnknownTarget(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for FlagError {}

/// The request in `args` (the arguments after the program name). The first
/// flag decides; `--screenshot=window` reads as `--screenshot window`.
///
/// # Errors
///
/// [`FlagError`] for a `--screenshot` without a valid target.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Launch, FlagError> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str() else {
            continue;
        };
        if arg == SCREENSHOT {
            let word = args.next().ok_or(FlagError::MissingTarget)?;
            return target(&word.to_string_lossy());
        }
        if let Some(word) = arg
            .strip_prefix(SCREENSHOT)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return target(word);
        }
        if arg == RECORD {
            return Ok(Launch::Record);
        }
        if arg == RECORD_FOCUSED {
            return Ok(Launch::RecordFocused);
        }
        if arg == STOP {
            return Ok(Launch::Stop);
        }
        if arg == PICK_OUTPUT {
            return Ok(Launch::PickOutput);
        }
    }
    Ok(Launch::Window)
}

fn target(word: &str) -> Result<Launch, FlagError> {
    word.parse::<TargetKind>()
        .map(Launch::Screenshot)
        .map_err(FlagError::UnknownTarget)
}

#[cfg(test)]
mod tests {
    use super::{parse, FlagError, Launch};
    use selenita_core::TargetKind;
    use std::ffi::OsString;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn each_screenshot_word_maps_to_its_target() {
        assert_eq!(
            parse(args(&["--screenshot", "screen"])),
            Ok(Launch::Screenshot(TargetKind::Screen))
        );
        assert_eq!(
            parse(args(&["--screenshot", "window"])),
            Ok(Launch::Screenshot(TargetKind::Window))
        );
        assert_eq!(
            parse(args(&["--screenshot=region"])),
            Ok(Launch::Screenshot(TargetKind::Region))
        );
    }

    #[test]
    fn a_wrong_screenshot_is_refused() {
        assert_eq!(
            parse(args(&["--screenshot"])),
            Err(FlagError::MissingTarget)
        );
        assert!(matches!(
            parse(args(&["--screenshot", "desktop"])),
            Err(FlagError::UnknownTarget(_))
        ));
    }

    #[test]
    fn recording_flags_are_read_and_paths_ignored() {
        assert_eq!(parse(args(&["--record"])), Ok(Launch::Record));
        assert_eq!(parse(args(&["--stop"])), Ok(Launch::Stop));
        assert_eq!(parse(args(&["--stop", "--record"])), Ok(Launch::Stop));
        assert_eq!(
            parse(args(&["--record-focused"])),
            Ok(Launch::RecordFocused)
        );
        assert_eq!(
            parse(args(&["--record-focused", "--record"])),
            Ok(Launch::RecordFocused)
        );
        assert_eq!(parse(args(&["--record=focused"])), Ok(Launch::Window));
        assert_eq!(parse(args(&["/tmp/captura.png"])), Ok(Launch::Window));
        assert_eq!(parse(args(&[])), Ok(Launch::Window));
        assert_eq!(
            parse(args(&["/tmp/a.png", "--screenshot", "screen"])),
            Ok(Launch::Screenshot(TargetKind::Screen))
        );
    }

    /// The portal runs the chooser with nothing else on the line; a path
    /// before it changes nothing.
    #[test]
    fn the_chooser_flag_is_read() {
        assert_eq!(parse(args(&["--pick-output"])), Ok(Launch::PickOutput));
        assert_eq!(
            parse(args(&["/tmp/a.png", "--pick-output"])),
            Ok(Launch::PickOutput)
        );
    }
}
