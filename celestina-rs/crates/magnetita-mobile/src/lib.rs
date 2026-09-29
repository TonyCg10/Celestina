#![forbid(unsafe_code)]

//! The phone side of Magnetita's own protocol — the crate the Android
//! application and the headless peer both link.
//!
//! What a phone does that a desktop does not: it is dialled by nobody it
//! has not scanned, it keeps one desktop pinned per directory, it pairs by
//! scanning, and it reports what only it knows. All of that is [`Phone`]
//! and [`PhoneSession`], plain Rust; the `mobile` module wraps them for
//! UniFFI so Kotlin gets one object per Rust object and no rule of its own.
//! What the desktop sends reaches Kotlin as one typed [`signal::DesktopSignal`];
//! the rules Kotlin needs before a session exists (the pairing preview, the
//! discovery ranking and the reconnection schedule) are `pairing` and
//! `discovery`. There is no Kotlin logic to keep in step: the bindings are
//! generated from this crate at build time.

pub mod discovery;
pub mod mobile;
pub mod pairing;
pub mod phone;
pub mod signal;
pub mod storage;

pub use phone::{Phone, PhoneSession};

uniffi::setup_scaffolding!();
