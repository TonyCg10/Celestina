//! The Wayland runtime of the shell: outputs, layer surfaces, `xdg_popup`
//! children, fractional scale, blur and input regions, frame pacing and EGL,
//! as the design specifies in §5.2
//! (`docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`).
//!
//! `unsafe` is allowed in [`egl`] only; this root denies it and every other
//! module forbids it outright.
#![deny(unsafe_code)]

pub mod blur;
mod data_device;
pub mod egl;
pub mod geometry;
mod handlers;
pub mod input;
pub mod metrics;
pub mod popup;
pub mod runtime;
pub mod surface;
mod text_input;

pub use geometry::{PixelRect, Rect};
pub use input::{CursorShape, InputEvent, Modifiers};
pub use popup::{AnchorEdge, CloseReason, Gravity, Placement, PopupAction, PopupId};
pub use runtime::{
    run, Anchoring, Config, FrameCx, KeyboardInteractivity, Layer, LayerSpec, OutputEvent,
    OutputId, OutputInfo, PopupShell, RunReport, Runtime, RuntimeCtl, RuntimeError, RuntimeGone,
    Sender, Shell, SurfaceCtl, SurfaceId,
};
