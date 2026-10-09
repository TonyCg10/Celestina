# CONV-1 — Suite conventions

- **Opened:** 2026-10-09
- **Plan ID:** suite-conventions
- **Status:** active
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
are an ADR enforced by the architecture contract.

## Scope

- `CONV-1-A` — the shared activation interface in `celestina-core` (feature
  `activation`) and its adoption by Siderita, Grafita, Hematita, Fluorita
  and Cuprita; Siderita's activator opens through the bus first; a scanner
  refuses a bus-name request outside the shared owner.
- `CONV-1-B` — the shared appearance file (`celestina-settings`) with the
  text scale, fed to every window.
- `CONV-1-C` — Cuprita's appearance section (also a row of Cuprita's own
  ledger).
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

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CONV-1-A | `suite:` | done | [inventory](../../inventories/2026-10-09-suite-conventions/CONV-1-A.numstat.tsv) | 70 files, +3328/-554 | Add the shared activation interface (`celestina_core::activation`, feature `activation`: claim first, `org.celestina.Application1` with `Activate()` and `Open(as paths)`, a bounded inbox, `open_in`) and adopt it in Siderita, Grafita, Hematita, Fluorita and Cuprita; remove the three private hand-off copies; Siderita opens Grafita, Fluorita and Hematita through the bus before spawning; add the activation scanner to the architecture contract. | [evidence](../../evidence/2026-10-09-shared-activation.md) | None |
| CONV-1-B | `suite:` | planned | — | — | Add the shared appearance file with the text scale and feed every window from it. | — | None |
| CONV-1-C | `suite:` | planned | — | — | Add Cuprita's appearance section with reduced motion and the text size; delivered under `cuprita:` as row CUP-1-I of Cuprita's own ledger, and closed here by reference. | — | None |
| CONV-1-D | `suite:` | planned | — | — | Add the suite's open-in entries to Siderita and the send action to Magnetita. | — | None |
| CONV-1-E | `suite:` | planned | — | — | Add drag-and-drop from Siderita into Grafita, Fluorita and Magnetita. | — | None |
| CONV-1-F | `suite:` | planned | — | — | Record the conventions as an ADR and enforce them in the architecture contract. | — | None |
