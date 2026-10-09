//! The plain values every layer exchanges: one snapshot per section.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    Ethernet,
    Wifi,
    Vpn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Open,
    Psk,
    Enterprise,
    /// WEP only: shown so the person sees why Cuprita will not join it.
    Wep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    /// Stable across snapshots: the connection or access-point identity.
    pub id: String,
    pub kind: NetworkKind,
    pub name: String,
    pub state: NetworkState,
    /// Wi-Fi strength, 0..=100; `None` for wired and VPN links.
    pub signal: Option<u8>,
    pub security: Security,
    /// A saved connection exists for it.
    pub known: bool,
    /// The device's first IPv4 address while this link is connected;
    /// `None` otherwise and for VPNs.
    pub address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkSnapshot {
    pub wifi_enabled: bool,
    pub airplane: bool,
    pub networks: Vec<Network>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Audio,
    Input,
    Phone,
    Computer,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub kind: DeviceKind,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    /// Battery percentage when the device reports one.
    pub battery: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothSnapshot {
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<BluetoothDevice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    Sink,
    Source,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioEndpoint {
    pub id: u32,
    pub kind: EndpointKind,
    pub name: String,
    pub description: String,
    /// Linear, 0.0..=`volume::VOLUME_MAX`.
    pub volume: f32,
    pub muted: bool,
    pub default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub volume: f32,
    pub muted: bool,
    /// The endpoint the stream plays to or records from.
    pub endpoint: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardProfile {
    pub card_id: u32,
    pub id: String,
    pub description: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioSnapshot {
    pub endpoints: Vec<AudioEndpoint>,
    pub streams: Vec<AudioStream>,
    pub profiles: Vec<CardProfile>,
}
