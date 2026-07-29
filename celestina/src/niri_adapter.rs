//! Read-only Niri event-stream adapter for the shell's first CP1 slice.
//!
//! Niri owns the event model and already provides a state reducer. This helper
//! keeps those compositor types out of Qt, reduces every relevant event to a
//! small workspace snapshot and reconnects after IPC loss. The C++ host only
//! marshals this line-delimited protocol onto the GUI thread.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use niri_ipc::socket::Socket;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use niri_ipc::{Event, Request, Response};
use serde::Serialize;

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Serialize)]
struct WorkspaceSnapshot {
    index: u8,
    label: String,
    output: Option<String>,
    active: bool,
    focused: bool,
    urgent: bool,
    active_window_title: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
struct ShellSnapshot {
    kind: &'static str,
    workspaces: Vec<WorkspaceSnapshot>,
}

#[derive(Serialize)]
struct Unavailable<'a> {
    kind: &'static str,
    reason: &'a str,
}

#[derive(Debug)]
enum AdapterError {
    Connect(io::Error),
    Request(io::Error),
    Rejected(String),
    Stream(io::Error),
    Encode(serde_json::Error),
    Flush(io::Error),
}

impl Display for AdapterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(error) => write!(formatter, "cannot connect to Niri IPC: {error}"),
            Self::Request(error) => {
                write!(formatter, "cannot request Niri's event stream: {error}")
            }
            Self::Rejected(message) => {
                write!(formatter, "Niri rejected the event stream: {message}")
            }
            Self::Stream(error) => write!(formatter, "Niri event stream ended: {error}"),
            Self::Encode(error) => write!(formatter, "cannot encode shell snapshot: {error}"),
            Self::Flush(error) => write!(formatter, "cannot flush shell snapshot: {error}"),
        }
    }
}

impl Error for AdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error)
            | Self::Request(error)
            | Self::Stream(error)
            | Self::Flush(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Rejected(_) => None,
        }
    }
}

fn shell_snapshot(state: &EventStreamState) -> ShellSnapshot {
    let mut workspaces = state
        .workspaces
        .workspaces
        .values()
        .map(|workspace| {
            let active_window = workspace
                .active_window_id
                .and_then(|id| state.windows.windows.get(&id));

            WorkspaceSnapshot {
                index: workspace.idx,
                label: workspace
                    .name
                    .clone()
                    .unwrap_or_else(|| workspace.idx.to_string()),
                output: workspace.output.clone(),
                active: workspace.is_active,
                focused: workspace.is_focused,
                urgent: workspace.is_urgent,
                active_window_title: active_window.and_then(|window| window.title.clone()),
            }
        })
        .collect::<Vec<_>>();

    workspaces.sort_by(|left, right| {
        left.output
            .cmp(&right.output)
            .then(left.index.cmp(&right.index))
    });

    ShellSnapshot {
        kind: "snapshot",
        workspaces,
    }
}

fn emit_json<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), AdapterError> {
    serde_json::to_writer(&mut *writer, value).map_err(AdapterError::Encode)?;
    writer.write_all(b"\n").map_err(AdapterError::Flush)?;
    writer.flush().map_err(AdapterError::Flush)
}

fn stream_session<W: Write>(
    writer: &mut W,
    emitted_snapshot: &mut bool,
) -> Result<(), AdapterError> {
    let mut socket = Socket::connect().map_err(AdapterError::Connect)?;
    let reply = socket
        .send(Request::EventStream)
        .map_err(AdapterError::Request)?;

    match reply {
        Ok(Response::Handled) => {}
        Ok(response) => {
            return Err(AdapterError::Rejected(format!(
                "unexpected response: {response:?}"
            )));
        }
        Err(message) => return Err(AdapterError::Rejected(message)),
    }

    let mut read_event = socket.read_events();
    let mut state = EventStreamState::default();
    let mut have_workspaces = false;
    let mut have_windows = false;
    let mut last_snapshot = None;

    loop {
        let event = read_event().map_err(AdapterError::Stream)?;
        match &event {
            Event::WorkspacesChanged { .. } => have_workspaces = true,
            Event::WindowsChanged { .. } => have_windows = true,
            _ => {}
        }
        state.apply(event);

        if have_workspaces && have_windows {
            let snapshot = shell_snapshot(&state);
            if last_snapshot.as_ref() != Some(&snapshot) {
                emit_json(writer, &snapshot)?;
                *emitted_snapshot = true;
                last_snapshot = Some(snapshot);
            }
        }
    }
}

fn run() -> Result<(), AdapterError> {
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    let mut last_error: Option<String> = None;

    loop {
        let mut emitted_snapshot = false;
        let error = match stream_session(&mut writer, &mut emitted_snapshot) {
            Ok(()) => AdapterError::Rejected("event stream ended without an error".into()),
            Err(error) => error,
        };
        let reason = error.to_string();

        if emitted_snapshot || last_error.as_deref() != Some(reason.as_str()) {
            eprintln!("celestina-niri-adapter: {reason}");
            emit_json(
                &mut writer,
                &Unavailable {
                    kind: "unavailable",
                    reason: &reason,
                },
            )?;
        }
        last_error = Some(reason);
        thread::sleep(RECONNECT_DELAY);
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("celestina-niri-adapter: fatal: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_json(state: &mut EventStreamState, json: &str) {
        let event = serde_json::from_str::<Event>(json).expect("valid Niri event fixture");
        state.apply(event);
    }

    #[test]
    fn snapshot_is_sorted_and_joins_the_active_window() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":8,"idx":2,"name":null,"output":"DP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null},{"id":3,"idx":1,"name":"web","output":"DP-1","is_urgent":true,"is_active":true,"is_focused":true,"active_window_id":42}]}}"#,
        );
        apply_json(
            &mut state,
            r#"{"WindowsChanged":{"windows":[{"id":42,"title":"Niri docs","app_id":"zen","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}]}}"#,
        );

        let snapshot = shell_snapshot(&state);
        assert_eq!(snapshot.workspaces.len(), 2);
        assert_eq!(snapshot.workspaces[0].label, "web");
        assert_eq!(
            snapshot.workspaces[0].active_window_title.as_deref(),
            Some("Niri docs")
        );
        assert_eq!(snapshot.workspaces[1].label, "2");
    }

    #[test]
    fn snapshot_changes_after_incremental_window_event() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"one","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":42}]}}"#,
        );
        apply_json(
            &mut state,
            r#"{"WindowsChanged":{"windows":[{"id":42,"title":"Terminal","app_id":"kitty","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}]}}"#,
        );
        let before = shell_snapshot(&state);

        apply_json(
            &mut state,
            r#"{"WindowOpenedOrChanged":{"window":{"id":42,"title":"Editor","app_id":"kitty","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}}}"#,
        );
        let after = shell_snapshot(&state);

        assert_ne!(before, after);
        assert_eq!(
            after.workspaces[0].active_window_title.as_deref(),
            Some("Editor")
        );
    }
}
