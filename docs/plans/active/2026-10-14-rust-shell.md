# RSH-1 — Register the Celestina shell in pure Rust

- **Opened:** 2026-10-14
- **Plan ID:** rust-shell
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-14 and asked for the implementation plan; the design is
  [the spec](../../superpowers/specs/2026-10-14-celestina-rust-shell-design.md)
  and the task breakdown is
  [the plan](../../superpowers/plans/2026-10-14-celestina-rust-shell.md)
- **Scope:** suite
- **Implementation checkpoint:** RSH-1
- **Author-validation checkpoint:** none

## Hypothesis

The halted Qt shell can move to `celestina-qt/` and the Rust shell take
`celestina/` without rewriting any inventory, history row or halted byte,
once the version contract knows a renamed owner and the staged-units guard
knows a moved inventory.

## Tangible outcome

`celestina-qt/` registered, halted and byte-identical to the old tree;
`celestina/` registered as `rust-shell` with its document set and a stub
binary at 0.1.0; the shared tokens as data consumed by QML and Rust.

## Scope

- `CEL-1-A0` — the hook-runner migration path: `pre-commit` measures a staged
  language scanner migration against HEAD's rows; dormant until `CEL-1-A1`.
- `CEL-1-A1` — the two guard rules, the halted-tree language rule, the rehearsal;
  stacked on `CEL-1-A0`, whose runner judges its scanner migration.
- `CEL-1-A2` — the landing tool's rename awareness: the plans the rename moves
  unchanged, the links that follow them, a new plan with no open row; dormant
  until `CEL-1-A`.
- `CEL-1-A` — the author's rename and registration.
- `CEL-1-C` — `celestina-style/tokens.toml`, the generator, both outputs and the drift checks.

## Exclusions

- The shell's own units (`CEL-1-B`, `-D` to `-H`), which belong to
  `celestina/docs/plans/active/2026-10-14-cel-1-island.md`.
- Any edit of `celestina-qt/` or `celestina-rs/crates/celestina-shell-core`.

## Build order

1. `CEL-1-A0`, then `CEL-1-A1`, then `CEL-1-A2`, then `CEL-1-A` (the author), then `CEL-1-C` after `CEL-1-B`.

## Implementation exit

`CEL-1-C` landed: `celestina-style/scripts/verify-production.sh` and
`celestina-rs/scripts/verify-production.sh` both run the token drift check
and the shell reads `celestina-tokens`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CEL-1-A0 | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A0.numstat.tsv) | 9 files, +359/-11 | Allow a language scanner migration through the pre-commit hook | [evidence](../../evidence/2026-10-15-scanner-migration-hook.md) | None |
| CEL-1-A1 | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A1.numstat.tsv) | 20 files, +1396/-93 | Add the history_owner alias to the version contract, inventory moves under a renamed halted owner, halted trees as history for the language scanner, and rehearse the shell rename | [evidence](../../evidence/2026-10-14-rename-rules.md) | None |
| CEL-1-A2 | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A2.numstat.tsv) | 8 files, +582/-11 | Allow the landing to accept a halted project's plans moved with its renamed tree and the links that follow them, and set aside a new plan with no open row | [evidence](../../evidence/2026-10-15-landing-renamed-owner.md) | None |
| CEL-1-A | `suite:` | planned | `celestina-qt/` (moved), `celestina/` (new), `docs/projects.toml`, `AGENTS.md`, `docs/version-history.tsv`, the references of the evidence's list | — | Move the halted shell to celestina-qt and register the Rust shell | `docs/evidence/2026-10-14-shell-rename.md` | None |
| CEL-1-C | `suite:` | planned | `celestina-style/tokens.toml`, `celestina-style/scripts/gen-tokens.py`, `celestina-style/CelestinaTokenData.qml`, `celestina-style/CelestinaTheme.qml`, `celestina-rs/crates/celestina-tokens`, both verifies, `docs/projects.toml`, `celestina/crates/celestina-paint/src/theme.rs` | — | Generate the style's tokens from one data file into QML and Rust under a drift check, and read them in the shell | `docs/evidence/2026-10-14-tokens-as-data.md` | None |
