//! `magnetita --send PATH…`: the send action other applications' "open with"
//! lists through the desktop entry.
//!
//! The argument parsing and the device decision are pure and live here so a
//! test reaches them; the bus calls are [`crate::devices`]'s, and `main` runs
//! them before Qt starts or after it has stopped — never on the Qt thread.
//! Only the desktop entry's explicit send action (`--send %F`) sends: one
//! connected device takes every file and the process exits; several open the
//! chooser window with the files already chosen, which sends on a worker while
//! the window stays up to show a failure; none says so and exits. A file given
//! on the main entry without the flag is ignored: nothing reaches a phone
//! unless the person asked for it to.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, PoisonError};

/// The flag that selects the send action.
pub const SEND_FLAG: &str = "--send";
/// Decide and report, but call nothing that sends: the read-only check.
pub const DRY_RUN_FLAG: &str = "--dry-run";

/// A parsed `--send` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendRequest {
    /// The files, absolute, byte for byte as the launcher gave them.
    pub paths: Vec<PathBuf>,
    pub dry_run: bool,
}

/// Why a `--send` invocation cannot run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendError {
    /// `--send` with no file after it.
    NoPaths,
}

/// The `--send` request in `args` (the arguments after the program name),
/// or `None` when the flag is absent and the window opens as usual. After the
/// flag every argument but `--dry-run` is a file; a relative path is taken
/// from `cwd`.
pub fn parse(
    args: impl IntoIterator<Item = OsString>,
    cwd: &Path,
) -> Option<Result<SendRequest, SendError>> {
    let mut args = args.into_iter();
    args.by_ref().find(|arg| arg == SEND_FLAG)?;
    let mut paths = Vec::new();
    let mut dry_run = false;
    for arg in args {
        if arg == DRY_RUN_FLAG {
            dry_run = true;
        } else {
            let path = PathBuf::from(arg);
            paths.push(if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            });
        }
    }
    if paths.is_empty() {
        return Some(Err(SendError::NoPaths));
    }
    Some(Ok(SendRequest { paths, dry_run }))
}

/// The arguments a launch without `--send` ignores: anything that is not a
/// `--` flag. The window opens as before and nothing is sent.
pub fn ignored_arguments(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.iter().any(|arg| arg == SEND_FLAG) {
        return Vec::new();
    }
    args.into_iter()
        .filter(|arg| !arg.as_encoded_bytes().starts_with(b"--"))
        .collect()
}

/// The first path that is not a regular file (following links): a launcher
/// hands files, and a folder or a vanished name is refused before the bus.
pub fn first_non_file(paths: &[PathBuf]) -> Option<&Path> {
    paths
        .iter()
        .find(|path| !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()))
        .map(PathBuf::as_path)
}

/// Where the files go, given the connected devices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Plan {
    /// No device is connected: nothing can be sent.
    NoDevice,
    /// Exactly one is: it takes every file, with no question asked.
    One(String),
    /// Several are: the person chooses in the window.
    Choose,
}

/// The plan for devices given as `(id, connected)`: only a connected device
/// can take a file.
pub fn plan<'a>(devices: impl IntoIterator<Item = (&'a str, bool)>) -> Plan {
    let mut connected = devices
        .into_iter()
        .filter(|(_, connected)| *connected)
        .map(|(id, _)| id);
    match (connected.next(), connected.next()) {
        (None, _) => Plan::NoDevice,
        (Some(id), None) => Plan::One(id.to_owned()),
        (Some(_), Some(_)) => Plan::Choose,
    }
}

/// What the chooser window sends: the connected devices as `(id, name)`, in
/// the order the window lists them, and the files.
#[derive(Clone, Debug, Default)]
pub struct Chooser {
    pub devices: Vec<(String, String)>,
    pub paths: Vec<PathBuf>,
}

/// The chooser of this launch, set by `main` before Qt starts.
static CHOOSER: Mutex<Option<Chooser>> = Mutex::new(None);
/// Whether a send the person chose failed, for the exit code.
static FAILED: Mutex<bool> = Mutex::new(false);

/// Hands the window its chooser.
pub fn set_chooser(chooser: Chooser) {
    *CHOOSER.lock().unwrap_or_else(PoisonError::into_inner) = Some(chooser);
}

/// The id of the chooser's device at `index`, and the files.
pub fn chosen(index: usize) -> Option<(String, Vec<PathBuf>)> {
    let chooser = CHOOSER.lock().unwrap_or_else(PoisonError::into_inner);
    let chooser = chooser.as_ref()?;
    let (id, _) = chooser.devices.get(index)?;
    Some((id.clone(), chooser.paths.clone()))
}

/// Records that the chosen send failed.
pub fn mark_failed() {
    *FAILED.lock().unwrap_or_else(PoisonError::into_inner) = true;
}

/// Whether the chosen send failed.
pub fn failed() -> bool {
    *FAILED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Chosen sends still running, so `main` waits for them before it exits.
static IN_FLIGHT: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

/// Runs the chooser's `send` on the calling worker: counted in flight until
/// it returns, and a failure recorded here, on the worker, so it counts even
/// when the window is gone before the answer could reach it.
pub fn run_chosen(send: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let result = send();
    if result.is_err() {
        mark_failed();
    }
    let (count, done) = &IN_FLIGHT;
    let mut count = count.lock().unwrap_or_else(PoisonError::into_inner);
    *count = count.saturating_sub(1);
    done.notify_all();
    result
}

/// Counts a chosen send in flight; call before handing it to a worker.
pub fn begin_chosen() {
    *IN_FLIGHT.0.lock().unwrap_or_else(PoisonError::into_inner) += 1;
}

/// How long `main` waits for a chosen send at exit: a worker the closing
/// model never started must not hang the process.
const WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Waits until every chosen send has finished or failed; one still running
/// after [`WAIT_LIMIT`] counts as failed.
pub fn wait_chosen() {
    let (count, done) = &IN_FLIGHT;
    let count = count.lock().unwrap_or_else(PoisonError::into_inner);
    let (count, timeout) = done
        .wait_timeout_while(count, WAIT_LIMIT, |count| *count > 0)
        .unwrap_or_else(PoisonError::into_inner);
    if timeout.timed_out() && *count > 0 {
        drop(count);
        mark_failed();
    }
}

/// Sends every file to `device_id`, one `SendFileUri` each; blocks, so it
/// runs on `main` or on a worker. The error names each file that failed.
pub fn send_files(device_id: &str, paths: &[PathBuf]) -> Result<(), String> {
    let connection = zbus::blocking::Connection::session()
        .map_err(|error| format!("session bus unavailable: {error}"))?;
    let failures: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            crate::devices::send_file(&connection, device_id, path)
                .err()
                .map(|error| format!("{}: {error}", path.display()))
        })
        .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// The argument parsing and the one-device decision. A test under `tests/`
/// cannot link the binary's generated Qt archive, so they live here.
#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::{parse, plan, Plan, SendError, SendRequest};

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn without_the_flag_the_window_opens_as_usual() {
        assert_eq!(parse(args(&[]), Path::new("/")), None);
        assert_eq!(parse(args(&["--mirror"]), Path::new("/")), None);
    }

    #[test]
    fn every_argument_after_the_flag_is_a_file() {
        assert_eq!(
            parse(
                args(&["--send", "/tmp/a.txt", "b.png"]),
                Path::new("/home/toni")
            ),
            Some(Ok(SendRequest {
                paths: vec![
                    PathBuf::from("/tmp/a.txt"),
                    PathBuf::from("/home/toni/b.png")
                ],
                dry_run: false,
            }))
        );
    }

    #[test]
    fn a_file_on_the_main_entry_is_ignored_and_never_sent() {
        assert_eq!(parse(args(&["foo"]), Path::new("/")), None);
        assert_eq!(
            super::ignored_arguments(args(&["foo", "--mirror"])),
            args(&["foo"])
        );
        assert!(super::ignored_arguments(args(&["--mirror"])).is_empty());
        assert!(super::ignored_arguments(args(&["--send", "foo"])).is_empty());
    }

    #[test]
    fn a_folder_or_a_missing_name_is_refused_and_a_file_passes() {
        let dir = std::env::temp_dir().join(format!("magnetita-send-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "a").unwrap();
        let request = parse(
            vec![OsString::from("--send"), dir.clone().into_os_string()],
            Path::new("/"),
        );
        let Some(Ok(request)) = request else {
            panic!("a folder still parses: {request:?}");
        };
        assert_eq!(super::first_non_file(&request.paths), Some(dir.as_path()));
        assert_eq!(super::first_non_file(std::slice::from_ref(&file)), None);
        let missing = dir.join("missing");
        assert_eq!(
            super::first_non_file(&[file.clone(), missing.clone()]),
            Some(missing.as_path())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_dry_run_flag_is_not_a_file() {
        assert_eq!(
            parse(args(&["--send", "--dry-run", "/tmp/a.txt"]), Path::new("/")),
            Some(Ok(SendRequest {
                paths: vec![PathBuf::from("/tmp/a.txt")],
                dry_run: true,
            }))
        );
    }

    #[test]
    fn the_flag_alone_is_an_error() {
        assert_eq!(
            parse(args(&["--send"]), Path::new("/")),
            Some(Err(SendError::NoPaths))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_crosses_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;
        let name = OsString::from_vec(b"/tmp/caf\xe9.txt".to_vec());
        let parsed = parse(vec![OsString::from("--send"), name.clone()], Path::new("/"));
        assert_eq!(
            parsed,
            Some(Ok(SendRequest {
                paths: vec![PathBuf::from(name)],
                dry_run: false,
            }))
        );
    }

    #[test]
    fn one_connected_device_takes_the_files_without_a_question() {
        assert_eq!(
            plan([("phone", true), ("tablet", false)]),
            Plan::One("phone".to_owned())
        );
    }

    #[test]
    fn several_connected_devices_ask_and_none_refuses() {
        assert_eq!(plan([("phone", true), ("tablet", true)]), Plan::Choose);
        assert_eq!(plan([("phone", false)]), Plan::NoDevice);
        assert_eq!(plan([]), Plan::NoDevice);
    }

    #[test]
    fn the_pick_names_a_listed_device_only() {
        super::set_chooser(super::Chooser {
            devices: vec![("a".into(), "A".into()), ("b".into(), "B".into())],
            paths: vec![PathBuf::from("/tmp/a.txt")],
        });
        assert_eq!(
            super::chosen(1),
            Some(("b".to_owned(), vec![PathBuf::from("/tmp/a.txt")]))
        );
        assert_eq!(super::chosen(2), None);
    }

    #[test]
    fn a_failed_chosen_send_is_marked_on_the_worker_and_waited_for() {
        super::begin_chosen();
        let worker = std::thread::spawn(|| super::run_chosen(|| Err("unreachable".to_owned())));
        super::wait_chosen();
        assert!(super::failed());
        assert!(worker.join().unwrap().is_err());
    }
}
