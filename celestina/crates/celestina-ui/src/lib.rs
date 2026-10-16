//! The UI tree of the shell: nodes laid out by taffy, keyed reconciliation,
//! `Animated` values on the One UI curve, hit-test and focus, as the design
//! specifies in §5.4
//! (`docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`).
//!
//! A product describes its surface as a [`View`] of keyed nodes, a pure
//! function of its state, and hands it to [`Tree::apply`] every time the
//! state changes. The tree matches the new view against the nodes it holds
//! by key, so a node whose key survives keeps its hover, press, focus and
//! running animations; inserted nodes fade in, removed ones fade out, moved
//! ones slide from where they were. [`Interaction`] turns pointer and key
//! events into [`UiEvent`]s naming keys. The text field (`Field`) comes with
//! CEL-1-E.
#![forbid(unsafe_code)]

mod anim;
mod curve;
mod field;
mod highlight;
mod input;
mod key;
mod layout;
mod paint;
mod reconcile;
#[cfg(test)]
mod testing;
mod tree;
pub mod view;

pub use anim::{Animated, Reduced};
pub use curve::{CubicBezier, Motion, EXIT, LINEAR, ONE_UI, SINE_IN_OUT_80, SINE_IN_OUT_90};
pub use field::{EditOp, Field, FieldId, MAX_CHARS};
pub use highlight::{focus_ring, Highlight, FOCUS_WIDTH};
pub use input::{Interaction, UiEvent, BTN_LEFT};
pub use key::Key;
pub use tree::{NodeId, Tree, TreeError};
pub use view::{View, ViewKind};
