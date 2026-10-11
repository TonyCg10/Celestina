//! The one-shot output request a focused recording leaves for the portal's
//! output chooser (SEL-4-A).
//!
//! `selenita --record-focused` records the monitor niri has focused without
//! the output dialog. The ScreenCast portal's backend, xdg-desktop-portal-wlr,
//! asks its configured chooser (`chooser_cmd`) which output to cast and reads
//! one line from its standard output, `Monitor: <name>` or the bare name.
//! With `selenita --pick-output` as that chooser, the recording worker writes
//! a request naming the focused output in [`request_file`] just before it
//! asks the portal, and the chooser consumes it: a request at most
//! [`FRESHNESS`] old answers at once, anything else (no request, a stale or
//! unreadable one) falls back to the previous chooser and its dialog.
//!
//! The request is two `key=value` lines, the output's name and when it was
//! written in milliseconds since the epoch:
//!
//! ```text
//! output=DP-1
//! written=1760140325123
//! ```
//!
//! It lives in the private `$XDG_RUNTIME_DIR/selenita` folder, is written
//! atomically (`0600`) and is consumed by renaming it away first, so two
//! choosers never both take it. Every function here that touches the file
//! blocks.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use celestina_core::atomic_file::{self, ReadError, WriteError};
use celestina_core::xdg::{self, PrivateDirError};

/// The request's file name inside `$XDG_RUNTIME_DIR/selenita`.
pub const REQUEST_NAME: &str = "pick-output";
/// How old a request may be and still answer: the portal runs the chooser
/// within a second of the request; a request older than this belongs to a
/// start that never reached the chooser.
pub const FRESHNESS: Duration = Duration::from_secs(30);
/// What the chooser prints before the output's name.
pub const CHOOSER_PREFIX: &str = "Monitor: ";
/// The longest output name a request may carry, in bytes.
pub const NAME_LIMIT: usize = 128;
/// The most a request file may hold; anything longer is not one.
const READ_LIMIT: u64 = 512;

const OUTPUT_KEY: &str = "output";
const WRITTEN_KEY: &str = "written";

/// The request file in the runtime folder `runtime_dir`.
#[must_use]
pub fn request_file(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("selenita").join(REQUEST_NAME)
}

/// Whether `name` can be an output's name in a request: 1 to
/// [`NAME_LIMIT`] bytes with no whitespace and no control character, so the
/// chooser's one line stays one line.
#[must_use]
pub fn is_output_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_LIMIT
        && !name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

/// One request: the output to cast and when it was asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickRequest {
    output: String,
    written: SystemTime,
}

impl PickRequest {
    /// A request for `output`, written at `written`.
    ///
    /// # Errors
    ///
    /// [`PickError::InvalidOutput`] for a name [`is_output_name`] refuses.
    pub fn new(output: &str, written: SystemTime) -> Result<Self, PickError> {
        if !is_output_name(output) {
            return Err(PickError::InvalidOutput(output.to_owned()));
        }
        Ok(Self {
            output: output.to_owned(),
            written,
        })
    }

    /// The output's name.
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }

    /// When the request was written.
    #[must_use]
    pub fn written(&self) -> SystemTime {
        self.written
    }

    /// The file's text.
    #[must_use]
    pub fn encode(&self) -> String {
        let millis = self
            .written
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        format!("{OUTPUT_KEY}={}\n{WRITTEN_KEY}={millis}\n", self.output)
    }

    /// The request in a file's `text`; unknown keys are ignored, each known
    /// one must appear once.
    ///
    /// # Errors
    ///
    /// [`PickError::Malformed`] for a missing, repeated or unreadable key,
    /// [`PickError::InvalidOutput`] for a name [`is_output_name`] refuses.
    pub fn parse(text: &str) -> Result<Self, PickError> {
        let mut output = None;
        let mut written = None;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let slot = match key {
                OUTPUT_KEY => &mut output,
                WRITTEN_KEY => &mut written,
                _ => continue,
            };
            if slot.replace(value).is_some() {
                return Err(PickError::Malformed(format!("{key} appears twice")));
            }
        }
        let output = output.ok_or_else(|| PickError::Malformed("no output".to_owned()))?;
        let millis = written
            .ok_or_else(|| PickError::Malformed("no time".to_owned()))?
            .parse::<u64>()
            .map_err(|error| PickError::Malformed(format!("time: {error}")))?;
        Self::new(output, UNIX_EPOCH + Duration::from_millis(millis))
    }

    /// Whether the request still answers at `now`: written at most
    /// [`FRESHNESS`] before it. A request from the future (the clock moved
    /// back) is not fresh: the dialog is the safe answer.
    #[must_use]
    pub fn is_fresh(&self, now: SystemTime) -> bool {
        now.duration_since(self.written)
            .is_ok_and(|age| age <= FRESHNESS)
    }

    /// The line the chooser prints: `Monitor: <name>`.
    #[must_use]
    pub fn chooser_line(&self) -> String {
        format!("{CHOOSER_PREFIX}{}", self.output)
    }
}

/// Why a request could not be written or read.
#[derive(Debug)]
pub enum PickError {
    /// The output's name cannot go in a request.
    InvalidOutput(String),
    /// The file is not a request.
    Malformed(String),
    /// The private folder could not be made or is not private.
    Folder(PrivateDirError),
    Write(WriteError),
    Read(ReadError),
    /// The request could not be claimed (renamed away) or removed.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for PickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput(name) => write!(formatter, "not an output name: {name:?}"),
            Self::Malformed(detail) => write!(formatter, "not a pick request: {detail}"),
            Self::Folder(error) => write!(formatter, "pick request folder: {error}"),
            Self::Write(error) => write!(formatter, "pick request: {error}"),
            Self::Read(error) => write!(formatter, "pick request: {error}"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} {}: {source}", path.display()),
        }
    }
}

impl Error for PickError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Folder(error) => Some(error),
            Self::Write(error) => Some(error),
            Self::Read(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::InvalidOutput(_) | Self::Malformed(_) => None,
        }
    }
}

/// Writes a request for `output` at `now` to `file`, making its folder
/// private (`0700`) and replacing any request already there.
///
/// # Errors
///
/// [`PickError::InvalidOutput`] for a name that cannot go in a request, the
/// folder's or the write's failure otherwise; `file` is then unchanged.
pub fn write_request(file: &Path, output: &str, now: SystemTime) -> Result<(), PickError> {
    let request = PickRequest::new(output, now)?;
    if let Some(folder) = file.parent() {
        xdg::ensure_private_dir(folder).map_err(PickError::Folder)?;
    }
    atomic_file::replace_private(file, request.encode().as_bytes())
        .map(|_| ())
        .map_err(PickError::Write)
}

/// What the chooser found at `file`.
#[derive(Debug)]
pub enum Taken {
    /// No request.
    Absent,
    /// A request that answers.
    Fresh(PickRequest),
    /// A request too old (or from the future): removed and ignored.
    Stale(PickRequest),
    /// A file that is not a request: removed and ignored.
    Invalid(PickError),
}

/// Consumes the request at `file`: it is claimed by renaming it to a name of
/// this process's own, read (bounded) and removed, whatever it holds.
///
/// # Errors
///
/// The claim failed for another reason than there being no request.
pub fn take_request(file: &Path, now: SystemTime) -> Result<Taken, PickError> {
    let claimed = file.with_file_name(format!("{REQUEST_NAME}.taken-{}", std::process::id()));
    match std::fs::rename(file, &claimed) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Taken::Absent),
        Err(source) => {
            return Err(PickError::Io {
                operation: "claiming",
                path: file.to_path_buf(),
                source,
            })
        }
    }
    let read = atomic_file::read_bounded(&claimed, READ_LIMIT);
    // Claimed under this process's own name, the file can no longer answer
    // anyone; a removal that fails leaves only that inert name behind.
    let _ = std::fs::remove_file(&claimed);
    let bytes = match read {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Ok(Taken::Absent),
        Err(error) => return Ok(Taken::Invalid(PickError::Read(error))),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(Taken::Invalid(PickError::Malformed("not UTF-8".to_owned())));
    };
    Ok(match PickRequest::parse(&text) {
        Ok(request) if request.is_fresh(now) => Taken::Fresh(request),
        Ok(request) => Taken::Stale(request),
        Err(error) => Taken::Invalid(error),
    })
}

/// Removes a request nobody consumed; whether there was one.
///
/// # Errors
///
/// The removal failed for another reason than there being no request.
pub fn clear_request(file: &Path) -> Result<bool, PickError> {
    match std::fs::remove_file(file) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(PickError::Io {
            operation: "removing",
            path: file.to_path_buf(),
            source,
        }),
    }
}
