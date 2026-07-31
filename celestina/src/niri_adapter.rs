//! Niri event-stream adapter for the shell.
//!
//! Niri owns the event model and already provides a state reducer. This helper
//! keeps those compositor types out of Qt, reduces every relevant event to a
//! small workspace snapshot and reconnects after IPC loss. The C++ host only
//! marshals this line-delimited protocol onto the GUI thread.
//!
//! The protocol has two directions. Downstream (helper → host) carries
//! `snapshot`, `unavailable` and `request` frames. Upstream (host → helper)
//! carries bounded focus requests: a request is read by a dedicated stdin
//! reader, queued on a bounded channel and performed by one worker over a
//! short-lived second socket, so a slow action never stalls the event stream.
//! A single shared writer serializes every frame — snapshots and request
//! results can never interleave on stdout.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufReader, BufWriter, Stdout};
use std::process::{self, ExitCode};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use celestina_shell_core::bounded;
use celestina_shell_core::lines::{read_bounded_line, HostLine, SharedWriter, WriteError};
use niri_ipc::socket::Socket;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use niri_ipc::{Action, Event, Request, Response, WorkspaceReferenceArg};
use serde::{Deserialize, Serialize};

const RECONNECT_DELAY: Duration = Duration::from_secs(1);
/// The host may not outrun the compositor: further requests are refused with a
/// visible failure instead of growing an unbounded backlog.
const COMMAND_QUEUE_CAPACITY: usize = 32;
/// Request ids are opaque to this helper — it only echoes them back — but a
/// bounded length keeps a hostile id out of the downstream frames.
const MAX_ID_CHARS: usize = 32;
const MAX_REASON_CHARS: usize = 200;

/// Every frame leaves through this one writer, so a request result can never
/// land in the middle of a snapshot line.
type AdapterWriter = Arc<SharedWriter<BufWriter<Stdout>>>;

#[derive(Debug, PartialEq, Serialize)]
struct WorkspaceSnapshot {
    /// Niri's stable workspace id, carried as a decimal string: it is a `u64`
    /// whose values the compositor explicitly refuses to constrain, and JSON
    /// numbers reach the Qt host as doubles. The host never interprets it — it
    /// echoes the string back when it requests focus.
    id: String,
    index: u8,
    label: String,
    output: String,
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

/// The result of one host request. `accepted` means the compositor handled the
/// request; it is not proof that the requested workspace is now active — only a
/// later snapshot can show that, and only the host correlates the two.
#[derive(Debug, PartialEq, Serialize)]
struct RequestFrame<'a> {
    kind: &'static str,
    id: &'a str,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl<'a> RequestFrame<'a> {
    fn accepted(id: &'a str) -> Self {
        Self {
            kind: "request",
            id,
            state: "accepted",
            reason: None,
        }
    }

    fn failed(id: &'a str, reason: &str) -> Self {
        Self {
            kind: "request",
            id,
            state: "failed",
            reason: Some(bounded(reason, MAX_REASON_CHARS)),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum HostCommand {
    /// Focus the workspace whose Niri id the host read from a snapshot.
    FocusWorkspace { id: String, workspace: String },
    /// Open Niri's own screenshot UI. The shell asks the compositor to
    /// capture; it never captures anything itself.
    Screenshot { id: String },
}

impl HostCommand {
    fn id(&self) -> &str {
        match self {
            Self::FocusWorkspace { id, .. } | Self::Screenshot { id } => id,
        }
    }
}

/// Enough of a command to answer a malformed one. A host that sends an
/// unreadable frame still learns its request failed instead of waiting out the
/// host-side timeout.
#[derive(Debug, Deserialize)]
struct CommandEnvelope {
    id: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Rejection {
    id: Option<String>,
    reason: String,
}

#[derive(Debug)]
enum AdapterError {
    Connect(io::Error),
    Request(io::Error),
    Rejected(String),
    Stream(io::Error),
    Emit(WriteError),
    Spawn(io::Error),
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
            Self::Emit(error) => write!(formatter, "cannot publish a shell frame: {error}"),
            Self::Spawn(error) => write!(formatter, "cannot start an adapter thread: {error}"),
        }
    }
}

impl Error for AdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error)
            | Self::Request(error)
            | Self::Stream(error)
            | Self::Spawn(error) => Some(error),
            Self::Emit(error) => Some(error),
            Self::Rejected(_) => None,
        }
    }
}

fn shell_snapshot(state: &EventStreamState) -> ShellSnapshot {
    let mut workspaces = state
        .workspaces
        .workspaces
        .values()
        // The shell renders one strip per physical output. Niri can briefly
        // retain an unassigned workspace while outputs change; that state is
        // valid compositor data but has no panel consumer, so keep it out of
        // the inter-process contract instead of serializing a nullable output.
        .filter_map(|workspace| {
            let output = workspace.output.clone()?;
            let active_window = workspace
                .active_window_id
                .and_then(|id| state.windows.windows.get(&id));

            Some(WorkspaceSnapshot {
                id: workspace.id.to_string(),
                index: workspace.idx,
                label: workspace
                    .name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| workspace.idx.to_string()),
                output,
                active: workspace.is_active,
                focused: workspace.is_focused,
                urgent: workspace.is_urgent,
                active_window_title: active_window.and_then(|window| window.title.clone()),
            })
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

fn emit_json<T: Serialize>(writer: &AdapterWriter, value: &T) -> Result<(), AdapterError> {
    writer.emit(value).map_err(AdapterError::Emit)
}

fn stream_session(writer: &AdapterWriter, emitted_snapshot: &mut bool) -> Result<(), AdapterError> {
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

fn stream_forever(writer: &AdapterWriter) -> Result<(), AdapterError> {
    let mut last_error: Option<String> = None;

    loop {
        let mut emitted_snapshot = false;
        let error = match stream_session(writer, &mut emitted_snapshot) {
            Ok(()) => AdapterError::Rejected("event stream ended without an error".into()),
            Err(error) => error,
        };
        let reason = error.to_string();

        if emitted_snapshot || last_error.as_deref() != Some(reason.as_str()) {
            eprintln!("celestina-niri-adapter: {reason}");
            emit_json(
                writer,
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

fn parse_command(line: &[u8]) -> Result<HostCommand, Rejection> {
    let command = serde_json::from_slice::<HostCommand>(line).map_err(|error| Rejection {
        id: serde_json::from_slice::<CommandEnvelope>(line)
            .ok()
            .and_then(|envelope| envelope.id)
            .filter(|id| !id.is_empty() && id.chars().count() <= MAX_ID_CHARS),
        reason: bounded(&error.to_string(), MAX_REASON_CHARS),
    })?;

    let id = command.id();
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(Rejection {
            // An unusable id cannot be echoed back: answering it would put
            // unbounded host input into a downstream frame.
            id: None,
            reason: "the command carries no usable request id".to_owned(),
        });
    }

    Ok(command)
}

/// One action, one short-lived socket. Every compositor request the host makes
/// goes through here, so "accepted" means the same thing for all of them: Niri
/// took it, not that the session already looks different.
fn perform(action: Action) -> Result<(), String> {
    let mut socket =
        Socket::connect().map_err(|error| format!("cannot connect to Niri: {error}"))?;
    let reply = socket
        .send(Request::Action(action))
        .map_err(|error| format!("cannot send the request: {error}"))?;

    match reply {
        Ok(Response::Handled) => Ok(()),
        Ok(response) => Err(format!("unexpected response: {response:?}")),
        Err(message) => Err(message),
    }
}

fn focus_workspace(workspace: &str) -> Result<(), String> {
    let id = workspace
        .parse::<u64>()
        .map_err(|_| "the request names an invalid workspace id".to_owned())?;

    perform(Action::FocusWorkspace {
        reference: WorkspaceReferenceArg::Id(id),
    })
}

/// Asks Niri to start its own screenshot UI, which saves where the session's
/// `screenshot-path` already points. Capture belongs to the compositor: the
/// shell asks for it and reimplements none of it.
fn screenshot() -> Result<(), String> {
    perform(Action::Screenshot {
        // Niri's own default, so the panel's button does exactly what the
        // compositor's screenshot action does; its UI still toggles the pointer.
        show_pointer: true,
        // `None` means the configured `screenshot-path`.
        path: None,
    })
}

/// Performs one queued action at a time over its own short-lived socket, so a
/// blocked or slow compositor round-trip cannot stall the event stream.
fn run_commands(receiver: &Receiver<HostCommand>, writer: &AdapterWriter) {
    while let Ok(command) = receiver.recv() {
        let outcome = match &command {
            HostCommand::FocusWorkspace { workspace, .. } => focus_workspace(workspace),
            HostCommand::Screenshot { .. } => screenshot(),
        };
        let frame = match &outcome {
            Ok(()) => RequestFrame::accepted(command.id()),
            Err(reason) => RequestFrame::failed(command.id(), reason),
        };
        if let Err(error) = emit_json(writer, &frame) {
            eprintln!("celestina-niri-adapter: {error}");
            return;
        }
    }
}

fn reject(writer: &AdapterWriter, rejection: &Rejection) {
    let Some(id) = rejection.id.as_deref() else {
        eprintln!(
            "celestina-niri-adapter: ignored an unusable host command: {}",
            rejection.reason
        );
        return;
    };
    if let Err(error) = emit_json(writer, &RequestFrame::failed(id, &rejection.reason)) {
        eprintln!("celestina-niri-adapter: {error}");
    }
}

fn queue_command(sender: &SyncSender<HostCommand>, writer: &AdapterWriter, command: HostCommand) {
    match sender.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id().to_owned()),
                reason: "the adapter's request queue is full".to_owned(),
            },
        ),
        Err(TrySendError::Disconnected(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id().to_owned()),
                reason: "the adapter's request worker is gone".to_owned(),
            },
        ),
    }
}

fn read_host_commands(sender: &SyncSender<HostCommand>, writer: &AdapterWriter) {
    let mut reader = BufReader::new(io::stdin());

    loop {
        match read_bounded_line(&mut reader) {
            Ok(HostLine::End) => return,
            Ok(HostLine::Oversized) => {
                eprintln!("celestina-niri-adapter: discarded an oversized host command");
            }
            Ok(HostLine::Complete(line)) => {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                match parse_command(&line) {
                    Ok(command) => queue_command(sender, writer, command),
                    Err(rejection) => reject(writer, &rejection),
                }
            }
            Err(error) => {
                eprintln!("celestina-niri-adapter: cannot read host commands: {error}");
                return;
            }
        }
    }
}

fn run() -> Result<(), AdapterError> {
    let writer: AdapterWriter = Arc::new(SharedWriter::new(BufWriter::new(io::stdout())));
    let (sender, receiver) = sync_channel::<HostCommand>(COMMAND_QUEUE_CAPACITY);

    let worker_writer = Arc::clone(&writer);
    let worker = thread::Builder::new()
        .name("niri-actions".to_owned())
        .spawn(move || run_commands(&receiver, &worker_writer))
        .map_err(AdapterError::Spawn)?;

    let reader_writer = Arc::clone(&writer);
    thread::Builder::new()
        .name("host-commands".to_owned())
        .spawn(move || {
            read_host_commands(&sender, &reader_writer);
            // Our stdin closed, so the host is gone or shutting down. Close the
            // queue and let the worker finish the action it may still hold —
            // that much shuts down deterministically. The event stream cannot:
            // niri-ipc's blocking read has no cancellation point, so once the
            // worker is joined the adapter leaves on purpose instead of
            // outliving the shell that owns it.
            drop(sender);
            if worker.join().is_err() {
                eprintln!("celestina-niri-adapter: the request worker panicked");
            }
            process::exit(0);
        })
        .map_err(AdapterError::Spawn)?;

    stream_forever(&writer)
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

    use std::io::Cursor;

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

    #[test]
    fn snapshot_omits_workspaces_without_an_output() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"detached","output":null,"is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );

        assert!(shell_snapshot(&state).workspaces.is_empty());
    }

    #[test]
    fn snapshot_carries_the_workspace_id_as_a_string() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":18446744073709551615,"idx":1,"name":"big","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );

        let snapshot = shell_snapshot(&state);
        assert_eq!(snapshot.workspaces[0].id, "18446744073709551615");
        let encoded = serde_json::to_string(&snapshot).expect("the snapshot serializes");
        // A JSON number would reach the host's double-typed parser rounded.
        assert!(encoded.contains(r#""id":"18446744073709551615""#));
    }

    #[test]
    fn a_focus_command_parses_its_request_and_workspace_ids() {
        let command = parse_command(br#"{"kind":"focus-workspace","id":"7","workspace":"12"}"#)
            .expect("a well-formed command");

        assert_eq!(
            command,
            HostCommand::FocusWorkspace {
                id: "7".to_owned(),
                workspace: "12".to_owned(),
            }
        );
        assert_eq!(command.id(), "7");
    }

    #[test]
    fn a_screenshot_command_carries_only_its_request_id() {
        // The compositor decides everything else — where it saves, whether the
        // pointer shows — so the host has nothing else to send.
        let command =
            parse_command(br#"{"kind":"screenshot","id":"7"}"#).expect("a well-formed command");

        assert_eq!(command, HostCommand::Screenshot { id: "7".to_owned() });
        assert_eq!(command.id(), "7");
    }

    #[test]
    fn an_unreadable_command_is_rejected_against_its_own_request_id() {
        let rejection = parse_command(br#"{"kind":"launch-rocket","id":"9"}"#)
            .expect_err("an unknown command kind is refused");

        assert_eq!(rejection.id.as_deref(), Some("9"));
        assert!(!rejection.reason.is_empty());
    }

    #[test]
    fn a_command_without_a_usable_id_is_never_echoed_back() {
        let long = "x".repeat(MAX_ID_CHARS + 1);
        let line = format!(r#"{{"kind":"focus-workspace","id":"{long}","workspace":"12"}}"#);

        let rejection = parse_command(line.as_bytes()).expect_err("an oversized id is refused");
        assert_eq!(rejection.id, None);
    }

    #[test]
    fn a_command_survives_the_line_the_framing_discarded() {
        // The framing itself belongs to the shared crate now; what belongs here
        // is that this adapter's own commands still parse on the far side of a
        // discarded line.
        let mut input = Vec::new();
        input.extend(std::iter::repeat_n(
            b'x',
            celestina_shell_core::lines::MAX_LINE_BYTES + 64,
        ));
        input.push(b'\n');
        input.extend_from_slice(br#"{"kind":"focus-workspace","id":"7","workspace":"12"}"#);
        input.push(b'\n');
        let mut reader = Cursor::new(input);

        assert_eq!(
            read_bounded_line(&mut reader).expect("reading does not fail"),
            HostLine::Oversized
        );
        let HostLine::Complete(line) =
            read_bounded_line(&mut reader).expect("reading does not fail")
        else {
            panic!("the command after an oversized line is lost");
        };
        assert_eq!(
            parse_command(&line).expect("a well-formed command").id(),
            "7"
        );
    }

    #[test]
    fn a_failed_request_frame_carries_a_bounded_reason() {
        let frame = RequestFrame::failed("7", &"z".repeat(MAX_REASON_CHARS + 50));

        assert_eq!(frame.state, "failed");
        assert_eq!(
            frame.reason.as_ref().map(|reason| reason.chars().count()),
            Some(MAX_REASON_CHARS)
        );
    }
}
