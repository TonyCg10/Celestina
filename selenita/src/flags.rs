//! The command line niri's key bindings use: `--screenshot
//! <screen|window|region>` takes a capture through the running instance (or
//! this launch, when none runs); `--record` and `--stop` are reserved for
//! SEL-1-B. Anything else on the line is ignored: a path handed to Selenita
//! never triggers anything.

use std::ffi::OsString;
use std::fmt;

use selenita_core::target::UnknownTarget;
use selenita_core::TargetKind;

pub const SCREENSHOT: &str = "--screenshot";
pub const RECORD: &str = "--record";
pub const STOP: &str = "--stop";

/// What a launch was asked to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Launch {
    /// Open (or raise) the window.
    Window,
    /// Take a capture of this target.
    Screenshot(TargetKind),
    /// A flag that arrives with a later unit.
    Reserved(&'static str),
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
            return Ok(Launch::Reserved(RECORD));
        }
        if arg == STOP {
            return Ok(Launch::Reserved(STOP));
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
    fn recording_flags_are_reserved_and_paths_ignored() {
        assert_eq!(parse(args(&["--record"])), Ok(Launch::Reserved("--record")));
        assert_eq!(parse(args(&["--stop"])), Ok(Launch::Reserved("--stop")));
        assert_eq!(parse(args(&["/tmp/captura.png"])), Ok(Launch::Window));
        assert_eq!(parse(args(&[])), Ok(Launch::Window));
        assert_eq!(
            parse(args(&["/tmp/a.png", "--screenshot", "screen"])),
            Ok(Launch::Screenshot(TargetKind::Screen))
        );
    }
}
