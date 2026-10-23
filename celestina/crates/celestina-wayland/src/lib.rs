//! The Wayland toolkit of the suite, in two profiles.
//!
//! The **shell profile** (feature `gl`, on by default) is the runtime of the
//! shell: outputs, layer surfaces, `xdg_popup` children, fractional scale,
//! blur and input regions, frame pacing and EGL, as the design specifies in
//! §5.2 (`docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`).
//!
//! The **lock profile** ([`lock`]) is the session-lock client of
//! `celestina-lock` (CEL-2-H design §4.13): a black cover on every output, the
//! keyboard and pointer, and one non-cloneable capability that ends the lock.
//! It needs no GPU, so it is what remains with `gl` off; the process that can
//! end the session lock links no EGL, GL, popup or clipboard code.
//!
//! `unsafe` is allowed in [`egl`] only; this root denies it and every other
//! module forbids it outright.
#![deny(unsafe_code)]

#[cfg(feature = "gl")]
pub mod blur;
#[cfg(feature = "gl")]
mod data_device;
#[cfg(feature = "gl")]
pub mod egl;
pub mod geometry;
#[cfg(feature = "gl")]
mod handlers;
mod ids;
pub mod input;
pub mod lock;
pub mod metrics;
mod output;
#[cfg(feature = "gl")]
pub mod popup;
#[cfg(feature = "gl")]
pub mod runtime;
mod sender;
#[cfg(feature = "gl")]
pub mod surface;
#[cfg(feature = "gl")]
mod text_input;

pub use geometry::{PixelRect, Rect};
pub use ids::{CloseReason, PopupId, SurfaceId};
pub use input::{CursorShape, InputEvent, Modifiers};
pub use output::{OutputId, OutputInfo};
#[cfg(feature = "gl")]
pub use popup::{AnchorEdge, Gravity, Placement, PopupAction};
#[cfg(feature = "gl")]
pub use runtime::{
    run, Anchoring, Config, FrameCx, KeyboardInteractivity, Layer, LayerSpec, OutputEvent,
    PopupShell, RunReport, Runtime, RuntimeCtl, RuntimeError, Shell, SurfaceCtl,
};
pub use sender::{RuntimeGone, Sender};
