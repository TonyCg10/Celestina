//! The glance workers (CEL-2-K): the system monitor and the weather, each
//! running only while something placed on the bar reads it (design §4).
//!
//! [`GlanceHandle`] holds exactly the workers the settings ask for. Both
//! need the `bar` duty on: a bar that is off shows no piece, so it reads
//! nothing and costs nothing. With the bar on:
//!
//! - the system monitor runs iff the `sysmon` piece is in a zone of the bar's
//!   layout;
//! - the weather worker runs iff the `weather` or the `clock` piece is in a
//!   zone **and** `shell.json` holds a valid location. Without one it is not
//!   even spawned: no thread, no request (design §3.1). A location change
//!   reaches the running worker through its queue; a worker found gone when
//!   one is sent is dropped, and the next apply starts another.
//!
//! [`GlanceHandle::apply`] is idempotent and moves from one state to the next
//! whenever the settings change. A worker no longer wanted is handed to a
//! [`Reaper`], so the loop never joins; [`GlanceHandle::stop`] joins every
//! one at exit. Starting a worker is behind the [`Spawner`] seam so the rules
//! are tested without `/proc` or the network.
//!
//! The location is never written to the journal: these lines say only
//! whether a worker runs.

use std::cell::RefCell;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::settings::{BarLayout, Duty, Location, Settings};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::weather::{Refused, WeatherProvider};
use crate::providers::{self, Reaper, Worker};

/// The id of the system monitor's piece.
pub const SYSMON_PIECE: &str = "sysmon";
/// The id of the weather's piece.
pub const WEATHER_PIECE: &str = "weather";
/// The id of the clock's piece, whose page shows the weather too.
pub const CLOCK_PIECE: &str = "clock";

/// How long stopping one worker waits for it.
const JOIN_WAIT: Duration = Duration::from_secs(2);

/// Whether `id` is in one of the layout's three zones.
#[must_use]
pub fn placed(layout: &BarLayout, id: &str) -> bool {
    [&layout.start, &layout.centre, &layout.end]
        .into_iter()
        .any(|zone| zone.iter().any(|placed| placed == id))
}

/// What the settings ask of the glance workers.
#[derive(Clone, PartialEq, Eq)]
pub struct Wants {
    /// The system monitor runs.
    pub sysmon: bool,
    /// The weather worker runs, for this place.
    pub weather: Option<Location>,
}

impl std::fmt::Debug for Wants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The coordinates stay out of logs.
        f.debug_struct("Wants")
            .field("sysmon", &self.sysmon)
            .field("weather", &self.weather.is_some())
            .finish()
    }
}

impl Wants {
    /// The rules of the module head, over `settings`.
    #[must_use]
    pub fn of(settings: &Settings) -> Self {
        if settings.duties.bar() != Duty::On {
            return Self {
                sysmon: false,
                weather: None,
            };
        }
        let layout = settings.bar_layout();
        let reads_weather = placed(&layout, WEATHER_PIECE) || placed(&layout, CLOCK_PIECE);
        Self {
            sysmon: placed(&layout, SYSMON_PIECE),
            weather: settings.weather.clone().filter(|_| reads_weather),
        }
    }
}

/// A reading published by a worker, kept only while that worker is
/// `running`: one that arrives after its worker was stopped (it was already
/// in the channel) must not bring back what was withdrawn.
#[must_use]
pub fn admit<T>(running: bool, reading: Option<T>) -> Option<T> {
    reading.filter(|_| running)
}

/// The loop's side of the weather worker.
pub trait WeatherControl: Send {
    /// Hands the worker a location, or none.
    ///
    /// # Errors
    ///
    /// Why the worker did not take it.
    fn set_location(&self, place: Option<Location>) -> Result<(), Refused>;
}

impl WeatherControl for WeatherProvider {
    fn set_location(&self, place: Option<Location>) -> Result<(), Refused> {
        WeatherProvider::set_location(self, place)
    }
}

/// How workers come to exist: the seam between the rules and the system.
pub trait Spawner {
    /// # Errors
    ///
    /// The failure to start the thread.
    fn sysmon(&self, shutdown: &Arc<AtomicBool>) -> io::Result<Worker>;
    /// # Errors
    ///
    /// The failure to start the thread.
    fn weather(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn WeatherControl>, Worker)>;
}

/// The real spawner: `/proc` and the weather service.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn sysmon(&self, shutdown: &Arc<AtomicBool>) -> io::Result<Worker> {
        providers::sysmon::spawn(self.sender.clone(), shutdown)
    }

    fn weather(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn WeatherControl>, Worker)> {
        let (provider, worker) = providers::weather::spawn(self.sender.clone(), shutdown)?;
        Ok((Box::new(provider), worker))
    }
}

struct WeatherSlot {
    control: Box<dyn WeatherControl>,
    worker: Worker,
    /// The location the worker last took; `None` until it took one.
    sent: Option<Location>,
}

impl WeatherSlot {
    /// Sends `place` unless the worker already took exactly that; returns
    /// whether the worker is still there. A full queue is not recorded, so
    /// the next apply sends it again; a worker gone is reported, and its slot
    /// is the caller's to drop.
    fn sync(&mut self, place: &Location) -> bool {
        if self.sent.as_ref() == Some(place) {
            return true;
        }
        match self.control.set_location(Some(place.clone())) {
            Ok(()) => {
                self.sent = Some(place.clone());
                true
            }
            Err(Refused::Busy) => {
                tracing::warn!("weather: the worker is busy; the location is sent again later");
                true
            }
            Err(Refused::Gone) => {
                tracing::warn!("weather: the worker has stopped; the next apply starts another");
                false
            }
        }
    }

    fn stop(self) {
        let WeatherSlot {
            control, worker, ..
        } = self;
        // Dropping the queue ends the worker by itself too.
        drop(control);
        worker.join_within(JOIN_WAIT);
    }
}

/// Which workers exist right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Running {
    pub sysmon: bool,
    pub weather: bool,
}

struct Inner {
    spawner: Box<dyn Spawner>,
    sysmon: Option<Worker>,
    weather: Option<WeatherSlot>,
    sysmon_reaper: Reaper,
    weather_reaper: Reaper,
}

/// The glance workers the settings ask for.
pub struct GlanceHandle {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for GlanceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlanceHandle")
            .field("running", &self.running())
            .finish()
    }
}

impl GlanceHandle {
    /// The handle over the real system: workers publish into `sender`.
    /// Nothing runs until [`GlanceHandle::apply`] finds a piece placed.
    #[must_use]
    pub fn new(sender: Sender<Message>) -> Self {
        Self::with_spawner(Box::new(Live { sender }))
    }

    /// The handle over `spawner`, for tests.
    #[must_use]
    pub fn with_spawner(spawner: Box<dyn Spawner>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                sysmon: None,
                weather: None,
                sysmon_reaper: Reaper::default(),
                weather_reaper: Reaper::default(),
            }),
        }
    }

    /// Which workers exist.
    #[must_use]
    pub fn running(&self) -> Running {
        let inner = self.inner.borrow();
        Running {
            sysmon: inner.sysmon.is_some(),
            weather: inner.weather.is_some(),
        }
    }

    /// Brings the workers to what `settings` ask for ([`Wants::of`]) and
    /// says which run afterwards; a reading of one that does not is the
    /// caller's to withdraw. Idempotent: called at start and after every
    /// settings change.
    pub fn apply(&self, settings: &Settings) -> Running {
        let wants = Wants::of(settings);
        {
            let mut inner = self.inner.borrow_mut();
            inner.apply_sysmon(wants.sysmon);
            inner.apply_weather(wants.weather.as_ref());
        }
        self.running()
    }

    /// Stops every worker and waits for them, and for any reaper still
    /// joining one. Also what dropping the handle does.
    pub fn stop(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(worker) = inner.sysmon.take() {
            worker.join_within(JOIN_WAIT);
        }
        if let Some(weather) = inner.weather.take() {
            weather.stop();
        }
        inner.sysmon_reaper.wait();
        inner.weather_reaper.wait();
    }
}

impl Drop for GlanceHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Inner {
    fn apply_sysmon(&mut self, wanted: bool) {
        match (wanted, self.sysmon.is_some()) {
            (true, false) => {
                self.sysmon_reaper.wait();
                match self.spawner.sysmon(&new_flag()) {
                    Ok(worker) => {
                        self.sysmon = Some(worker);
                        tracing::info!("system monitor on");
                    }
                    Err(err) => tracing::warn!("system monitor: the worker did not start: {err}"),
                }
            }
            (false, true) => {
                if let Some(worker) = self.sysmon.take() {
                    self.sysmon_reaper = Reaper::spawn("sysmon", move || {
                        worker.join_within(JOIN_WAIT);
                    });
                    tracing::info!("system monitor off");
                }
            }
            _ => {}
        }
    }

    fn apply_weather(&mut self, wanted: Option<&Location>) {
        match (wanted, self.weather.as_mut()) {
            (Some(place), Some(slot)) => {
                if !slot.sync(place) {
                    if let Some(slot) = self.weather.take() {
                        self.retire_weather(slot);
                    }
                }
            }
            (Some(place), None) => {
                self.weather_reaper.wait();
                match self.spawner.weather(&new_flag()) {
                    Ok((control, worker)) => {
                        let mut slot = WeatherSlot {
                            control,
                            worker,
                            sent: None,
                        };
                        if slot.sync(place) {
                            self.weather = Some(slot);
                            tracing::info!("weather on");
                        } else {
                            self.retire_weather(slot);
                        }
                    }
                    Err(err) => tracing::warn!("weather: the worker did not start: {err}"),
                }
            }
            (None, Some(_)) => {
                if let Some(slot) = self.weather.take() {
                    self.retire_weather(slot);
                    tracing::info!("weather off");
                }
            }
            (None, None) => {}
        }
    }

    /// Hands `slot` to the reaper. The previous reap, if any, is waited for
    /// first, so no stop is ever left unjoined.
    fn retire_weather(&mut self, slot: WeatherSlot) {
        self.weather_reaper.wait();
        self.weather_reaper = Reaper::spawn("weather", move || slot.stop());
    }
}

/// A shutdown flag for one worker: joining it must not stop the shell.
fn new_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::settings::BarChoice;
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    type Log = Arc<Mutex<Vec<String>>>;

    fn said(log: &Log) -> Vec<String> {
        log.lock().expect("log").clone()
    }

    fn fake_worker(name: &'static str, shutdown: &Arc<AtomicBool>, log: &Log) -> Worker {
        let flag = Arc::clone(shutdown);
        let log = Arc::clone(log);
        Worker::spawn(name, shutdown, move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            log.lock().expect("log").push(format!("{name} stopped"));
        })
        .expect("a thread")
    }

    struct FakeWeather {
        log: Log,
        /// Refuse the next command once, as a full queue.
        busy: Arc<AtomicBool>,
        /// Refuse the next command once, as a worker that has stopped.
        gone: Arc<AtomicBool>,
    }

    impl WeatherControl for FakeWeather {
        fn set_location(&self, place: Option<Location>) -> Result<(), Refused> {
            let label = place
                .as_ref()
                .map_or("none", |p| p.label.as_str())
                .to_owned();
            if self.busy.swap(false, Ordering::AcqRel) {
                self.log
                    .lock()
                    .expect("log")
                    .push(format!("weather at {label} (busy)"));
                return Err(Refused::Busy);
            }
            if self.gone.swap(false, Ordering::AcqRel) {
                self.log
                    .lock()
                    .expect("log")
                    .push(format!("weather at {label} (gone)"));
                return Err(Refused::Gone);
            }
            self.log
                .lock()
                .expect("log")
                .push(format!("weather at {label}"));
            Ok(())
        }
    }

    struct Fake {
        log: Log,
        busy: Arc<AtomicBool>,
        gone: Arc<AtomicBool>,
    }

    impl Spawner for Fake {
        fn sysmon(&self, shutdown: &Arc<AtomicBool>) -> io::Result<Worker> {
            self.log.lock().expect("log").push("sysmon spawned".into());
            Ok(fake_worker("sysmon", shutdown, &self.log))
        }
        fn weather(
            &self,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn WeatherControl>, Worker)> {
            self.log.lock().expect("log").push("weather spawned".into());
            Ok((
                Box::new(FakeWeather {
                    log: Arc::clone(&self.log),
                    busy: Arc::clone(&self.busy),
                    gone: Arc::clone(&self.gone),
                }),
                fake_worker("weather", shutdown, &self.log),
            ))
        }
    }

    struct Flags {
        busy: Arc<AtomicBool>,
        gone: Arc<AtomicBool>,
    }

    fn fake_flags() -> (GlanceHandle, Log, Flags) {
        let log = Log::default();
        let flags = Flags {
            busy: Arc::new(AtomicBool::new(false)),
            gone: Arc::new(AtomicBool::new(false)),
        };
        let handle = GlanceHandle::with_spawner(Box::new(Fake {
            log: Arc::clone(&log),
            busy: Arc::clone(&flags.busy),
            gone: Arc::clone(&flags.gone),
        }));
        (handle, log, flags)
    }

    fn fake() -> (GlanceHandle, Log, Arc<AtomicBool>) {
        let (handle, log, flags) = fake_flags();
        (handle, log, flags.busy)
    }

    /// The bar on, with `end` as its end zone and a location labelled
    /// `place`, if any.
    fn with(end: &[&str], place: Option<&str>) -> Settings {
        let mut settings = Settings {
            bar: BarChoice {
                end: Some(end.iter().map(|id| (*id).to_owned()).collect()),
                ..BarChoice::default()
            },
            weather: place.and_then(|label| Location::new(40.42, -3.70, label)),
            ..Settings::default()
        };
        settings.duties.bar = Some(Duty::On);
        settings
    }

    const NOTHING: Running = Running {
        sysmon: false,
        weather: false,
    };

    #[test]
    fn the_defaults_start_nothing() {
        let (handle, log, _) = fake();
        handle.apply(&Settings::default());
        assert_eq!(handle.running(), NOTHING);
        assert!(said(&log).is_empty());
    }

    #[test]
    fn placement_is_read_from_every_zone() {
        let layout = BarLayout {
            start: vec!["a".into()],
            centre: vec!["sysmon".into()],
            end: vec!["clock".into()],
            tray_hidden: vec!["weather".into()],
        };
        assert!(placed(&layout, SYSMON_PIECE));
        assert!(placed(&layout, CLOCK_PIECE));
        // A hidden tray id is not a placed piece.
        assert!(!placed(&layout, WEATHER_PIECE));
    }

    #[test]
    fn the_system_monitor_follows_its_placement() {
        let (handle, log, _) = fake();
        handle.apply(&with(&["sysmon"], None));
        assert_eq!(
            handle.running(),
            Running {
                sysmon: true,
                weather: false
            }
        );
        // The same settings again start nothing more.
        handle.apply(&with(&["tray", "sysmon"], None));
        assert_eq!(said(&log), ["sysmon spawned"]);
        handle.apply(&with(&["tray"], None));
        assert_eq!(handle.running(), NOTHING);
        // On again only once the old one is gone.
        handle.apply(&with(&["sysmon"], None));
        let lines = said(&log);
        let stopped = lines.iter().position(|l| l == "sysmon stopped");
        let respawned = lines.iter().rposition(|l| l == "sysmon spawned");
        assert!(stopped.is_some() && stopped < respawned, "{lines:?}");
    }

    #[test]
    fn no_location_never_spawns_the_weather_worker() {
        let (handle, log, _) = fake();
        for end in [&["weather"][..], &["clock"], &["weather", "clock"]] {
            handle.apply(&with(end, None));
            assert_eq!(handle.running(), NOTHING);
        }
        assert!(said(&log).is_empty(), "{:?}", said(&log));
        // A location with nothing placed that reads it starts nothing either.
        handle.apply(&with(&["tray", "sysmon"], Some("Madrid")));
        assert!(!handle.running().weather);
        assert!(!said(&log).iter().any(|l| l.starts_with("weather")));
    }

    #[test]
    fn weather_or_clock_with_a_location_runs_the_worker_and_takes_changes() {
        let (handle, log, _) = fake();
        handle.apply(&with(&["clock"], Some("Madrid")));
        assert!(handle.running().weather);
        // The same location is not sent twice; the weather piece too keeps it.
        handle.apply(&with(&["clock"], Some("Madrid")));
        handle.apply(&with(&["weather"], Some("Madrid")));
        handle.apply(&with(&["weather"], Some("Casa")));
        assert_eq!(
            said(&log),
            ["weather spawned", "weather at Madrid", "weather at Casa"]
        );
        // The location removed: the worker goes, it is not told "none".
        handle.apply(&with(&["weather"], None));
        assert!(!handle.running().weather);
        handle.stop();
        assert!(said(&log).contains(&"weather stopped".to_owned()));
        assert!(!said(&log).contains(&"weather at none".to_owned()));
        // Unplaced with the location still there: it goes as well.
        handle.apply(&with(&["clock"], Some("Madrid")));
        handle.apply(&with(&["tray"], Some("Madrid")));
        assert!(!handle.running().weather);
    }

    #[test]
    fn a_refused_location_is_sent_again_by_the_next_apply() {
        let (handle, log, busy) = fake();
        busy.store(true, Ordering::Release);
        handle.apply(&with(&["weather"], Some("Madrid")));
        handle.apply(&with(&["weather"], Some("Madrid")));
        handle.apply(&with(&["weather"], Some("Madrid")));
        assert_eq!(
            said(&log),
            [
                "weather spawned",
                "weather at Madrid (busy)",
                "weather at Madrid"
            ]
        );
    }

    #[test]
    fn stop_and_drop_join_every_worker() {
        let (handle, log, _) = fake();
        handle.apply(&with(&["sysmon", "clock"], Some("Madrid")));
        assert_eq!(
            handle.running(),
            Running {
                sysmon: true,
                weather: true
            }
        );
        handle.stop();
        assert_eq!(handle.running(), NOTHING);
        let lines = said(&log);
        assert!(lines.contains(&"sysmon stopped".to_owned()), "{lines:?}");
        assert!(lines.contains(&"weather stopped".to_owned()), "{lines:?}");

        let (handle, log, _) = fake();
        handle.apply(&with(&["sysmon"], None));
        drop(handle);
        assert!(said(&log).contains(&"sysmon stopped".to_owned()));
    }

    #[test]
    fn wants_never_show_the_coordinates() {
        let wants = Wants::of(&with(&["weather"], Some("Madrid")));
        let shown = format!("{wants:?}");
        assert_eq!(shown, "Wants { sysmon: false, weather: true }");
        assert!(!shown.contains("40") && !shown.contains("Madrid"));
    }

    #[test]
    fn a_bar_that_is_off_runs_nothing_whatever_is_placed() {
        let (handle, log, _) = fake();
        let mut off = with(&["sysmon", "weather", "clock"], Some("Madrid"));
        for bar in [None, Some(Duty::Off)] {
            off.duties.bar = bar;
            assert_eq!(handle.apply(&off), NOTHING);
        }
        assert!(said(&log).is_empty(), "{:?}", said(&log));

        // On, both start; off again, both go.
        let on = with(&["sysmon", "weather", "clock"], Some("Madrid"));
        assert_eq!(
            handle.apply(&on),
            Running {
                sysmon: true,
                weather: true
            }
        );
        assert_eq!(handle.apply(&off), NOTHING);
        handle.stop();
        let lines = said(&log);
        assert!(lines.contains(&"sysmon stopped".to_owned()), "{lines:?}");
        assert!(lines.contains(&"weather stopped".to_owned()), "{lines:?}");
    }

    #[test]
    fn a_weather_worker_found_gone_is_dropped_and_started_again_by_the_next_apply() {
        let (handle, log, flags) = fake_flags();
        assert!(handle.apply(&with(&["weather"], Some("Madrid"))).weather);
        // The worker died; the next change sent finds it gone.
        flags.gone.store(true, Ordering::Release);
        assert!(!handle.apply(&with(&["weather"], Some("Casa"))).weather);
        // The next apply starts another and hands it the location.
        assert!(handle.apply(&with(&["weather"], Some("Casa"))).weather);
        handle.stop();
        let lines = said(&log);
        let spawned = lines.iter().filter(|l| *l == "weather spawned").count();
        assert_eq!(spawned, 2, "{lines:?}");
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("weather at"))
                .collect::<Vec<_>>(),
            [
                "weather at Madrid",
                "weather at Casa (gone)",
                "weather at Casa"
            ],
        );
        // Both instances were joined.
        let stopped = lines.iter().filter(|l| *l == "weather stopped").count();
        assert_eq!(stopped, 2, "{lines:?}");
    }

    #[test]
    fn a_weather_worker_gone_at_its_first_location_is_not_kept() {
        let (handle, log, flags) = fake_flags();
        flags.gone.store(true, Ordering::Release);
        assert!(!handle.apply(&with(&["clock"], Some("Madrid"))).weather);
        assert!(handle.apply(&with(&["clock"], Some("Madrid"))).weather);
        handle.stop();
        assert_eq!(
            said(&log)
                .iter()
                .filter(|l| *l == "weather spawned")
                .count(),
            2
        );
    }

    #[test]
    fn a_reading_is_admitted_only_from_a_running_worker() {
        assert_eq!(admit(true, Some(7)), Some(7));
        assert_eq!(admit(false, Some(7)), None);
        assert_eq!(admit::<u8>(true, None), None);
    }

    /// The real weather worker under the handle, with a counting fetcher and
    /// a sink the test reads: the glue's whole path but the network.
    struct LiveWeather {
        asked: Arc<std::sync::atomic::AtomicUsize>,
        sink: std::sync::mpsc::Sender<Option<crate::providers::weather::WeatherReading>>,
    }

    struct Counting(Arc<std::sync::atomic::AtomicUsize>);

    impl crate::providers::weather::Fetch for Counting {
        fn get(&mut self, _url: &str) -> Option<Vec<u8>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some(br#"{"current":{"temperature_2m":21.2,"weather_code":1,"is_day":1}}"#.to_vec())
        }
    }

    impl Spawner for LiveWeather {
        fn sysmon(&self, _shutdown: &Arc<AtomicBool>) -> io::Result<Worker> {
            Err(io::Error::other("no system monitor in this test"))
        }
        fn weather(
            &self,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn WeatherControl>, Worker)> {
            let sink = self.sink.clone();
            let start = std::time::Instant::now();
            let (provider, worker) = crate::providers::weather::spawn_with(
                Counting(Arc::clone(&self.asked)),
                shutdown,
                move |reading| sink.send(reading).is_ok(),
                move || crate::providers::weather::Now {
                    mono_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
                    unix_s: 0,
                },
            )?;
            Ok((Box::new(provider), worker))
        }
    }

    #[test]
    fn removing_the_location_clears_the_reading_and_stops_the_requests() {
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (sink, published) = std::sync::mpsc::channel();
        let handle = GlanceHandle::with_spawner(Box::new(LiveWeather {
            asked: Arc::clone(&asked),
            sink,
        }));
        let wait = Duration::from_secs(5);

        // What the app holds: a reading, admitted while its worker runs.
        let running = handle.apply(&with(&["weather"], Some("Madrid")));
        let arrived = published.recv_timeout(wait).expect("a publication");
        let mut held = admit(running.weather, arrived);
        assert_eq!(held.as_ref().map(|r| r.celsius), Some(21));
        assert_eq!(asked.load(Ordering::SeqCst), 1);

        // The location removed in `shell.json`: the worker no longer runs,
        // so the app withdraws what it held, and anything still in flight
        // is not admitted.
        let running = handle.apply(&with(&["weather"], None));
        assert!(!running.weather);
        held = admit(running.weather, held);
        assert_eq!(held, None);
        handle.stop();
        for late in published.try_iter() {
            assert_eq!(admit(handle.running().weather, late), None);
        }
        // Nothing asks any more.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(asked.load(Ordering::SeqCst), 1);
        assert!(!handle.running().weather);
    }
}
