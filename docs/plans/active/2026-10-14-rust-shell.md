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
- `CEL-1-A3` — the landing preflight's inventory check: the inventories the
  rename moves unchanged are not edits of tracked inventories; dormant until
  `CEL-1-A`.
- `CEL-1-A4` — the landing's `unbump` step: a renamed halted owner's version
  sources are read at their new paths against the former owner's at the fork
  point; dormant until `CEL-1-A`.
- `CEL-1-A` — the author's rename and registration.
- `CEL-1-C` — `celestina-style/tokens.toml`, the generator, both outputs and the drift checks.
- `RSH-1-DOCS` — the root roadmap's boxes and the root status brought up to date after CEL-1 landed.
- `CEL-2-OPEN` — opens the CEL-2 parity program: the salvage list for CEL-2-B and CEL-2-E, the CEL-2 plan, and the move of the CEL-1 plan to the archive.
- `CEL-2-SALV` — extends the salvage list of `AGENTS.md` "Halted projects" for the remaining CEL-2 units (C, D, F, G, H, I, J, K, L).

## Exclusions

- The shell's own units (`CEL-1-B`, `-D` to `-H`), which belong to
  `celestina/docs/plans/archive/2026-10-14-cel-1-island.md`.
- Any edit of `celestina-qt/` or `celestina-rs/crates/celestina-shell-core`.

## Build order

1. `CEL-1-A0`, then `CEL-1-A1`, then `CEL-1-A2`, then `CEL-1-A3`, then `CEL-1-A4`, then `CEL-1-A` (the author), then `CEL-1-C` after `CEL-1-B`, then `RSH-1-DOCS` after CEL-1 landed, then `CEL-2-OPEN`, then `CEL-2-SALV`.

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
| CEL-1-A3 | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A3.numstat.tsv) | 8 files, +268/-4 | Allow the landing to accept a halted project's inventories moved with its renamed tree | [evidence](../../evidence/2026-10-15-landing-moved-inventories.md) | None |
| CEL-1-A4 | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A4.numstat.tsv) | 8 files, +347/-26 | Allow the landing to unbump a renamed halted owner at its new paths | [evidence](../../evidence/2026-10-15-landing-unbump-renamed-owner.md) | None |
| CEL-1-A | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-A.numstat.tsv) | 1019 files, +100484/-97065 | Move the halted shell to celestina-qt and register the Rust shell | [evidence](../../evidence/2026-10-14-shell-rename.md) | None |
| CEL-1-C | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-1-C.numstat.tsv) | 55 files, +4590/-545 | Add the style's tokens as one data file generated into QML and Rust under a drift check, and read them in the shell | [evidence](../../evidence/2026-10-14-tokens-as-data.md) | None |
| RSH-1-DOCS | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/RSH-1-DOCS.numstat.tsv) | 5 files, +101/-12 | Mark the root roadmap's boxes for the landed units and bring the root status up to date after CEL-1 | [evidence](../../evidence/2026-10-16-rsh-1-docs.md) | None |
| CEL-2-OPEN | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-2-OPEN.numstat.tsv) | 14 files, +293/-98 | Open the Celestina parity program: extend the salvage list for CEL-2-B and CEL-2-E, add the CEL-2 plan and move the CEL-1 plan to the archive | [evidence](../../evidence/2026-10-16-cel-2-open.md) | None |
| CEL-2-SALV | `suite:` | done | [inventory](../../inventories/2026-10-14-rust-shell/CEL-2-SALV.numstat.tsv) | 4 files, +89/-3 | Add the salvage list entries for the remaining CEL-2 units | [evidence](../../evidence/2026-10-17-cel-2-salv.md) | None |
