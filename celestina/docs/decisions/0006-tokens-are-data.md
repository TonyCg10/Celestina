# ADR 0006: Tokens are data

- **Date:** 2026-10-14
- **Status:** accepted

## Context

`CelestinaTheme.qml` holds about 400 token values as QML literals and no data
file exists. A Rust shell beside QML applications would drift from them
unless both read one source. See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §5.7.

## Decision

`celestina-style/tokens.toml` is the single source of every token. A
standard-library Python generator writes the QML singleton's data and the
`celestina-tokens` crate; both outputs are committed, and both verify entries
regenerate them into a temporary directory and fail on any difference.

## Consequences

- CEL-1-B uses local constants until CEL-1-C delivers the data file, the
  generator and the drift check; CEL-1-C deletes the constants.
- Ink and Light schemes are later tables of the same keys, with their own
  decision record and measured contrast.

## Revisit when

A token cannot be expressed as data both consumers read, or the generated
outputs become a review burden larger than the drift they prevent.
