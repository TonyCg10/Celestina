# CONV-1 — Suite conventions

- **Opened:** 2026-10-09
- **Plan ID:** suite-conventions
- **Closed:** 2026-10-09
- **Successor:** none; the author named no next suite checkpoint. All 6 ledger rows are done
- **Status:** done
- **Authorization:** the author approved the design in brainstorming on
  2026-10-09 and asked for the program to be implemented; the design is
  [the spec](../../superpowers/specs/2026-10-09-suite-conventions-design.md)
  and the task breakdown is
  [the plan](../../superpowers/plans/2026-10-09-suite-conventions.md)
- **Scope:** suite
- **Implementation checkpoint:** CONV-1
- **Author-validation checkpoint:** none

## Hypothesis

One shared activation interface, one appearance file, open-with inside the
suite and drag-and-drop between the applications make the five first-party
applications behave as one system, each convention with a single owner in
`celestina-rs` and a guard that keeps private copies from coming back.

## Tangible outcome

Every application serves `org.celestina.<App>` through
`celestina_core::activation`; a second launch reaches the running window.
Reduced motion and the text scale come from
`~/.config/celestina/appearance.toml`, edited in Cuprita. Siderita opens the
suite's applications through the bus and offers them in its context menu;
Grafita, Fluorita and Magnetita accept `text/uri-list` drops. The conventions
are an ADR; the architecture contract enforces the activation convention.

## Scope

- `CONV-1-A` — the shared activation interface in `celestina-core` (feature
  `activation`) and its adoption by Siderita, Grafita, Hematita, Fluorita
  and Cuprita; Siderita's activator opens through the bus first; a scanner
  refuses a bus-name request outside the shared owner.
- `CONV-1-B` — the shared appearance file (`celestina-settings`) with the
  text scale, fed to every window.
- `CONV-1-C` — Cuprita's appearance section, delivered as Cuprita's own
  unit `CUP-1-I` (see the ledger note).
- `CONV-1-D` — open-with inside the suite: Siderita's direct targets and
  Magnetita's send action.
- `CONV-1-E` — drag-and-drop from Siderita into Grafita, Fluorita and
  Magnetita.
- `CONV-1-F` — exit: the ADR, the documents and the guard.

## Exclusions

- The halted Celestina shell, its `org.celestina.Shell1` name and its portal
  `Settings` backend.
- A light colour scheme, the accent, a settings daemon and per-application
  settings migration (spec §1).

## Build order

`CONV-1-A` to `CONV-1-F` in order, one `suite` unit per commit except
`CONV-1-C` (`cuprita:`).

## Implementation exit

The checkpoint closes when every row below is `done` and, on the landed
`main`, `bash scripts/check-architecture-contract.sh`,
`sh scripts/check-documentation-contract.sh` and
`python3 scripts/check-language-contract.py` exit 0.

## Change and commit ledger

`CONV-1-C` was delivered as `CUP-1-I` under `cuprita:`, with its row and
[inventory](../../../cuprita/docs/inventories/2026-10-08-cup-1-foundation/CUP-1-I.numstat.tsv)
in [Cuprita's foundation plan](../../../cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md);
it has no row here, because a `done` row needs an inventory of its own.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CONV-1-A | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-A.numstat.tsv) | 70 files, +3328/-554 | Add the shared activation interface (`celestina_core::activation`, feature `activation`: claim first, `org.celestina.Application1` with `Activate()` and `Open(as paths)`, a bounded inbox, `open_in`) and adopt it in Siderita, Grafita, Hematita, Fluorita and Cuprita; remove the three private hand-off copies; Siderita opens Grafita, Fluorita and Hematita through the bus before spawning; add the activation scanner to the architecture contract. | [evidence](../../evidence/2026-10-09-shared-activation.md) | None |
| CONV-1-B | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-B.numstat.tsv) | 62 files, +3013/-106 | Add the shared appearance file with the text scale and feed every window from it. | [evidence](../../evidence/2026-10-09-shared-appearance.md) | None |
| CONV-1-D | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-D.numstat.tsv) | 32 files, +1690/-57 | Add the suite's open-in entries to Siderita and the send action to Magnetita; add `celestina_settings::load_stored` for read-modify-save callers. | [evidence](../../evidence/2026-10-09-open-with.md) | None |
| CONV-1-E | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-E.numstat.tsv) | 22 files, +987/-34 | Add drag-and-drop from Siderita into Grafita, Fluorita and Magnetita. | [evidence](../../evidence/2026-10-09-drag-and-drop.md) | None |
| CONV-1-F | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-F.numstat.tsv) | 20 files, +417/-24 | Record the conventions as ADR 0012, the documents and the per-application checks; Cuprita's appearance save starts from `load_stored()`. | [evidence](../../evidence/2026-10-09-suite-conventions-exit.md) | None |
| CONV-1-G | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-G.numstat.tsv) | 10 files, +178/-97 | Archive the delivered suite conventions plan through its own administrative unit and reconcile the suite roadmap, status and plan indexes | [evidence](../../evidence/2026-10-09-suite-conventions-archive.md) | None |
