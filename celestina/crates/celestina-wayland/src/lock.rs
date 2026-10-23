//! The session-lock profile of the toolkit (CEL-2-H design §4.6, §4.13): the
//! Wayland client of `celestina-lock`.
//!
//! What it does for the product, so product code cannot get it wrong:
//!
//! * it asks the compositor for `ext_session_lock_v1`, creates exactly one
//!   lock surface per output (also for an output that appears later) and
//!   commits a black `wl_shm` cover on every `configure` before the product
//!   hears of it ([`flow`], [`cover`]);
//! * it delivers the keyboard and pointer of the surface under focus, with
//!   `Key`'s `Debug` redacting what was typed;
//! * it hands the product one [`UnlockCapability`] at `locked`, which is the
//!   only way to end the lock;
//! * it does not stop on `SIGINT`, `SIGTERM` or `SIGHUP`: a signal any process
//!   of the user can send must not unlock the session (it is logged).
//!
//! What it never binds: layer shell, `xdg_wm_base`, the data device, primary
//! selection, text input, background effect, cursor shape or the
//! single-pixel buffer. No paste, no input-method activation, no popup, no
//! GPU: the globals are the list [`BOUND_GLOBALS`].
//!
//! Painting is the product's, into memory the toolkit owns: after `locked` it
//! calls [`LockCtl::present`] with a closure that draws on one of two
//! `wl_shm` buffers ([`canvas`]). `wp_viewporter` and `wp_fractional_scale_v1`
//! are bound, when the compositor offers them, only to give those buffers the
//! real pixel size of a fractionally scaled output.
#![forbid(unsafe_code)]

mod canvas;
mod capability;
mod connect;
mod cover;
mod flow;
mod handlers;

use calloop::{
    channel::{self, Event as ChannelEvent},
    signals::{Signal, Signals},
    EventLoop, LoopHandle, LoopSignal,
};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState, seat::SeatState,
    session_lock::SessionLockState, shm::Shm,
};
use wayland_client::{
    globals::{registry_queue_init, BindError, GlobalError},
    ConnectError, DispatchError, Proxy,
};
use wayland_protocols::{
    ext::session_lock::v1::client::ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    wp::{
        fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        viewporter::client::wp_viewporter::WpViewporter,
    },
};

pub use canvas::{FrameBuf, PresentError};
pub use capability::{UnlockCapability, UnlockError};
pub use connect::{ConnectPolicy, Refusal, LAUNCHER_SHELL, LAUNCHER_VARIABLE, NEST_MARKER};
pub use handlers::LockRuntime;

use crate::{input::InputEvent, output::OutputId, sender::Sender};

/// The Wayland interfaces the lock profile binds, by name. `wl_output` is
/// bound by sctk's `OutputState` (which also takes `zxdg_output_manager_v1`
/// when the compositor has it, for the logical size) and `wl_seat` by its
/// `SeatState`. Nothing outside this list is bound (T-W2).
pub const BOUND_GLOBALS: &[&str] = &[
    "wl_compositor",
    "wl_shm",
    "wl_output",
    "zxdg_output_manager_v1",
    "wl_seat",
    "ext_session_lock_manager_v1",
    // Optional: a frame at a fractional scale needs both; without them the
    // frame is the logical size at scale 1.
    "wp_viewporter",
    "wp_fractional_scale_manager_v1",
];

/// Signals the lock profile logs and ignores instead of exiting: exactly
/// `SIGINT`, `SIGTERM`, `SIGHUP`, `SIGQUIT`, `SIGABRT`, `SIGALRM`, `SIGUSR1`
/// and `SIGUSR2`. The rest is the residual X2 of F27, and none of it unlocks;
/// it can only kill or stop the lock, and the compositor keeps the session
/// covered: `SIGKILL` and `SIGSTOP` (they cannot be caught); `SIGSEGV`,
/// `SIGBUS`, `SIGFPE` and `SIGILL` (terminate the process with a core dump
/// when sent with `kill`); `SIGTSTP`, `SIGTTIN` and `SIGTTOU` (stop it);
/// `SIGVTALRM`, `SIGPROF`, `SIGIO`, `SIGPWR`, `SIGSYS`, `SIGXCPU`, `SIGXFSZ`,
/// `SIGSTKFLT` and `SIGTRAP`; and the real-time signals. `SIGABRT` raised by
/// the process itself (`abort`) still terminates it: `abort` unblocks the
/// signal first. `SIGPIPE` (the Rust runtime sets it to ignored before `main`),
/// `SIGCHLD`, `SIGCONT` (on a process that is not stopped), `SIGURG` and
/// `SIGWINCH` have no effect on a running process: their default action is to
/// ignore them, so they need neither a place in this list nor a residual.
pub const IGNORED_SIGNALS: &[Signal] = &[
    Signal::SIGINT,
    Signal::SIGTERM,
    Signal::SIGHUP,
    Signal::SIGQUIT,
    Signal::SIGABRT,
    Signal::SIGALRM,
    Signal::SIGUSR1,
    Signal::SIGUSR2,
];

/// What happened that the product must react to.
#[derive(Debug)]
pub enum LockEvent {
    /// The compositor confirmed the lock: every output is covered or blanked
    /// by the compositor. Delivered once, with the only capability that ends
    /// the lock.
    Locked(UnlockCapability),
    /// The compositor ended the lock request: refused while another client
    /// holds the lock (before `Locked`), or ended by the compositor's own
    /// policy (after it). The runtime keeps running until the product stops it.
    Finished,
    /// A lock surface was configured and its cover committed. `size` is the
    /// logical size the compositor asked for.
    SurfaceConfigured { output: OutputId, size: (u32, u32) },
    /// An output appeared; its lock surface is created at once when a lock
    /// is requested.
    OutputAdded(crate::output::OutputInfo),
    /// An output went away; its lock surface is destroyed.
    OutputRemoved(OutputId),
    /// The seat gained or lost its keyboard capability.
    Keyboard { present: bool },
    /// What is on screen is stale and the product should draw its scene
    /// again: the output's scale changed under a shown frame, or a frame was
    /// refused for lack of a free buffer and one came back.
    Redraw,
}

/// The product side of the lock profile.
pub trait LockShell: 'static {
    /// What workers publish through the [`Sender`].
    type Message: Send + 'static;
    /// A lock event; see [`LockEvent`].
    fn event(&mut self, ev: LockEvent, ctl: &mut LockCtl<'_>);
    /// Pointer and keyboard input on the lock surface of `output`.
    fn input(&mut self, output: OutputId, ev: InputEvent, ctl: &mut LockCtl<'_>);
    /// A message a worker sent through the [`Sender`].
    fn message(&mut self, msg: Self::Message, ctl: &mut LockCtl<'_>);
}

/// What the product can ask of the runtime during a call.
pub struct LockCtl<'a> {
    signal: &'a LoopSignal,
    keyboard: bool,
    surfaces: &'a mut dyn canvas::Surfaces,
}

impl LockCtl<'_> {
    /// Stops the loop once this call returns; `run_lock` returns after it.
    /// Stopping without having unlocked leaves the session locked: dropping
    /// a locked `SessionLock` is a protocol error the compositor answers by
    /// ending this client, and the session stays locked (fails secure).
    pub fn stop(&mut self) {
        self.signal.stop();
    }

    /// The seat currently has a keyboard.
    pub fn has_keyboard(&self) -> bool {
        self.keyboard
    }

    /// Draws one frame on the lock surface of `output` and shows it.
    ///
    /// `paint` gets one of the output's two buffers ([`FrameBuf`]) and
    /// returns the rectangle it changed in buffer pixels, relative to the
    /// frame shown before; `None` shows nothing new. The error says why no
    /// frame was shown; the surface then keeps what it had, the cover or the
    /// previous frame, and the session stays locked.
    pub fn present(
        &mut self,
        output: OutputId,
        paint: impl FnOnce(FrameBuf<'_>) -> Option<crate::PixelRect>,
    ) -> Result<(), PresentError> {
        let mut paint = Some(paint);
        self.surfaces.present(output, &mut |frame| {
            paint.take().and_then(|paint| paint(frame))
        })
    }

    /// Buffer pixels per logical pixel on `output`'s lock surface right now
    /// (`1.0` when unknown or not fractionally scaled).
    #[must_use]
    pub fn scale_of(&self, output: OutputId) -> f64 {
        self.surfaces.scale_of(output)
    }
}

impl<'a> LockCtl<'a> {
    pub(crate) fn new(
        signal: &'a LoopSignal,
        keyboard: bool,
        surfaces: &'a mut dyn canvas::Surfaces,
    ) -> Self {
        Self {
            signal,
            keyboard,
            surfaces,
        }
    }
}

/// What stops the lock profile from starting or running.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("cannot {what}")]
    Loop {
        what: &'static str,
        #[source]
        source: calloop::Error,
    },
    #[error("cannot connect to the Wayland display")]
    Connect(#[source] ConnectError),
    /// The connection guard refused to connect (round 5, ADR 0022 A4):
    /// nothing was written to the compositor.
    #[error("{}", .0.explained())]
    Refused(#[source] Refusal),
    /// The compositor's socket could not be opened.
    #[error("cannot open the compositor's socket: {0}")]
    Socket(#[source] std::io::Error),
    #[error("cannot read the Wayland registry")]
    Registry(#[source] GlobalError),
    #[error("the compositor lacks the required global {interface}")]
    MissingGlobal {
        interface: &'static str,
        #[source]
        source: BindError,
    },
    /// The compositor does not offer `ext_session_lock_manager_v1`; nothing
    /// was requested.
    #[error("the compositor does not offer ext_session_lock_v1")]
    NoSessionLock,
    #[error("the Wayland round trip failed")]
    Dispatch(#[source] DispatchError),
    #[error("the lock request could not be sent")]
    Request(#[source] smithay_client_toolkit::error::GlobalError),
    /// The event loop ended with an error after the lock was requested: the
    /// Wayland connection broke (the compositor quit or crashed, or a
    /// protocol error killed this client).
    #[error("the Wayland connection was lost")]
    ConnectionLost(#[source] calloop::Error),
    /// The product's `setup` refused to go on (a worker it needs could not
    /// start); nothing was requested.
    #[error("the product's setup failed")]
    Setup(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// `EPROTO`: how the Wayland source reports a protocol error through the loop
/// (it returns `Errno::PROTO` as a `calloop::Error::IoError`).
const EPROTO: i32 = 71;

/// Whether `err`, or anything it wraps, is a protocol error. The loop does
/// not hand the source's error over as it is: calloop 0.14 wraps what a source
/// returns in `OtherError(Box<dyn Error>)`, so a protocol error leaves
/// `EventLoop::run` as `OtherError(Box<calloop::Error::IoError(EPROTO)>)`.
/// The walk therefore unwraps `OtherError` recursively (bounded by the depth
/// of the error value) and also accepts a bare `io::Error` and a
/// `WaylandError::Protocol`.
fn wraps_protocol_error(err: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(err) = err.downcast_ref::<calloop::Error>() {
        return match err {
            calloop::Error::IoError(io) => io.raw_os_error() == Some(EPROTO),
            calloop::Error::OtherError(inner) => wraps_protocol_error(&**inner),
            _ => false,
        };
    }
    if let Some(io) = err.downcast_ref::<std::io::Error>() {
        return io.raw_os_error() == Some(EPROTO);
    }
    matches!(
        err.downcast_ref::<wayland_client::backend::WaylandError>(),
        Some(wayland_client::backend::WaylandError::Protocol(_))
    )
}

impl LockError {
    /// Whether this failure is the compositor killing the client with a
    /// protocol error (the compositor is alive, the session stays locked),
    /// as opposed to a broken connection (the compositor is gone).
    #[must_use]
    pub fn is_protocol_error(&self) -> bool {
        match self {
            Self::ConnectionLost(err) => wraps_protocol_error(err),
            _ => false,
        }
    }
}

/// Binds a global the lock profile can do without: a missing one only costs
/// the fractional-scale path.
fn bind_optional<I, S>(
    globals: &wayland_client::globals::GlobalList,
    qh: &wayland_client::QueueHandle<LockRuntime<S>>,
) -> Option<I>
where
    I: Proxy + 'static,
    S: LockShell,
    handlers::NoEvents: smithay_client_toolkit::dispatch2::Dispatch2<I, LockRuntime<S>>,
{
    globals.bind::<I, _, _>(qh, 1..=1, handlers::NoEvents).ok()
}

fn insert_error<T>(what: &'static str) -> impl FnOnce(calloop::InsertError<T>) -> LockError {
    move |err| LockError::Loop {
        what,
        source: err.error,
    }
}

/// Logs and ignores [`IGNORED_SIGNALS`] on `handle`'s loop;
/// `observe` sees each one (tests count them; the lock passes a no-op).
///
/// The signals are blocked in the **calling thread** and read through a
/// signalfd, so this must run before any other thread of the process is
/// started: a thread created earlier does not inherit the mask and would
/// terminate the process on a signal the policy means to ignore.
pub fn insert_signal_policy<D: 'static>(
    handle: &LoopHandle<'static, D>,
    mut observe: impl FnMut(Signal) + 'static,
) -> Result<(), LockError> {
    let signals = Signals::new(IGNORED_SIGNALS).map_err(|source| LockError::Loop {
        what: "create the signal source",
        source,
    })?;
    handle
        .insert_source(signals, move |fired, _, _| {
            tracing::warn!(signal = ?fired.signal(), "signal ignored: the session stays locked");
            observe(fired.signal());
        })
        .map_err(insert_error("insert the signal source"))?;
    Ok(())
}

/// Connects, requests the session lock, covers every output and runs until
/// the product stops the loop (`LockCtl::stop`) or the connection breaks.
///
/// `policy` says how the process was launched and is **checked before the
/// connection is used** ([`connect`]): a launch the environment does not
/// support ends with [`LockError::Refused`] and no byte sent.
///
/// `setup` runs once the connection and the globals exist and **before** the
/// lock is requested: the place to start workers and register loop sources,
/// and for `celestina-lock` to announce `requested`. Signals are set up
/// before `setup`, so threads it starts inherit their mask. When `setup`
/// fails (typically [`LockError::Setup`]) the lock is not requested.
pub fn run_lock<S: LockShell>(
    policy: ConnectPolicy,
    shell: S,
    setup: impl FnOnce(
        Sender<S::Message>,
        &LoopHandle<'static, LockRuntime<S>>,
    ) -> Result<(), LockError>,
) -> Result<(), LockError> {
    let mut event_loop: EventLoop<'static, LockRuntime<S>> =
        EventLoop::try_new().map_err(|source| LockError::Loop {
            what: "create the event loop",
            source,
        })?;
    let handle = event_loop.handle();
    insert_signal_policy(&handle, |_| {})?;

    let conn = connect::connect(policy)?;
    let (globals, mut queue) =
        registry_queue_init::<LockRuntime<S>>(&conn).map_err(LockError::Registry)?;
    let qh = queue.handle();
    let offers_lock = globals.contents().with_list(|list| {
        list.iter()
            .any(|global| global.interface == ExtSessionLockManagerV1::interface().name)
    });
    if !offers_lock {
        return Err(LockError::NoSessionLock);
    }
    let missing = |interface: &'static str| {
        move |source: BindError| LockError::MissingGlobal { interface, source }
    };
    let viewporter = bind_optional::<WpViewporter, S>(&globals, &qh);
    let fractional_scale = bind_optional::<WpFractionalScaleManagerV1, S>(&globals, &qh);
    let shm = Shm::bind(&globals, &qh).map_err(missing("wl_shm"))?;
    let canvas = canvas::Canvas::new(
        Shm::from(shm.wl_shm().clone()),
        qh.clone(),
        viewporter,
        fractional_scale,
    );
    let env = handlers::Env {
        conn: conn.clone(),
        signal: event_loop.get_signal(),
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        seat_state: SeatState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh).map_err(missing("wl_compositor"))?,
        shm,
        canvas,
        session_lock_state: SessionLockState::new(&globals, &qh),
        session_lock: None,
        keyboard: None,
        pointer: None,
        modifiers: crate::input::Modifiers::default(),
        keyboard_focus: None,
    };
    let mut rt = LockRuntime::new(env, shell);

    let (sender, receiver) = channel::channel::<S::Message>();
    handle
        .insert_source(receiver, |event, _, rt| {
            if let ChannelEvent::Msg(msg) = event {
                rt.message(msg);
            }
        })
        .map_err(insert_error("insert the message channel"))?;

    // Two round trips, as the shell's runtime does: the first binds the
    // outputs and the seat, the second delivers their names and capabilities.
    queue.roundtrip(&mut rt).map_err(LockError::Dispatch)?;
    queue.roundtrip(&mut rt).map_err(LockError::Dispatch)?;

    WaylandSource::new(conn.clone(), queue)
        .insert(handle.clone())
        .map_err(insert_error("insert the Wayland source"))?;

    // An `Err` here means no lock is requested at all.
    setup(Sender::new(sender), &handle)?;

    rt.request_lock(&qh)?;
    // Send the request now: the first loop iteration would flush it, but the
    // lock must not wait for a poll round.
    conn.flush()
        .map_err(|err| LockError::ConnectionLost(calloop::Error::OtherError(Box::new(err))))?;

    event_loop
        .run(None, &mut rt, |_| {})
        .map_err(LockError::ConnectionLost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::backend::{protocol::ProtocolError, WaylandError};

    fn protocol() -> WaylandError {
        WaylandError::Protocol(ProtocolError {
            code: 1,
            object_id: 3,
            object_interface: "ext_session_lock_v1".into(),
            message: "invalid".into(),
        })
    }

    /// Runs a real `EventLoop` whose source fails the way
    /// `calloop-wayland-source` 0.4.1 does on a protocol error (`Error =
    /// calloop::Error`, `IoError(EPROTO)`), and returns what `run` yields:
    /// calloop wraps the source's error again in `OtherError` (finding I-1).
    fn error_the_loop_yields(failure: calloop::Error) -> calloop::Error {
        use calloop::generic::Generic;
        use calloop::{Interest, Mode, PostAction};
        use std::io::Write as _;
        use std::os::unix::net::UnixStream;

        let (mut writer, reader) = UnixStream::pair().expect("socketpair");
        writer.write_all(b"x").expect("write");
        let mut event_loop: EventLoop<'static, Option<calloop::Error>> =
            EventLoop::try_new().expect("event loop");
        let mut failure = Some(failure);
        event_loop
            .handle()
            .insert_source(
                Generic::new_with_error::<calloop::Error>(reader, Interest::READ, Mode::Level),
                move |_, _, _| match failure.take() {
                    Some(err) => Err(err),
                    None => Ok(PostAction::Remove),
                },
            )
            .expect("insert");
        let mut data = None;
        event_loop
            .run(None, &mut data, |_| {})
            .expect_err("the failing source ends the loop")
    }

    #[test]
    fn a_protocol_error_that_comes_out_of_a_real_loop_is_a_protocol_error() {
        let yielded = error_the_loop_yields(calloop::Error::IoError(
            std::io::Error::from_raw_os_error(EPROTO),
        ));
        // The shape the loop really produces: wrapped, not at the top level.
        assert!(matches!(yielded, calloop::Error::OtherError(_)));
        assert!(LockError::ConnectionLost(yielded).is_protocol_error());
    }

    #[test]
    fn a_broken_connection_that_comes_out_of_a_real_loop_is_not_a_protocol_error() {
        let yielded = error_the_loop_yields(calloop::Error::IoError(std::io::Error::from(
            std::io::ErrorKind::BrokenPipe,
        )));
        assert!(matches!(yielded, calloop::Error::OtherError(_)));
        assert!(!LockError::ConnectionLost(yielded).is_protocol_error());
    }

    #[test]
    fn protocol_errors_are_told_from_a_broken_connection() {
        let through_the_source = LockError::ConnectionLost(calloop::Error::IoError(
            std::io::Error::from_raw_os_error(EPROTO),
        ));
        assert!(through_the_source.is_protocol_error());
        let at_the_first_flush =
            LockError::ConnectionLost(calloop::Error::OtherError(Box::new(protocol())));
        assert!(at_the_first_flush.is_protocol_error());
        let broken = LockError::ConnectionLost(calloop::Error::IoError(std::io::Error::from(
            std::io::ErrorKind::BrokenPipe,
        )));
        assert!(!broken.is_protocol_error());
        let io_at_flush = LockError::ConnectionLost(calloop::Error::OtherError(Box::new(
            WaylandError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
        )));
        assert!(!io_at_flush.is_protocol_error());
        assert!(!LockError::NoSessionLock.is_protocol_error());
    }
}
