//! The pure domain of the Celestina shell (`celestina/`): the notifications
//! state machine, launcher ranking, the workspace model, coalescing and the
//! shell's durable settings, as plain Rust types the product consumes in
//! process (design §6.2,
//! `docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`). It
//! knows nothing about Wayland, GL or D-Bus. CEL-1-B copies its first modules
//! from `celestina-shell-core` under the root contract's salvage rule.
