//! Cuprita's domain: the networks, Bluetooth devices and audio endpoints the
//! window shows, the three backend traits that read and command them, scripted
//! fakes of those traits, and the pure logic the pages lean on. No Qt here,
//! and no D-Bus in the traits: the NetworkManager client (`nm`, feature `nm`)
//! is the one module that speaks to the bus; the adapter lives in
//! `cuprita/src`.

pub mod agent;
pub mod audio;
pub mod bluetooth;
pub mod error;
pub mod fake;
pub mod model;
pub mod network;
#[cfg(feature = "nm")]
pub mod nm;
pub mod order;
pub mod volume;
