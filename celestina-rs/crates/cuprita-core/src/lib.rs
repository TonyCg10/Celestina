//! Cuprita's domain: the networks, Bluetooth devices and audio endpoints the
//! window shows, the three backend traits that read and command them, scripted
//! fakes of those traits, and the pure logic the pages lean on. No Qt here,
//! and no D-Bus in the traits: the NetworkManager client (`nm`, feature `nm`)
//! and the BlueZ client (`bluez`, feature `bluez`) are the modules that speak
//! to the bus, over the mechanics they share in `bus`; the audio client
//! (`wpctl`, feature `wpctl`) drives WirePlumber's command line. The adapter
//! lives in `cuprita/src`.

pub mod agent;
pub mod audio;
pub mod bluetooth;
#[cfg(feature = "bluez")]
pub mod bluez;
#[cfg(any(feature = "nm", feature = "bluez"))]
mod bus;
pub mod error;
pub mod fake;
pub mod model;
pub mod network;
#[cfg(feature = "nm")]
pub mod nm;
pub mod order;
pub mod volume;
#[cfg(feature = "wpctl")]
pub mod wpctl;
