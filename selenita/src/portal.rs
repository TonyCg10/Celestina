//! The ScreenCast portal client: `org.freedesktop.portal.ScreenCast` on the
//! session bus, asked for one monitor the person picks in the portal's own
//! dialog, answering with the PipeWire node the recording reads and the
//! remote's file descriptor.
//!
//! Every method of the portal answers at once with a request handle and
//! sends its real answer later as a `Response` signal on that handle, so the
//! signal is subscribed to before the call and waited for with a deadline
//! (long enough for the person to choose). The session is closed when the
//! [`Session`] is dropped, so the compositor stops casting. Everything here
//! blocks and runs on the recording worker; [`cancel_pending`] is the one
//! call for another thread, which closes the request the worker waits on
//! (the window is quitting) so the worker comes back at once.

use std::collections::HashMap;
use std::fmt;
use std::os::fd::AsFd;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENCAST: &str = "org.freedesktop.portal.ScreenCast";
const REQUEST: &str = "org.freedesktop.portal.Request";
const SESSION: &str = "org.freedesktop.portal.Session";

/// `SelectSources`' `types`: a monitor.
const SOURCE_MONITOR: u32 = 1;
/// `SelectSources`' `cursor_mode`: the pointer drawn into the picture.
const CURSOR_EMBEDDED: u32 = 2;

/// The portal's answer codes.
const RESPONSE_SUCCESS: u32 = 0;
const RESPONSE_CANCELLED: u32 = 1;

/// How long a method call itself may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the portal gets to create a session.
const SESSION_DEADLINE: Duration = Duration::from_secs(15);
/// How long the person gets in the chooser.
const CHOICE_DEADLINE: Duration = Duration::from_secs(180);

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Why the portal gave no stream.
#[derive(Debug)]
pub enum PortalError {
    Bus(String),
    /// The person closed the chooser.
    Cancelled,
    /// The portal answered another code.
    Refused(u32),
    /// The answer lacked the session or the streams.
    Malformed(&'static str),
    /// No answer within the deadline.
    Timeout(&'static str),
}

impl fmt::Display for PortalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(detail) => write!(formatter, "portal: {detail}"),
            Self::Cancelled => formatter.write_str("portal: cancelled by the person"),
            Self::Refused(code) => write!(formatter, "portal: response {code}"),
            Self::Malformed(what) => write!(formatter, "portal: no {what} in the response"),
            Self::Timeout(step) => write!(formatter, "portal: no response to {step} in time"),
        }
    }
}

impl std::error::Error for PortalError {}

impl From<zbus::Error> for PortalError {
    fn from(error: zbus::Error) -> Self {
        Self::Bus(error.to_string())
    }
}

/// The monitor stream the portal started.
#[derive(Debug)]
pub struct Stream {
    /// The PipeWire node.
    pub node: u32,
    /// The portal's PipeWire remote, which sees that node.
    pub fd: Option<OwnedFd>,
}

/// An open portal session; dropping it closes the session.
pub struct Session {
    connection: Connection,
    path: OwnedObjectPath,
}

impl Drop for Session {
    fn drop(&mut self) {
        let closed = Proxy::new(&self.connection, PORTAL_NAME, &self.path, SESSION)
            .and_then(|proxy| proxy.call::<_, _, ()>("Close", &()));
        if let Err(error) = closed {
            eprintln!("selenita: the portal session did not close: {error}");
        }
    }
}

/// Asks the portal for one monitor: `CreateSession`, `SelectSources`
/// (monitors, the pointer embedded), `Start` (the person chooses) and
/// `OpenPipeWireRemote`.
///
/// # Errors
///
/// [`PortalError`] for the bus, a cancel, a refusal or a timeout.
pub fn open_monitor() -> Result<(Session, Stream), PortalError> {
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(CALL_TIMEOUT)
        .build()?;
    let portal = Proxy::new(&connection, PORTAL_NAME, PORTAL_PATH, SCREENCAST)?;
    let sender = connection
        .unique_name()
        .map(|name| name.trim_start_matches(':').replace('.', "_"))
        .ok_or(PortalError::Malformed("unique name"))?;

    let session_token = token();
    let mut options = HashMap::new();
    options.insert("session_handle_token", Value::from(session_token));
    let results = call(
        &connection,
        &portal,
        &sender,
        "CreateSession",
        options,
        |options| (options,),
        SESSION_DEADLINE,
    )?;
    let session_path: String = results
        .get("session_handle")
        .and_then(|value| String::try_from(value.clone()).ok())
        .ok_or(PortalError::Malformed("session handle"))?;
    let session = Session {
        connection: connection.clone(),
        path: OwnedObjectPath::try_from(session_path)
            .map_err(|error| PortalError::Bus(error.to_string()))?,
    };

    let mut options = HashMap::new();
    options.insert("types", Value::U32(SOURCE_MONITOR));
    options.insert("multiple", Value::Bool(false));
    options.insert("cursor_mode", Value::U32(CURSOR_EMBEDDED));
    call(
        &connection,
        &portal,
        &sender,
        "SelectSources",
        options,
        |options| (session.path.clone(), options),
        CHOICE_DEADLINE,
    )?;

    let results = call(
        &connection,
        &portal,
        &sender,
        "Start",
        HashMap::new(),
        |options| (session.path.clone(), "", options),
        CHOICE_DEADLINE,
    )?;
    let streams: Vec<(u32, HashMap<String, OwnedValue>)> = results
        .get("streams")
        .and_then(|value| Vec::try_from(value.clone()).ok())
        .ok_or(PortalError::Malformed("streams"))?;
    let node = streams
        .first()
        .map(|(node, _)| *node)
        .ok_or(PortalError::Malformed("stream"))?;

    let fd = match portal.call::<_, _, zbus::zvariant::OwnedFd>(
        "OpenPipeWireRemote",
        &(session.path.clone(), HashMap::<&str, Value<'_>>::new()),
    ) {
        Ok(fd) => {
            let fd = OwnedFd::from(fd);
            // The bus hands the descriptor over without close-on-exec: set
            // it, so a `grim` or `wl-copy` forked meanwhile by the capture
            // worker does not inherit the remote. The child that needs it
            // gets it on its standard input, which `Command` dups anew.
            if let Err(error) = rustix::io::fcntl_setfd(fd.as_fd(), rustix::io::FdFlags::CLOEXEC) {
                eprintln!("selenita: the PipeWire remote keeps no close-on-exec: {error}");
            }
            Some(fd)
        }
        // Without the remote `pipewiresrc` connects to the session's own
        // daemon, where the node is visible under the wlr backend.
        Err(error) => {
            eprintln!("selenita: no PipeWire remote from the portal: {error}");
            None
        }
    };
    Ok((session, Stream { node, fd }))
}

fn token() -> String {
    format!(
        "selenita_{}_{}",
        std::process::id(),
        NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
    )
}

/// What the worker waits for: the portal's answer, or a cancel from the
/// quitting window.
enum Answer {
    Response(zbus::Message),
    Cancelled,
}

/// The request the worker is waiting on, for [`cancel_pending`].
struct Pending {
    connection: Connection,
    path: String,
    answer: Sender<Answer>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Closes the portal request the worker is waiting on, if any (its dialog
/// goes away), and wakes the worker with a cancel. Called from the main
/// thread when the window quits while the person is still choosing.
pub fn cancel_pending() {
    let Some(waiting) = pending().take() else {
        return;
    };
    let closed = Proxy::new(
        &waiting.connection,
        PORTAL_NAME,
        waiting.path.as_str(),
        REQUEST,
    )
    .and_then(|request| request.call::<_, _, ()>("Close", &()));
    if let Err(error) = closed {
        eprintln!("selenita: the portal request did not close: {error}");
    }
    let _ = waiting.answer.send(Answer::Cancelled);
}

/// Calls `method` with `options` (a `handle_token` added) and waits for the
/// `Response` on the request handle the token names; the success results.
fn call<'a, B>(
    connection: &Connection,
    portal: &Proxy<'_>,
    sender: &str,
    method: &'static str,
    mut options: HashMap<&'a str, Value<'static>>,
    body: impl FnOnce(HashMap<&'a str, Value<'static>>) -> B,
    deadline: Duration,
) -> Result<HashMap<String, OwnedValue>, PortalError>
where
    B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
{
    let handle_token = token();
    let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{handle_token}");
    options.insert("handle_token", Value::from(handle_token.clone()));

    // Subscribed before the call, so an answer that comes at once is not
    // missed; the iterator blocks without a deadline, so a thread of its own
    // waits on it and the worker waits on the channel with one.
    let request = Proxy::new(connection, PORTAL_NAME, request_path.as_str(), REQUEST)?;
    let mut responses = request.receive_signal("Response")?;
    let (sender_half, receiver) = mpsc::channel();
    let waker = sender_half.clone();
    std::thread::Builder::new()
        .name("selenita-portal".to_owned())
        .spawn(move || {
            if let Some(message) = responses.next() {
                let _ = sender_half.send(Answer::Response(message));
            }
        })
        .map_err(|error| PortalError::Bus(error.to_string()))?;
    *pending() = Some(Pending {
        connection: connection.clone(),
        path: request_path.clone(),
        answer: waker,
    });

    let answer = (|| {
        let handle: OwnedObjectPath = portal.call(method, &body(options))?;
        if handle.as_str() != request_path {
            return Err(PortalError::Bus(format!(
                "{method}: request handle {handle} is not the one subscribed to"
            )));
        }
        receiver
            .recv_timeout(deadline)
            .map_err(|_| PortalError::Timeout(method))
    })();
    // Whatever came, nothing waits any more.
    *pending() = None;
    let message = match answer? {
        Answer::Response(message) => message,
        Answer::Cancelled => return Err(PortalError::Cancelled),
    };
    let (code, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    match code {
        RESPONSE_SUCCESS => Ok(results),
        RESPONSE_CANCELLED => Err(PortalError::Cancelled),
        other => Err(PortalError::Refused(other)),
    }
}
