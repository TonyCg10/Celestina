//! The loop that owns one playback session on its own thread.
//!
//! Both hosts — Fluorita's player and Siderita's embedded modal — run a session
//! the same way: open it off the GUI thread, start it at once for sound or
//! once the surface has a render context for a picture, hand the surface the
//! backend's handle, forward what the person asks through the core's
//! [`PlaybackSession`] so a request the model refuses never reaches the
//! backend, and publish only what the model accepted from the backend's own
//! reports. That loop lives here, once; a host only says how to deliver what
//! it produces ([`SessionHost`]) and carries the commands in.
//!
//! The session is always closed before the loop returns, and the loop returns
//! only on `Stop` or when the host drops its sender — so joining the thread is
//! the whole of a deterministic shutdown. A picture that fails to start is not
//! an exception: by then the surface has built its render context from this
//! session's handle, so the failure is reported and the session stays open
//! until the host's close has gone through [`fluorita_core::SurfaceHandshake`]
//! and the surface has let go. Only a session that never handed out a handle —
//! one that fails to open, or sound that fails to start — closes on its own.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use fluorita_core::{
    MediaId, MediaKind, PlaybackRequest, PlaybackSession, PlaybackState, ReportOutcome, Stream,
    StreamKind,
};

use crate::backend::{FrameStats, MediaEngine, SessionRequest};

/// How long the loop waits for a backend report before looking at its inbox.
/// Short enough that a pause feels immediate, long enough not to spin.
pub const POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// How often frame statistics are taken while a host wants them.
pub const PACING_INTERVAL: Duration = Duration::from_secs(1);

/// What the host tells the session thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionCommand {
    /// The surface has a render context: load the media and begin.
    Start,
    /// A transport request, exactly as a click or the bus asked for it.
    Transport(PlaybackRequest),
    /// Close the session and leave.
    Stop,
}

/// One accepted report, reduced to what a host shows.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionSnapshot {
    pub state: PlaybackState,
    pub position: Option<Duration>,
    pub duration: Option<Duration>,
    /// The confirmed output level.
    pub volume: Option<f64>,
    /// True between a request and the report that confirms it.
    pub pending: bool,
    pub error: Option<String>,
    /// What the file holds, audio first and then subtitles, and what is
    /// playing out of it.
    pub streams: Vec<Stream>,
    pub audio: Option<i64>,
    pub subtitle: Option<i64>,
    /// The confirmed playback rate.
    pub speed: f64,
}

/// Where the loop delivers what it produces. Every method runs on the session
/// thread and should only hand its value over — queue it to the GUI thread —
/// never block.
pub trait SessionHost {
    /// The backend handle a surface renders from. Sent once, after opening.
    fn render_handle(&self, address: u64);
    /// A report the model accepted.
    fn snapshot(&self, snapshot: SessionSnapshot);
    /// The session could not open or start; `message` is what a person reads.
    fn failed(&self, message: String);
    /// Whether frame statistics are wanted right now. Asked every turn, so it
    /// should be an atomic load.
    fn wants_frame_stats(&self) -> bool {
        false
    }
    /// One reading of how the picture is arriving, at most every
    /// [`PACING_INTERVAL`] while [`SessionHost::wants_frame_stats`] says so.
    fn frame_stats(&self, _stats: FrameStats) {}
}

/// What to play and how.
pub struct SessionPlan<'a> {
    pub path: &'a std::path::Path,
    pub kind: MediaKind,
    /// Adjusts the request the loop builds — silence, looping, a start
    /// offset — before the session opens.
    pub shape: &'a dyn Fn(SessionRequest) -> SessionRequest,
}

/// Runs one session until the host stops it or goes away.
pub fn run_session(
    engine: &dyn MediaEngine,
    plan: &SessionPlan<'_>,
    commands: &Receiver<SessionCommand>,
    host: &dyn SessionHost,
) {
    let mut truth = PlaybackSession::new();
    let media = MediaId::from_path(plan.path);
    let Ok(generation) = truth.select(media, plan.kind) else {
        host.failed(crate::copy::SESSION_NOT_STARTED.to_owned());
        return;
    };

    // Only a moving picture needs a surface; audio would pay for a GPU and a
    // render context it never draws into.
    let presenting = plan.kind.capabilities().has_video && plan.kind != MediaKind::Image;
    let mut request = SessionRequest::new(plan.path.to_path_buf(), generation);
    if presenting {
        request = request.embedded_video();
    }
    let request = (plan.shape)(request);

    let mut session = match engine.open_session(request) {
        Ok(session) => session,
        Err(error) => {
            host.failed(error.user_message());
            return;
        }
    };

    // Audio has no surface to wait for, so it starts here. A picture waits
    // for the surface's render context, which arrives as `Start`: loaded any
    // earlier, the backend has no video output and ends at once.
    if !presenting {
        if let Err(error) = session.start() {
            host.failed(error.user_message());
            session.close();
            return;
        }
    }
    if let Some(handle) = session.render_handle() {
        host.render_handle(handle.value());
    }

    let mut last_sample = Instant::now();
    loop {
        match commands.try_recv() {
            Ok(SessionCommand::Start) => {
                // Reported, not ended: a surface is rendering from this
                // session's handle, and destroying the backend under it is
                // undefined behaviour in the render API. The host closes
                // through the handshake, which ends here with `Stop`.
                if let Err(error) = session.start() {
                    host.failed(error.user_message());
                }
            }
            Ok(SessionCommand::Transport(action)) => {
                if truth.request(action).is_ok() {
                    let _ = session.request(action);
                }
            }
            Ok(SessionCommand::Stop) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        if host.wants_frame_stats() && last_sample.elapsed() >= PACING_INTERVAL {
            host.frame_stats(session.frame_stats());
            last_sample = Instant::now();
        }

        if let Some(report) = session.poll(POLL_TIMEOUT) {
            if truth.apply(&report) == ReportOutcome::Applied {
                host.snapshot(snapshot_of(&truth));
            }
        }
    }

    session.close();
}

fn snapshot_of(truth: &PlaybackSession) -> SessionSnapshot {
    let streams = truth.streams();
    SessionSnapshot {
        state: truth.state(),
        position: truth.position(),
        duration: truth.duration(),
        volume: truth.volume(),
        pending: truth.pending_transport().is_some() || truth.is_seeking(),
        error: truth.error().map(str::to_owned),
        streams: streams
            .of(StreamKind::Audio)
            .chain(streams.of(StreamKind::Subtitle))
            .cloned()
            .collect(),
        audio: streams.selected(StreamKind::Audio),
        subtitle: streams.selected(StreamKind::Subtitle),
        speed: truth.speed().rate(),
    }
}

#[cfg(test)]
mod tests {
    use super::{run_session, SessionCommand, SessionHost, SessionPlan, SessionSnapshot};
    use crate::backend::{
        ArtworkJob, EngineSession, FrameStats, MediaEngine, ProbeBudget, ProbeReport, RenderHandle,
        SessionRequest, VideoOutput,
    };
    use crate::error::{EngineError, EngineResult};
    use celestina_core::{CancellationToken, Generation};
    use fluorita_core::{EngineReport, MediaKind, PlaybackRequest, PlaybackState, ReportKind};
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// What the fake backend was asked to do, in order.
    #[derive(Default)]
    struct Log(Mutex<Vec<String>>);

    impl Log {
        fn push(&self, entry: impl Into<String>) {
            self.0.lock().expect("log").push(entry.into());
        }

        fn entries(&self) -> Vec<String> {
            self.0.lock().expect("log").clone()
        }
    }

    struct FakeEngine {
        log: Arc<Log>,
        refuse: bool,
        /// Whether the session's `start` fails, as a load the backend refuses.
        fail_start: bool,
    }

    impl MediaEngine for FakeEngine {
        fn probe(
            &self,
            _path: &Path,
            _budget: ProbeBudget,
            _cancellation: &CancellationToken,
        ) -> EngineResult<ProbeReport> {
            Err(EngineError::Cancelled)
        }

        fn publish_artwork(&self, _request: &ArtworkJob) -> EngineResult<PathBuf> {
            Err(EngineError::Cancelled)
        }

        fn open_session(&self, request: SessionRequest) -> EngineResult<Box<dyn EngineSession>> {
            if self.refuse {
                return Err(EngineError::UnusableSource {
                    path: request.source,
                    reason: "the fake refuses",
                });
            }
            self.log.push(format!(
                "open embedded={}",
                request.video_output == VideoOutput::Embedded
            ));
            Ok(Box::new(FakeSession {
                log: Arc::clone(&self.log),
                generation: request.generation,
                reports: vec![ReportKind::State(PlaybackState::Playing)],
                embedded: request.video_output == VideoOutput::Embedded,
                fail_start: self.fail_start,
            }))
        }
    }

    struct FakeSession {
        log: Arc<Log>,
        generation: Generation,
        reports: Vec<ReportKind>,
        embedded: bool,
        fail_start: bool,
    }

    impl EngineSession for FakeSession {
        fn generation(&self) -> Generation {
            self.generation
        }

        fn request(&mut self, request: PlaybackRequest) -> EngineResult<()> {
            self.log.push(format!("request {request:?}"));
            Ok(())
        }

        fn poll(&mut self, timeout: Duration) -> Option<EngineReport> {
            if self.reports.is_empty() {
                std::thread::sleep(timeout.min(Duration::from_millis(2)));
                return None;
            }
            Some(EngineReport {
                generation: self.generation,
                kind: self.reports.remove(0),
            })
        }

        fn start(&mut self) -> EngineResult<()> {
            self.log.push("start");
            if self.fail_start {
                return Err(EngineError::Cancelled);
            }
            Ok(())
        }

        fn frame_stats(&self) -> FrameStats {
            FrameStats::default()
        }

        fn render_handle(&self) -> Option<RenderHandle> {
            self.embedded.then(|| RenderHandle::from_address(0x1000))
        }

        fn close(&mut self) {
            self.log.push("close");
        }
    }

    #[derive(Default)]
    struct Host {
        handles: Mutex<Vec<u64>>,
        snapshots: Mutex<Vec<SessionSnapshot>>,
        failures: Mutex<Vec<String>>,
    }

    impl SessionHost for Host {
        fn render_handle(&self, address: u64) {
            self.handles.lock().expect("host").push(address);
        }

        fn snapshot(&self, snapshot: SessionSnapshot) {
            self.snapshots.lock().expect("host").push(snapshot);
        }

        fn failed(&self, message: String) {
            self.failures.lock().expect("host").push(message);
        }
    }

    fn run(kind: MediaKind, refuse: bool, commands: &[SessionCommand]) -> (Vec<String>, Host) {
        let log = Arc::new(Log::default());
        let engine = FakeEngine {
            log: Arc::clone(&log),
            refuse,
            fail_start: false,
        };
        let host = Host::default();
        let (sender, receiver) = mpsc::channel();
        for command in commands {
            sender.send(*command).expect("queued");
        }
        drop(sender);
        let shape = |request: SessionRequest| request;
        run_session(
            &engine,
            &SessionPlan {
                path: Path::new("/m/clip.mkv"),
                kind,
                shape: &shape,
            },
            &receiver,
            &host,
        );
        (log.entries(), host)
    }

    #[test]
    fn a_film_waits_for_its_surface_before_loading() {
        let (log, host) = run(
            MediaKind::Video,
            false,
            &[SessionCommand::Start, SessionCommand::Stop],
        );

        assert_eq!(log, vec!["open embedded=true", "start", "close"]);
        assert_eq!(*host.handles.lock().expect("host"), vec![0x1000]);
    }

    #[test]
    fn sound_starts_at_once_and_never_asks_for_a_surface() {
        let (log, host) = run(MediaKind::Audio, false, &[SessionCommand::Stop]);

        assert_eq!(log, vec!["open embedded=false", "start", "close"]);
        assert!(host.handles.lock().expect("host").is_empty());
    }

    #[test]
    fn a_host_that_goes_away_still_closes_the_session() {
        // No `Stop`: the sender was simply dropped.
        let (log, _) = run(MediaKind::Audio, false, &[]);

        assert_eq!(log.last().map(String::as_str), Some("close"));
    }

    #[test]
    fn a_request_the_model_refuses_never_reaches_the_backend() {
        // A picture has no transport; the model says so and the backend is
        // not asked.
        let (log, _) = run(
            MediaKind::Image,
            false,
            &[
                SessionCommand::Transport(PlaybackRequest::Play),
                SessionCommand::Stop,
            ],
        );

        assert!(
            !log.iter().any(|entry| entry.starts_with("request")),
            "{log:?}"
        );
    }

    #[test]
    fn a_request_the_model_accepts_is_forwarded() {
        let (log, _) = run(
            MediaKind::Audio,
            false,
            &[
                SessionCommand::Transport(PlaybackRequest::Pause),
                SessionCommand::Stop,
            ],
        );

        assert!(log.iter().any(|entry| entry == "request Pause"), "{log:?}");
    }

    #[test]
    fn a_session_that_cannot_open_says_so_and_holds_nothing() {
        let (log, host) = run(MediaKind::Video, true, &[SessionCommand::Stop]);

        assert!(log.is_empty());
        assert_eq!(host.failures.lock().expect("host").len(), 1);
        assert!(host.handles.lock().expect("host").is_empty());
    }

    #[test]
    fn only_what_the_model_accepted_is_published() {
        let log = Arc::new(Log::default());
        let engine = FakeEngine {
            log: Arc::clone(&log),
            refuse: false,
            fail_start: false,
        };
        let host = Host::default();
        let (sender, receiver) = mpsc::channel();
        let shape = |request: SessionRequest| request;
        let (engine, host_ref, shape) = (&engine, &host, &shape);
        std::thread::scope(|scope| {
            scope.spawn(move || {
                run_session(
                    engine,
                    &SessionPlan {
                        path: Path::new("/m/pista.flac"),
                        kind: MediaKind::Audio,
                        shape,
                    },
                    &receiver,
                    host_ref,
                );
            });
            // Long enough for the fake's one report to be polled.
            std::thread::sleep(Duration::from_millis(100));
            sender.send(SessionCommand::Stop).expect("stop");
        });

        let snapshots = host.snapshots.lock().expect("host");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].state, PlaybackState::Playing);
    }

    #[test]
    fn a_film_that_fails_to_start_stays_open_until_its_host_closes_it() {
        // The surface built its render context from this session's handle
        // before `Start` was sent. Closing here would destroy the backend
        // under a context the render thread still draws from; only the
        // host's close, through the handshake, may end it.
        let log = Arc::new(Log::default());
        let engine = FakeEngine {
            log: Arc::clone(&log),
            refuse: false,
            fail_start: true,
        };
        let host = Host::default();
        let (sender, receiver) = mpsc::channel();
        let shape = |request: SessionRequest| request;
        let (engine, host_ref, shape) = (&engine, &host, &shape);
        let reported = || !host.failures.lock().expect("host").is_empty();
        let before_stop = std::thread::scope(|scope| {
            scope.spawn(move || {
                run_session(
                    engine,
                    &SessionPlan {
                        path: Path::new("/m/clip.mkv"),
                        kind: MediaKind::Video,
                        shape,
                    },
                    &receiver,
                    host_ref,
                );
            });
            sender.send(SessionCommand::Start).expect("start");
            // Waits for the failure to be reported, bounded, then gives the
            // loop a few more turns in which a premature close would show.
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !reported() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(super::POLL_TIMEOUT * 2);
            // Recorded, not asserted, inside the scope: a failed assertion
            // here would leave the session thread waiting for a `Stop` that
            // never comes, and the test would hang instead of failing.
            let seen = (log.entries(), host.failures.lock().expect("host").len());
            // A loop that already left has dropped its receiver; that is what
            // the assertions below report, not this send.
            let _ = sender.send(SessionCommand::Stop);
            seen
        });

        assert_eq!(before_stop.0, vec!["open embedded=true", "start"]);
        assert_eq!(before_stop.1, 1);
        assert_eq!(log.entries().last().map(String::as_str), Some("close"));
    }
}
