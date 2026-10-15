# ADR 0005: cosmic-text is the only text stack

- **Date:** 2026-10-14
- **Status:** accepted

## Context

femtovg's own text layout has no tabular figures, a fixed weight and no font
fallback; the shell needs all three and an editor for the launcher's field.
Two text stacks would measure and paint differently. See
[the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §2 and §5.3.

## Decision

Every piece of text is shaped, laid out and edited by cosmic-text: Inter
Variable from `celestina-style/fonts` with the system's fonts for fallback,
glyphs rasterised by swash into an atlas the painter draws as textured quads.
Layout measurement uses the same buffer that is painted.

## Consequences

- The glyph atlas is bounded (one 1024x1024 page, the least recently used
  glyph evicted when it is full) and measured in CEL-1-B's evidence.
- Tabular figures and weight are `TextStyle` fields, not separate fonts.

## Revisit when

The atlas or the IME path costs more than the design's budget in CEL-1-B or
CEL-1-E.
