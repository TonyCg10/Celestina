# CUP-2 — Ungate the pure wpctl status parser

- **Opened:** 2026-11-25
- **Plan ID:** wpctl-status
- **Status:** active
- **Authorization:** on 2026-11-25 the author approved the CEL-I-004 audio
  design, which moves Cuprita's pure `wpctl status` parser out of the `wpctl`
  feature of `cuprita-core` so the shell can reuse it
- **Scope:** cuprita
- **Implementation checkpoint:** CUP-2
- **Author-validation checkpoint:** none

## Hypothesis

The parser of `wpctl status` is pure text in, model out, yet it sits behind
the `wpctl` feature with the process code. A shell that needs the same parser
must either enable a feature that spawns processes or copy the parser. Moving
the pure part into its own module, outside the feature, lets both read one
parser with no behaviour change.

## Tangible outcome

- `cuprita_core::wpctl_status::{Card, Status, parse_status, parse_volume}`
  available without any feature, with unchanged signatures, and exposes which
  streams are playing, from the `[active]` port lines.
- `cuprita_core::wpctl` re-exports the same names, so Cuprita's callers and
  `examples/snapshot.rs` compile unchanged.

## Scope

- `CUP-2-A`: move `Card`, `Status`, `parse_status`, `parse_volume` and their
  private helpers (`Section`, `untree`, `numbered`, `volume_suffix`,
  `strip_api_tag`) and their unit tests from `wpctl.rs` into
  `wpctl_status.rs`; declare the module without a `cfg` in `lib.rs`, and
  expose which streams are playing, from the `[active]` port lines.

## Exclusions

- Any behaviour change. `parse_links`, `stream_endpoints`, `parse_inspect`,
  `parse_profiles` and the process code stay in `wpctl.rs`.
- The shell's own use of the parser (a later shell unit).

## Build order

1. `CUP-2-A`.

## Implementation exit

`cargo test -p cuprita-core` passes with default features and with
`--no-default-features --features nm,bluez`; clippy with `-D warnings` passes
in both; `cargo fmt --all -- --check` and the three repository guards pass;
`cargo check` in `cuprita/` compiles. Evidence:
[wpctl-status](../../evidence/2026-11-25-wpctl-status.md).

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CUP-WPCTL-STATUS | `cuprita:` | done | [inventory](../../inventories/2026-11-25-wpctl-status/CUP-WPCTL-STATUS.numstat.tsv) | 9 files, +455/-238 | Move the pure wpctl status parser out of the `wpctl` feature of `cuprita-core`, with its tests, and exposes which streams are playing, from the `[active]` port lines | [evidence](../../evidence/2026-11-25-wpctl-status.md) | None |

This plan records intent; it grants no authority.
