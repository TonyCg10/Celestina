//! Asking the desktop which folder to map.
//!
//! Fluorita has no file browser and is not going to grow one: choosing a folder
//! is exactly what `org.freedesktop.portal.FileChooser` exists for, and on this
//! desktop the request is answered by Siderita's portal backend, so the user
//! picks a folder in the file manager they already use.
//!
//! Three properties matter here and none of them is convenience:
//!
//! - **It never runs on the GUI thread.** A portal request lasts exactly as
//!   long as the person takes to decide, which can be minutes. The caller owns
//!   a worker; this module only blocks the thread it was handed.
//! - **It ends when its host does.** The wait watches the host's token. A
//!   request withdrawn that way is closed on the desktop's side too
//!   (`org.freedesktop.portal.Request.Close`), its private bus connection is
//!   closed, and the listener that connection fed ends with it — so a window
//!   closing while the dialog is open joins this worker within one receive
//!   slice instead of waiting on a person, and no thread is left behind.
//! - **It reports what happened.** No portal, a refused request, a cancelled
//!   dialog and a returned folder are four different answers, and a caller that
//!   could not tell them apart would show "added" for a dialog nobody
//!   confirmed.
//! - **What comes back is input.** The portal returns a URI chosen outside this
//!   process. It is read by the suite's one strict `file://` parser and handed
//!   on as raw bytes; the domain then applies its own rules to it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use celestina_core::CancellationToken;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// The bus name, path and interface the portal is reached at.
const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const FILE_CHOOSER: &str = "org.freedesktop.portal.FileChooser";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";

/// How long the whole exchange may take. Long enough for a person to browse and
/// decide; bounded so a backend that never answers releases the worker instead
/// of holding a thread for the lifetime of the application.
const DEADLINE: Duration = Duration::from_secs(300);

/// How long one wait for a portal signal lasts. Short enough that the deadline
/// above — and the host's token — are noticed on a bus that has gone quiet,
/// rather than only between two messages that may never come.
const RECEIVE_SLICE: Duration = Duration::from_millis(500);

/// How long the listener gets to notice its connection closed. Closing the
/// socket ends its stream at once; this bound is what keeps a bus library that
/// behaves otherwise from turning a closed dialog into a frozen window.
const LISTENER_GRACE: Duration = Duration::from_secs(1);

/// What the desktop answered.
#[derive(Debug)]
pub enum FolderChoice {
    /// The user chose this folder. The bytes are the portal's, undecoded by
    /// anything but the canonical percent codec.
    Chosen(PathBuf),
    /// The dialog was dismissed. Not a failure, and not something to report as
    /// one.
    Cancelled,
    /// The desktop could not be asked. The message is shown to the user,
    /// because a button that silently does nothing is worse than one that says
    /// why it could not.
    Unavailable(String),
}

/// Opens the desktop's folder chooser and waits for the answer, or until
/// `cancellation` is requested — which withdraws the dialog and reads as
/// [`FolderChoice::Cancelled`].
///
/// Blocking by construction: call it from a worker thread.
#[must_use]
/// `start`, when given, is the folder the dialog opens at (the portal's
/// `current_folder`, its bytes as they are): a folder dropped on the window
/// is offered there, one confirmation away.
pub fn choose(title: &str, start: Option<&Path>, cancellation: &CancellationToken) -> FolderChoice {
    match request(title, Wanted::Directory(start), cancellation) {
        Ok(choice) => choice,
        Err(error) => FolderChoice::Unavailable(error),
    }
}

/// Opens the desktop's file chooser, showing pictures, and waits for the
/// answer.
///
/// The same request with one option flipped and a filter added, rather than a
/// second portal client: a chooser that answers a path is one conversation
/// whatever it was asked to find.
#[must_use]
pub fn choose_picture(
    title: &str,
    filter_label: &str,
    cancellation: &CancellationToken,
) -> FolderChoice {
    match request(title, Wanted::Picture(filter_label), cancellation) {
        Ok(choice) => choice,
        Err(error) => FolderChoice::Unavailable(error),
    }
}

/// What the chooser is being asked for.
#[derive(Clone, Copy)]
enum Wanted<'a> {
    /// A folder, the dialog opened at the one given.
    Directory(Option<&'a Path>),
    /// A picture, labelled for the dialog's filter row.
    Picture(&'a str),
}

fn request(
    title: &str,
    wanted: Wanted<'_>,
    cancellation: &CancellationToken,
) -> Result<FolderChoice, String> {
    // A connection of its own, never the process's shared one: closing it is
    // how the listener below is stopped, and nothing else may lose its bus
    // when that happens.
    let connection = zbus::blocking::Connection::session()
        .map_err(|error| format!("no session bus: {error}"))?;

    // Subscribe before asking. The backend may answer before the reply to
    // `OpenFile` is even dispatched, and a listener created afterwards would
    // wait forever for a signal that already went past.
    let responses = zbus::blocking::MessageIterator::for_match_rule(
        zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(REQUEST_INTERFACE)
            .map_err(|error| format!("malformed match rule: {error}"))?
            .member("Response")
            .map_err(|error| format!("malformed match rule: {error}"))?
            .build(),
        &connection,
        None,
    )
    .map_err(|error| format!("cannot listen for the answer: {error}"))?;

    // Signals are received on their own thread and cross as they arrive.
    // `zbus`'s blocking iterator has no timed receive, so the deadline and the
    // host's token could only ever be checked *between* two messages: a
    // backend that took the request and then said nothing held this worker for
    // the life of the process. The listener ends when its answer has nowhere
    // to go, or when the connection closes under it; `ended` is dropped as it
    // leaves, which is how this thread knows it may be joined.
    let (signals, incoming) = std::sync::mpsc::channel();
    let (ended, listener_left) = std::sync::mpsc::channel::<()>();
    let listener = std::thread::Builder::new()
        .name("fluorita-portal".to_owned())
        .spawn(move || {
            let _ended = ended;
            for message in responses {
                if signals.send(message).is_err() {
                    return;
                }
            }
        })
        .map_err(|error| format!("cannot listen for the answer: {error}"))?;

    let mut asked = Asked::default();
    let answer = exchange(
        &connection,
        title,
        wanted,
        &incoming,
        cancellation,
        &mut asked,
    );

    // A request the desktop still has open is withdrawn, so no dialog is left
    // on screen for an answer nobody will read. Best effort: a portal that
    // cannot be told is one that will time the dialog out on its own.
    if let (Some(handle), false) = (asked.handle.as_ref(), asked.answered) {
        let _ = connection.call_method(
            Some(PORTAL_SERVICE),
            handle.as_ref(),
            Some(REQUEST_INTERFACE),
            "Close",
            &(),
        );
    }
    // Closing the socket ends the listener's stream.
    let _ = connection.close();
    drop(incoming);
    match listener_left.recv_timeout(LISTENER_GRACE) {
        Ok(()) | Err(RecvTimeoutError::Disconnected) => {
            let _ = listener.join();
        }
        // Left to end with its connection rather than joined: waiting longer
        // is the freeze this bound exists to prevent.
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("fluorita: the portal listener did not stop in time");
        }
    }
    answer
}

/// What became of the request on the desktop's side.
#[derive(Default)]
struct Asked {
    /// The request object the portal created, once it did.
    handle: Option<OwnedObjectPath>,
    /// Whether the portal answered it, which closes it on its own.
    answered: bool,
}

/// Asks, then waits for the answer to this request, the deadline, or the
/// host's token, whichever comes first.
fn exchange(
    connection: &zbus::blocking::Connection,
    title: &str,
    wanted: Wanted<'_>,
    incoming: &Receiver<zbus::Result<zbus::Message>>,
    cancellation: &CancellationToken,
    asked: &mut Asked,
) -> Result<FolderChoice, String> {
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("multiple", Value::Bool(false));
    options.insert("modal", Value::Bool(true));
    match wanted {
        Wanted::Directory(start) => {
            options.insert("directory", Value::Bool(true));
            if let Some(start) = start {
                // `ay`, NUL-terminated: the name crosses as bytes, so a folder
                // whose name is not UTF-8 is still the one the dialog opens.
                let mut bytes = std::os::unix::ffi::OsStrExt::as_bytes(start.as_os_str()).to_vec();
                bytes.push(0);
                options.insert("current_folder", Value::from(bytes));
            }
        }
        Wanted::Picture(label) => {
            options.insert("directory", Value::Bool(false));
            // The portal's filter shape: a label, then a list of (kind, spec)
            // pairs where kind 1 is a media type. Globs would have to spell
            // every extension the toolkit reads; the media type says what is
            // meant once.
            let patterns = vec![Value::from((1u32, "image/*".to_owned()))];
            options.insert(
                "filters",
                Value::from(vec![Value::from((label.to_owned(), patterns))]),
            );
        }
    }

    let handle: OwnedObjectPath = connection
        .call_method(
            Some(PORTAL_SERVICE),
            PORTAL_PATH,
            Some(FILE_CHOOSER),
            "OpenFile",
            // No parent window handle: Fluorita is a Wayland client and has no
            // exported surface identifier to give, so the portal places the
            // dialog itself rather than being told a lie about the parent.
            &("", title, options),
        )
        .map_err(|error| format!("the desktop has no folder chooser: {error}"))?
        .body()
        .deserialize()
        .map_err(|error| format!("the folder chooser answered unexpectedly: {error}"))?;
    asked.handle = Some(handle.clone());

    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        // The host is going away or asked for something else: the answer has
        // nowhere to go, and the caller withdraws the dialog.
        if cancellation.is_cancel_requested() {
            return Ok(FolderChoice::Cancelled);
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err("the folder chooser did not answer in time".to_owned());
        }
        let message = match incoming.recv_timeout(RECEIVE_SLICE.min(deadline - now)) {
            Ok(message) => message,
            // Silence is not an answer, and not a failure either: the person
            // may still be browsing. The deadline above decides when it is.
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("the folder chooser stopped answering".to_owned())
            }
        };
        let message = message.map_err(|error| format!("the folder chooser failed: {error}"))?;
        // Several requests can be in flight on this bus; only ours counts.
        if message.header().path() != Some(&handle.as_ref()) {
            continue;
        }
        // Answered: the portal closes a request once it has responded.
        asked.answered = true;
        let (code, results): (u32, HashMap<String, OwnedValue>) = message
            .body()
            .deserialize()
            .map_err(|error| format!("the folder chooser answered unexpectedly: {error}"))?;
        // Anything but zero means the user did not confirm a choice. The portal
        // distinguishes "cancelled" from "ended some other way", and neither is
        // an error to report at the user.
        if code != 0 {
            return Ok(FolderChoice::Cancelled);
        }
        return Ok(first_folder(&results).map_or(FolderChoice::Cancelled, FolderChoice::Chosen));
    }
}

/// The first `file://` URI of the answer, as a path.
///
/// A `directory: true` request returns at most one, but the field is a list by
/// contract, and a non-local URI is not something this library can scan.
fn first_folder(results: &HashMap<String, OwnedValue>) -> Option<PathBuf> {
    let uris: Vec<String> = results.get("uris")?.try_clone().ok()?.try_into().ok()?;
    uris.iter().find_map(|uri| local_path(uri))
}

/// The local folder a URI names, read by the suite's one strict parser: a
/// foreign host, a malformed escape, a query or a NUL names nothing here.
fn local_path(uri: &str) -> Option<PathBuf> {
    celestina_core::file_uri::to_path(uri).ok()
}

#[cfg(test)]
mod tests {
    use super::local_path;
    use std::path::PathBuf;

    #[test]
    fn a_local_uri_becomes_the_path_it_names() {
        assert_eq!(
            local_path("file:///home/toni/Pictures"),
            Some(PathBuf::from("/home/toni/Pictures"))
        );
        // A space is the case a naive split on `/` still gets right and a naive
        // decode-free path does not.
        assert_eq!(
            local_path("file:///mnt/my%20photos"),
            Some(PathBuf::from("/mnt/my photos"))
        );
        // `localhost` is this machine spelled the long way; the activation
        // path always accepted it, and now both read a URI the same way.
        assert_eq!(
            local_path("file://localhost/home/toni/Pictures"),
            Some(PathBuf::from("/home/toni/Pictures"))
        );
    }

    #[test]
    fn anything_that_is_not_a_local_folder_is_refused() {
        // Another machine: nothing here can scan it.
        assert_eq!(local_path("file://elsewhere/photos"), None);
        // Not a file URI at all.
        assert_eq!(local_path("smb://elsewhere/photos"), None);
        assert_eq!(local_path("/home/toni/Pictures"), None);
        // A malformed escape is refused rather than guessed at.
        assert_eq!(local_path("file:///mnt/half%2"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_arrives_as_the_bytes_it_was() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = local_path("file:///mnt/foto%FFs").expect("an absolute local path");

        assert_eq!(path.as_os_str(), OsStr::from_bytes(b"/mnt/foto\xFFs"));
        assert!(
            !path.to_string_lossy().is_empty(),
            "the replacement character would mean the name was mangled"
        );
    }
}
