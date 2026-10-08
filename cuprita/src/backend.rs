//! Which backends the controllers drive: the scripted fakes when
//! `CUPRITA_FAKE=1`, the real clients otherwise. Until CUP-1-C..E land the
//! real clients, they answer `Unavailable` to everything.

use cuprita_core::audio::Audio;
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::error::{AudioError, BluetoothError, NetworkError};
use cuprita_core::fake::{FakeAudio, FakeBluetooth, FakeNetwork};
use cuprita_core::model::{AudioSnapshot, BluetoothSnapshot, NetworkSnapshot};
use cuprita_core::network::Network;

fn fake() -> bool {
    std::env::var_os("CUPRITA_FAKE").is_some_and(|value| value == "1")
}

pub fn network() -> Box<dyn Network> {
    if fake() {
        Box::new(FakeNetwork::scripted())
    } else {
        Box::new(Offline)
    }
}

pub fn bluetooth() -> Box<dyn Bluetooth> {
    if fake() {
        Box::new(FakeBluetooth::scripted())
    } else {
        Box::new(Offline)
    }
}

pub fn audio() -> Box<dyn Audio> {
    if fake() {
        Box::new(FakeAudio::scripted())
    } else {
        Box::new(Offline)
    }
}

/// The stand-in for a real client that does not exist yet.
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

impl Audio for Offline {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError> {
        Err(AudioError::Unavailable)
    }
    fn set_default(&mut self, _id: u32) -> Result<(), AudioError> {
        Err(AudioError::Unavailable)
    }
    fn set_volume(&mut self, _id: u32, _volume: f32) -> Result<(), AudioError> {
        Err(AudioError::Unavailable)
    }
    fn set_muted(&mut self, _id: u32, _muted: bool) -> Result<(), AudioError> {
        Err(AudioError::Unavailable)
    }
    fn set_profile(&mut self, _card_id: u32, _profile: &str) -> Result<(), AudioError> {
        Err(AudioError::Unavailable)
    }
}
