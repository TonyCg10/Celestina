//! Cuprita's domain: the networks, Bluetooth devices and audio endpoints the
//! window shows, the three backend traits that read and command them, scripted
//! fakes of those traits, and the pure logic the pages lean on. No Qt and no
//! D-Bus here: the real clients and the adapter live in `cuprita/src`.

pub mod agent;
pub mod audio;
pub mod bluetooth;
pub mod error;
pub mod fake;
pub mod model;
pub mod network;
pub mod order;
pub mod volume;
