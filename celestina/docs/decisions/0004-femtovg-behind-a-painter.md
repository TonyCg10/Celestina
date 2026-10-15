# ADR 0004: femtovg behind a Painter contract

- **Date:** 2026-10-14
- **Status:** accepted

## Context

The shell paints rounded glass, text, icons and images at fractional scale.
femtovg over OpenGL ES 3 through EGL drew the spike's Island within budget; a
wgpu painter over Vulkan would need an own SDF painter of 1 500 to 2 500 lines
with no need while niri blurs. The EGL config needs a stencil buffer or
femtovg paints square corners. See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §2, §4 and §5.3.

## Decision

`celestina-paint` exposes a concrete `Painter` with a small surface
(`begin`, `end`, rounded fills and strokes, `haze`, images, glyph runs,
icons, clips, state, opacity, scale) over femtovg. The backend is a private
module, so a wgpu backend later is one module and no API change.

## Consequences

- One `Painter` per process; the GL context is made current per surface.
- The EGL config always requests an 8-bit stencil buffer.

## Revisit when

femtovg cannot hold the frame budget at 4K@165 Hz, or the shell needs a
material the compositor's blur cannot supply.
