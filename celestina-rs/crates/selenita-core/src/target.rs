//! What a capture takes and where it goes.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use crate::geometry::Geometry;

/// What a capture takes, once resolved: an output, the focused window or a
/// region the person drew.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    /// One output by name, or every output when the name is empty.
    Screen {
        output: String,
    },
    /// The focused window. niri gives its size always and its position only
    /// for a floating window; `geometry` is at `0,0` when the position is
    /// unknown, and the window is then taken through niri itself.
    Window {
        /// niri's window id, which its `screenshot-window --id` takes.
        id: u64,
        geometry: Geometry,
        app_id: String,
        title: String,
    },
    Region {
        geometry: Geometry,
    },
}

impl Target {
    #[must_use]
    pub fn kind(&self) -> TargetKind {
        match self {
            Self::Screen { .. } => TargetKind::Screen,
            Self::Window { .. } => TargetKind::Window,
            Self::Region { .. } => TargetKind::Region,
        }
    }
}

/// The three choices before they are resolved: the words of the capture
/// card, of `--screenshot` and of `org.celestina.Selenita1.Capture`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TargetKind {
    Screen,
    Window,
    Region,
}

impl TargetKind {
    pub const ALL: [Self; 3] = [Self::Screen, Self::Window, Self::Region];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Window => "window",
            Self::Region => "region",
        }
    }

    /// Whether Selenita's own window, when shown, would be in the picture or
    /// in the way. A screen or a region shows whatever is on screen, the
    /// window included; the window target takes the focused window, which is
    /// Selenita itself while the person presses its button. So the window
    /// steps aside for every target whenever it is shown.
    #[must_use]
    pub fn hides_own_window(self, window_shown: bool) -> bool {
        window_shown
    }
}

/// Whether the picture must wait for Selenita's hidden window to leave the
/// screen: only when the target may show the output Selenita was on. A region
/// may be drawn anywhere; a screen capture of every output (`""`) includes
/// it; an unknown output on either side counts as the same one.
#[must_use]
pub fn needs_settle(
    kind: TargetKind,
    target_output: Option<&str>,
    own_output: Option<&str>,
) -> bool {
    match (kind, target_output, own_output) {
        (TargetKind::Region, _, _) | (_, None, _) | (_, _, None) => true,
        (TargetKind::Screen, Some(""), _) => true,
        (_, Some(target), Some(own)) => target == own,
    }
}

impl fmt::Display for TargetKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A word that is not `screen`, `window` or `region`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownTarget(pub String);

impl fmt::Display for UnknownTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown capture target {:?} (screen, window or region)",
            self.0
        )
    }
}

impl std::error::Error for UnknownTarget {}

impl FromStr for TargetKind {
    type Err = UnknownTarget;

    fn from_str(word: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == word)
            .ok_or_else(|| UnknownTarget(word.to_owned()))
    }
}

/// One capture as asked: the resolved target, the delay before it and its
/// destinations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capture {
    pub target: Target,
    pub delay: Duration,
    pub to_clipboard: bool,
    pub to_file: bool,
}

/// The delays the capture card offers, in seconds.
pub const DELAYS: [u32; 4] = [0, 3, 5, 10];

/// `seconds` if it is one of [`DELAYS`], else no delay: a stray value from
/// QML never makes the person wait an unexpected time.
#[must_use]
pub fn delay_from_seconds(seconds: i64) -> Duration {
    DELAYS
        .into_iter()
        .find(|known| i64::from(*known) == seconds)
        .map_or(Duration::ZERO, |known| {
            Duration::from_secs(u64::from(known))
        })
}
