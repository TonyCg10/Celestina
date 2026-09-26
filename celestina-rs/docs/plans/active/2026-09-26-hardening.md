# RS-H1 — Shared owners after the monorepo audit

- **Opened:** 2026-09-26
- **Plan ID:** hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 monorepo audit the author asked
  for its whole program to be done; the findings and rulings are in
  [the audit evidence](../../../../docs/evidence/2026-09-26-monorepo-audit.md)
  and the workspace findings in
  [the shared crates record](../../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md)
- **Scope:** celestina-rs
- **Implementation checkpoint:** RS-H1
- **Author-validation checkpoint:** none

## Hypothesis

The recipes the audit found copied across the suite (file-URI parsing, the
runtime directory, private and media writes, bounded state reads, the
`.desktop` scan, a non-blocking cancellation query) can each gain one tested
owner in `celestina-core` without changing any existing function, so every
consumer adopts it later in its own bug unit.

## Tangible outcome

`celestina-core` exports a strict `file_uri` parser, `xdg::runtime_dir()`
with no `/tmp` fallback, private-state and media-landing writers,
`read_bounded`, a truthful post-rename outcome, `desktop_entry::{read, scan}`
and a non-blocking cancellation query, each with tests; no consumer switches
yet; and the workspace README, AGENTS and crate docs match the checkout.

## Scope

- `RS-H1-A` (P-6) — the dormant owners and the documentation corrections.
  Ruling R-A3 keeps it purely additive: RS-10's value unescaping and any change
  to `is_cancelled` land with a consumer's bug unit.

## Exclusions

- `dotfiles-core` retention (RS-11). The crate has no consumer anywhere in the
  monorepo; ruling R-A4 keeps it pending the author's decision, which is to
  remove it with its component scope or to name its consumer and plan.
- The unscheduled Minor findings RS-8 (non-canonical `pathkey` keys, a
  behaviour change that lands with a consumer), RS-12, RS-14, RS-15, RS-16
  and RS-17.
- Any consumer adoption; each lands in the consumer's own plan.

## Build order

1. `RS-H1-A` from `main`, after the pipeline units of the suite plan (P-2).

## Implementation exit

`cargo test -p celestina-core --offline --locked` and
`cargo clippy --all-targets` pass with the new tests. The unit lands as
`celestina-rs-maintenance`; the workspace is not versioned, and the landing
rebuilds and verifies every app that links a changed crate.

## Change and commit ledger

Paths are repository-relative. The row's `Intended change` ends with the
program id and the audit findings it closes.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| RS-H1-A | `celestina-rs:` | planned | `celestina-rs/crates/celestina-core/src/` (`atomic_file.rs`, `xdg.rs`, `percent.rs` or a new `file_uri` module, `desktop_entry.rs`, `lib.rs` cancellation, `image.rs` doc); `celestina-rs/crates/celestina-shell-core/src/lib.rs` (crate doc); `celestina-rs/README.md`; `celestina-rs/AGENTS.md`; `celestina-rs/STATUS.md` | — | Add dormant owners: a strict `file_uri` → `PathBuf` parser, `xdg::runtime_dir()` with no `/tmp` fallback, private-state and media-landing writers, `read_bounded`, a truthful post-rename outcome, `desktop_entry::{read, scan}` (regular file, 64 KiB, one shadowing rule), and a non-blocking cancellation query. Correct the workspace README, AGENTS and crate docs. The unit is purely additive (ruling R-A3): no existing function changes behaviour, so RS-10's value unescaping and any change to `is_cancelled` land in a consumer's bug unit. (P-6: Owner halves of RS-1, RS-2/FLU-2, RS-3, RS-4, RS-6, RS-9, RS-10; RS-7; RS-19) | `cargo test -p celestina-core --offline --locked` (new tests: localhost vs foreign host, `%XX`, NUL, non-UTF-8 bytes; relative runtime dir → `None`; file 0600, dirs 0700, mode preserved, no-replace refuses; FIFO and oversize refused; fsync-failure outcome; shadowing and cap; hermetic dir order; pause-after-cancel); `cargo clippy --all-targets`. No consumer switches, so there are no product bumps. The landing rebuilds every linking app. | None |

This plan records intent; it grants no authority.
