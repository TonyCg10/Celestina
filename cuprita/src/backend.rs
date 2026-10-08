//! Which backends the controllers drive: the scripted fakes when
//! `CUPRITA_FAKE=1`, the real clients otherwise. The network is
//! NetworkManager's and Bluetooth is BlueZ's (with Cuprita's pairing agent);
//! audio is PipeWire's, through WirePlumber's `wpctl`. A section whose
//! service cannot be reached answers `Unavailable` to everything.

use cuprita_core::audio::Audio;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use cuprita_core::agent::{AgentAnswer, AgentRequest};
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::bluez::{self, BluezBluetooth};
use cuprita_core::error::{AudioError, BluetoothError, NetworkError};
use cuprita_core::fake::{FakeAudio, FakeBluetooth, FakeNetwork};
use cuprita_core::model::{AudioSnapshot, BluetoothSnapshot, NetworkSnapshot};
use cuprita_core::network::Network;
use cuprita_core::nm::{self, NmNetwork};
use cuprita_core::wpctl::{self, WpctlAudio};

use crate::controller::Keepalive;

fn fake() -> bool {
    std::env::var_os("CUPRITA_FAKE").is_some_and(|value| value == "1")
}

/// The network backend and the watcher that calls `on_change` when
/// NetworkManager reports a change; `on_notice` receives what the client has
/// to say outside a command (a join that failed after its call returned).
/// Blocking: call it on the worker. The fake has no watcher; it only changes
/// through commands, which re-read anyway.
pub fn network(
    on_change: impl Fn() + Send + 'static,
    on_notice: impl Fn(String) + Send + Sync + 'static,
) -> (Box<dyn Network>, Option<Keepalive>) {
    if fake() {
        return (Box::new(FakeNetwork::scripted()), None);
    }
    match NmNetwork::connect_system() {
        Ok(mut client) => {
            client.set_notify(on_notice);
            // Without the watcher the page still updates after each command.
            let watcher = nm::watch(on_change)
                .ok()
                .map(|handle| Box::new(handle) as Keepalive);
            (Box::new(client), watcher)
        }
        Err(_) => (Box::new(Offline), None),
    }
}

/// The Bluetooth backend and the watcher that calls `on_change` when BlueZ
/// reports a change. Blocking: call it on the worker. The fake has no
/// watcher; it only changes through commands, which re-read anyway.
pub fn bluetooth(on_change: impl Fn() + Send + 'static) -> (Box<dyn Bluetooth>, Option<Keepalive>) {
    if fake() {
        return (Box::new(FakeBluetooth::scripted()), None);
    }
    match BluezBluetooth::connect_system() {
        Ok(client) => {
            // Without the watcher the page still updates after each command.
            let watcher = bluez::watch(on_change)
                .ok()
                .map(|handle| Box::new(handle) as Keepalive);
            (Box::new(client), watcher)
        }
        Err(_) => (Box::new(Offline), None),
    }
}

/// Abandons the pairing under way with `address` on a thread of its own (the
/// Bluetooth worker is blocked inside it). Best effort: a failure is logged.
/// Nothing to abandon under the fakes.
pub fn cancel_pairing(address: String) {
    if fake() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("cuprita-cancel-pairing".to_owned())
        .spawn(move || {
            if let Err(error) = bluez::cancel_pairing(&address) {
                eprintln!("Cuprita: the pairing with {address} could not be cancelled: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("Cuprita: the pairing could not be cancelled: {error}");
    }
}

/// The pairing agent, exported and registered with BlueZ, or `None` under
/// the fakes or when BlueZ refuses it (pairing then fails with BlueZ's own
/// error instead of asking). Blocking: call it off the Qt thread.
pub fn agent(requests: Sender<AgentRequest>, answers: Receiver<AgentAnswer>) -> Option<Keepalive> {
    if fake() {
        return None;
    }
    match bluez::serve_agent(requests, answers) {
        Ok(handle) => Some(Box::new(handle)),
        Err(error) => {
            eprintln!("Cuprita: the pairing agent could not be registered: {error}");
            None
        }
    }
}

/// How often a missing PipeWire session is looked for again.
const AUDIO_RETRY: Duration = Duration::from_secs(2);

/// The audio backend and the watcher that calls `on_change` when the
/// PipeWire graph changes. Blocking: call it on the worker. The fake has no
/// watcher; it only changes through commands, which re-read anyway. When
/// PipeWire is not there yet, the backend answers `Unavailable` and a thread
/// looks for it every 2 s; once found it asks for a re-read, and that read
/// connects the real client and its watcher, without a restart.
pub fn audio(on_change: impl Fn() + Send + Sync + 'static) -> (Box<dyn Audio>, Option<Keepalive>) {
    if fake() {
        return (Box::new(FakeAudio::scripted()), None);
    }
    let on_change: Arc<dyn Fn() + Send + Sync> = Arc::new(on_change);
    let watcher_change = Arc::clone(&on_change);
    let mut backend = Reconnecting::new(move || {
        let client = WpctlAudio::connect_session()?;
        let change = Arc::clone(&watcher_change);
        // Without the watcher the page still updates after each command.
        let watcher = wpctl::watch(move || change())
            .ok()
            .map(|handle| Box::new(handle) as Keepalive);
        Ok((Box::new(client) as Box<dyn Audio>, watcher))
    });
    if !backend.connect() {
        let spawned = std::thread::Builder::new()
            .name("cuprita-audio-retry".to_owned())
            .spawn(move || loop {
                std::thread::sleep(AUDIO_RETRY);
                if WpctlAudio::connect_session().is_ok() {
                    on_change();
                    break;
                }
            });
        if let Err(error) = spawned {
            eprintln!("Cuprita: PipeWire will not be looked for again: {error}");
        }
    }
    (Box::new(backend), None)
}

/// Ends what outlives the window: the audio watcher's `pw-mon`.
pub fn shutdown() {
    if !fake() {
        wpctl::stop_monitors();
    }
}

type Connect = Box<dyn FnMut() -> Result<(Box<dyn Audio>, Option<Keepalive>), AudioError> + Send>;

/// An audio backend that connects on demand: until `connect` succeeds every
/// call answers `Unavailable`, and each snapshot tries again.
pub struct Reconnecting {
    connect: Connect,
    live: Option<(Box<dyn Audio>, Option<Keepalive>)>,
}

impl Reconnecting {
    pub fn new(
        connect: impl FnMut() -> Result<(Box<dyn Audio>, Option<Keepalive>), AudioError>
            + Send
            + 'static,
    ) -> Self {
        Self {
            connect: Box::new(connect),
            live: None,
        }
    }

    /// Connects if not yet connected; `true` once connected.
    pub fn connect(&mut self) -> bool {
        if self.live.is_none() {
            self.live = (self.connect)().ok();
        }
        self.live.is_some()
    }

    fn client(&mut self) -> Result<&mut Box<dyn Audio>, AudioError> {
        self.live
            .as_mut()
            .map(|(client, _)| client)
            .ok_or(AudioError::Unavailable)
    }
}

impl Audio for Reconnecting {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError> {
        self.connect();
        self.client()?.snapshot()
    }
    fn set_default(&mut self, id: u32) -> Result<(), AudioError> {
        self.client()?.set_default(id)
    }
    fn set_volume(&mut self, id: u32, volume: f32) -> Result<(), AudioError> {
        self.client()?.set_volume(id, volume)
    }
    fn set_muted(&mut self, id: u32, muted: bool) -> Result<(), AudioError> {
        self.client()?.set_muted(id, muted)
    }
    fn set_profile(&mut self, card_id: u32, profile: &str) -> Result<(), AudioError> {
        self.client()?.set_profile(card_id, profile)
    }
}

/// The stand-in for a service that cannot be reached.
struct Offline;

impl Network for Offline {
    fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn set_wifi_enabled(&mut self, _on: bool) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn set_airplane(&mut self, _on: bool) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn connect(&mut self, _id: &str, _password: Option<&str>) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn disconnect(&mut self, _id: &str) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn forget(&mut self, _id: &str) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
    fn set_vpn_active(&mut self, _id: &str, _on: bool) -> Result<(), NetworkError> {
        Err(NetworkError::Unavailable)
    }
}

impl Bluetooth for Offline {
    fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn set_powered(&mut self, _on: bool) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn set_discovering(&mut self, _on: bool) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn pair(&mut self, _address: &str) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn connect(&mut self, _address: &str) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn disconnect(&mut self, _address: &str) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
    fn forget(&mut self, _address: &str) -> Result<(), BluetoothError> {
        Err(BluetoothError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::Reconnecting;
    use cuprita_core::audio::Audio;
    use cuprita_core::error::AudioError;
    use cuprita_core::fake::FakeAudio;

    #[test]
    fn audio_connects_once_pipewire_appears() {
        let mut attempts = 0;
        let mut backend = Reconnecting::new(move || {
            attempts += 1;
            if attempts < 3 {
                Err(AudioError::Unavailable)
            } else {
                Ok((Box::new(FakeAudio::scripted()) as Box<dyn Audio>, None))
            }
        });
        assert!(!backend.connect());
        assert_eq!(backend.snapshot(), Err(AudioError::Unavailable));
        assert_eq!(backend.set_muted(40, true), Err(AudioError::Unavailable));
        assert_eq!(backend.snapshot().map(|s| s.endpoints.len()), Ok(3));
        assert_eq!(backend.set_muted(40, true), Ok(()));
    }
}
