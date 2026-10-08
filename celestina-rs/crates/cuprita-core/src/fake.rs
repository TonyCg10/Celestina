//! Scripted backends: deterministic state that every command mutates, so the
//! adapter, the QML pages and the tests run without NetworkManager, BlueZ or
//! PipeWire. `CUPRITA_FAKE=1` makes the binary use them.

use crate::audio::Audio;
use crate::bluetooth::Bluetooth;
use crate::error::{AudioError, BluetoothError, NetworkError};
use crate::model::{
    AudioEndpoint, AudioSnapshot, AudioStream, BluetoothDevice, BluetoothSnapshot, CardProfile,
    DeviceKind, EndpointKind, Network as NetworkEntry, NetworkKind, NetworkSnapshot, NetworkState,
    Security,
};
use crate::network::Network;
use crate::order::order_networks;
use crate::volume::clamp_volume;

#[derive(Debug, Clone)]
pub struct FakeNetwork {
    pub state: NetworkSnapshot,
}

impl FakeNetwork {
    /// A wired link in use, a saved PSK Wi-Fi at 72, an open cafe Wi-Fi at 20
    /// and a saved VPN that is down.
    #[must_use]
    pub fn scripted() -> Self {
        let entry = |id: &str, kind, name: &str, state, signal, security, known| NetworkEntry {
            id: id.to_owned(),
            kind,
            name: name.to_owned(),
            state,
            signal,
            security,
            known,
        };
        Self {
            state: NetworkSnapshot {
                wifi_enabled: true,
                airplane: false,
                networks: vec![
                    entry(
                        "wired",
                        NetworkKind::Ethernet,
                        "Ethernet",
                        NetworkState::Connected,
                        None,
                        Security::Open,
                        true,
                    ),
                    entry(
                        "home-wifi",
                        NetworkKind::Wifi,
                        "Home",
                        NetworkState::Disconnected,
                        Some(72),
                        Security::Psk,
                        true,
                    ),
                    entry(
                        "open-cafe",
                        NetworkKind::Wifi,
                        "Corner Cafe",
                        NetworkState::Disconnected,
                        Some(20),
                        Security::Open,
                        false,
                    ),
                    entry(
                        "office-vpn",
                        NetworkKind::Vpn,
                        "Office",
                        NetworkState::Disconnected,
                        None,
                        Security::Open,
                        true,
                    ),
                ],
            },
        }
    }

    fn find(&mut self, id: &str) -> Result<&mut NetworkEntry, NetworkError> {
        self.state
            .networks
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or_else(|| NetworkError::NotFound(id.to_owned()))
    }
}

impl Network for FakeNetwork {
    fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError> {
        let mut snapshot = self.state.clone();
        if !snapshot.wifi_enabled || snapshot.airplane {
            snapshot.networks.retain(|n| n.kind != NetworkKind::Wifi);
        }
        order_networks(&mut snapshot.networks);
        Ok(snapshot)
    }

    fn set_wifi_enabled(&mut self, on: bool) -> Result<(), NetworkError> {
        self.state.wifi_enabled = on;
        Ok(())
    }

    fn set_airplane(&mut self, on: bool) -> Result<(), NetworkError> {
        self.state.airplane = on;
        Ok(())
    }

    fn connect(&mut self, id: &str, password: Option<&str>) -> Result<(), NetworkError> {
        let network = self.find(id)?;
        if network.security != Security::Open && !network.known && password.is_none() {
            return Err(NetworkError::Denied);
        }
        network.state = NetworkState::Connected;
        network.known = true;
        Ok(())
    }

    fn disconnect(&mut self, id: &str) -> Result<(), NetworkError> {
        self.find(id)?.state = NetworkState::Disconnected;
        Ok(())
    }

    fn forget(&mut self, id: &str) -> Result<(), NetworkError> {
        let network = self.find(id)?;
        network.known = false;
        network.state = NetworkState::Disconnected;
        Ok(())
    }

    fn set_vpn_active(&mut self, id: &str, on: bool) -> Result<(), NetworkError> {
        let network = self.find(id)?;
        if network.kind != NetworkKind::Vpn {
            return Err(NetworkError::NotFound(id.to_owned()));
        }
        network.state = if on {
            NetworkState::Connected
        } else {
            NetworkState::Disconnected
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FakeBluetooth {
    pub state: BluetoothSnapshot,
}

impl FakeBluetooth {
    /// Paired headphones at 80 % battery and a phone that was never paired.
    #[must_use]
    pub fn scripted() -> Self {
        Self {
            state: BluetoothSnapshot {
                powered: true,
                discovering: false,
                devices: vec![
                    BluetoothDevice {
                        address: "AA:BB:CC:00:00:01".to_owned(),
                        name: "Headphones".to_owned(),
                        kind: DeviceKind::Audio,
                        paired: true,
                        connected: false,
                        trusted: true,
                        battery: Some(80),
                    },
                    BluetoothDevice {
                        address: "AA:BB:CC:00:00:02".to_owned(),
                        name: "Phone".to_owned(),
                        kind: DeviceKind::Phone,
                        paired: false,
                        connected: false,
                        trusted: false,
                        battery: None,
                    },
                ],
            },
        }
    }

    fn find(&mut self, address: &str) -> Result<&mut BluetoothDevice, BluetoothError> {
        self.state
            .devices
            .iter_mut()
            .find(|d| d.address == address)
            .ok_or_else(|| BluetoothError::NotFound(address.to_owned()))
    }

    fn powered(&self) -> Result<(), BluetoothError> {
        if self.state.powered {
            Ok(())
        } else {
            Err(BluetoothError::Unavailable)
        }
    }
}

impl Bluetooth for FakeBluetooth {
    fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError> {
        let mut snapshot = self.state.clone();
        if !snapshot.powered {
            snapshot.devices.clear();
        }
        Ok(snapshot)
    }

    fn set_powered(&mut self, on: bool) -> Result<(), BluetoothError> {
        self.state.powered = on;
        if !on {
            self.state.discovering = false;
            for device in &mut self.state.devices {
                device.connected = false;
            }
        }
        Ok(())
    }

    fn set_discovering(&mut self, on: bool) -> Result<(), BluetoothError> {
        self.powered()?;
        self.state.discovering = on;
        Ok(())
    }

    fn pair(&mut self, address: &str) -> Result<(), BluetoothError> {
        self.powered()?;
        let device = self.find(address)?;
        device.paired = true;
        device.trusted = true;
        Ok(())
    }

    fn connect(&mut self, address: &str) -> Result<(), BluetoothError> {
        self.powered()?;
        let device = self.find(address)?;
        if !device.paired {
            return Err(BluetoothError::Failed("not paired".to_owned()));
        }
        device.connected = true;
        Ok(())
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BluetoothError> {
        self.find(address)?.connected = false;
        Ok(())
    }

    fn forget(&mut self, address: &str) -> Result<(), BluetoothError> {
        let before = self.state.devices.len();
        self.state.devices.retain(|d| d.address != address);
        if self.state.devices.len() == before {
            return Err(BluetoothError::NotFound(address.to_owned()));
        }
        Ok(())
    }
}

/// The ids the scripted audio graph uses, so tests name them.
pub mod ids {
    pub const SINK_A: u32 = 40;
    pub const SINK_B: u32 = 41;
    pub const SOURCE: u32 = 50;
    pub const STREAM_MUSIC: u32 = 60;
    pub const STREAM_BROWSER: u32 = 61;
    pub const CARD: u32 = 30;
}

#[derive(Debug, Clone)]
pub struct FakeAudio {
    pub state: AudioSnapshot,
}

impl FakeAudio {
    /// Speakers (default) and HDMI out, one microphone, two applications
    /// playing, and a card with two profiles.
    #[must_use]
    pub fn scripted() -> Self {
        let endpoint = |id, kind, name: &str, description: &str, default| AudioEndpoint {
            id,
            kind,
            name: name.to_owned(),
            description: description.to_owned(),
            volume: 0.6,
            muted: false,
            default,
        };
        let stream = |id, app_name: &str, app_icon: &str| AudioStream {
            id,
            app_name: app_name.to_owned(),
            app_icon: app_icon.to_owned(),
            volume: 1.0,
            muted: false,
            endpoint: ids::SINK_A,
        };
        let profile = |id: &str, description: &str, active| CardProfile {
            card_id: ids::CARD,
            id: id.to_owned(),
            description: description.to_owned(),
            active,
        };
        Self {
            state: AudioSnapshot {
                endpoints: vec![
                    endpoint(
                        ids::SINK_A,
                        EndpointKind::Sink,
                        "alsa_output.analog-stereo",
                        "Speakers",
                        true,
                    ),
                    endpoint(
                        ids::SINK_B,
                        EndpointKind::Sink,
                        "alsa_output.hdmi-stereo",
                        "HDMI",
                        false,
                    ),
                    endpoint(
                        ids::SOURCE,
                        EndpointKind::Source,
                        "alsa_input.analog-stereo",
                        "Microphone",
                        true,
                    ),
                ],
                streams: vec![
                    stream(ids::STREAM_MUSIC, "Music", "audio-x-generic"),
                    stream(ids::STREAM_BROWSER, "Browser", "web-browser"),
                ],
                profiles: vec![
                    profile("output:analog-stereo", "Analog Stereo", true),
                    profile("output:hdmi-stereo", "HDMI Stereo", false),
                ],
            },
        }
    }
}

impl Audio for FakeAudio {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError> {
        Ok(self.state.clone())
    }

    fn set_default(&mut self, id: u32) -> Result<(), AudioError> {
        let kind = self
            .state
            .endpoints
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.kind)
            .ok_or_else(|| AudioError::NotFound(id.to_string()))?;
        for endpoint in &mut self.state.endpoints {
            if endpoint.kind == kind {
                endpoint.default = endpoint.id == id;
            }
        }
        Ok(())
    }

    fn set_volume(&mut self, id: u32, volume: f32) -> Result<(), AudioError> {
        let volume = clamp_volume(volume);
        if let Some(e) = self.state.endpoints.iter_mut().find(|e| e.id == id) {
            e.volume = volume;
        } else if let Some(s) = self.state.streams.iter_mut().find(|s| s.id == id) {
            s.volume = volume;
        } else {
            return Err(AudioError::NotFound(id.to_string()));
        }
        Ok(())
    }

    fn set_muted(&mut self, id: u32, muted: bool) -> Result<(), AudioError> {
        if let Some(e) = self.state.endpoints.iter_mut().find(|e| e.id == id) {
            e.muted = muted;
        } else if let Some(s) = self.state.streams.iter_mut().find(|s| s.id == id) {
            s.muted = muted;
        } else {
            return Err(AudioError::NotFound(id.to_string()));
        }
        Ok(())
    }

    fn set_profile(&mut self, card_id: u32, profile: &str) -> Result<(), AudioError> {
        let card: Vec<_> = self
            .state
            .profiles
            .iter_mut()
            .filter(|p| p.card_id == card_id)
            .collect();
        if !card.iter().any(|p| p.id == profile) {
            return Err(AudioError::NotFound(profile.to_owned()));
        }
        for p in card {
            p.active = p.id == profile;
        }
        Ok(())
    }
}
