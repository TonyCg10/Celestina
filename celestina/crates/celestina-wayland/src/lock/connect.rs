//! The connection guard (round 5, ADR 0022 amendment A4): the lock client
//! never locks a session by accident.
//!
//! On 2026-10-22 a bare `celestina-lock --rehearsal`, started with the
//! `WAYLAND_DISPLAY` of the author's desktop inherited, connected to the real
//! compositor and locked the real session; the lock ignores `SIGTERM` and a
//! rehearsal never unlocks, so the author had to restart. This module is the
//! one place that opens the connection of every client of the lock profile
//! (`celestina-lock`, `unlock_probe`, `lock_cover`, `paint_probe`), and it
//! decides **before a single byte is sent** whether this process may talk to
//! that compositor at all.
//!
//! The caller states how it was launched with a [`ConnectPolicy`]; there is
//! no default, and [`run_lock`](super::run_lock) takes one. Each policy has a
//! precondition the guard checks itself, so a caller cannot claim more than
//! the environment shows:
//!
//! * [`ConnectPolicy::HandedInSocket`]: `WAYLAND_SOCKET` names a descriptor.
//!   The environment does not point at any compositor; the connection is the
//!   one a harness (a test's socketpair) handed over. Round 6 (finding I-A):
//!   that is no longer taken on trust, for production or for a nest-only
//!   caller alike. The value must be a decimal descriptor number of at least 3
//!   (parsed as `wayland-client` parses it; 0 to 2 are the standard streams),
//!   else the launch is refused as [`Refusal::BadSocket`]. The guard then
//!   adopts the descriptor (`Connection::connect_to_env`, which sends
//!   nothing), reads the `SO_PEERCRED` of the socket itself and accepts it only
//!   if the peer is a process of the same user that is **this process's
//!   parent** (the harness or test that launched us and made the socketpair)
//!   or a **verified nest** (as below). Anything else is dropped before a
//!   request exists. The shell's real lock is never started with
//!   `WAYLAND_SOCKET`, so the production rule is the same. Residual (Info,
//!   round 7): "peer == parent" takes for granted that the parent is a
//!   harness. A parent that is itself a compositor or a proxy that makes a
//!   socketpair for its children (sway/swaybg style, waypipe) would hand the
//!   lock to the real compositor. niri does not launch clients with
//!   `WAYLAND_SOCKET`, so that is not a plausible accident.
//! * [`ConnectPolicy::ShellLaunched`]: the environment carries
//!   `CELESTINA_LOCK_LAUNCHER=shell`, which the shell's supervisor adds to
//!   the exact allowlist it starts the lock with. The marker is a
//!   **declaration of intent against accidents**, not a security boundary: a
//!   process that can set an environment variable can also lock the session
//!   through any other client.
//! * [`ConnectPolicy::VerifiedNest`]: the process at the other end of the
//!   Wayland socket is a niri started by `scripts/dev-session.sh`. The guard
//!   connects once, reads the peer's `SO_PEERCRED` pid and uid (safe, through
//!   `rustix`), reads `/proc/<pid>/cmdline` and requires an argument that
//!   names [`NEST_MARKER`] (the nest's niri is started with
//!   `-c …/celestina-dev-session.kdl -- /bin/sh -c … celestina-dev-session`).
//!   It **fails closed**: an unreadable `/proc`, an unknown peer, another
//!   user's process or a missing marker is a refusal. The very stream that
//!   was checked becomes the connection, so there is no gap between the
//!   check and the use; on refusal it is dropped without a byte written.
//!   Anything that can run a process can fake a command line; this too
//!   guards against accidents.
//!
//! [`ConnectPolicy::detect`] chooses the policy from the environment for a
//! binary: a `nest_only` caller (`--rehearsal`, the probes) never gets
//! `ShellLaunched`, marker or not. Round 6 corrects the sentence this used to
//! end with ("a rehearsal always needs a nest"): a rehearsal needs a verified
//! nest **or** a handed-in socket whose other end is its own parent (see
//! above); nothing else.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use wayland_client::{ConnectError, Connection};

use super::LockError;

/// The variable the shell's supervisor adds to the environment it starts the
/// lock with.
pub const LAUNCHER_VARIABLE: &str = "CELESTINA_LOCK_LAUNCHER";
/// Its value.
pub const LAUNCHER_SHELL: &str = "shell";
/// What a nest's niri carries in its command line: an argument whose last
/// path component is this, or this plus `.kdl` (`dev-session.sh` names the
/// generated configuration `…/celestina-dev-session.kdl` and the `$0` of the
/// command niri runs `celestina-dev-session`).
pub const NEST_MARKER: &str = "celestina-dev-session";

/// How the process says it was launched. See the module documentation for
/// what each demands of the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectPolicy {
    /// Started by the shell's supervisor (`CELESTINA_LOCK_LAUNCHER=shell`).
    ShellLaunched,
    /// The compositor must be a nest started by `dev-session.sh`.
    VerifiedNest,
    /// The connection is the descriptor `WAYLAND_SOCKET` names.
    HandedInSocket,
}

impl ConnectPolicy {
    /// The policy the environment supports for a binary. `nest_only` is true
    /// for everything that is not the production lock (`--rehearsal`, the
    /// probes): it can never be `ShellLaunched`.
    #[must_use]
    pub fn detect(nest_only: bool) -> Self {
        Self::decide(nest_only, |name| std::env::var_os(name))
    }

    fn decide(nest_only: bool, read: impl Fn(&str) -> Option<OsString>) -> Self {
        if read("WAYLAND_SOCKET").is_some() {
            Self::HandedInSocket
        } else if !nest_only && read(LAUNCHER_VARIABLE).is_some_and(|v| v == LAUNCHER_SHELL) {
            Self::ShellLaunched
        } else {
            Self::VerifiedNest
        }
    }
}

/// Why the guard refused to connect. Nothing was sent to the compositor.
#[derive(Debug, thiserror::Error)]
pub enum Refusal {
    #[error(
        "refusing to lock: this process was not started by the shell \
         ({LAUNCHER_VARIABLE}={LAUNCHER_SHELL} is not set) and is not talking to a verified nest"
    )]
    NotLaunchedByShell,
    /// `WAYLAND_SOCKET` is set but is not a decimal descriptor number of at
    /// least 3 (checked before the toolkit adopts anything).
    #[error("refusing to lock: WAYLAND_SOCKET is not a descriptor number of 3 or more")]
    BadSocket,
    /// A handed-in socket whose other end is neither the process that
    /// launched us nor a verified nest.
    #[error(
        "refusing to lock: the socket in WAYLAND_SOCKET is connected to a process (pid {pid}) \
         that is neither the one that started this lock nor a nest started by dev-session.sh"
    )]
    NotTheHarness { pid: i32 },
    #[error("refusing to lock: the compositor's process is unknown ({0})")]
    PeerUnknown(#[source] io::Error),
    #[error("refusing to lock: the compositor at the other end is another user's process")]
    PeerForeign,
    #[error("refusing to lock: cannot read the command line of the compositor (pid {pid})")]
    CmdlineUnreadable {
        pid: i32,
        #[source]
        source: io::Error,
    },
    #[error(
        "refusing to lock: the compositor (pid {pid}) is not a nest started by dev-session.sh"
    )]
    NotANest { pid: i32 },
}

impl Refusal {
    /// The refusal and what the author does instead, for the log.
    #[must_use]
    pub fn explained(&self) -> String {
        format!(
            "{self}; to rehearse the lock use celestina/scripts/lock-rehearsal.sh, or run it \
             from a shell attached to a nest (dev-session.sh --shell); the production lock is \
             started by the Celestina shell"
        )
    }
}

/// The compositor's identity as the kernel reports it for a connected socket.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Peer {
    pub pid: i32,
    pub uid: u32,
}

/// Whether one `/proc/<pid>/cmdline` (NUL-separated) names a nest.
pub(crate) fn names_a_nest(cmdline: &[u8]) -> bool {
    cmdline.split(|b| *b == 0).any(|arg| {
        let last = arg.rsplit(|b| *b == b'/').next().unwrap_or_default();
        last == NEST_MARKER.as_bytes() || last == format!("{NEST_MARKER}.kdl").as_bytes()
    })
}

/// The verdict on a peer, from facts only: `peer` is what `SO_PEERCRED` gave,
/// `own_uid` this process's, `cmdline` how `/proc` answered. Every doubt is a
/// refusal.
pub(crate) fn judge_nest(
    peer: Result<Peer, io::Error>,
    own_uid: u32,
    cmdline: impl FnOnce(i32) -> io::Result<Vec<u8>>,
) -> Result<(), Refusal> {
    let peer = peer.map_err(Refusal::PeerUnknown)?;
    // Defence in depth, not reachable on the real path (round 6, m-b):
    // `peer_of` takes the pid from `rustix`, whose `UCred::pid` is a non-zero
    // `Pid`, so a kernel answer of 0 (a peer in another pid namespace) never
    // arrives here as 0; decoding that case without `unsafe` is not possible
    // with `rustix` 1.1.5 (residual, Info). The branch keeps the verdict
    // total for any other source of a `Peer` and is exercised by a unit test.
    if peer.pid <= 0 {
        return Err(Refusal::PeerUnknown(io::Error::other(
            "the peer's pid is not in this namespace",
        )));
    }
    if peer.uid != own_uid {
        return Err(Refusal::PeerForeign);
    }
    let bytes = cmdline(peer.pid).map_err(|source| Refusal::CmdlineUnreadable {
        pid: peer.pid,
        source,
    })?;
    if names_a_nest(&bytes) {
        Ok(())
    } else {
        Err(Refusal::NotANest { pid: peer.pid })
    }
}

/// The verdict on a handed-in socket (`WAYLAND_SOCKET`): the peer must be of
/// our own user and either `parent` (the harness that made the socketpair and
/// started us) or a verified nest. `parent` is `0` when unknown, which never
/// matches. Every doubt is a refusal.
pub(crate) fn judge_handed_in(
    peer: Result<Peer, io::Error>,
    own_uid: u32,
    parent: i32,
    cmdline: impl FnOnce(i32) -> io::Result<Vec<u8>>,
) -> Result<(), Refusal> {
    let peer = peer.map_err(Refusal::PeerUnknown)?;
    if peer.pid > 0 && peer.uid == own_uid && peer.pid == parent {
        return Ok(());
    }
    match judge_nest(Ok(peer), own_uid, cmdline) {
        Err(Refusal::NotANest { pid }) => Err(Refusal::NotTheHarness { pid }),
        other => other,
    }
}

/// The descriptor `WAYLAND_SOCKET` names: a plain decimal number of at least
/// 3, by the one rule of `celestina_lock_proto::wayland_fd` (round 7: the
/// lock's launcher uses the same function, so the two cannot disagree on
/// which descriptor survives the close sweep and which one is adopted).
pub(crate) fn handed_in_number(value: &OsStr) -> Result<i32, Refusal> {
    celestina_lock_proto::wayland_fd(value.to_str()).ok_or(Refusal::BadSocket)
}

fn peer_of(stream: &impl AsFd) -> io::Result<Peer> {
    // `rustix` types the pid as non-zero: a kernel answer of 0 (a peer outside
    // this pid namespace) is not representable and cannot be decoded here
    // without `unsafe`. Residual (Info, round 6, m-b): that case is not
    // reachable in the lock's deployments (one pid namespace), and the
    // `pid <= 0` branch of `judge_nest` is the defence if a different decoder
    // ever reports it.
    let cred = rustix::net::sockopt::socket_peercred(stream)?;
    Ok(Peer {
        pid: cred.pid.as_raw_nonzero().get(),
        uid: cred.uid.as_raw(),
    })
}

/// The compositor's socket path from the two variables, as the Wayland
/// libraries read them: a display name that is an absolute path stands for
/// itself.
fn socket_path(display: &OsStr, runtime: Option<&OsStr>) -> Option<PathBuf> {
    if display.is_empty() {
        return None;
    }
    let display = PathBuf::from(display);
    if display.is_absolute() {
        return Some(display);
    }
    // As the toolkit: a runtime directory that is not absolute names no
    // compositor (round 6, m-c).
    let runtime = PathBuf::from(runtime.filter(|r| !r.is_empty())?);
    runtime.is_absolute().then(|| runtime.join(display))
}

/// The pid of the process that started this one, or `0` when it has none in
/// this namespace (never matches a peer).
fn parent_pid() -> i32 {
    rustix::process::getppid().map_or(0, |pid| pid.as_raw_nonzero().get())
}

/// Opens the connection the policy allows, or refuses. Called once, by
/// `run_lock`, before any Wayland request exists.
///
/// # Errors
///
/// [`LockError::Refused`] when the policy's condition does not hold (nothing
/// was written to any socket), or the usual connection failures.
pub(crate) fn connect(policy: ConnectPolicy) -> Result<Connection, LockError> {
    connect_with(policy, |name| std::env::var_os(name))
}

fn connect_with(
    policy: ConnectPolicy,
    read: impl Fn(&str) -> Option<OsString>,
) -> Result<Connection, LockError> {
    if policy == ConnectPolicy::HandedInSocket {
        let value = read("WAYLAND_SOCKET").ok_or(LockError::Refused(Refusal::BadSocket))?;
        handed_in_number(&value).map_err(LockError::Refused)?;
        // The toolkit adopts the descriptor and sends nothing yet; the check
        // runs on the very socket that becomes the connection. On refusal the
        // connection is dropped before any request exists.
        let conn = Connection::connect_to_env().map_err(LockError::Connect)?;
        judge_handed_in(
            peer_of(&conn.backend().poll_fd()),
            rustix::process::getuid().as_raw(),
            parent_pid(),
            |pid| std::fs::read(format!("/proc/{pid}/cmdline")),
        )
        .map_err(LockError::Refused)?;
        return Ok(conn);
    }
    if policy == ConnectPolicy::ShellLaunched
        && !read(LAUNCHER_VARIABLE).is_some_and(|v| v == LAUNCHER_SHELL)
    {
        return Err(LockError::Refused(Refusal::NotLaunchedByShell));
    }
    let display = read("WAYLAND_DISPLAY").ok_or(LockError::Connect(ConnectError::NoCompositor))?;
    let runtime = read("XDG_RUNTIME_DIR");
    let path = socket_path(&display, runtime.as_deref())
        .ok_or(LockError::Connect(ConnectError::NoCompositor))?;
    let stream = UnixStream::connect(&path).map_err(LockError::Socket)?;
    if policy == ConnectPolicy::VerifiedNest {
        judge_nest(
            peer_of(&stream),
            rustix::process::getuid().as_raw(),
            |pid| std::fs::read(format!("/proc/{pid}/cmdline")),
        )
        .map_err(LockError::Refused)?;
    }
    Connection::from_socket(stream).map_err(LockError::Connect)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn the_policy_follows_the_environment_and_a_rehearsal_never_trusts_the_marker() {
        let marker = (LAUNCHER_VARIABLE, LAUNCHER_SHELL);
        let display = ("WAYLAND_DISPLAY", "wayland-1");
        assert_eq!(
            ConnectPolicy::decide(false, env(&[marker, display])),
            ConnectPolicy::ShellLaunched
        );
        // Bare: no marker, a display from a desktop session.
        assert_eq!(
            ConnectPolicy::decide(false, env(&[display])),
            ConnectPolicy::VerifiedNest
        );
        // A marker with another value is no marker.
        assert_eq!(
            ConnectPolicy::decide(false, env(&[(LAUNCHER_VARIABLE, "yes"), display])),
            ConnectPolicy::VerifiedNest
        );
        // --rehearsal and the probes: always a nest.
        assert_eq!(
            ConnectPolicy::decide(true, env(&[marker, display])),
            ConnectPolicy::VerifiedNest
        );
        // A handed-in socket is the harness's connection, whatever else is set.
        for nest_only in [false, true] {
            assert_eq!(
                ConnectPolicy::decide(nest_only, env(&[("WAYLAND_SOCKET", "5"), display])),
                ConnectPolicy::HandedInSocket
            );
        }
    }

    #[test]
    fn a_command_line_names_a_nest_by_an_argument_not_by_a_substring() {
        let nest = b"niri\0-c\0/run/user/1000/celestina-dev-session.kdl\0--\0/bin/sh\0";
        assert!(names_a_nest(nest));
        let once = b"niri\0-c\0/tmp/x.kdl\0--\0/bin/sh\0-c\0echo hi\0celestina-dev-session\0";
        assert!(names_a_nest(once));
        assert!(!names_a_nest(b"niri\0--session\0"));
        assert!(!names_a_nest(b""));
        // A substring of a longer word is not the marker.
        assert!(!names_a_nest(
            b"niri\0-c\0/x/not-celestina-dev-sessions.kdl\0"
        ));
        assert!(!names_a_nest(b"niri\0-c\0/x/celestina-dev-session-old\0"));
    }

    fn me() -> Peer {
        Peer {
            pid: 4242,
            uid: 1000,
        }
    }

    #[test]
    fn t_e_a_nest_peer_is_accepted_and_every_doubt_is_a_refusal() {
        let nest = |_| Ok(b"niri\0-c\0/r/celestina-dev-session.kdl\0".to_vec());
        assert!(judge_nest(Ok(me()), 1000, nest).is_ok());

        // Not a nest.
        let plain = |_| Ok(b"niri\0--session\0".to_vec());
        assert!(matches!(
            judge_nest(Ok(me()), 1000, plain),
            Err(Refusal::NotANest { pid: 4242 })
        ));
        // /proc unreadable: fail closed.
        let denied = |_| Err(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(matches!(
            judge_nest(Ok(me()), 1000, denied),
            Err(Refusal::CmdlineUnreadable { pid: 4242, .. })
        ));
        // The peer cannot be identified.
        let never = |_| -> io::Result<Vec<u8>> { unreachable_for_test() };
        assert!(matches!(
            judge_nest(Err(io::Error::from_raw_os_error(95)), 1000, never),
            Err(Refusal::PeerUnknown(_))
        ));
        // A pid of 0: another namespace.
        assert!(matches!(
            judge_nest(Ok(Peer { pid: 0, uid: 1000 }), 1000, never),
            Err(Refusal::PeerUnknown(_))
        ));
        // Another user's process never counts, whatever its command line says.
        assert!(matches!(
            judge_nest(Ok(Peer { pid: 4242, uid: 0 }), 1000, nest),
            Err(Refusal::PeerForeign)
        ));
    }

    /// The closure that must not be reached: a refusal decided before `/proc`
    /// is read.
    fn unreachable_for_test() -> io::Result<Vec<u8>> {
        Err(io::Error::other(
            "/proc was read although the peer was refused",
        ))
    }

    #[test]
    fn the_socket_path_follows_the_wayland_convention() {
        let path = |d: &str, r: Option<&str>| socket_path(OsStr::new(d), r.map(OsStr::new));
        assert_eq!(
            path("wayland-1", Some("/run/user/1")),
            Some(PathBuf::from("/run/user/1/wayland-1"))
        );
        assert_eq!(path("/tmp/s", None), Some(PathBuf::from("/tmp/s")));
        assert_eq!(path("wayland-1", None), None);
        assert_eq!(path("wayland-1", Some("")), None);
        assert_eq!(path("", Some("/run/user/1")), None);
        // Round 6, m-c: a relative runtime directory names no compositor.
        assert_eq!(path("wayland-1", Some("run/user/1")), None);
        assert_eq!(path("wayland-1", Some(".")), None);
        // An absolute display does not consult the runtime directory.
        assert_eq!(
            path("/tmp/s", Some("relative")),
            Some(PathBuf::from("/tmp/s"))
        );
    }

    #[test]
    fn a_handed_in_socket_must_name_a_descriptor_of_three_or_more_as_the_toolkit_reads_it() {
        use std::os::unix::ffi::OsStrExt as _;
        for good in ["3", "5", "57"] {
            assert!(handed_in_number(OsStr::new(good)).is_ok(), "{good:?}");
        }
        assert_eq!(handed_in_number(OsStr::new("5")).ok(), Some(5));
        for bad in [
            "",
            "0",
            "1",
            "2",
            "-1",
            "x",
            "5x",
            " 5",
            "5\n",
            "5 6",
            "99999999999",
            "0x10",
            // Round 7: the sign the toolkit would accept is refused here.
            "+5",
        ] {
            assert!(
                matches!(handed_in_number(OsStr::new(bad)), Err(Refusal::BadSocket)),
                "{bad:?}"
            );
        }
        assert!(matches!(
            handed_in_number(OsStr::from_bytes(b"\xff5")),
            Err(Refusal::BadSocket)
        ));
    }

    #[test]
    fn a_handed_in_socket_is_accepted_from_the_parent_or_a_nest_and_from_nobody_else() {
        let parent = Peer {
            pid: 100,
            uid: 1000,
        };
        let nest_cmd = |_| Ok(b"niri\0-c\0/r/celestina-dev-session.kdl\0".to_vec());
        let plain_cmd = |_| Ok(b"python3\0-c\0x\0".to_vec());
        let never = |_| -> io::Result<Vec<u8>> { unreachable_for_test() };
        // The harness that made the socketpair and started us: /proc is not
        // even read.
        assert!(judge_handed_in(Ok(parent), 1000, 100, never).is_ok());
        // A verified nest that is not our parent.
        assert!(judge_handed_in(Ok(me()), 1000, 100, nest_cmd).is_ok());
        // Some other process of the user: refused, with its pid.
        assert!(matches!(
            judge_handed_in(Ok(me()), 1000, 100, plain_cmd),
            Err(Refusal::NotTheHarness { pid: 4242 })
        ));
        // The parent of another user never counts.
        assert!(matches!(
            judge_handed_in(Ok(Peer { pid: 100, uid: 0 }), 1000, 100, never),
            Err(Refusal::PeerForeign)
        ));
        // An unknown peer and an unreadable /proc fail closed.
        assert!(matches!(
            judge_handed_in(Err(io::Error::from_raw_os_error(95)), 1000, 100, never),
            Err(Refusal::PeerUnknown(_))
        ));
        let denied = |_| Err(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(matches!(
            judge_handed_in(Ok(me()), 1000, 100, denied),
            Err(Refusal::CmdlineUnreadable { pid: 4242, .. })
        ));
        // A parent that is unknown (0) is no match, not even for a pid of 0.
        assert!(matches!(
            judge_handed_in(Ok(Peer { pid: 0, uid: 1000 }), 1000, 0, never),
            Err(Refusal::PeerUnknown(_))
        ));
        assert!(matches!(
            judge_handed_in(Ok(me()), 1000, 0, plain_cmd),
            Err(Refusal::NotTheHarness { pid: 4242 })
        ));
    }

    #[test]
    fn a_policy_whose_condition_does_not_hold_is_refused_before_any_socket_is_opened() {
        // `ShellLaunched` without the marker: refused although a display is
        // named (the path does not exist; it is never touched).
        let result = connect_with(
            ConnectPolicy::ShellLaunched,
            env(&[
                ("WAYLAND_DISPLAY", "w"),
                ("XDG_RUNTIME_DIR", "/nonexistent"),
            ]),
        );
        assert!(matches!(
            result,
            Err(LockError::Refused(Refusal::NotLaunchedByShell))
        ));
        // `HandedInSocket` without a socket, and with a value that is no
        // descriptor of 3 or more: refused before the toolkit adopts
        // anything.
        for pairs in [
            &[][..],
            &[("WAYLAND_SOCKET", "")][..],
            &[("WAYLAND_SOCKET", "0")][..],
            &[("WAYLAND_SOCKET", "2")][..],
            &[("WAYLAND_SOCKET", "x")][..],
        ] {
            let result = connect_with(ConnectPolicy::HandedInSocket, env(pairs));
            assert!(
                matches!(result, Err(LockError::Refused(Refusal::BadSocket))),
                "{pairs:?}"
            );
        }
    }

    #[test]
    fn a_nest_check_against_a_listener_that_is_not_a_nest_refuses_and_sends_nothing() {
        use std::io::Read as _;
        use std::os::unix::net::UnixListener;
        let dir =
            std::env::temp_dir().join(format!("celestina-connect-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let listener = UnixListener::bind(dir.join("wl")).expect("bind");
        let result = connect_with(
            ConnectPolicy::VerifiedNest,
            env(&[
                ("WAYLAND_DISPLAY", "wl"),
                ("XDG_RUNTIME_DIR", dir.to_str().expect("utf-8")),
            ]),
        );
        // The listener is this very test process: no marker in its command
        // line, same user.
        assert!(matches!(
            result,
            Err(LockError::Refused(Refusal::NotANest { .. }))
        ));
        let (mut accepted, _) = listener.accept().expect("the check connected once");
        let mut rest = Vec::new();
        accepted.read_to_end(&mut rest).expect("eof");
        assert!(rest.is_empty(), "nothing was written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
