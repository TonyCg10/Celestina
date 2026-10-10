//! The niri IPC client: the socket in `$NIRI_SOCKET`, one JSON request line
//! and one JSON reply line, the same exchange `niri msg --json` makes.
//!
//! A reply is `{"Ok":{"<Request>":<value>}}` or `{"Err":"<message>"}`; the
//! parsers read the inner value, which is exactly what `niri msg --json`
//! prints, so the fixtures are captured with it.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::geometry::Geometry;

/// How long one exchange may take: niri answers in microseconds, and a
/// compositor that does not answer must not hold a capture.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// The focused window as niri describes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Window {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    /// The window's own size in logical pixels.
    pub size: (u32, u32),
    /// Where the window sits in its workspace's view, when niri says (a
    /// floating window); `None` for a tiled one.
    pub position: Option<(i32, i32)>,
    pub workspace_id: Option<u64>,
    /// When the window last had the focus, on niri's monotonic clock;
    /// `None` for a window never focused.
    pub focused_at: Option<Duration>,
}

impl Window {
    /// The window's rectangle: at its position in the workspace view when
    /// niri gives one, else at `0,0`.
    #[must_use]
    pub fn geometry(&self) -> Geometry {
        let (x, y) = self.position.unwrap_or((0, 0));
        Geometry {
            x,
            y,
            w: self.size.0,
            h: self.size.1,
        }
    }
}

/// A workspace and the output it is on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workspace {
    pub id: u64,
    pub output: Option<String>,
}

/// The window a window capture means: the most recently focused window that
/// is not `own_app_id` (Selenita itself, which has the focus while its button
/// is pressed). With Selenita not among them it is the focused window.
#[must_use]
pub fn pick_window<'a>(windows: &'a [Window], own_app_id: &str) -> Option<&'a Window> {
    windows
        .iter()
        .filter(|window| window.app_id != own_app_id)
        .filter(|window| window.focused_at.is_some())
        .max_by_key(|window| window.focused_at)
}

/// The output `window` is on, through its workspace.
#[must_use]
pub fn output_of<'a>(window: &Window, workspaces: &'a [Workspace]) -> Option<&'a str> {
    let id = window.workspace_id?;
    workspaces
        .iter()
        .find(|workspace| workspace.id == id)
        .and_then(|workspace| workspace.output.as_deref())
}

/// One enabled output: its connector name and its logical rectangle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Output {
    pub name: String,
    pub logical: Geometry,
}

/// Why niri could not answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NiriError {
    /// `$NIRI_SOCKET` is unset: not a niri session.
    NoSocket,
    /// The socket could not be reached or the exchange broke.
    Io(String),
    /// niri answered with an error.
    Refused(String),
    /// The reply is not what this request answers.
    Protocol(String),
    /// No window has the focus.
    NoFocusedWindow,
}

impl fmt::Display for NiriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSocket => formatter.write_str("NIRI_SOCKET is not set"),
            Self::Io(detail) => write!(formatter, "niri socket: {detail}"),
            Self::Refused(detail) => write!(formatter, "niri refused: {detail}"),
            Self::Protocol(detail) => write!(formatter, "unexpected niri reply: {detail}"),
            Self::NoFocusedWindow => formatter.write_str("no window has the focus"),
        }
    }
}

impl std::error::Error for NiriError {}

/// The socket path from `$NIRI_SOCKET`.
///
/// # Errors
///
/// [`NiriError::NoSocket`] outside a niri session.
pub fn socket() -> Result<PathBuf, NiriError> {
    std::env::var_os("NIRI_SOCKET")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or(NiriError::NoSocket)
}

/// Asks niri for the focused window.
///
/// # Errors
///
/// The exchange's failure, or [`NiriError::NoFocusedWindow`].
pub fn focused_window(socket: &Path) -> Result<Window, NiriError> {
    let value = request(socket, "FocusedWindow")?;
    window_from(&value)
}

/// Asks niri for every window.
///
/// # Errors
///
/// The exchange's failure.
pub fn windows(socket: &Path) -> Result<Vec<Window>, NiriError> {
    windows_from(&request(socket, "Windows")?)
}

/// Asks niri for the workspaces.
///
/// # Errors
///
/// The exchange's failure.
pub fn workspaces(socket: &Path) -> Result<Vec<Workspace>, NiriError> {
    workspaces_from(&request(socket, "Workspaces")?)
}

/// The windows in `niri msg --json windows`'s text.
///
/// # Errors
///
/// [`NiriError::Protocol`] for text that is not that reply.
pub fn parse_windows(json: &str) -> Result<Vec<Window>, NiriError> {
    windows_from(&parse(json)?)
}

/// The workspaces in `niri msg --json workspaces`'s text.
///
/// # Errors
///
/// [`NiriError::Protocol`] for text that is not that reply.
pub fn parse_workspaces(json: &str) -> Result<Vec<Workspace>, NiriError> {
    workspaces_from(&parse(json)?)
}

fn windows_from(value: &Value) -> Result<Vec<Window>, NiriError> {
    value
        .as_array()
        .ok_or_else(|| NiriError::Protocol("windows are not a list".to_owned()))?
        .iter()
        .map(window_from)
        .collect()
}

fn workspaces_from(value: &Value) -> Result<Vec<Workspace>, NiriError> {
    let list = value
        .as_array()
        .ok_or_else(|| NiriError::Protocol("workspaces are not a list".to_owned()))?;
    list.iter()
        .map(|workspace| {
            Ok(Workspace {
                id: workspace
                    .get("id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| NiriError::Protocol("a workspace without an id".to_owned()))?,
                output: workspace
                    .get("output")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// Asks niri for the enabled outputs, left to right.
///
/// # Errors
///
/// The exchange's failure.
pub fn outputs(socket: &Path) -> Result<Vec<Output>, NiriError> {
    let value = request(socket, "Outputs")?;
    outputs_from(&value)
}

/// The focused window in `niri msg --json focused-window`'s text.
///
/// # Errors
///
/// [`NiriError::Protocol`] for text that is not that reply, and
/// [`NiriError::NoFocusedWindow`] for `null`.
pub fn parse_focused_window(json: &str) -> Result<Window, NiriError> {
    window_from(&parse(json)?)
}

/// The enabled outputs in `niri msg --json outputs`'s text, left to right.
///
/// # Errors
///
/// [`NiriError::Protocol`] for text that is not that reply.
pub fn parse_outputs(json: &str) -> Result<Vec<Output>, NiriError> {
    outputs_from(&parse(json)?)
}

fn parse(json: &str) -> Result<Value, NiriError> {
    serde_json::from_str(json).map_err(|error| NiriError::Protocol(error.to_string()))
}

/// One exchange: the request's name as a JSON string on a line, the reply's
/// line back, unwrapped to the value under `Ok.<name>`.
fn request(socket: &Path, name: &str) -> Result<Value, NiriError> {
    let io = |error: std::io::Error| NiriError::Io(error.to_string());
    let mut stream = UnixStream::connect(socket).map_err(io)?;
    stream.set_read_timeout(Some(TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(TIMEOUT)).map_err(io)?;
    stream
        .write_all(format!("\"{name}\"\n").as_bytes())
        .map_err(io)?;
    stream.flush().map_err(io)?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(io)?;
    unwrap_reply(&line, name)
}

/// The value a reply line carries for the request `name`.
///
/// # Errors
///
/// [`NiriError::Refused`] for an `Err` reply, [`NiriError::Protocol`] for
/// anything else that is not `{"Ok":{name: value}}`.
pub fn unwrap_reply(line: &str, name: &str) -> Result<Value, NiriError> {
    let mut reply = parse(line)?;
    if let Some(error) = reply.get("Err") {
        return Err(NiriError::Refused(
            error
                .as_str()
                .map_or_else(|| error.to_string(), str::to_owned),
        ));
    }
    reply
        .get_mut("Ok")
        .and_then(|ok| ok.get_mut(name))
        .map(Value::take)
        .ok_or_else(|| NiriError::Protocol(format!("no Ok.{name} in the reply")))
}

fn window_from(value: &Value) -> Result<Window, NiriError> {
    if value.is_null() {
        return Err(NiriError::NoFocusedWindow);
    }
    let protocol = |what: &str| NiriError::Protocol(format!("focused window without {what}"));
    let id = value
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| protocol("an id"))?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let layout = value.get("layout").ok_or_else(|| protocol("a layout"))?;
    let size = pair(layout.get("window_size"))
        .and_then(|(w, h)| Some((whole_u32(w)?, whole_u32(h)?)))
        .ok_or_else(|| protocol("a window size"))?;
    let offset = pair(layout.get("window_offset_in_tile")).unwrap_or((0.0, 0.0));
    let position = pair(layout.get("tile_pos_in_workspace_view"))
        .and_then(|(x, y)| Some((whole_i32(x + offset.0)?, whole_i32(y + offset.1)?)));
    let focused_at = value.get("focus_timestamp").and_then(|stamp| {
        let secs = stamp.get("secs")?.as_u64()?;
        let nanos = u32::try_from(stamp.get("nanos")?.as_u64()?).ok()?;
        Some(Duration::new(secs, nanos))
    });
    Ok(Window {
        id,
        app_id: text("app_id"),
        title: text("title"),
        size,
        position,
        workspace_id: value.get("workspace_id").and_then(Value::as_u64),
        focused_at,
    })
}

fn outputs_from(value: &Value) -> Result<Vec<Output>, NiriError> {
    let map = value
        .as_object()
        .ok_or_else(|| NiriError::Protocol("outputs are not an object".to_owned()))?;
    let mut outputs: Vec<Output> = map
        .iter()
        .filter_map(|(key, output)| {
            // A disabled output has no logical rectangle and cannot be
            // captured.
            let logical = output.get("logical").filter(|logical| !logical.is_null())?;
            let number = |key: &str| logical.get(key).and_then(Value::as_i64);
            let geometry = Geometry {
                x: i32::try_from(number("x")?).ok()?,
                y: i32::try_from(number("y")?).ok()?,
                w: u32::try_from(number("width")?).ok()?,
                h: u32::try_from(number("height")?).ok()?,
            };
            let name = output
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(key)
                .to_owned();
            Some(Output {
                name,
                logical: geometry,
            })
        })
        .collect();
    outputs.sort_by_key(|output| (output.logical.x, output.logical.y));
    Ok(outputs)
}

fn pair(value: Option<&Value>) -> Option<(f64, f64)> {
    let items = value?.as_array()?;
    match items.as_slice() {
        [a, b] => Some((a.as_f64()?, b.as_f64()?)),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_u32(value: f64) -> Option<u32> {
    let rounded = value.round();
    (0.0..=f64::from(u32::MAX))
        .contains(&rounded)
        .then_some(rounded as u32)
}

#[allow(clippy::cast_possible_truncation)]
fn whole_i32(value: f64) -> Option<i32> {
    let rounded = value.round();
    (f64::from(i32::MIN)..=f64::from(i32::MAX))
        .contains(&rounded)
        .then_some(rounded as i32)
}
