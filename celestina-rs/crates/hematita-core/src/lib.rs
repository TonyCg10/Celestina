//! Hematita's domain: what the kernel reports, as typed values.
//!
//! Every function here takes the text a caller read from `/proc` or `/sys`
//! and answers a value or an error naming what was unreadable. Nothing here
//! opens a file, spawns a thread or knows what a percentage should look like
//! on screen; that is the application's business.

pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod history;
pub mod memory;
pub mod network;
pub mod passwd;
pub mod process;
pub mod process_view;
pub mod rate;
pub mod ratio;
pub mod sensors;
pub mod services;
