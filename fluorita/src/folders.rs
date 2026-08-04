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
//! - **It reports what happened.** No portal, a refused request, a cancelled
//!   dialog and a returned folder are four different answers, and a caller that
//!   could not tell them apart would show "added" for a dialog nobody
//!   confirmed.
//! - **What comes back is input.** The portal returns a URI chosen outside this
//!   process. It is decoded with the suite's canonical codec and handed on as
//!   raw bytes; the domain then applies its own rules to it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use zbus::zvariant::{OwnedValue, Value};

/// The bus name, path and interface the portal is reached at.
const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const FILE_CHOOSER: &str = "org.freedesktop.portal.FileChooser";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";

/// How long the whole exchange may take. Long enough for a person to browse and
/// decide; bounded so a backend that never answers releases the worker instead
/// of holding a thread for the lifetime of the application.
const DEADLINE: Duration = Duration::from_secs(300);

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

/// Opens the desktop's folder chooser and waits for the answer.
///
/// Blocking by construction: call it from a worker thread.
#[must_use]
pub fn choose(title: &str) -> FolderChoice {
    match request(title) {
        Ok(choice) => choice,
        Err(error) => FolderChoice::Unavailable(error),
    }
}

fn request(title: &str) -> Result<FolderChoice, String> {
    let connection = zbus::blocking::Connection::session()
        .map_err(|error| format!("no session bus: {error}"))?;

    // Subscribe before asking. The backend may answer before the reply to
    // `OpenFile` is even dispatched, and a listener created afterwards would
    // wait forever for a signal that already went past.
    let mut responses = zbus::blocking::MessageIterator::for_match_rule(
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

    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("directory", Value::Bool(true));
    options.insert("multiple", Value::Bool(false));
    options.insert("modal", Value::Bool(true));

    let handle: zbus::zvariant::OwnedObjectPath = connection
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

    let deadline = std::time::Instant::now() + DEADLINE;
    while std::time::Instant::now() < deadline {
        let Some(message) = responses.next() else {
            return Err("the folder chooser stopped answering".to_owned());
        };
        let message = message.map_err(|error| format!("the folder chooser failed: {error}"))?;
        // Several requests can be in flight on this bus; only ours counts.
        if message.header().path() != Some(&handle.as_ref()) {
            continue;
        }
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
    Err("the folder chooser did not answer in time".to_owned())
}

/// The first `file://` URI of the answer, as a path.
///
/// A `directory: true` request returns at most one, but the field is a list by
/// contract, and a non-local URI is not something this library can scan.
fn first_folder(results: &HashMap<String, OwnedValue>) -> Option<PathBuf> {
    let uris: Vec<String> = results.get("uris")?.try_clone().ok()?.try_into().ok()?;
    uris.iter().find_map(|uri| local_path(uri))
}

fn local_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    // A `file://host/path` URI names another machine; there is nothing local to
    // scan and guessing would map the wrong directory.
    let encoded = match encoded.find('/') {
        Some(0) => encoded,
        _ => return None,
    };
    let bytes = celestina_core::percent::decode(encoded);
    let path = celestina_core::percent::path_from_bytes(&bytes);
    path.is_absolute().then_some(path)
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
    }

    #[test]
    fn anything_that_is_not_a_local_folder_is_refused() {
        // Another machine: nothing here can scan it.
        assert_eq!(local_path("file://elsewhere/photos"), None);
        // Not a file URI at all.
        assert_eq!(local_path("smb://elsewhere/photos"), None);
        assert_eq!(local_path("/home/toni/Pictures"), None);
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
