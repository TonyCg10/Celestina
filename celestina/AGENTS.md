# Celestina — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
the shell's constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md),
  and [VALIDATION.md](VALIDATION.md)
- [The shell design](../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md),
  §1–§6
- [Visual design](../celestina-style/DESIGN.md)
- [The landing contract](../docs/contracts/landing.md)
- [The decision records](docs/decisions/README.md)

## Local boundary

- The crate layout is the design's §6.1: `crates/celestina-wayland`,
  `crates/celestina-paint` and `crates/celestina-ui` are the toolkit and know
  nothing about the Island or notifications; `src/` is the product; the pure
  domain is `celestina-rs/crates/celestina-shell`, which knows nothing about
  Wayland, GL or D-Bus. Dependencies point from the product down, never back.
- `unsafe` lives only in `crates/celestina-wayland/src/egl.rs` and, since the
  session lock (CEL-2-H, design §4.11, H-3), in
  `crates/celestina-lock-verify/src/pam.rs`: the hand-written `libpam`
  declarations, the conversation callback and the `malloc`'d responses PAM
  frees. Each documents its invariants at every block, and every other module
  forbids `unsafe` (the verifier's crate root cannot say `forbid` over a module
  that allows, so `crates/celestina-lock-verify/tests/source_guards.rs` keeps
  it so).
- The halted Qt shell (`celestina-qt/`, `celestina-rs/crates/celestina-shell-core`)
  was removed from the tree. Code it salvaged keeps the source path and commit
  in its head comment, as the root contract's "Halted projects" section
  records; nothing more is recovered from its Git history.
- Product copy is Spanish and lives in `src/copy.rs`, whose head says
  `language-contract: product-copy`; the session lock's own screen keeps its
  copy in `crates/celestina-lock/src/copy.rs` under the same head; everything
  else is English.

## Local verification

- `celestina/scripts/build-production.sh`
- `celestina/scripts/verify-production.sh`
- `celestina/scripts/status-production.sh`
- `celestina/scripts/complete-production.sh` for deployable bug and milestone
  exits
- `celestina/scripts/deploy-production.sh`
- `celestina/scripts/smoke.sh --binary PATH`
- `celestina/scripts/lock-rehearsal.sh` (the session lock's steps R-1 to R-9;
  nested niri, a fake logind and no PAM service only, never the live session;
  it stops processes by recorded pid, never by name)

Verification exercises the canonical release artifact without touching the
installed binary; completion updates it without starting or replacing the live
shell. `activate-production.sh` runs only on the author's explicit request.
