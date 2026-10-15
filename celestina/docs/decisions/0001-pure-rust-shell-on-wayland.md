# ADR 0001: A pure Rust shell on Wayland, for niri only

- **Date:** 2026-10-14
- **Status:** accepted

## Context

The Qt/QML shell was halted on 2026-09-27: Qt/QML could not reach the Rust
domain in process, and the layer-shell surfaces fought the toolkit. The study
of 2026-10-14 compared a C++ toolkit of the suite's own with Rust over mature
crates and recommended Rust; the spike of the same day measured it on the
author's machine (frame time p50 0.12 ms at 3840x2160@165 Hz and scale 1.5,
zero frames in rest, 73 MB resident with two outputs, an `xdg_popup` with its
own blur and grab over a layer surface). See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §1 and §4.

## Decision

The shell is one Rust program on Wayland, for niri only, with a small toolkit
of its own: `celestina-wayland` over smithay-client-toolkit, `celestina-paint`
over femtovg and cosmic-text, and `celestina-ui` over taffy. The compositor
supplies the glass through `ext-background-effect-v1`; the shell never blurs.
No other compositor is supported.

## Consequences

- The toolkit's scope is fixed by the design's §5.4; anything a later delivery
  needs that is not a node kind, an animation or a field is a product concern.
- `unsafe` is confined to `crates/celestina-wayland/src/egl.rs` (the EGL and
  GL loader calls), the one exception the author approved in the design.
- A compositor other than niri is out of scope for good.

## Revisit when

The toolkit cannot meet the design's frame, memory or interaction targets on
the author's session (CEL-1-H's live rows), or GPUI pinned to a commit, the
plan B the study named, becomes the cheaper path.
