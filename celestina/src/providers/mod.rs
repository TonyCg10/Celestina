//! The shell's worker threads. Each provider owns its blocking IO on a named
//! thread started through [`Worker`], publishes [`crate::message::Message`]
//! values into the loop and, when it takes commands, receives them through
//! its own bounded queue (design §5.1).

pub mod desktop;
pub mod niri;
pub mod notifications;
pub mod settings;
pub mod tray;
pub mod wallpaper;
pub mod worker;

pub use worker::Worker;
