//! The phone duty (CEL-2-J): the author's phone as Magnetita lists it
//! (battery, connection, media). Off by default (parity design §4.1).
//!
//! [`PhoneHandle`] holds the Magnetita worker the settings ask for and
//! nothing else: `duties.phone` on has the worker, off has none, and
//! [`PhoneHandle::apply`] moves from one to the other whenever `shell.json`
//! changes, with no restart. Starting the worker is behind [`Spawner`] so the
//! rules are tested without a bus. The state the pieces read is
//! [`celestina_shell::phone::PhoneState`], kept by the app and updated by
//! [`reduce`]; a duty that is off always reads [`PhoneState::Absent`].

use std::cell::RefCell;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use celestina_shell::phone::PhoneState;
use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::duty::DutyWorker;
use crate::providers::magnetita::{
    self, MagnetitaConfig, MagnetitaEvent, MagnetitaHandle, MediaKind, PhoneRequest,
};
use crate::providers::Worker;

/// What the Magnetita worker sends to the loop: the news and the run of the
/// worker that sent it (see [`crate::providers::duty`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneNews {
    pub run: u64,
    pub event: MagnetitaEvent,
}

/// The loop's side of the Magnetita worker.
pub trait PhoneControl: Send {
    /// # Errors
    ///
    /// Why the worker did not take the request.
    fn request(&self, request: PhoneRequest) -> Result<(), String>;
}

impl PhoneControl for MagnetitaHandle {
    fn request(&self, request: PhoneRequest) -> Result<(), String> {
        MagnetitaHandle::request(self, request).map_err(|e| e.to_string())
    }
}

/// How the worker comes to exist: the seam between the rules and the bus.
pub trait Spawner {
    /// Starts the worker; its news is stamped with `run`.
    ///
    /// # Errors
    ///
    /// The failure to start the thread.
    fn magnetita(
        &self,
        run: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn PhoneControl>, Worker)>;
}

/// The real spawner: the session bus.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn magnetita(
        &self,
        run: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn PhoneControl>, Worker)> {
        let sender = self.sender.clone();
        let (handle, worker) = magnetita::spawn(
            &MagnetitaConfig::session(),
            move |event| {
                sender
                    .send(Message::Phone(Box::new(PhoneNews { run, event })))
                    .is_ok()
            },
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }
}

struct Inner {
    spawner: Box<dyn Spawner>,
    duty: DutyWorker<dyn PhoneControl>,
}

/// The phone duty: the worker, if the duty is on.
pub struct PhoneHandle {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for PhoneHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneHandle")
            .field("running", &self.running())
            .finish()
    }
}

impl PhoneHandle {
    /// The handle over the session bus; nothing runs until
    /// [`PhoneHandle::apply`] finds the duty on.
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
                duty: DutyWorker::new("phone"),
            }),
        }
    }

    /// Whether the worker exists.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.borrow().duty.is_on()
    }

    /// Whether news stamped `run` is the running worker's.
    #[must_use]
    pub fn accepts(&self, run: u64) -> bool {
        self.inner.borrow().duty.accepts(run)
    }

    /// Brings the worker to what `settings` ask for. Idempotent: it runs at
    /// start and after every settings change. Returns whether a worker
    /// started or stopped, so the caller resets what it shows when the duty
    /// went off.
    pub fn apply(&self, settings: &Settings) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Inner { spawner, duty } = &mut *inner;
        duty.set(settings.duties.phone == Duty::On, |run, flag| {
            spawner.magnetita(run, flag)
        })
    }

    /// Queues `request` for the running worker.
    ///
    /// # Errors
    ///
    /// Why it was not queued: the duty is off, the device is not listed, the
    /// queue is full.
    pub fn request(&self, request: PhoneRequest) -> Result<(), String> {
        match self.inner.borrow().duty.control() {
            Some(control) => control.request(request),
            None => Err("the phone duty is off".to_owned()),
        }
    }

    /// Stops the worker and waits for it. Also what dropping the handle does.
    pub fn stop(&self) {
        self.inner.borrow_mut().duty.stop();
    }
}

/// Applies one worker event to `state`; returns whether the state changed
/// (and the bar must be drawn again).
pub fn reduce(state: &mut PhoneState, event: MagnetitaEvent) -> bool {
    let next = match event {
        MagnetitaEvent::State(next) => next,
        MagnetitaEvent::Absent => PhoneState::Absent,
    };
    if *state == next {
        return false;
    }
    *state = next;
    true
}

/// The request `phone media <kind>` makes: the connected device's media
/// action, or `None` while no phone is connected.
#[must_use]
pub fn media_request(state: &PhoneState, kind: MediaKind) -> Option<PhoneRequest> {
    match state {
        PhoneState::Absent => None,
        PhoneState::Present(phone) => Some(PhoneRequest::MediaAction {
            device_id: phone.id.clone(),
            action: kind,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use celestina_shell::phone::Phone;

    use super::*;

    fn present(id: &str, battery: u8) -> PhoneState {
        PhoneState::Present(Phone {
            id: id.to_owned(),
            name: "Pixel".to_owned(),
            battery: Some(battery),
            charging: false,
            media: None,
        })
    }

    #[test]
    fn reduction_replaces_the_state_and_reports_only_a_change() {
        let mut state = PhoneState::Absent;
        assert!(!reduce(&mut state, MagnetitaEvent::Absent));
        assert!(reduce(&mut state, MagnetitaEvent::State(present("a", 80))));
        assert!(!reduce(&mut state, MagnetitaEvent::State(present("a", 80))));
        assert!(reduce(&mut state, MagnetitaEvent::State(present("a", 79))));
        assert!(reduce(&mut state, MagnetitaEvent::Absent));
        assert_eq!(state, PhoneState::Absent);
    }

    #[test]
    fn media_goes_to_the_connected_device_only() {
        assert_eq!(media_request(&PhoneState::Absent, MediaKind::Next), None);
        assert_eq!(
            media_request(&present("dev-1", 50), MediaKind::PlayPause),
            Some(PhoneRequest::MediaAction {
                device_id: "dev-1".to_owned(),
                action: MediaKind::PlayPause,
            })
        );
    }

    /// Counts starts and records the runs and requests, with no bus.
    #[derive(Clone, Default)]
    struct Fake {
        runs: Arc<Mutex<Vec<u64>>>,
        requests: Arc<Mutex<Vec<PhoneRequest>>>,
        fail: Arc<AtomicBool>,
    }

    struct FakeControl(Arc<Mutex<Vec<PhoneRequest>>>);

    impl PhoneControl for FakeControl {
        fn request(&self, request: PhoneRequest) -> Result<(), String> {
            self.0.lock().expect("lock").push(request);
            Ok(())
        }
    }

    impl Spawner for Fake {
        fn magnetita(
            &self,
            run: u64,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn PhoneControl>, Worker)> {
            if self.fail.load(Ordering::Relaxed) {
                return Err(io::Error::other("no thread"));
            }
            self.runs.lock().expect("lock").push(run);
            let flag = Arc::clone(shutdown);
            let worker = Worker::spawn("test-phone", shutdown, move || {
                while !flag.load(Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })?;
            Ok((Box::new(FakeControl(Arc::clone(&self.requests))), worker))
        }
    }

    fn settings(phone: Duty) -> Settings {
        let mut settings = Settings::default();
        settings.duties.phone = phone;
        settings
    }

    #[test]
    fn the_worker_follows_the_duty_on_off_on() {
        let fake = Fake::default();
        let handle = PhoneHandle::with_spawner(Box::new(fake.clone()));
        assert!(!handle.running());
        assert!(!handle.apply(&settings(Duty::Off)), "off stays off");

        assert!(handle.apply(&settings(Duty::On)));
        assert!(handle.running());
        assert!(!handle.apply(&settings(Duty::On)), "idempotent");
        assert!(handle.accepts(1));
        assert!(!handle.accepts(2));

        assert!(handle.apply(&settings(Duty::Off)));
        assert!(!handle.running());
        assert!(!handle.accepts(1), "news of a stopped worker is stale");
        assert!(handle.request(media()).is_err());

        assert!(handle.apply(&settings(Duty::On)));
        assert!(handle.accepts(2), "a new run has a new number");
        assert!(!handle.accepts(1), "the old run stays stale");
        assert_eq!(*fake.runs.lock().expect("lock"), vec![1, 2]);

        handle.request(media()).expect("queued while on");
        assert_eq!(fake.requests.lock().expect("lock").len(), 1);
        handle.stop();
        assert!(!handle.running());
    }

    fn media() -> PhoneRequest {
        PhoneRequest::MediaAction {
            device_id: "dev-1".to_owned(),
            action: MediaKind::Next,
        }
    }

    #[test]
    fn a_file_from_1_3_0_starts_no_worker() {
        // The older file has no `phone` key: both new duties read off.
        let older =
            Settings::from_bytes(br#"{"duties":{"idle":"on"}}"#).expect("a valid older file");
        let fake = Fake::default();
        let handle = PhoneHandle::with_spawner(Box::new(fake.clone()));
        assert!(!handle.apply(&older));
        assert!(!handle.running());
        assert!(fake.runs.lock().expect("lock").is_empty());
    }

    #[test]
    fn a_worker_that_does_not_start_is_tried_again_next_time() {
        let fake = Fake::default();
        fake.fail.store(true, Ordering::Relaxed);
        let handle = PhoneHandle::with_spawner(Box::new(fake.clone()));
        assert!(!handle.apply(&settings(Duty::On)));
        assert!(!handle.running());
        fake.fail.store(false, Ordering::Relaxed);
        assert!(handle.apply(&settings(Duty::On)));
        assert!(handle.running());
    }
}
