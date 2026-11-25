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
- `celestina/scripts/smoke.sh --binary PATH` (the full smoke: every phase)
- `celestina/scripts/smoke.sh --binary PATH --only ID[,ID...]` (the named
  phases, in the script's order)
- `celestina/scripts/smoke.sh --list` (prints the phase ids)
- `celestina/scripts/lock-rehearsal.sh` (the session lock's steps R-1 to R-9;
  nested niri, a fake logind and no PAM service only, never the live session;
  it stops processes by recorded pid, never by name)

A session runs the smoke phases of the area it touched; the landing always
runs the full smoke. A phase whose rest-frame check is inconclusive is retried
alone, up to three attempts.

A measuring run (`smoke.sh`, `lock-rehearsal.sh`, `wallpaper-backdrop-probe.sh`)
holds the host-wide nest lock (`scripts/nest-run.sh`): a second one waits
`CELESTINA_NEST_LOCK_WAIT` seconds (default 1800), then fails with status 69
naming the holder, and `TERM` ends a smoke together with its nest. Never run
two at once by other means; the counts of both are worthless. A caller that
bounds a smoke uses `TERM` first and a limit that covers the wait plus the run,
for example `timeout -k 60 2700 sh scripts/smoke.sh --binary PATH`, or lowers
`CELESTINA_NEST_LOCK_WAIT`; never `timeout -s KILL`, which cannot be handled
(the run's nest would outlive it). `flock` does not serve waiters in arrival
order, so a waiting run may be passed over until its patience ends.

The fast checks a session runs from `celestina/`, on the toolchain
`celestina/rust-toolchain.toml` pins:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`

Keep `--workspace`: a plain `cargo clippy --all-targets` in `celestina/` skips
the member crates entirely (only the root `celestina` package is checked).
The `celestina` workflow runs the same three checks on the same pinned
toolchain, and the landing stops while that workflow's
latest run on `main` is red (see `docs/contracts/landing.md`, "A red main").

Verification exercises the canonical release artifact without touching the
installed binary; completion updates it without starting or replacing the live
shell. `activate-production.sh` runs only on the author's explicit request.
