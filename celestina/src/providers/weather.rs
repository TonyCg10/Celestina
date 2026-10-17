//! The weather worker (CEL-2-K): the one place in the shell that touches the
//! network, and only under the author's recorded decision (glance design §3).
//!
//! `celestina_shell::weather` owns every rule: what a request may carry, what
//! an answer must look like to be believed, how long a reading counts and how
//! long to leave the service alone after a failure. This module only performs
//! the request, through `curl` (the tool the session already has, rather than
//! a TLS stack linked into the shell for one small GET), and decides when.
//!
//! The worker starts with no location and asks nothing until the loop hands it
//! one through [`WeatherProvider::set_location`]. With no location there is no
//! request at all, ever: removing the location withdraws the reading and stops
//! the asking. The shell never works out where the person is.
//!
//! The halted shell's `celestina-qt/src/provider_adapter/weather.rs` was read
//! as reference for the decision loop; nothing is copied from it.

use std::ffi::OsString;
use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina_shell::settings::Location;
use celestina_shell::weather::{
    next_step, read, request_url, still_worth_showing, Cached, Next, MAX_RESPONSE_BYTES,
};
use celestina_wayland::Sender;

use super::Worker;
use crate::message::Message;

/// How often the worker looks at its queue and the shutdown flag.
const POLL: Duration = Duration::from_millis(100);
/// How often the decision is revisited, in milliseconds of the worker's
/// clock. The decision is almost always "keep"; this is only how promptly a
/// stale reading is noticed and withdrawn.
const TICK_MS: u64 = 30 * 1000;
/// The least time between two request attempts, whatever caused them: a
/// person editing the location in `shell.json` key by key must not turn every
/// save into a request.
pub const MIN_ASK_GAP_MS: u64 = 60 * 1000;
/// Location changes that may wait for the worker. Only the last one matters.
const COMMAND_QUEUE: usize = 4;
/// The program run for a request, resolved through `PATH`.
const CURL: &str = "curl";
/// curl's own limit on the whole transfer, in seconds.
const CURL_MAX_TIME_SECONDS: &str = "15";
/// The hard limit after which the child is killed whatever it is doing: a
/// curl that ignores its own `--max-time` never becomes a hanging worker.
const HARD_LIMIT: Duration = Duration::from_secs(20);

/// One reading as the surfaces show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeatherReading {
    /// Whole degrees Celsius.
    pub celsius: i16,
    /// The service's weather code; the surface maps it to words.
    pub code: u8,
    /// Whether the service says it is daytime at the location.
    pub daylight: bool,
    /// The location's label from `shell.json`. Never sent anywhere.
    pub label: String,
    /// When the reading arrived, in seconds since the Unix epoch.
    pub taken_unix: u64,
}

/// Performs one GET. `None` is any failure: the caller does not tell a
/// refusal from a timeout or an oversized body, all of them mean "no new
/// reading, and a pause".
pub trait Fetch: Send {
    fn get(&mut self, url: &str) -> Option<Vec<u8>>;
}

/// The arguments `curl` runs with for `url`, in order.
///
/// - `--disable` comes first, as curl requires: no `.curlrc` is read, so no
///   header, cookie, user agent or proxy the person configured for other uses
///   rides along with this request.
/// - `--fail` makes an HTTP error a failure rather than an error page read as
///   weather; `--silent --show-error` keeps progress off any stream (stderr is
///   discarded anyway).
/// - `--proto =https` refuses any other scheme, and `--max-filesize` lets curl
///   stop early when the size is announced; the read is bounded regardless.
/// - `--` ends the options, so the URL is never read as one; it is last.
#[must_use]
pub fn curl_args(url: &str) -> Vec<String> {
    let max_bytes = MAX_RESPONSE_BYTES.to_string();
    [
        "--disable",
        "--fail",
        "--silent",
        "--show-error",
        "--max-time",
        CURL_MAX_TIME_SECONDS,
        "--proto",
        "=https",
        "--max-filesize",
        max_bytes.as_str(),
        "--",
        url,
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect()
}

/// The real fetcher: one `curl` child per request, never through a shell.
///
/// stdin and stderr are null and stdout is read on a helper thread, bounded to
/// [`MAX_RESPONSE_BYTES`] plus one byte (so an oversized body is recognised
/// and refused). The child is killed after [`HARD_LIMIT`], on an oversized
/// body, or when the shell shuts down, and it is always reaped.
#[derive(Debug)]
pub struct CurlFetch {
    program: OsString,
    hard_limit: Duration,
    shutdown: Arc<AtomicBool>,
}

impl CurlFetch {
    /// A fetcher running `curl` from `PATH` that gives up its request when
    /// `shutdown` is raised.
    #[must_use]
    pub fn new(shutdown: &Arc<AtomicBool>) -> Self {
        Self::with_program(CURL, shutdown)
    }

    /// [`CurlFetch::new`] running `program` instead of `curl`, with the same
    /// arguments; the tests' stand-in for curl.
    #[must_use]
    pub fn with_program(program: impl Into<OsString>, shutdown: &Arc<AtomicBool>) -> Self {
        Self {
            program: program.into(),
            hard_limit: HARD_LIMIT,
            shutdown: Arc::clone(shutdown),
        }
    }

    /// The same fetcher with another hard limit, so a test need not wait
    /// [`HARD_LIMIT`].
    #[cfg(test)]
    #[must_use]
    fn with_hard_limit(mut self, limit: Duration) -> Self {
        self.hard_limit = limit;
        self
    }

    fn stopping(&self, deadline: Instant) -> bool {
        self.shutdown.load(Ordering::Acquire) || Instant::now() >= deadline
    }

    /// Waits for the child to exit by itself until `deadline` or shutdown,
    /// then kills it; it is reaped in every case.
    fn reap(&self, child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if !self.stopping(deadline) => thread::sleep(Duration::from_millis(10)),
                Ok(None) | Err(_) => {
                    kill_and_reap(child);
                    return None;
                }
            }
        }
    }
}

/// Kills `child` (an error means it already exited) and reaps it.
fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    if let Err(err) = child.wait() {
        tracing::warn!(%err, "weather: the curl child could not be reaped");
    }
}

impl Fetch for CurlFetch {
    fn get(&mut self, url: &str) -> Option<Vec<u8>> {
        // The URL comes from `request_url`; anything else is not this request.
        if !url.starts_with("https://") {
            return None;
        }
        let deadline = Instant::now() + self.hard_limit;
        let mut child = match Command::new(&self.program)
            .args(curl_args(url))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                tracing::warn!(%err, "weather: curl could not be started");
                return None;
            }
        };
        let Some(stdout) = child.stdout.take() else {
            kill_and_reap(&mut child);
            return None;
        };

        let (done, finished) = mpsc::sync_channel::<io::Result<Vec<u8>>>(1);
        let reader = thread::Builder::new()
            .name("weather-read".to_owned())
            .spawn(move || {
                let mut body = Vec::new();
                let limit = u64::try_from(MAX_RESPONSE_BYTES).map_or(u64::MAX, |max| max + 1);
                let result = stdout.take(limit).read_to_end(&mut body).map(|_| body);
                let _ = done.send(result);
            });
        let reader = match reader {
            Ok(reader) => reader,
            Err(err) => {
                tracing::warn!(%err, "weather: no thread to read curl's output");
                kill_and_reap(&mut child);
                return None;
            }
        };

        let body = loop {
            match finished.recv_timeout(POLL) {
                Ok(result) => break result.ok(),
                Err(RecvTimeoutError::Disconnected) => break None,
                Err(RecvTimeoutError::Timeout) if self.stopping(deadline) => {
                    kill_and_reap(&mut child);
                    // The killed child closed the pipe, so the reader ends; it
                    // is joined only once it has said so, keeping this bounded.
                    if finished.recv_timeout(Duration::from_secs(1)).is_ok() {
                        let _ = reader.join();
                    }
                    return None;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        };
        let _ = reader.join();

        let Some(body) = body.filter(|body| body.len() <= MAX_RESPONSE_BYTES) else {
            // Oversized or unreadable: whatever curl is still writing is not
            // the reply this shell asked for.
            kill_and_reap(&mut child);
            return None;
        };
        let status = self.reap(&mut child, deadline)?;
        status.success().then_some(body)
    }
}

enum WeatherCommand {
    SetLocation(Option<Location>),
}

/// Why the worker did not take a location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// [`COMMAND_QUEUE`] changes already wait; the worker is alive and the
    /// change may be sent again.
    Busy,
    /// The worker has stopped: nothing will ever read the queue again.
    Gone,
}

/// The loop's handle on the worker: the bounded command queue.
#[derive(Debug, Clone)]
pub struct WeatherProvider {
    commands: SyncSender<WeatherCommand>,
}

impl std::fmt::Debug for WeatherCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // The coordinates stay out of logs.
            Self::SetLocation(place) => formatter
                .debug_tuple("SetLocation")
                .field(&place.is_some())
                .finish(),
        }
    }
}

impl WeatherProvider {
    /// Hands the worker the location from `shell.json`, or none. With none
    /// the worker asks nothing and withdraws what it showed.
    ///
    /// # Errors
    ///
    /// [`Refused::Busy`] when [`COMMAND_QUEUE`] changes already wait,
    /// [`Refused::Gone`] when the worker has stopped.
    pub fn set_location(&self, place: Option<Location>) -> Result<(), Refused> {
        match self.commands.try_send(WeatherCommand::SetLocation(place)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(Refused::Busy),
            Err(TrySendError::Disconnected(_)) => Err(Refused::Gone),
        }
    }
}

/// One look at the worker's two clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Now {
    /// Milliseconds on a monotonic clock: every freshness, pause and pacing
    /// decision is taken on it, so a wall clock set back or forward (a
    /// suspend, NTP, the person) neither keeps an old reading nor hammers
    /// the service.
    pub mono_ms: u64,
    /// Seconds since the Unix epoch by the wall clock, only to say when a
    /// reading arrived.
    pub unix_s: u64,
}

/// Milliseconds elapsed since `start`, saturating.
fn monotonic_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Seconds since the Unix epoch by the wall clock (0 before it).
fn wall_clock_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Starts the worker with the real fetcher and clock, publishing into
/// `sender`.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(WeatherProvider, Worker)> {
    let start = Instant::now();
    spawn_with(
        CurlFetch::new(shutdown),
        shutdown,
        move |reading| sender.send(Message::Weather(reading)).is_ok(),
        move || Now {
            mono_ms: monotonic_ms(start),
            unix_s: wall_clock_s(),
        },
    )
}

/// [`spawn`] with its fetcher, sink and clock given; `publish` returns
/// whether the reading was accepted (false ends the worker).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_with(
    fetch: impl Fetch + 'static,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(Option<WeatherReading>) -> bool + Send + 'static,
    clock: impl FnMut() -> Now + Send + 'static,
) -> io::Result<(WeatherProvider, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("weather", shutdown, move || {
        run(fetch, &queue, &flag, publish, clock);
    })?;
    Ok((WeatherProvider { commands }, worker))
}

fn run(
    mut fetch: impl Fetch,
    queue: &Receiver<WeatherCommand>,
    shutdown: &AtomicBool,
    mut publish: impl FnMut(Option<WeatherReading>) -> bool,
    mut clock: impl FnMut() -> Now,
) {
    let mut engine = Engine::default();
    let mut last_step: Option<u64> = None;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let mut changed = false;
        match queue.recv_timeout(POLL) {
            Ok(first) => {
                // Only the last of a burst of changes counts.
                for WeatherCommand::SetLocation(place) in
                    std::iter::once(first).chain(queue.try_iter().take(COMMAND_QUEUE))
                {
                    changed |= engine.set_location(place);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = clock();
        let due = last_step.is_none_or(|last| now.mono_ms < last || now.mono_ms - last >= TICK_MS)
            || engine.paced_until.is_some_and(|until| now.mono_ms >= until);
        if !(changed || due) {
            continue;
        }
        last_step = Some(now.mono_ms);
        // A reading of the old place goes before the request for the new
        // one, which may block for the whole hard limit.
        if let Some(withdrawn) = engine.withdrawal() {
            if !publish(withdrawn) {
                tracing::debug!("weather: the loop is gone; the worker stops");
                return;
            }
        }
        if let Some(reading) = engine.step(now, &mut fetch) {
            if !publish(reading) {
                tracing::debug!("weather: the loop is gone; the worker stops");
                return;
            }
        }
    }
}

/// The decision loop's state, apart from threads and time.
#[derive(Debug, Default)]
struct Engine {
    location: Option<Location>,
    held: Option<Held>,
    last_failure_ms: Option<u64>,
    /// When the last request was attempted, whatever its outcome.
    last_attempt_ms: Option<u64>,
    /// A request is due but waits for [`MIN_ASK_GAP_MS`]; when it may go.
    paced_until: Option<u64>,
    /// What was last published, so only a change is published again.
    shown: Option<WeatherReading>,
}

/// A reading held, with the wall-clock second it arrived (for display only).
#[derive(Clone, Copy, Debug)]
struct Held {
    cached: Cached,
    unix_s: u64,
}

fn same_place(a: &Location, b: &Location) -> bool {
    a.latitude().to_bits() == b.latitude().to_bits()
        && a.longitude().to_bits() == b.longitude().to_bits()
}

impl Engine {
    /// Takes a new location; returns whether anything changed. Other
    /// coordinates drop the held reading and the failure pause, so the new
    /// place is asked for as soon as [`MIN_ASK_GAP_MS`] allows; a new label
    /// alone keeps the reading.
    fn set_location(&mut self, place: Option<Location>) -> bool {
        if self.location == place {
            return false;
        }
        let moved = match (&self.location, &place) {
            (Some(old), Some(new)) => !same_place(old, new),
            _ => true,
        };
        if moved {
            self.held = None;
            self.last_failure_ms = None;
        }
        self.location = place;
        true
    }

    /// The withdrawal owed before anything blocks: a reading is shown that
    /// is no longer held (the place moved or went), so it must not stay on
    /// screen while the new place is asked for.
    fn withdrawal(&mut self) -> Option<Option<WeatherReading>> {
        if self.held.is_none() && self.shown.is_some() {
            return self.show(None);
        }
        None
    }

    /// One decision at `now`; returns the new publication when it differs
    /// from the last one.
    fn step(&mut self, now: Now, fetch: &mut impl Fetch) -> Option<Option<WeatherReading>> {
        let now_ms = now.mono_ms;
        // The real clock is monotonic, but it is injected: one that went
        // back must not make an old reading look current, nor stretch a
        // pause into one that never ends.
        if self.held.is_some_and(|held| held.cached.taken_ms > now_ms) {
            self.held = None;
        }
        for mark in [&mut self.last_failure_ms, &mut self.last_attempt_ms] {
            if mark.is_some_and(|at| at > now_ms) {
                *mark = Some(now_ms);
            }
        }

        let Some(place) = &self.location else {
            // No location: no request, and nothing shown.
            self.held = None;
            self.last_failure_ms = None;
            self.paced_until = None;
            return self.show(None);
        };

        self.paced_until = None;
        let cached = self.held.map(|held| held.cached);
        if next_step(cached, self.last_failure_ms, now_ms) == Next::Ask {
            match self.last_attempt_ms {
                Some(last) if now_ms.saturating_sub(last) < MIN_ASK_GAP_MS => {
                    self.paced_until = Some(last.saturating_add(MIN_ASK_GAP_MS));
                }
                _ => {
                    self.last_attempt_ms = Some(now_ms);
                    match fetch.get(&request_url(place)).as_deref().and_then(read) {
                        Some(reading) => {
                            self.held = Some(Held {
                                cached: Cached {
                                    reading,
                                    taken_ms: now_ms,
                                },
                                unix_s: now.unix_s,
                            });
                            self.last_failure_ms = None;
                        }
                        // A refusal and an unreadable answer are the same
                        // thing: no new reading, and a pause before asking
                        // again.
                        None => self.last_failure_ms = Some(now_ms),
                    }
                }
            }
        }

        // A reading that stopped being current is absent, not stale.
        self.held = self
            .held
            .filter(|held| still_worth_showing(held.cached, now_ms));
        let visible = self.held.map(|held| WeatherReading {
            celsius: held.cached.reading.celsius,
            code: held.cached.reading.code,
            daylight: held.cached.reading.daylight,
            label: place.label.clone(),
            taken_unix: held.unix_s,
        });
        self.show(visible)
    }

    fn show(&mut self, visible: Option<WeatherReading>) -> Option<Option<WeatherReading>> {
        if self.shown == visible {
            return None;
        }
        self.shown.clone_from(&visible);
        Some(visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::weather::{FRESH_MS, RETRY_MS};
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU64, AtomicUsize};

    const T0: u64 = 1_800_000_000_000;

    /// Both clocks at `ms`: the tests' monotonic clock happens to read like
    /// the wall clock, so a reading's `taken_unix` is `ms / 1000`.
    fn at(ms: u64) -> Now {
        Now {
            mono_ms: ms,
            unix_s: ms / 1000,
        }
    }

    fn dublin() -> Location {
        Location::new(53.349_805, -6.260_31, "Dublin").expect("a place")
    }

    fn body(celsius: &str, code: u8, is_day: u8) -> Vec<u8> {
        format!(
            r#"{{"current":{{"temperature_2m":{celsius},"weather_code":{code},"is_day":{is_day}}}}}"#
        )
        .into_bytes()
    }

    /// Answers from a script, recording every URL asked for.
    #[derive(Default)]
    struct Scripted {
        answers: VecDeque<Option<Vec<u8>>>,
        asked: Vec<String>,
    }

    impl Scripted {
        fn answering(answers: impl IntoIterator<Item = Option<Vec<u8>>>) -> Self {
            Self {
                answers: answers.into_iter().collect(),
                asked: Vec::new(),
            }
        }
    }

    impl Fetch for Scripted {
        fn get(&mut self, url: &str) -> Option<Vec<u8>> {
            self.asked.push(url.to_owned());
            self.answers.pop_front().flatten()
        }
    }

    /// Counts its calls and then panics: reaching it is the failure.
    struct Forbidden(Arc<AtomicUsize>);

    impl Fetch for Forbidden {
        fn get(&mut self, url: &str) -> Option<Vec<u8>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("the network was reached without a location: {url}");
        }
    }

    fn reading(celsius: i16, code: u8, daylight: bool, taken_ms: u64) -> WeatherReading {
        WeatherReading {
            celsius,
            code,
            daylight,
            label: "Dublin".to_owned(),
            taken_unix: taken_ms / 1000,
        }
    }

    #[test]
    fn without_a_location_nothing_is_ever_asked_and_nothing_published() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut fetch = Forbidden(Arc::clone(&calls));
        let mut engine = Engine::default();
        for tick in 0..10_000 {
            assert_eq!(engine.step(at(T0 + tick * TICK_MS), &mut fetch), None);
        }
        assert!(!engine.set_location(None));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_network_guard_holds_for_the_running_worker() {
        let calls = Arc::new(AtomicUsize::new(0));
        let published = Arc::new(AtomicUsize::new(0));
        let now = Arc::new(AtomicU64::new(T0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker) = spawn_with(
            Forbidden(Arc::clone(&calls)),
            &shutdown,
            {
                let published = Arc::clone(&published);
                move |_| {
                    published.fetch_add(1, Ordering::SeqCst);
                    true
                }
            },
            {
                let now = Arc::clone(&now);
                // Every look at the clock is a whole tick (and more) later.
                move || at(now.fetch_add(TICK_MS * 7, Ordering::SeqCst))
            },
        )
        .expect("the worker starts");
        // Several polls, each one a decision on the advancing clock.
        let start = Instant::now();
        while now.load(Ordering::SeqCst) < T0 + TICK_MS * 7 * 5 {
            assert!(start.elapsed() < Duration::from_secs(5), "the worker runs");
            thread::sleep(Duration::from_millis(10));
        }
        worker.join();
        assert_eq!(calls.load(Ordering::SeqCst), 0, "no request");
        assert_eq!(published.load(Ordering::SeqCst), 0, "no publication");
    }

    #[test]
    fn a_location_asks_once_and_publishes_the_rounded_reading_and_label() {
        let mut fetch = Scripted::answering([Some(body("12.6", 3, 1))]);
        let mut engine = Engine::default();
        assert!(engine.set_location(Some(dublin())));

        assert_eq!(
            engine.step(at(T0), &mut fetch),
            Some(Some(reading(13, 3, true, T0)))
        );
        assert_eq!(fetch.asked, vec![request_url(&dublin())]);
    }

    #[test]
    fn a_fresh_reading_is_kept_and_nothing_is_asked() {
        let mut fetch = Scripted::answering([Some(body("-0.4", 0, 0))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert_eq!(
            engine.step(at(T0), &mut fetch),
            Some(Some(reading(0, 0, false, T0)))
        );
        let mut now = T0;
        while now + TICK_MS < T0 + FRESH_MS {
            now += TICK_MS;
            assert_eq!(engine.step(at(now), &mut fetch), None, "unchanged at {now}");
        }
        assert_eq!(fetch.asked.len(), 1);
    }

    #[test]
    fn a_failure_waits_the_retry_pause_before_asking_again() {
        let mut fetch = Scripted::answering([None, Some(body("8", 61, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert_eq!(engine.step(at(T0), &mut fetch), None, "nothing to show yet");

        let mut now = T0;
        while now + TICK_MS < T0 + RETRY_MS {
            now += TICK_MS;
            assert_eq!(engine.step(at(now), &mut fetch), None);
        }
        assert_eq!(fetch.asked.len(), 1, "no second request inside the pause");

        let after = T0 + RETRY_MS;
        assert_eq!(
            engine.step(at(after), &mut fetch),
            Some(Some(reading(8, 61, true, after)))
        );
        assert_eq!(fetch.asked.len(), 2);
    }

    #[test]
    fn a_reading_that_stops_being_fresh_is_withdrawn_when_the_retry_fails() {
        let mut fetch = Scripted::answering([Some(body("20", 1, 1)), None]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert!(matches!(engine.step(at(T0), &mut fetch), Some(Some(_))));

        assert_eq!(engine.step(at(T0 + FRESH_MS), &mut fetch), Some(None));
        assert_eq!(fetch.asked.len(), 2, "it asked once it went stale");
        // Withdrawn once, not again on every tick.
        assert_eq!(engine.step(at(T0 + FRESH_MS + TICK_MS), &mut fetch), None);
    }

    #[test]
    fn removing_the_location_withdraws_and_stops_asking() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut fetch = Scripted::answering([Some(body("20", 1, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert!(matches!(engine.step(at(T0), &mut fetch), Some(Some(_))));

        assert!(engine.set_location(None));
        let mut forbidden = Forbidden(Arc::clone(&calls));
        assert_eq!(engine.step(at(T0 + 1), &mut forbidden), Some(None));
        for tick in 1..100 {
            assert_eq!(engine.step(at(T0 + tick * FRESH_MS), &mut forbidden), None);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_changed_location_withdraws_at_once_and_asks_after_the_gap() {
        let mut fetch = Scripted::answering([Some(body("20", 1, 1)), Some(body("30", 2, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert_eq!(engine.withdrawal(), None, "nothing shown yet");
        assert!(matches!(engine.step(at(T0), &mut fetch), Some(Some(_))));

        let lisbon = Location::new(38.72, -9.14, "Lisboa").expect("a place");
        assert!(engine.set_location(Some(lisbon.clone())));
        // The Dublin reading goes before anything else happens.
        assert_eq!(engine.withdrawal(), Some(None));
        assert_eq!(engine.withdrawal(), None, "withdrawn once");
        // Inside the gap since the last attempt nothing is asked.
        assert_eq!(engine.step(at(T0 + 1_000), &mut fetch), None);
        assert_eq!(engine.paced_until, Some(T0 + MIN_ASK_GAP_MS));
        assert_eq!(engine.step(at(T0 + MIN_ASK_GAP_MS - 1), &mut fetch), None);
        assert_eq!(fetch.asked.len(), 1);

        let mut expected = reading(30, 2, true, T0 + MIN_ASK_GAP_MS);
        expected.label = "Lisboa".to_owned();
        assert_eq!(
            engine.step(at(T0 + MIN_ASK_GAP_MS), &mut fetch),
            Some(Some(expected))
        );
        assert_eq!(engine.paced_until, None);
        assert_eq!(
            fetch.asked,
            vec![request_url(&dublin()), request_url(&lisbon)]
        );
    }

    #[test]
    fn a_burst_of_location_edits_asks_at_most_once_a_gap() {
        let answers = (0..100).map(|_| Some(body("5", 0, 1)));
        let mut fetch = Scripted::answering(answers);
        let mut engine = Engine::default();
        let mut now = T0;
        for step in 0..120_u32 {
            let place = Location::new(40.0 + f64::from(step % 7), -3.0, "Sitio").expect("a place");
            engine.set_location(Some(place));
            let _ = engine.withdrawal();
            let _ = engine.step(at(now), &mut fetch);
            now += 1_000;
        }
        // Two minutes of edits, one a second: the first ask and one a gap.
        assert_eq!(fetch.asked.len(), 2, "{:?}", fetch.asked.len());
    }

    #[test]
    fn a_failure_after_a_move_does_not_shorten_the_gap() {
        let mut fetch = Scripted::answering([None, Some(body("1", 0, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert_eq!(
            engine.step(at(T0), &mut fetch),
            None,
            "the first ask failed"
        );
        // The move drops the failure pause, not the gap.
        let lisbon = Location::new(38.72, -9.14, "Lisboa").expect("a place");
        engine.set_location(Some(lisbon));
        assert_eq!(engine.step(at(T0 + 1), &mut fetch), None);
        assert_eq!(fetch.asked.len(), 1);
        assert!(matches!(
            engine.step(at(T0 + MIN_ASK_GAP_MS), &mut fetch),
            Some(Some(_))
        ));
    }

    #[test]
    fn the_reading_says_the_wall_clock_while_deciding_on_the_monotonic_one() {
        let mut fetch = Scripted::answering([Some(body("5", 0, 1)), Some(body("6", 0, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        let first = Now {
            mono_ms: 10,
            unix_s: 1_800_000_000,
        };
        let shown = engine.step(first, &mut fetch).flatten().expect("a reading");
        assert_eq!(shown.taken_unix, 1_800_000_000);
        // The wall clock jumps a day ahead (and then back a year): only the
        // monotonic clock counts, so the reading stays and nothing is asked.
        for unix_s in [1_800_086_400, 1_768_464_000] {
            let later = Now {
                mono_ms: 10 + TICK_MS,
                unix_s,
            };
            assert_eq!(engine.step(later, &mut fetch), None);
        }
        assert_eq!(fetch.asked.len(), 1);
        // And it goes when the monotonic clock says so.
        let stale = Now {
            mono_ms: 10 + FRESH_MS,
            unix_s: 0,
        };
        let renewed = engine
            .step(stale, &mut fetch)
            .flatten()
            .expect("asked again");
        assert_eq!((renewed.celsius, renewed.taken_unix), (6, 0));
    }

    #[test]
    fn the_monotonic_clock_starts_near_zero_and_never_goes_back() {
        let start = Instant::now();
        let first = monotonic_ms(start);
        let second = monotonic_ms(start);
        assert!(first < 1_000 && second >= first);
    }

    #[test]
    fn a_new_label_alone_republishes_without_asking() {
        let mut fetch = Scripted::answering([Some(body("5", 0, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert!(matches!(engine.step(at(T0), &mut fetch), Some(Some(_))));

        let renamed = Location::new(53.349_805, -6.260_31, "Casa").expect("a place");
        assert!(engine.set_location(Some(renamed)));
        let mut expected = reading(5, 0, true, T0);
        expected.label = "Casa".to_owned();
        assert_eq!(engine.step(at(T0 + 1), &mut fetch), Some(Some(expected)));
        assert_eq!(fetch.asked.len(), 1);
    }

    #[test]
    fn a_hostile_reply_is_no_reading() {
        let hostile = [
            Some(b"<html>captive portal</html>".to_vec()),
            Some(vec![b' '; MAX_RESPONSE_BYTES + 1]),
            Some(body("1e9", 3, 1)),
            Some(body("NaN", 3, 1)),
            Some(br#"{"current":{"weather_code":3}}"#.to_vec()),
        ];
        for answer in hostile {
            let mut fetch = Scripted::answering([answer]);
            let mut engine = Engine::default();
            engine.set_location(Some(dublin()));
            assert_eq!(engine.step(at(T0), &mut fetch), None);
            assert_eq!(engine.last_failure_ms, Some(T0), "treated as a failure");
        }
    }

    #[test]
    fn a_clock_stepped_back_does_not_keep_an_old_reading_or_hurry_a_request() {
        let mut fetch = Scripted::answering([Some(body("5", 0, 1)), Some(body("6", 0, 1))]);
        let mut engine = Engine::default();
        engine.set_location(Some(dublin()));
        assert!(matches!(engine.step(at(T0), &mut fetch), Some(Some(_))));
        let back = T0 - 3_600_000;
        assert_eq!(engine.step(at(back), &mut fetch), Some(None));
        assert_eq!(
            fetch.asked.len(),
            1,
            "the gap counts from the clock's new now"
        );
        assert_eq!(
            engine.step(at(back + MIN_ASK_GAP_MS), &mut fetch),
            Some(Some(reading(6, 0, true, back + MIN_ASK_GAP_MS)))
        );
    }

    #[test]
    fn the_running_worker_publishes_after_a_location_and_withdraws_on_removal() {
        let (published, received) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let fetch = Scripted::answering([Some(body("12.4", 3, 0))]);
        let (provider, worker) = spawn_with(
            fetch,
            &shutdown,
            move |reading| published.send(reading).is_ok(),
            || at(T0),
        )
        .expect("the worker starts");

        provider.set_location(Some(dublin())).expect("queued");
        let wait = Duration::from_secs(5);
        assert_eq!(
            received.recv_timeout(wait).expect("a reading"),
            Some(reading(12, 3, false, T0))
        );
        provider.set_location(None).expect("queued");
        assert_eq!(received.recv_timeout(wait).expect("a withdrawal"), None);
        worker.join();
    }

    #[test]
    fn a_full_queue_is_busy_and_a_dropped_one_is_gone() {
        let (commands, queue) = mpsc::sync_channel(1);
        let provider = WeatherProvider { commands };
        assert_eq!(provider.set_location(None), Ok(()));
        assert_eq!(provider.set_location(Some(dublin())), Err(Refused::Busy));
        drop(queue);
        assert_eq!(provider.set_location(Some(dublin())), Err(Refused::Gone));
    }

    #[test]
    fn the_running_worker_withdraws_the_old_place_before_asking_for_the_new() {
        // The second request blocks until the test lets it answer.
        struct Gated {
            calls: usize,
            gate: Receiver<()>,
        }
        impl Fetch for Gated {
            fn get(&mut self, _url: &str) -> Option<Vec<u8>> {
                self.calls += 1;
                if self.calls > 1 {
                    self.gate.recv_timeout(Duration::from_secs(5)).ok()?;
                }
                Some(body("20", 1, 1))
            }
        }
        let (open, gate) = mpsc::channel();
        let (published, received) = mpsc::channel();
        let now = Arc::new(AtomicU64::new(T0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker) = spawn_with(
            Gated { calls: 0, gate },
            &shutdown,
            move |reading| published.send(reading).is_ok(),
            {
                let now = Arc::clone(&now);
                move || at(now.load(Ordering::SeqCst))
            },
        )
        .expect("the worker starts");
        let wait = Duration::from_secs(5);

        provider.set_location(Some(dublin())).expect("queued");
        assert!(matches!(received.recv_timeout(wait), Ok(Some(_))));
        now.store(T0 + MIN_ASK_GAP_MS, Ordering::SeqCst);
        let lisbon = Location::new(38.72, -9.14, "Lisboa").expect("a place");
        provider.set_location(Some(lisbon)).expect("queued");
        // Withdrawn while the request for Lisboa is still waiting.
        assert_eq!(received.recv_timeout(wait).expect("a withdrawal"), None);
        open.send(()).expect("the request waits");
        let renewed = received
            .recv_timeout(wait)
            .expect("a reading")
            .expect("shown");
        assert_eq!(renewed.label, "Lisboa");
        worker.join();
    }

    #[test]
    fn curl_runs_without_a_shell_or_identifiers_and_the_url_last() {
        let url = request_url(&dublin());
        let args = curl_args(&url);

        assert_eq!(args.first().map(String::as_str), Some("--disable"));
        assert_eq!(args.last(), Some(&url));
        let end = args
            .iter()
            .position(|arg| arg == "--")
            .expect("the separator");
        assert_eq!(end, args.len() - 2, "only the URL follows `--`");
        for flag in ["--fail", "--silent", "--max-time"] {
            assert!(args[..end].iter().any(|arg| arg == flag), "{flag}");
        }
        let forbidden = [
            "-H",
            "--header",
            "-b",
            "--cookie",
            "-c",
            "--cookie-jar",
            "-A",
            "--user-agent",
            "-e",
            "--referer",
            "-u",
            "--user",
            "-K",
            "--config",
            "-d",
            "--data",
            "-L",
            "--location",
            "--netrc",
        ];
        for arg in &args[..end] {
            assert!(!forbidden.contains(&arg.as_str()), "{arg}");
            assert!(!arg.contains(' ') && !arg.contains(';') && !arg.contains('$'));
        }
        assert_eq!(CURL, "curl", "a program, not a shell command line");
        assert!(!url.contains("Dublin"), "the label never leaves");
    }

    #[test]
    fn a_non_https_url_is_refused_without_running_anything() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut fetch = CurlFetch::new(&shutdown);
        assert_eq!(fetch.get("file:///etc/passwd"), None);
        assert_eq!(fetch.get("-K/etc/passwd"), None);
    }

    /// A directory of its own for one test's stand-in programs.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("celestina-weather-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// Writes an executable shell script standing in for curl.
    fn fake_curl(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("curl");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("the script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .expect("executable");
        path
    }

    /// Whether `pid` still names a process, a zombie included.
    fn exists(pid: &str) -> bool {
        std::path::Path::new("/proc").join(pid).exists()
    }

    fn pid_in(dir: &std::path::Path) -> String {
        std::fs::read_to_string(dir.join("pid"))
            .expect("the script wrote its pid")
            .trim()
            .to_owned()
    }

    const URL: &str = "https://api.example.invalid/v1/forecast";

    #[test]
    fn the_curl_fetcher_returns_a_successful_body_and_refuses_a_failure() {
        let dir = scratch("success");
        let shutdown = Arc::new(AtomicBool::new(false));
        let ok = fake_curl(&dir, "printf '%s' '{\"current\":{}}'");
        assert_eq!(
            CurlFetch::with_program(&ok, &shutdown).get(URL),
            Some(br#"{"current":{}}"#.to_vec())
        );
        // `--fail`: an HTTP error is curl's exit 22, never a body.
        let failing = fake_curl(&dir, "printf 'error page'; exit 22");
        assert_eq!(CurlFetch::with_program(&failing, &shutdown).get(URL), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_curl_fetcher_refuses_an_oversized_body() {
        let dir = scratch("oversize");
        let shutdown = Arc::new(AtomicBool::new(false));
        let exact = fake_curl(&dir, &format!("head -c {MAX_RESPONSE_BYTES} /dev/zero"));
        let body = CurlFetch::with_program(&exact, &shutdown).get(URL);
        assert_eq!(body.map(|body| body.len()), Some(MAX_RESPONSE_BYTES));
        let script = format!("echo $$ > '{}/pid'; exec cat /dev/zero", dir.display());
        let endless = fake_curl(&dir, &script);
        let start = Instant::now();
        assert_eq!(CurlFetch::with_program(&endless, &shutdown).get(URL), None);
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(!exists(&pid_in(&dir)), "killed and reaped");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hanging_curl_is_killed_at_the_hard_limit_and_reaped() {
        let dir = scratch("hang");
        let shutdown = Arc::new(AtomicBool::new(false));
        let script = format!("echo $$ > '{}/pid'; exec sleep 30", dir.display());
        let hanging = fake_curl(&dir, &script);
        let mut fetch = CurlFetch::with_program(&hanging, &shutdown)
            .with_hard_limit(Duration::from_millis(500));
        let start = Instant::now();
        assert_eq!(fetch.get(URL), None);
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(500), "{took:?}");
        assert!(took < Duration::from_secs(5), "{took:?}");
        assert!(!exists(&pid_in(&dir)), "no zombie left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_curl_is_no_reading() {
        let dir = scratch("missing");
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut fetch = CurlFetch::with_program(dir.join("no-such-curl"), &shutdown);
        assert_eq!(fetch.get(URL), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_ends_a_request_in_flight_promptly() {
        let dir = scratch("shutdown");
        let shutdown = Arc::new(AtomicBool::new(false));
        let script = format!("echo $$ > '{}/pid'; exec sleep 30", dir.display());
        let hanging = fake_curl(&dir, &script);
        let mut fetch = CurlFetch::with_program(&hanging, &shutdown);
        let raiser = {
            let shutdown = Arc::clone(&shutdown);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                shutdown.store(true, Ordering::Release);
            })
        };
        let start = Instant::now();
        assert_eq!(fetch.get(URL), None);
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "{:?}",
            start.elapsed()
        );
        raiser.join().expect("the raiser");
        assert!(!exists(&pid_in(&dir)), "killed and reaped");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
