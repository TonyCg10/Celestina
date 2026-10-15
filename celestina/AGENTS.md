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
- `unsafe` lives only in `crates/celestina-wayland/src/egl.rs`, with the
  invariants documented at each call; every other module forbids it.
- Never edit `celestina-qt/` or `celestina-rs/crates/celestina-shell-core`.
  Copy from them only what the root contract's "Halted projects" section
  names, recording the source path and commit in the copy's head comment.
- Product copy is Spanish and lives in `src/copy.rs`, whose head says
  `language-contract: product-copy`; everything else is English.

## Local verification

- `celestina/scripts/build-production.sh`
- `celestina/scripts/verify-production.sh`
- `celestina/scripts/status-production.sh`
- `celestina/scripts/complete-production.sh` for deployable bug and milestone
  exits
- `celestina/scripts/deploy-production.sh`
- `celestina/scripts/smoke.sh --binary PATH`

Verification exercises the canonical release artifact without touching the
installed binary; completion updates it without starting or replacing the live
shell. `activate-production.sh` runs only on the author's explicit request.
