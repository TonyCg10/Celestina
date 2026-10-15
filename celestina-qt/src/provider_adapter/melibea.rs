//! Melibea subscription and action transport for the aggregate provider.
//!
//! One cancellable worker owns the long-lived subscription. Restore and close
//! use short-lived connections because Melibea v1 accepts one request per
//! connection. An action is accepted when Melibea accepts it and confirmed
//! only when the subscribed authoritative projection no longer contains the
//! window.

use std::collections::BTreeMap;
use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::command::Outcome;
use celestina_shell_core::melibea::{
    decode_message, encode_request, ActionStatus, BubbleAnchor, ClientEnvelope, ErrorCode, Message,
    Operation, Projection, WindowTransition, MAX_MESSAGE_BYTES,
};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::{Map, Value};

use super::tools::lock_runtime;
use super::worker::Worker;

pub const NAME: &str = "melibea";

const SOCKET_ENV: &str = "MELIBEA_SOCKET";
const RUNTIME_ENV: &str = "XDG_RUNTIME_DIR";
const DEFAULT_SOCKET: &str = "melibea.sock";
const RECONNECT_DELAY: Duration = Duration::from_millis(500);
const IO_POLL: Duration = Duration::from_millis(250);
const ACTION_TIMEOUT: Duration = Duration::from_secs(2);
const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_PENDING: usize = 64;

/// What the authoritative projection must say before a request is confirmed.
///
/// Minimizing puts a window into the minimized set; restoring or closing takes it out. Both are
/// read from Melibea's projection rather than assumed from the action's own reply, so a request
/// only settles once Niri has actually said so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Settles {
    WhenPresent,
    WhenAbsent,
}

impl Settles {
    const fn of(operation: Operation) -> Self {
        match operation {
            Operation::Minimize => Self::WhenPresent,
            Operation::Restore | Operation::Close => Self::WhenAbsent,
        }
    }

    const fn is_satisfied_by(self, present: bool) -> bool {
        match self {
            Self::WhenPresent => present,
            Self::WhenAbsent => !present,
        }
    }
}

#[derive(Clone, Debug)]
struct PendingAction {
    window_id: u64,
    settles: Settles,
    armed: bool,
    deadline: Instant,
}

#[derive(Debug, Default)]
struct BridgeState {
    projection: Projection,
    pending: BTreeMap<String, PendingAction>,
}

impl BridgeState {
    /// Whether an action may be sent at all, checked immediately before socket IO.
    ///
    /// The shell frame can still advertise Melibea as available for a short time after this
    /// subscription has already withdrawn its projection. Acting then would change a window in
    /// Niri while holding no authoritative state from which to publish the result or confirm it,
    /// so the request would sit pending until it expired even though the window really moved.
    ///
    /// Capacity and duplicate ids are checked here too, because refusing after the action has
    /// already been applied reports a failure for something that actually happened.
    fn can_dispatch(&self, request_id: &str) -> Result<(), String> {
        if !self.projection.ready() {
            return Err("Melibea published no authoritative window state to act on".to_owned());
        }
        if self.pending.len() >= MAX_PENDING {
            return Err("too many Melibea actions are awaiting confirmation".to_owned());
        }
        if self.pending.contains_key(request_id) {
            return Err("that Melibea request is already awaiting confirmation".to_owned());
        }
        Ok(())
    }

    fn reserve(
        &mut self,
        request_id: &str,
        window_id: u64,
        settles: Settles,
    ) -> Result<(), String> {
        if self.pending.len() >= MAX_PENDING {
            return Err("too many Melibea actions are awaiting confirmation".to_owned());
        }
        self.pending.insert(
            request_id.to_owned(),
            PendingAction {
                window_id,
                settles,
                armed: false,
                deadline: Instant::now() + CONFIRMATION_TIMEOUT,
            },
        );
        Ok(())
    }

    fn arm(&mut self, request_id: &str) -> Option<Outcome> {
        let pending = self.pending.get_mut(request_id)?;
        pending.armed = true;
        if self.projection.ready()
            && pending
                .settles
                .is_satisfied_by(self.projection.contains(pending.window_id))
        {
            self.pending.remove(request_id);
            return Some(Outcome::confirmed(request_id.to_owned()));
        }
        None
    }

    fn observe(&mut self) -> Vec<Outcome> {
        if !self.projection.ready() {
            return Vec::new();
        }
        let confirmed: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, pending)| {
                pending.armed
                    && pending
                        .settles
                        .is_satisfied_by(self.projection.contains(pending.window_id))
            })
            .map(|(request_id, _)| request_id.clone())
            .collect();
        confirmed
            .into_iter()
            .filter_map(|request_id| {
                self.pending
                    .remove(&request_id)
                    .map(|_| Outcome::confirmed(request_id))
            })
            .collect()
    }

    fn expire(&mut self, now: Instant) -> Vec<Outcome> {
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.armed && pending.deadline <= now)
            .map(|(request_id, _)| request_id.clone())
            .collect();
        expired
            .into_iter()
            .filter_map(|request_id| {
                self.pending.remove(&request_id).map(|_| {
                    Outcome::failed(
                        request_id,
                        "Melibea did not publish the requested state in time",
                    )
                })
            })
            .collect()
    }

    fn fail_all(&mut self, reason: &str) -> Vec<Outcome> {
        // Leave reservations that have not been armed yet in place. The
        // command worker must write `accepted` first; it will then arm and
        // either observe absence or time out. Dropping the reservation here
        // during that narrow race would publish accepted with no terminal
        // result.
        let failed: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.armed)
            .map(|(request_id, _)| request_id.clone())
            .collect();
        failed
            .into_iter()
            .filter_map(|request_id| {
                self.pending
                    .remove(&request_id)
                    .map(|_| Outcome::failed(request_id, reason))
            })
            .collect()
    }
}

fn state() -> &'static Mutex<BridgeState> {
    static STATE: OnceLock<Mutex<BridgeState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BridgeState::default()))
}

fn lock_state() -> std::sync::MutexGuard<'static, BridgeState> {
    match state().lock() {
        Ok(locked) => locked,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn provider_id() -> ProviderId {
    ProviderId::new(NAME).expect("the static Melibea provider name is valid")
}

fn socket_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(SOCKET_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    env::var_os(RUNTIME_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|directory| directory.join(DEFAULT_SOCKET))
        .ok_or_else(|| "neither MELIBEA_SOCKET nor XDG_RUNTIME_DIR is set".to_owned())
}

fn read_bounded_message(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let read = reader
        .take(u64::try_from(MAX_MESSAGE_BYTES + 1).unwrap_or(u64::MAX))
        .read_until(b'\n', &mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.len() > MAX_MESSAGE_BYTES || line.last() != Some(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Melibea sent an oversized or unterminated message",
        ));
    }
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(Some(line))
}

fn window_row(window: &celestina_shell_core::melibea::Window) -> Value {
    let mut row = Map::new();
    // Decimal text is exact across the Rust/helper/Qt/QML seam. A JSON double
    // cannot represent every compositor u64 identity.
    row.insert("id".to_owned(), Value::String(window.id.to_string()));
    if let Some(app_id) = &window.app_id {
        row.insert("appId".to_owned(), Value::String(app_id.clone()));
    }
    if let Some(title) = &window.title {
        row.insert("title".to_owned(), Value::String(title.clone()));
    }
    if let Some(icon_name) = &window.icon_name {
        row.insert("iconName".to_owned(), Value::String(icon_name.clone()));
    }
    Value::Object(row)
}

fn payload(projection: &Projection) -> Payload {
    let mut payload = Payload::new();
    payload.insert("available".to_owned(), Value::Bool(true));
    payload.insert(
        "revision".to_owned(),
        Value::String(projection.revision().unwrap_or_default().to_string()),
    );
    payload.insert(
        "windows".to_owned(),
        Value::Array(projection.windows().iter().map(window_row).collect()),
    );
    payload
}

fn publish_projection(runtime: &Mutex<ProviderRuntime>) -> Result<(), String> {
    let state = lock_state();
    if !state.projection.ready() {
        drop(state);
        lock_runtime(runtime).withdraw(&provider_id());
        return Ok(());
    }
    let reading = payload(&state.projection);
    drop(state);
    lock_runtime(runtime)
        .publish(&provider_id(), reading)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn settle(runtime: &Mutex<ProviderRuntime>, outcomes: Vec<Outcome>) {
    if outcomes.is_empty() {
        return;
    }
    let mut runtime = lock_runtime(runtime);
    for outcome in outcomes {
        runtime.settle(outcome);
    }
}

fn withdraw(runtime: &Mutex<ProviderRuntime>, reason: &str) {
    let outcomes = {
        let mut state = lock_state();
        state.projection = Projection::default();
        state.fail_all(reason)
    };
    let mut runtime = lock_runtime(runtime);
    runtime.withdraw(&provider_id());
    for outcome in outcomes {
        runtime.settle(outcome);
    }
}

fn handle_subscription_message(
    line: &[u8],
    runtime: &Mutex<ProviderRuntime>,
) -> Result<(), String> {
    let message = decode_message(line).map_err(|error| error.to_string())?;
    match message {
        state_message @ (Message::Snapshot { .. } | Message::Changes { .. }) => {
            let outcomes = {
                let mut state = lock_state();
                state
                    .projection
                    .apply(state_message)
                    .map_err(|error| error.to_string())?;
                state.observe()
            };
            publish_projection(runtime)?;
            settle(runtime, outcomes);
            Ok(())
        }
        unavailable @ Message::Unavailable { .. } => {
            lock_state()
                .projection
                .apply(unavailable)
                .map_err(|error| error.to_string())?;
            withdraw(runtime, "Melibea lost its authoritative Niri state");
            Ok(())
        }
        Message::Error(error) => Err(format!(
            "Melibea subscription failed with {:?}: {}",
            error.code, error.message
        )),
        Message::ActionResult(_) => {
            Err("Melibea sent an action result on the subscription".to_owned())
        }
    }
}

fn subscribe_once(runtime: &Mutex<ProviderRuntime>, shutdown: &AtomicBool) -> Result<(), String> {
    let path = socket_path()?;
    let mut stream = UnixStream::connect(&path)
        .map_err(|error| format!("cannot connect to {}: {error}", path.display()))?;
    stream
        .set_read_timeout(Some(IO_POLL))
        .map_err(|error| format!("cannot bound Melibea reads: {error}"))?;
    let request = encode_request(&ClientEnvelope::subscribe())
        .map_err(|error| format!("cannot encode Melibea subscription: {error}"))?;
    stream
        .write_all(&request)
        .map_err(|error| format!("cannot subscribe to Melibea: {error}"))?;

    let mut reader = BufReader::new(stream);
    while !shutdown.load(Ordering::Acquire) {
        match read_bounded_message(&mut reader) {
            Ok(Some(line)) => handle_subscription_message(&line, runtime)?,
            Ok(None) => return Err("Melibea closed the subscription".to_owned()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                let outcomes = lock_state().expire(Instant::now());
                settle(runtime, outcomes);
            }
            Err(error) => return Err(format!("cannot read Melibea subscription: {error}")),
        }
    }
    Ok(())
}

fn run(runtime: &Mutex<ProviderRuntime>, shutdown: &AtomicBool) {
    let mut last_error = String::new();
    while !shutdown.load(Ordering::Acquire) {
        let result = subscribe_once(runtime, shutdown);
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let reason = result
            .err()
            .unwrap_or_else(|| "Melibea subscription ended".to_owned());
        withdraw(runtime, "Melibea connection was lost before confirmation");
        if reason != last_error {
            eprintln!("celestina-provider-adapter: {reason}");
            last_error = reason;
        }
        thread::sleep(RECONNECT_DELAY);
    }
    withdraw(runtime, "Melibea provider stopped before confirmation");
}

/// Starts the one subscribed Melibea provider.
///
/// # Errors
///
/// Returns a thread-spawn failure. A missing daemon is ordinary degraded state
/// handled by the worker's reconnect loop.
pub fn spawn(
    runtime: &Arc<Mutex<ProviderRuntime>>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<Worker> {
    lock_runtime(runtime).register(provider_id());
    let worker_runtime = Arc::clone(runtime);
    let worker_shutdown = Arc::clone(shutdown);
    Worker::spawn("provider-melibea", shutdown, move || {
        run(&worker_runtime, &worker_shutdown);
    })
}

fn parse_window_id(options: &Payload) -> Result<u64, String> {
    options
        .get("window_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "the Melibea action names no exact window id".to_owned())?
        .parse::<u64>()
        .map_err(|_| "the Melibea action carries an invalid window id".to_owned())
}

fn parse_coordinate(options: &Payload, key: &str) -> Result<f64, String> {
    options
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("the Melibea anchor carries no usable '{key}'"))
}

/// Reads the presentation hint the shell attached to this action.
///
/// The hint is optional, and read from the request rather than remembered, so a stale bubble
/// rectangle cannot survive a shell restart. Anything malformed is refused here instead of
/// travelling to Melibea to be rejected there.
fn parse_transition(options: &Payload) -> Result<Option<WindowTransition>, String> {
    let Some(kind) = options.get("transition") else {
        return Ok(None);
    };
    match kind.as_str() {
        Some("disabled") => Ok(Some(WindowTransition::Disabled)),
        Some("anchored") => {
            let output = options
                .get("anchor_output")
                .and_then(Value::as_str)
                .ok_or_else(|| "the Melibea anchor names no output".to_owned())?;
            let anchor = BubbleAnchor::new(
                output,
                parse_coordinate(options, "anchor_x")?,
                parse_coordinate(options, "anchor_y")?,
                parse_coordinate(options, "anchor_width")?,
                parse_coordinate(options, "anchor_height")?,
            )
            .ok_or_else(|| "the Melibea anchor is not a usable rectangle".to_owned())?;
            Ok(Some(WindowTransition::Anchored { anchor }))
        }
        _ => Err("the Melibea action carries an unknown transition".to_owned()),
    }
}

/// Why one attempt failed, when that matters to what happens next.
enum AttemptError {
    /// Melibea refused the whole envelope because it does not speak that version. The action
    /// did not happen, which is what makes retrying it safe rather than a second command.
    UnsupportedVersion,
    Fatal(String),
}

impl From<AttemptError> for String {
    fn from(error: AttemptError) -> Self {
        match error {
            AttemptError::UnsupportedVersion => {
                "Melibea does not speak this shell's protocol version".to_owned()
            }
            AttemptError::Fatal(message) => message,
        }
    }
}

fn send_action(
    path: &Path,
    request: &ClientEnvelope,
    operation: Operation,
    window_id: Option<u64>,
) -> Result<(ActionStatus, u64), AttemptError> {
    let line = encode_request(request)
        .map_err(|error| AttemptError::Fatal(format!("cannot encode Melibea action: {error}")))?;
    let mut stream = UnixStream::connect(path).map_err(|error| {
        AttemptError::Fatal(format!("cannot connect to {}: {error}", path.display()))
    })?;
    stream
        .set_read_timeout(Some(ACTION_TIMEOUT))
        .map_err(|error| {
            AttemptError::Fatal(format!("cannot bound Melibea action reads: {error}"))
        })?;
    stream
        .set_write_timeout(Some(ACTION_TIMEOUT))
        .map_err(|error| {
            AttemptError::Fatal(format!("cannot bound Melibea action writes: {error}"))
        })?;
    stream
        .write_all(&line)
        .map_err(|error| AttemptError::Fatal(format!("cannot send Melibea action: {error}")))?;
    let mut reader = BufReader::new(stream);
    let response = read_bounded_message(&mut reader)
        .map_err(|error| AttemptError::Fatal(format!("cannot read Melibea action: {error}")))?
        .ok_or_else(|| {
            AttemptError::Fatal("Melibea closed the action without a response".to_owned())
        })?;

    match decode_message(&response).map_err(|error| AttemptError::Fatal(error.to_string()))? {
        // A named action must be answered for exactly the window it named. A focused
        // minimize named none, so Melibea's reply is what says which window it resolved,
        // and that resolved id is what the confirmation then watches for.
        Message::ActionResult(result)
            if result.operation == operation
                && result.requested_id == window_id
                && result.window_id.is_some()
                && (window_id.is_none() || result.window_id == window_id) =>
        {
            let resolved = result.window_id.ok_or_else(|| {
                AttemptError::Fatal("Melibea named no window in its reply".to_owned())
            })?;
            Ok((result.status, resolved))
        }
        Message::ActionResult(_) => Err(AttemptError::Fatal(
            "Melibea answered for a different action or window".to_owned(),
        )),
        Message::Error(error) => match error.code {
            ErrorCode::IncompatibleVersion => Err(AttemptError::UnsupportedVersion),
            ErrorCode::InvalidRequest => Err(AttemptError::Fatal(format!(
                "Melibea invalid request: {}",
                error.message
            ))),
            ErrorCode::Unavailable => Err(AttemptError::Fatal(format!(
                "Melibea unavailable: {}",
                error.message
            ))),
            ErrorCode::ActionFailed => Err(AttemptError::Fatal(format!(
                "Melibea action failed: {}",
                error.message
            ))),
        },
        _ => Err(AttemptError::Fatal(
            "Melibea action returned no action result".to_owned(),
        )),
    }
}

fn envelope_for(
    operation: Operation,
    window_id: Option<u64>,
    transition: Option<WindowTransition>,
) -> Result<ClientEnvelope, String> {
    match window_id {
        Some(window_id) => ClientEnvelope::action_with_transition(operation, window_id, transition)
            .ok_or_else(|| "Melibea does not carry a transition for that action".to_owned()),
        None => Ok(ClientEnvelope::minimize_focused(transition)),
    }
}

fn request_action(
    operation: Operation,
    window_id: Option<u64>,
    transition: Option<WindowTransition>,
    request_id: &str,
) -> Result<(ActionStatus, u64), String> {
    let path = socket_path()?;
    // Only whether there was a hint outlives the envelope, so the hint itself is moved into
    // it rather than copied for the sake of a later question.
    let had_transition = transition.is_some();
    // Everything that can be refused without touching Niri is refused first, so a rejected
    // action never becomes a window that moved and a request that says it did not.
    let request = envelope_for(operation, window_id, transition)?;

    // The last check before any side effect. Losing the projection after this point is an
    // ordinary race, already handled by reserve/arm, reconnection failure, and the bounded
    // timeout; an accepted Niri state change is never rolled back.
    lock_state().can_dispatch(request_id)?;

    match send_action(&path, &request, operation, window_id) {
        Ok(result) => Ok(result),
        // A Melibea that speaks only v1 refuses the whole envelope, so the window did not
        // move and there is nothing to undo. The hint is presentation: losing the travel is
        // worth far less than losing the action, so the same request goes again without it.
        // Only a request that actually carried a hint can be retried this way — otherwise the
        // refusal is about something this shell cannot fix by asking again.
        Err(AttemptError::UnsupportedVersion) if had_transition => {
            let plain = envelope_for(operation, window_id, None)?;
            send_action(&path, &plain, operation, window_id).map_err(String::from)
        }
        Err(error) => Err(String::from(error)),
    }
}

pub fn action(verb: &str, options: &Payload, request_id: &str) -> Result<(), String> {
    let operation = match verb {
        "minimize" => Operation::Minimize,
        "restore" => Operation::Restore,
        "close" => Operation::Close,
        _ => return Err(format!("Melibea does not serve the verb '{verb}'")),
    };
    // Only a minimize may leave the window unnamed: it then asks Niri to resolve whatever is
    // focused, which cannot race the way reading focus here and acting afterwards would.
    // Restoring or closing names a window that is already a bubble.
    let window_id = match (operation, options.get("window_id")) {
        (Operation::Minimize, None) => None,
        _ => Some(parse_window_id(options)?),
    };
    // Closing has nowhere to travel to, so a hint on it is a caller mistake rather than
    // something to quietly drop.
    let transition = parse_transition(options)?;
    if transition.is_some() && operation == Operation::Close {
        return Err("Melibea does not carry a transition for a close".to_owned());
    }
    let (status, resolved_id) = request_action(operation, window_id, transition, request_id)?;
    if !status.accepts_state_confirmation() {
        return Err(match status {
            ActionStatus::WindowNotFound => "Melibea could not find that window".to_owned(),
            ActionStatus::Blocked => "Niri blocked the requested window action".to_owned(),
            _ => "Melibea refused the requested window action".to_owned(),
        });
    }
    lock_state().reserve(request_id, resolved_id, Settles::of(operation))
}

pub fn arm(request_id: &str, runtime: &Mutex<ProviderRuntime>) {
    if let Some(outcome) = lock_state().arm(request_id) {
        lock_runtime(runtime).settle(outcome);
    }
}

pub fn discard(request_id: &str) {
    lock_state().pending.remove(request_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell_core::melibea::{Request, Window};

    fn snapshot(revision: u64, ids: &[u64]) -> Message {
        Message::Snapshot {
            revision,
            windows: ids
                .iter()
                .map(|id| Window {
                    id: *id,
                    app_id: Some("org.example.App".to_owned()),
                    title: Some(format!("window {id}")),
                    icon_name: None,
                })
                .collect(),
        }
    }

    fn options(pairs: &[(&str, Value)]) -> Payload {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn minimize_is_confirmed_by_presence_and_restore_by_absence() {
        // The two directions are opposites, and reading them from the projection is what keeps a
        // request from settling on the action's own optimistic reply.
        let mut bridge = BridgeState::default();
        bridge.projection.apply(snapshot(1, &[])).expect("snapshot");

        bridge
            .reserve("min", 42, Settles::WhenPresent)
            .expect("reserved");
        assert_eq!(bridge.arm("min"), None, "nothing is minimized yet");

        bridge
            .projection
            .apply(snapshot(2, &[42]))
            .expect("snapshot");
        assert_eq!(
            bridge.observe(),
            vec![Outcome::confirmed("min".to_owned())],
            "the window appearing as a bubble is what confirms a minimize"
        );

        bridge
            .reserve("res", 42, Settles::WhenAbsent)
            .expect("reserved");
        assert_eq!(bridge.arm("res"), None, "it is still minimized");
        bridge.projection.apply(snapshot(3, &[])).expect("snapshot");
        assert_eq!(bridge.observe(), vec![Outcome::confirmed("res".to_owned())]);
    }

    #[test]
    fn an_already_satisfied_minimize_confirms_as_soon_as_it_is_armed() {
        let mut bridge = BridgeState::default();
        bridge
            .projection
            .apply(snapshot(1, &[42]))
            .expect("snapshot");
        bridge
            .reserve("min", 42, Settles::WhenPresent)
            .expect("reserved");
        assert_eq!(
            bridge.arm("min"),
            Some(Outcome::confirmed("min".to_owned()))
        );
    }

    #[test]
    fn a_transition_is_read_from_the_action_rather_than_remembered() {
        assert_eq!(parse_transition(&options(&[])).expect("no hint"), None);

        assert_eq!(
            parse_transition(&options(&[("transition", Value::from("disabled"))]))
                .expect("reduced motion"),
            Some(WindowTransition::Disabled)
        );

        let anchored = parse_transition(&options(&[
            ("transition", Value::from("anchored")),
            ("anchor_output", Value::from("DP-1")),
            ("anchor_x", Value::from(1874.0)),
            ("anchor_y", Value::from(9.0)),
            ("anchor_width", Value::from(22.0)),
            ("anchor_height", Value::from(22.0)),
        ]))
        .expect("anchored");
        assert_eq!(
            anchored,
            Some(WindowTransition::Anchored {
                anchor: BubbleAnchor::new("DP-1", 1874., 9., 22., 22.).expect("valid anchor")
            })
        );
    }

    #[test]
    fn dropping_the_hint_leaves_the_action_itself_intact() {
        // What the downgrade retry actually sends. A Melibea that speaks only v1 refuses the
        // whole envelope, so nothing happened and the same action may go again — but it must
        // go as the *same* action, naming the same window, only without the presentation.
        let anchor = BubbleAnchor::new("DP-1", 1874., 9., 22., 22.).expect("valid anchor");
        let hinted = envelope_for(
            Operation::Restore,
            Some(42),
            Some(WindowTransition::Anchored { anchor }),
        )
        .expect("supported");
        let plain = envelope_for(Operation::Restore, Some(42), None).expect("supported");

        assert_eq!(hinted.version, 2);
        assert_eq!(
            plain.version, 1,
            "the retry must land inside the frozen v1 contract"
        );
        assert_eq!(
            plain.request,
            Request::Restore {
                window_id: 42,
                transition: None
            }
        );

        // And a focused minimize keeps asking Niri to resolve focus, rather than silently
        // becoming a request for some window this shell picked itself.
        let focused = envelope_for(Operation::Minimize, None, None).expect("supported");
        assert_eq!(
            focused.request,
            Request::Minimize {
                window_id: None,
                transition: None
            }
        );
    }

    #[test]
    fn a_malformed_hint_is_refused_here_rather_than_sent_to_melibea() {
        // An unknown kind, a missing output, a missing coordinate and an unusable rectangle are
        // all caller mistakes. Refusing them locally keeps a bad hint from reaching Niri at all.
        assert!(parse_transition(&options(&[("transition", Value::from("sideways"))])).is_err());
        assert!(parse_transition(&options(&[("transition", Value::from(7))])).is_err());
        assert!(
            parse_transition(&options(&[("transition", Value::from("anchored"))])).is_err(),
            "an anchored hint with no anchor must be refused"
        );

        let mut incomplete = vec![
            ("transition", Value::from("anchored")),
            ("anchor_output", Value::from("DP-1")),
            ("anchor_x", Value::from(1874.0)),
            ("anchor_y", Value::from(9.0)),
            ("anchor_width", Value::from(22.0)),
        ];
        assert!(
            parse_transition(&options(&incomplete)).is_err(),
            "a missing height must be refused"
        );

        incomplete.push(("anchor_height", Value::from(0.0)));
        assert!(
            parse_transition(&options(&incomplete)).is_err(),
            "an empty rectangle must be refused"
        );
    }

    #[test]
    fn dispatch_requires_a_ready_authoritative_projection() {
        // The shell frame may still say Melibea is available for a moment after this
        // subscription withdrew its projection. Acting then would move a window in Niri with no
        // authoritative state to publish the result from or confirm it against.
        let mut bridge = BridgeState::default();
        assert!(
            bridge.can_dispatch("1").is_err(),
            "a bridge with no projection must refuse to act"
        );

        bridge
            .projection
            .apply(snapshot(1, &[42]))
            .expect("snapshot");
        assert!(bridge.can_dispatch("1").is_ok());

        // Losing the projection again closes the door immediately.
        bridge.projection = Projection::default();
        assert!(bridge.can_dispatch("1").is_err());
    }

    #[test]
    fn capacity_and_duplicate_ids_are_refused_before_an_action_can_run() {
        // Refusing only after the action has been applied reports a failure for a window that
        // really moved. Both refusals therefore belong before any socket IO.
        let mut bridge = BridgeState::default();
        bridge
            .projection
            .apply(snapshot(1, &[42]))
            .expect("snapshot");

        bridge
            .reserve("taken", 42, Settles::WhenAbsent)
            .expect("first reservation");
        assert!(
            bridge.can_dispatch("taken").is_err(),
            "a request id already awaiting confirmation must not be dispatched twice"
        );
        assert!(bridge.can_dispatch("fresh").is_ok());

        for index in 0..MAX_PENDING {
            let _ = bridge.reserve(&format!("filler-{index}"), 42, Settles::WhenAbsent);
        }
        assert!(
            bridge.can_dispatch("fresh").is_err(),
            "a full pending table must refuse before the action runs"
        );
    }

    #[test]
    fn a_request_cannot_confirm_before_accepted_is_armed() {
        let mut bridge = BridgeState::default();
        bridge
            .projection
            .apply(snapshot(1, &[42]))
            .expect("snapshot");
        bridge
            .reserve("7", 42, Settles::WhenAbsent)
            .expect("reserved");
        bridge.projection.apply(snapshot(2, &[])).expect("snapshot");
        assert!(bridge.observe().is_empty());
        assert_eq!(bridge.arm("7"), Some(Outcome::confirmed("7".to_owned())));
    }

    #[test]
    fn only_authoritative_absence_confirms_an_armed_action() {
        let mut bridge = BridgeState::default();
        bridge
            .projection
            .apply(snapshot(1, &[42, 43]))
            .expect("snapshot");
        bridge
            .reserve("7", 42, Settles::WhenAbsent)
            .expect("reserved");
        assert_eq!(bridge.arm("7"), None);
        bridge
            .projection
            .apply(snapshot(2, &[43]))
            .expect("snapshot");
        assert_eq!(bridge.observe(), vec![Outcome::confirmed("7".to_owned())]);
    }

    #[test]
    fn exact_decimal_window_ids_cross_the_provider_boundary() {
        let mut projection = Projection::default();
        let exact = u64::MAX;
        projection.apply(snapshot(1, &[exact])).expect("snapshot");
        let reading = payload(&projection);
        assert_eq!(
            reading["windows"][0]["id"],
            Value::String(exact.to_string())
        );
    }

    #[test]
    fn an_unavailable_connection_fails_only_armed_requests() {
        let mut bridge = BridgeState::default();
        bridge
            .reserve("unarmed", 1, Settles::WhenAbsent)
            .expect("reserved");
        bridge
            .reserve("armed", 2, Settles::WhenAbsent)
            .expect("reserved");
        assert_eq!(bridge.arm("armed"), None);
        assert_eq!(
            bridge.fail_all("lost"),
            vec![Outcome::failed("armed".to_owned(), "lost")]
        );
    }
}
