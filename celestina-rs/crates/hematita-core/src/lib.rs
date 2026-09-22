//! Hematita's domain: what the kernel reports, as typed values.
//!
//! Every function here takes the text a caller read from `/proc` or `/sys`
//! and answers a value or an error naming what was unreadable. Nothing here
//! opens a file, spawns a thread or knows what a percentage should look like
//! on screen; that is the application's business.

pub mod cpu;
pub mod history;
pub mod memory;
pub mod ratio;
