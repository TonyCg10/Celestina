//! One process-wide model of the removable volumes and the phones.
//!
//! Each tab used to ask UDisks2 and Magnetita for itself, on the Qt thread,
//! over a bus connection opened for that one call, and started its own pair of
//! watcher threads — so one hotplug became one synchronous reload per tab, and
//! a Magnetita that was activatable but slow to start could hold the window
//! for the bus's whole default timeout.
//!
//! Now two workers own the bus, each with one connection it keeps for the life
//! of the process: one for UDisks2 on the system bus, one for Magnetita on the
//! session bus. They list once at start and again after every change signal
//! (coalesced over a short burst) or explicit reload, keep the last answer
//! here, and wake every tab through its Qt thread — the same shape as the
//! folder watch register. A tab reads the kept answer, which costs a lock and
//! no IO. The phone actions (ring, media keys, send a file) ride the Magnetita
//! worker too, so none of them blocks the window either.
//!
//! Both workers are best-effort: a bus that is not there is an empty sidebar
//! section or an error line, never a freeze.

use core::pin::Pin;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::CxxQtThread;
use zbus::blocking::Connection;

use crate::controller::qobject::SideritaController;
use crate::devices::Device;
use crate::volumes::Volume;

/// How long a burst of change signals is gathered before listing again.
const COALESCE: Duration = Duration::from_millis(300);

/// What the Magnetita worker is asked to do.
#[derive(Debug, Eq, PartialEq)]
enum PhoneRequest {
    Reload,
    Ring(String),
    Media(String, String),
    Send(String, PathBuf),
}

#[derive(Default)]
struct Model {
    listeners: Vec<CxxQtThread<SideritaController>>,
    volumes: Option<Result<Vec<Volume>, String>>,
    phones: Option<Vec<Device>>,
    udisks: Option<Sender<()>>,
    magnetita: Option<Sender<PhoneRequest>>,
}

fn model() -> &'static Mutex<Model> {
    static MODEL: OnceLock<Mutex<Model>> = OnceLock::new();
    MODEL.get_or_init(|| Mutex::new(Model::default()))
}

/// Registers a tab to hear about device changes, starting the two workers the
/// first time any tab asks.
pub(crate) fn subscribe(listener: CxxQtThread<SideritaController>) {
    let Ok(mut state) = model().lock() else {
        return;
    };
    state.listeners.push(listener);
    if state.udisks.is_none() {
        let (sender, receiver) = mpsc::channel();
        if spawn("siderita-udisks", {
            let sender = sender.clone();
            move || run_volumes(&receiver, sender)
        }) {
            state.udisks = Some(sender);
        }
    }
    if state.magnetita.is_none() {
        let (sender, receiver) = mpsc::channel();
        if spawn("siderita-magnetita", {
            let sender = sender.clone();
            move || run_phones(&receiver, sender)
        }) {
            state.magnetita = Some(sender);
        }
    }
}

/// The last volume listing, if one has arrived.
pub(crate) fn volumes() -> Option<Result<Vec<Volume>, String>> {
    model().lock().ok()?.volumes.clone()
}

/// The last phone listing, if one has arrived.
pub(crate) fn phones() -> Option<Vec<Device>> {
    model().lock().ok()?.phones.clone()
}

/// Asks for a fresh volume listing — after a mount or an unmount, which UDisks2
/// reports as a property change rather than as a hotplug.
pub(crate) fn reload_volumes() {
    if let Some(sender) = model().lock().ok().and_then(|state| state.udisks.clone()) {
        let _ = sender.send(());
    }
}

pub(crate) fn ring(device_id: &str) {
    ask_magnetita(PhoneRequest::Ring(device_id.to_owned()));
}

pub(crate) fn media_action(device_id: &str, action: &str) {
    ask_magnetita(PhoneRequest::Media(device_id.to_owned(), action.to_owned()));
}

pub(crate) fn send_file(device_id: &str, path: PathBuf) {
    ask_magnetita(PhoneRequest::Send(device_id.to_owned(), path));
}

fn ask_magnetita(request: PhoneRequest) {
    if let Some(sender) = model()
        .lock()
        .ok()
        .and_then(|state| state.magnetita.clone())
    {
        let _ = sender.send(request);
    }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> bool {
    match std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
    {
        Ok(_) => true,
        Err(error) => {
            eprintln!("Siderita: could not start the {name} worker: {error}");
            false
        }
    }
}

/// How a tab re-reads the part of the model that changed.
type Reread = fn(Pin<&mut SideritaController>);

/// Keeps `update` in the model and has every tab `reread` it; a tab whose
/// queue fails is gone and is forgotten here.
fn publish(update: impl FnOnce(&mut Model), reread: Reread) {
    let Ok(mut state) = model().lock() else {
        return;
    };
    update(&mut state);
    state
        .listeners
        .retain(|listener| listener.queue(reread).is_ok());
}

/// The UDisks2 worker: one system-bus connection, a listing at start and after
/// every coalesced hotplug or reload request.
fn run_volumes(requests: &Receiver<()>, sender: Sender<()>) {
    let connection = match crate::volumes::system_bus() {
        Ok(connection) => connection,
        Err(message) => {
            publish(
                |state| state.volumes = Some(Err(message)),
                SideritaController::load_volumes,
            );
            return;
        }
    };
    if let Err(error) = crate::volumes::forward_changes(&connection, move || {
        let _ = sender.send(());
    }) {
        eprintln!("Siderita: device hotplug watch unavailable: {error}");
    }
    loop {
        let listed = crate::volumes::list_volumes(&connection);
        publish(
            |state| state.volumes = Some(listed),
            SideritaController::load_volumes,
        );
        if requests.recv().is_err() {
            return;
        }
        while requests.recv_timeout(COALESCE).is_ok() {}
    }
}

/// The Magnetita worker: one session-bus connection; listings at start and
/// after every coalesced change or reload, and the phone actions in between.
fn run_phones(requests: &Receiver<PhoneRequest>, sender: Sender<PhoneRequest>) {
    let connection = match Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("Siderita: session bus unavailable for Magnetita: {error}");
            publish(
                |state| state.phones = Some(Vec::new()),
                SideritaController::load_phones,
            );
            return;
        }
    };
    if let Err(error) = crate::devices::forward_changes(&connection, move || {
        let _ = sender.send(PhoneRequest::Reload);
    }) {
        eprintln!("Siderita: Magnetita change watch unavailable: {error}");
    }
    let mut reload = true;
    loop {
        if reload {
            let listed = crate::devices::list_devices(&connection);
            publish(
                |state| state.phones = Some(listed),
                SideritaController::load_phones,
            );
        }
        let Ok(first) = requests.recv() else {
            return;
        };
        let (again, actions) = gather(requests, first, COALESCE);
        for action in actions {
            match action {
                PhoneRequest::Ring(id) => crate::devices::ring(&connection, &id),
                PhoneRequest::Media(id, action) => {
                    crate::devices::media_action(&connection, &id, &action);
                }
                PhoneRequest::Send(id, path) => crate::devices::send_file(&connection, &id, &path),
                PhoneRequest::Reload => {}
            }
        }
        reload = again;
    }
}

/// Collects a burst that started with `first`: whether any of it asked for a
/// reload, and the actions, in order. Actions end the wait at once — a person
/// pressed a button — while reloads keep gathering until `quiet` passes
/// without one.
fn gather(
    requests: &Receiver<PhoneRequest>,
    first: PhoneRequest,
    quiet: Duration,
) -> (bool, Vec<PhoneRequest>) {
    let mut reload = false;
    let mut actions = Vec::new();
    let mut next = Some(first);
    while let Some(request) = next.take() {
        match request {
            PhoneRequest::Reload => {
                reload = true;
                // A quiet spell, or a closed channel, ends the burst.
                next = requests.recv_timeout(quiet).ok();
            }
            action => {
                actions.push(action);
                next = requests.try_recv().ok();
            }
        }
    }
    (reload, actions)
}

#[cfg(test)]
mod tests {
    use super::{gather, PhoneRequest};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_burst_of_changes_is_one_reload() {
        let (sender, receiver) = mpsc::channel();
        for _ in 0..5 {
            sender.send(PhoneRequest::Reload).expect("send");
        }
        let (reload, actions) = gather(&receiver, PhoneRequest::Reload, Duration::from_millis(20));
        assert!(reload);
        assert!(actions.is_empty());
        assert!(receiver.try_recv().is_err(), "the whole burst was taken");
    }

    #[test]
    fn an_action_is_kept_in_order_and_does_not_wait_for_quiet() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(PhoneRequest::Media("a".into(), "Next".into()))
            .expect("send");
        let started = std::time::Instant::now();
        let (reload, actions) = gather(
            &receiver,
            PhoneRequest::Ring("a".into()),
            Duration::from_secs(5),
        );
        assert!(!reload);
        assert_eq!(
            actions,
            vec![
                PhoneRequest::Ring("a".into()),
                PhoneRequest::Media("a".into(), "Next".into())
            ]
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
