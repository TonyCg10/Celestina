# AUD-1 — Monorepo hardening

- **Opened:** 2026-09-26
- **Plan ID:** monorepo-hardening
- **Status:** active
- **Authorization:** after the 2026-09-26 audit the author asked for the
  whole program to be done; the rulings R-A1 to R-A8 that settled its open
  questions are recorded in
  [the audit evidence](../../evidence/2026-09-26-monorepo-audit.md)
- **Scope:** suite
- **Implementation checkpoint:** AUD-1
- **Author-validation checkpoint:** none

## Hypothesis

Restoring the pipeline's enforcement first (green CI, fingerprints that cover
every crate an app links, and a landing that accepts stacked branches) lets
each of the audit's delivery units land as one single-prefix commit with a
trustworthy guard chain, and no project needs a second active checkpoint to
carry its share.

## Tangible outcome

The audit is durable evidence: one consolidated record and seven area records
keep every finding with its original ID. Every scheduled finding has a ledger
row in exactly one plan: the suite rows below, a `<project>-H1` hardening plan
in each idle project, or a new row of the active plan in each busy one. When
the suite rows close, GitHub `contracts` is green on `main`, a fix to any
linked crate stales and rebuilds every app that links it, a dependent unit can
be prepared on a branch stacked on its dependency, the Magnetita protocol has
one owner on both ends, and the hooks judge the index with committed rules.

## Scope

- `AUD-1-A` — record the audit as eight evidence records under
  `docs/evidence/2026-09-26-monorepo-audit*.md` and open the plans that carry
  the program: this plan; `siderita/docs/plans/active/2026-09-26-hardening.md`
  (`SID-H1`), `hematita/docs/plans/active/2026-09-26-hardening.md`
  (`HEM-H1`), `grafita/docs/plans/active/2026-09-26-hardening.md`
  (`GRA-H1`), `fluorita/docs/plans/active/2026-09-26-hardening.md`
  (`FLU-H1`) and `celestina-rs/docs/plans/active/2026-09-26-hardening.md`
  (`RS-H1`), with their roadmaps, statuses and plan indexes; and new rows in
  the active plans of Celestina (`SURF-1-E`, `SURF-1-F`), CelestinaStyle
  (`STYLE-G7-N`), Magnetita (`MAG-D1-D`, `MAG-D1-E`, plus the MAG-25 waiver)
  and Magnetita Android (`AND-6-D`).
- `AUD-1-B` (P-1) — CI green again and the style guard over Hematita.
- `AUD-1-C` (P-0b) — the landing accepts a stacked branch.
- `AUD-1-D` (P-2) — production inputs derived from Cargo, Magnetita
  Android's inputs declared.
- `AUD-1-E` (P-13) — one owner for the Magnetita protocol rules, negotiated
  capabilities on both ends; genuinely cross-suite, so it is a suite row that
  bumps Magnetita and Magnetita Android.
- `AUD-1-F` (P-20) — tooling integrity, speed and governance documents.

The program ids (P-0b, P-1 to P-20), their dependencies and the findings each
closes are in section 4 of
[the audit evidence](../../evidence/2026-09-26-monorepo-audit.md).

## Exclusions

- Everything that needs the author's machine: production builds,
  verification with Qt, libmpv, CMake or the Android SDK, deployment, and the
  landing of every unit except the documentation-only ones. Under ruling
  R-A7 each such unit is prepared on its own `unit/<project>/<unit>` branch,
  stacked on its dependency's branch, and landed by the author with
  `scripts/land-unit.py` in program order.
- `dotfiles-core` retention (RS-11). Ruling R-A4 keeps the crate; its missing
  consumer is recorded as an exclusion of the celestina-rs plan for the
  author's decision.
- The unscheduled Minor backlog listed in section 4 of the audit evidence
  (RS-8, RS-11, RS-12, RS-14, RS-15, RS-16, RS-17, SID-21, SID-22, SID-23,
  HEM-16, GRA-6, MAG-23, SH-16, SH-18, FLU-22); each is taken when its file is
  next touched.
- Pulling TOOL-8 and TOOL-9 forward (question 10 of the audit). No ruling
  settled it; until `AUD-1-F` lands, run
  `sh scripts/check-documentation-contract.sh` locally before each landing.

## Build order

1. `AUD-1-A`, documentation only, lands first.
2. `AUD-1-B` (P-1) and `AUD-1-C` (P-0b) from `main`.
3. `AUD-1-D` (P-2), stacked on `AUD-1-B`.
4. The project units in program order, each on its own branch: `AND-6-D`
   (P-3), `SID-H1-A` (P-4), `GRA-H1-A` (P-5), `RS-H1-A` (P-6), then the units
   that adopt its owners (`FLU-H1-A`, `SID-H1-B`, `HEM-H1-A`, `SURF-1-E`,
   `MAG-D1-D`, `MAG-D1-E`), then `FLU-H1-B`, `SID-H1-C`, `STYLE-G7-N`,
   `SURF-1-F`, `HEM-H1-B` and `GRA-H1-B`.
5. `AUD-1-E` (P-13), stacked on `MAG-D1-E` (P-12) and `AND-6-D` (P-3).
6. `AUD-1-F` (P-20), stacked on `AUD-1-C`.

## Implementation exit

Each row's `Automated evidence` names its own exit. The checkpoint closes
when every suite row is `done` and, on the landed `main`:

```sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
bash scripts/test-land-unit.sh
```

pass and GitHub `contracts` is green. The project rows close with their own
plans; a pending author validation never keeps this checkpoint open.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| AUD-1-A | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-A.numstat.tsv) | 13 files, +3915/-9 | Record the monorepo audit as evidence and open the hardening plans | [evidence](../../evidence/2026-09-26-monorepo-audit.md) | None |
| AUD-1-B | `suite:` | planned | `celestina-style/scripts/check-style-contract.sh`; `scripts/test-version-contract.py`; `scripts/test-architecture-scanners.sh`; `.github/workflows/contracts.yml`; the two token violations under `hematita/qml/` | — | Derive the style-guard roots and the version-owner fixture from `docs/projects.toml`. Fix the two Hematita token violations in the same commit, because a guard coverage change and the violations it reveals must land together or the published revision is red. Add the three missing tests to `contracts.yml`. If the Hematita token fix changes what a person sees, the implementer reports it and the unit lands as `suite-bug` bumping Hematita (ruling R-A6). (P-1: TOOL-1, TOOL-2, TOOL-13) | `bash scripts/test-architecture-scanners.sh`, `python3 scripts/test-version-contract.py`, `bash scripts/check-architecture-contract.sh` (style guard now covers `hematita/qml`), `python3 scripts/test-language-contract.py`, `sh scripts/test-production-artifacts.sh`, `sh scripts/test-production-common.sh`; GitHub `contracts` green on the landed commit | None |
| AUD-1-C | `suite:` | active | `scripts/landing.py` (`discover_unit`, `settled_rows`, `merge_plan`, `other_unit_record`, `SEAL_COLUMNS`); `scripts/land-unit.py` (`resolve_unit`, `hot_merge`); `scripts/test-land-unit.py`; `scripts/worktree.sh` (`open --from`); `scripts/test-worktree.sh`; `docs/contracts/landing.md`; `docs/evidence/2026-09-26-monorepo-hardening-stacked-landing.md` | — | Support stacked unit branches in the landing (P-0b, ruling R-A1) | [evidence](../../evidence/2026-09-26-monorepo-hardening-stacked-landing.md): `bash scripts/test-land-unit.sh` (stacked-branch fixtures whose dependency landed and did not), `sh scripts/test-worktree.sh`, `sh scripts/check-documentation-contract.sh` | None |
| AUD-1-D | `suite:` | planned | `docs/projects.toml` (`production_inputs`, Magnetita Android's inputs); `scripts/production_artifact.py` (fingerprint, toolchain probe); `scripts/agent-context.py`; the empty-inputs guard; fixtures in `scripts/test-architecture-scanners.sh` and `scripts/test-production-artifacts.sh` | — | Fingerprint each app's `cargo metadata` path-package closure, or guard that `production_inputs` contains it. Declare Magnetita Android's inputs. Reject buildable projects with empty inputs. Compare the recorded toolchain. Print consumers in agent context. (P-2: TOOL-3, TOOL-4, MAG-9, FLU-21, TOOL-21, TOOL-22) | New positive and negative fixtures in `test-architecture-scanners.sh` / `test-production-artifacts.sh`; `production_artifact.py check` reports magnetita, grafita, fluorita, hematita and android stale against their old manifests. The landing then rebuilds all of them (needs Qt, libmpv and the Android SDK). | None |
| AUD-1-E | `suite:` | planned | `celestina-rs/crates/magnetita-proto/`; `celestina-rs/crates/magnetita-mobile/src/phone.rs` and `mobile.rs`; `celestina-rs/crates/magnetita-core/src/clipboard.rs`; `celestina-rs/crates/magnetitad/` (negotiated gating); `magnetita/qml/pages/MessagesPage.qml`; `magnetita-android/app/src/main/java/org/celestina/magnetita/link/` (`LinkController.kt`, `DesktopSignal.kt`); `magnetita-android/app/src/main/java/org/celestina/magnetita/phone/Messages.kt`; `magnetita-android/app/src/main/java/org/celestina/magnetita/share/Downloads.kt` | — | Advertise the real capability set, negotiate, and gate on both ends. Export the typed signals, `check_path`, backoff, discovery ranking, bounds and the sanitiser through UniFFI, and delete the Kotlin copies. Use one clipboard bound. Make Messages signal-driven and query threads with a limit. Move the upload to its own coroutine. Suspend on channels. Cap received offers. Add FFI tests. This is genuinely cross-suite: negotiated gating must change daemon, mobile crate and APK atomically, or one side refuses the other. `scripts/land-unit.py` refuses a `suite-bug`, so the author records the two bumps by hand, as the landing contract says. (P-13: MAG-8, MAG-12, MAG-19, MAG-28; AND-2, AND-3, AND-5, AND-7, AND-9) | `cargo test -p magnetita-proto -p magnetita-mobile -p magnetitad --offline` (a peer that declines a capability; unknown FFI codes refused); JVM tests after the copies are removed; Gradle verify; Magnetita `complete-production.sh` plus Android build and verify | None |
| AUD-1-F | `suite:` | planned | `.githooks/`; `scripts/documentation_contract.py`; `scripts/land-unit.py`; `scripts/landing.py`; `scripts/production_artifact.py`; `scripts/qmllint-cxxqt.sh`; `scripts/check-language-contract.py` and `scripts/language-baseline.tsv`; `scripts/commit_scope.py`; `scripts/worktree.sh`; `scripts/audit-version-commits.py`; `.github/workflows/contracts.yml`; `STATUS.md`; `ROADMAP.md`; `README.md`; `AGENTS.md`; `docs/contracts/versioning.md`; `docs/superpowers/` | — | Run hooks from an extracted `HEAD:scripts`. Run the documentation contract over the index. Add `pre-merge-commit`. Scope errata. Deploy only after the push, and let `--abort` report or restore. Scope shared verification inputs per project. Add a post-build re-check, network timeouts and a safe `close`. Batch and memoise the inventory checks. Audit only the pushed range in CI. Select qmllint modules by URI. Restrict exemption markers by suffix and path through a declared scanner migration. Add a real-guard landing test. One git runner and registry loader. Fix the root STATUS, ROADMAP, README and the versioning "Agent workflow". Register or migrate `docs/superpowers/`. (P-20: TOOL-5, TOOL-6, TOOL-7, TOOL-8, TOOL-9, TOOL-10, TOOL-11, TOOL-12, TOOL-14, TOOL-15, TOOL-16, TOOL-17, TOOL-18, TOOL-19, TOOL-20) | `test-commit-scope.sh` and `test-staged-units.sh` fixtures (partial staging, merge, edited worktree guard ignored); a real-guard case in `test-land-unit.py`; documentation guard hook-mode time recorded under 5 s; `test-qmllint-target.sh` multi-module fixture; `test-language-contract.py` marker fixtures plus the "Resolved language debt" field; `check-documentation-contract.sh`; `audit-version-commits.py` | None |

`AUD-1-E` lands as `suite-bug` bumping Magnetita and Magnetita Android;
`scripts/land-unit.py` refuses a `suite-bug`, so the author records both
bumps by hand, as the landing contract says. `AUD-1-B` lands as
`suite-maintenance` unless its Hematita token fix changes what a person sees,
in which case it becomes `suite-bug` bumping Hematita (ruling R-A6). This plan
records intent; it grants no authority.
