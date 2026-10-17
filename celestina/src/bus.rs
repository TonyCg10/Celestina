//! `org.celestina.Shell1`: the shell's command surface and single instance.
//!
//! The running shell serves [`INTERFACE`] at [`OBJECT_PATH`] and only then
//! claims [`BUS_NAME`] with `DoNotQueue`, so the instant it owns the name a
//! client can already be answered. A second shell finds the name taken
//! ([`BusError::Running`]) and exits; it never waits in the queue to inherit
//! the name. A client (`celestina <verb>`) calls `Command(as) -> (b, s)` with
//! the verb's words and a bounded wait.
//!
//! zbus runs blocking, on its own executor thread (no async runtime in the
//! shell, design §6.4). The served object only parses the words, which any
//! peer may send, and queues the verb into the loop; it never touches a
//! surface. The claim pattern follows `celestina-core`'s `activation`, whose
//! interface (`org.celestina.Application1`: `Activate`, `Open`) is the
//! applications' and not the shell's.

use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use celestina_wayland::Sender;
use zbus::{
    blocking::{connection::Builder, Connection},
    fdo::{RequestNameFlags, RequestNameReply},
};

use crate::{command::Verb, message::Message, providers::Worker};

/// The shell's well-known name.
pub const BUS_NAME: &str = "org.celestina.Shell";
/// Where the shell serves [`INTERFACE`].
pub const OBJECT_PATH: &str = "/org/celestina/Shell";
/// The command surface.
pub const INTERFACE: &str = "org.celestina.Shell1";
/// How long a client waits for the shell's answer.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(3);
/// How often the bus worker looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(100);

/// Why the bus could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    /// Another process owns [`BUS_NAME`]: the shell is already running.
    #[error("the shell is running")]
    Running,
    /// Nobody owns [`BUS_NAME`].
    #[error("the shell is not running")]
    NotRunning,
    /// The shell did not answer within [`CALL_TIMEOUT`].
    #[error("the shell did not answer within {} s", CALL_TIMEOUT.as_secs())]
    Timeout,
    /// The shell answered that it cannot run the verb.
    #[error("the shell refused the verb: {0}")]
    Refused(String),
    /// No session bus, or the bus failed the exchange.
    #[error("the session bus failed")]
    Bus(#[source] zbus::Error),
    /// The bus worker's thread could not start.
    #[error("cannot start the bus worker")]
    Spawn(#[source] io::Error),
}

/// Where the served object puts a parsed verb; `false` when the loop is
/// gone.
type Sink = Box<dyn Fn(Verb) -> bool + Send + Sync>;

/// The served object.
struct Shell1 {
    sink: Sink,
}

#[zbus::interface(name = "org.celestina.Shell1")]
impl Shell1 {
    /// Runs the verb `words` spell: `(true, "")` once it is queued,
    /// `(false, why)` for words that are not a verb or a shell that is
    /// stopping.
    fn command(&self, words: Vec<String>) -> (bool, String) {
        match Verb::parse(&words) {
            Ok(verb) => {
                if (self.sink)(verb) {
                    (true, String::new())
                } else {
                    (false, "the shell is stopping".to_owned())
                }
            }
            Err(err) => (false, err.to_string()),
        }
    }
}

/// Serves the command surface on the session bus, then claims
/// [`BUS_NAME`]. The returned worker keeps the connection until `shutdown`
/// is raised, then releases the name.
///
/// # Errors
///
/// [`BusError::Running`] when another shell owns the name; [`BusError::Bus`]
/// without a session bus; [`BusError::Spawn`] when the worker cannot start.
pub fn claim_and_serve(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> Result<Worker, BusError> {
    let sink: Sink = Box::new(move |verb| sender.send(Message::Command(verb)).is_ok());
    claim_on(Builder::session().map_err(BusError::Bus)?, sink, shutdown)
}

fn claim_on(
    builder: Builder<'static>,
    sink: Sink,
    shutdown: &Arc<AtomicBool>,
) -> Result<Worker, BusError> {
    let connection = builder
        .serve_at(OBJECT_PATH, Shell1 { sink })
        .and_then(Builder::build)
        .map_err(BusError::Bus)?;
    // `DoNotQueue`: without it the bus answers `InQueue`, and a second shell
    // would wait in the queue to inherit the name when the first exits.
    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
        Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
            return Err(BusError::Running)
        }
        Err(err) => return Err(BusError::Bus(err)),
    }
    let flag = Arc::clone(shutdown);
    Worker::spawn("shell-bus", shutdown, move || {
        while !flag.load(Ordering::Acquire) {
            thread::sleep(POLL);
        }
        if let Err(err) = connection.release_name(BUS_NAME) {
            tracing::debug!(%err, "releasing {BUS_NAME}");
        }
        drop(connection);
    })
    .map_err(BusError::Spawn)
}

/// Forwards `verb` to the running shell and waits at most
/// [`CALL_TIMEOUT`] for its answer.
///
/// # Errors
///
/// [`BusError::NotRunning`], [`BusError::Timeout`], [`BusError::Refused`],
/// or [`BusError::Bus`] without a session bus.
pub fn send_verb(verb: &Verb) -> Result<(), BusError> {
    send_on(Builder::session().map_err(BusError::Bus)?, verb)
}

fn send_on(builder: Builder<'static>, verb: &Verb) -> Result<(), BusError> {
    let connection = builder
        .method_timeout(CALL_TIMEOUT)
        .build()
        .map_err(BusError::Bus)?;
    let (done, why) = call(&connection, &verb.words()).map_err(classify)?;
    if done {
        Ok(())
    } else {
        Err(BusError::Refused(why))
    }
}

fn call(connection: &Connection, words: &[String]) -> zbus::Result<(bool, String)> {
    let reply = connection.call_method(
        Some(BUS_NAME),
        OBJECT_PATH,
        Some(INTERFACE),
        "Command",
        &(words,),
    )?;
    reply.body().deserialize()
}

/// A failed call as the client reports it.
fn classify(err: zbus::Error) -> BusError {
    if let zbus::Error::InputOutput(io) = &err {
        if io.kind() == io::ErrorKind::TimedOut {
            return BusError::Timeout;
        }
    }
    // zbus maps a reply's error name onto the typed `fdo::Error` variants,
    // whether it arrived as `MethodError` or as `FDO`.
    match zbus::fdo::Error::from(err.clone()) {
        zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
            BusError::NotRunning
        }
        _ => BusError::Bus(err),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader},
        path::PathBuf,
        process::{Child, Command, Stdio},
        sync::mpsc,
    };

    use super::*;
    use crate::pieces::PageId;

    /// A `dbus-daemon` of the test's own, removed when dropped.
    struct PrivateBus {
        child: Child,
        dir: PathBuf,
        address: String,
    }

    impl PrivateBus {
        /// `None` (with a notice) where `dbus-daemon` is not installed.
        fn start(name: &str) -> Option<PrivateBus> {
            let dir =
                std::env::temp_dir().join(format!("celestina-bus-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).ok()?;
            let spawned = Command::new("dbus-daemon")
                .arg("--session")
                .arg("--nofork")
                .arg("--print-address=1")
                .arg(format!("--address=unix:path={}", dir.join("bus").display()))
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child) = spawned else {
                eprintln!("dbus-daemon is not installed: the bus test is skipped");
                return None;
            };
            let mut address = String::new();
            let read = child
                .stdout
                .take()
                .map(|out| BufReader::new(out).read_line(&mut address));
            if !matches!(read, Some(Ok(n)) if n > 0) {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Some(PrivateBus {
                child,
                dir,
                address: address.trim().to_owned(),
            })
        }

        fn builder(&self) -> Builder<'static> {
            match Builder::address(self.address.as_str()) {
                Ok(b) => b,
                Err(err) => panic!("bad bus address {}: {err}", self.address),
            }
        }
    }

    impl Drop for PrivateBus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn sink_to(tx: mpsc::Sender<Verb>) -> Sink {
        let tx = std::sync::Mutex::new(tx);
        Box::new(move |verb| tx.lock().is_ok_and(|tx| tx.send(verb).is_ok()))
    }

    #[test]
    fn errors_are_classified_by_their_dbus_variant() {
        let fdo = |e: zbus::fdo::Error| zbus::Error::FDO(Box::new(e));
        for gone in [
            zbus::fdo::Error::ServiceUnknown("no owner".into()),
            zbus::fdo::Error::NameHasNoOwner("no owner".into()),
        ] {
            assert!(matches!(classify(fdo(gone)), BusError::NotRunning));
        }
        assert!(matches!(
            classify(fdo(zbus::fdo::Error::AccessDenied("no".into()))),
            BusError::Bus(_)
        ));
        let timed_out = std::io::Error::from(io::ErrorKind::TimedOut);
        assert!(matches!(
            classify(zbus::Error::InputOutput(Arc::new(timed_out))),
            BusError::Timeout
        ));
    }

    #[test]
    fn one_shell_owns_the_name_and_answers_verbs_until_it_stops() {
        let Some(bus) = PrivateBus::start("claim") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let worker = claim_on(bus.builder(), sink_to(tx.clone()), &shutdown)
            .expect("the first shell claims the name");

        // A second shell finds it running.
        let second = Arc::new(AtomicBool::new(false));
        assert!(matches!(
            claim_on(bus.builder(), sink_to(tx.clone()), &second),
            Err(BusError::Running)
        ));

        // A client's verb reaches the first shell.
        let verb = Verb::IslandOpen(PageId::TRAY);
        send_on(bus.builder(), &verb).expect("the verb is delivered");
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(verb));

        // A verb with text payloads crosses the bus word for word.
        let verb = Verb::WallpaperSet {
            target: crate::command::Target::Output("DP-1".to_owned()),
            file: "sea.png".to_owned(),
        };
        send_on(bus.builder(), &verb).expect("the wallpaper verb is delivered");
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(verb));

        // A file name that is a path is refused before it is queued.
        let connection = bus.builder().build().expect("a client connection");
        let path = ["wallpaper", "set", "all", "../x.png"].map(str::to_owned);
        let (done, _) = call(&connection, &path).expect("an answer");
        assert!(!done);
        assert!(rx.try_recv().is_err());

        // Words that are not a verb are refused with the reason.
        let connection = bus.builder().build().expect("a client connection");
        let (done, why) =
            call(&connection, &["island".to_owned(), "fly".to_owned()]).expect("an answer");
        assert!(!done);
        assert_eq!(why, "unknown verb \"island fly\"");
        assert!(rx.try_recv().is_err());

        // Stopping releases the name: the next shell owns it.
        worker.join();
        assert!(matches!(
            send_on(bus.builder(), &Verb::Quit),
            Err(BusError::NotRunning)
        ));
        let third = Arc::new(AtomicBool::new(false));
        let next = claim_on(bus.builder(), sink_to(tx), &third).expect("the name is free again");
        next.join();
    }
}
