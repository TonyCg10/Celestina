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
- **Withdrawn:** the Celestina shell, halted by the author on 2026-09-27; see
  [Halted projects](../../../AGENTS.md#halted-projects) and the addendum of
  [the audit evidence](../../evidence/2026-09-26-monorepo-audit.md#addendum-2026-09-27-shell-halted)

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
in each idle project, or a new row of the active plan in each busy one. The
shell's findings are the exception: they were withdrawn when the author
halted the shell on 2026-09-27. When
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
  the active plans of Celestina (`SURF-1-E`, `SURF-1-F`; both withdrawn:
  shell halted 2026-09-27), CelestinaStyle
  (`STYLE-G7-N`), Magnetita (`MAG-D1-D`, `MAG-D1-E`, plus the MAG-25 waiver)
  and Magnetita Android (`AND-6-D`).
- `AUD-1-B` (P-1) — CI green again and the style guard over Hematita.
- `AUD-1-C` (P-0b) — the landing accepts a stacked branch.
- `HALT-SHELL-0` — teach the landing to skip a project the registry marks
  `halted`, dormant until `HALT-SHELL` adds the marker, so that unit's own
  landing does not build, verify or deploy the shell.
- `AUD-1-D` (P-2) — production inputs guarded against each app's Cargo
  path-package closure, Magnetita Android's inputs declared.
- `AUD-1-E` (P-13) — one owner for the Magnetita protocol rules, negotiated
  capabilities on both ends; genuinely cross-suite, so it is a suite row that
  bumps Magnetita and Magnetita Android.
- `AUD-1-F` (P-20) — tooling integrity, speed and governance documents.
- `AUD-1-G` — one project per registered artifact path: the Rust workspace
  builds into its own target directory, so it no longer overwrites the
  `magnetitad` Magnetita deploys.
- `AUD-1-H` — the hooks accept the merge of `origin/main` into a stacked
  branch that the landing contract prescribes.
- `AUD-1-J` — at the author's request of 2026-10-06, prune every build tree
  to its registered artifacts at the end of each landing, and remove the
  shared session Cargo target when the last session closes.
- `AUD-1-K` — at the author's request of 2026-10-06, keep the release cache
  in the prune and remove only the debug builds, so a small change rebuilds
  incrementally.
- `AUD-1-L` — at the author's request of 2026-10-06, let every application
  load its QML from the source tree in development, so a QML change such as a
  colour needs a restart instead of a build.
- `AUD-1-M` — at the author's request of 2026-10-07, carry Siderita's glass
  canvas to every application: a guard that counts, per application, a main
  window that is not transparent and one without `CelestinaBackdrop`, against
  a shrink-only ratchet the application units lower to 0.
- `HALT-SHELL` — record the author's halt of the Celestina shell
  (2026-09-27) in the root contract, the registry and the shell's documents,
  and make the guards and the landing honour it.

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
  next touched, except SH-16 and SH-18 (withdrawn: shell halted 2026-09-27).
- The Celestina shell (withdrawn: shell halted 2026-09-27). `SURF-1-E`
  (P-10) and `SURF-1-F` (P-17) are withdrawn from the program and frozen,
  unedited, in the shell's own plan; its `SH-*` findings stay recorded in the audit
  evidence and are not acted on. The shell halves of other rows fall with
  them: `RS-H1-A` leaves `celestina-rs/crates/celestina-shell-core`
  untouched; SH-6's token and contract half, which exists only for the
  shell's lock, leaves `STYLE-G7-N`; no unit adopts a shared owner into the
  shell; and no landing rebuilds it. The shared owners themselves (the owner
  halves of RS-1, RS-2, RS-3 and RS-6) and every other product's adoption are
  unaffected.
- Pulling TOOL-8 and TOOL-9 forward (question 10 of the audit). No ruling
  settled it; until `AUD-1-F` lands, run
  `sh scripts/check-documentation-contract.sh` locally before each landing.

## Build order

1. `AUD-1-A`, documentation only, lands first.
2. `AUD-1-B` (P-1) and `AUD-1-C` (P-0b) from `main`.
3. `AUD-1-D` (P-2), stacked on `AUD-1-B`.
4. The project units in program order, each on its own branch: `AND-6-D`
   (P-3), `SID-H1-A` (P-4), `GRA-H1-A` (P-5), `RS-H1-A` (P-6), then the units
   that adopt its owners (`FLU-H1-A`, `SID-H1-B`, `HEM-H1-A`, `MAG-D1-D`,
   `MAG-D1-E`), then `FLU-H1-B`, `SID-H1-C`, `STYLE-G7-N`, `HEM-H1-B` and
   `GRA-H1-B`. `SURF-1-E` (P-10) and `SURF-1-F` (P-17) are withdrawn: shell
   halted 2026-09-27.
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
| AUD-1-B | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-B.numstat.tsv) | 11 files, +410/-51 | Derive the style-guard roots and the version-owner fixture from `docs/projects.toml`. Fix the two Hematita token violations in the same commit, because a guard coverage change and the violations it reveals must land together or the published revision is red. Add the three missing tests to `contracts.yml`. If the Hematita token fix changes what a person sees, the implementer reports it and the unit lands as `suite-bug` bumping Hematita (ruling R-A6). (P-1: TOOL-1, TOOL-2, TOOL-13) | [evidence](../../evidence/2026-09-26-monorepo-hardening-ci.md) | None |
| AUD-1-C | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-C.numstat.tsv) | 10 files, +1492/-74 | Support stacked unit branches in the landing (P-0b, ruling R-A1) | [evidence](../../evidence/2026-09-26-monorepo-hardening-stacked-landing.md) | None |
| HALT-SHELL-0 | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/HALT-SHELL-0.numstat.tsv) | 6 files, +218/-2 | Make the landing skip halted projects before the halt is recorded | [evidence](../../evidence/2026-09-28-landing-skips-halted.md) | None |
| AUD-1-D | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-D.numstat.tsv) | 15 files, +1362/-25 | Guard that each app's production inputs hold its Cargo path-package closure. Declare Magnetita Android's inputs. Reject buildable projects with empty inputs. Compare the recorded toolchain. Print consumers in agent context. (P-2: TOOL-3, TOOL-4, MAG-9, FLU-21, TOOL-21, TOOL-22) | [evidence](../../evidence/2026-09-26-monorepo-hardening-inputs.md) | None |
| AUD-1-E | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-E.numstat.tsv) | 74 files, +5393/-1769 | Advertise the real capability set, negotiate, and gate on both ends. Export the typed signals, `check_path`, backoff, discovery ranking, bounds and the sanitiser through UniFFI, and delete the Kotlin copies. Use one clipboard bound. Make Messages signal-driven and query threads with a limit. Move the upload to its own coroutine. Suspend on channels. Cap received offers. Add FFI tests. This is genuinely cross-suite: negotiated gating must change daemon, mobile crate and APK atomically, or one side refuses the other. `scripts/land-unit.py` refuses a `suite-bug`, so the author records the two bumps by hand, as the landing contract says. Carried: ruling R-A16 exports `AND-6-D`'s pairing preview and LAN rule and deletes the Kotlin `PairPreview` and `LanAddress` with their rule tests moved to Rust; `RS-13` makes `magnetita-mobile`'s bindgen feature opt-in and names it in `magnetita-android/scripts/build-native.sh` in the same landing. (P-13: MAG-8, MAG-12, MAG-19, MAG-28; AND-2, AND-3, AND-5, AND-7, AND-9) | [evidence](../../evidence/2026-09-26-negotiated-capabilities.md) | `VAL-AND-2` |
| AUD-1-F | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-F.numstat.tsv) | 48 files, +3482/-717 | Run hooks from an extracted `HEAD:scripts`. Run the documentation contract over the index. Add `pre-merge-commit`. Scope errata. Deploy only after the push, and let `--abort` report or restore. Scope shared verification inputs per project. Add a post-build re-check, network timeouts and a safe `close`. Batch and memoise the inventory checks. Audit only the pushed range in CI. Select qmllint modules by URI. Restrict exemption markers by suffix and path through a declared scanner migration. Add a real-guard landing test. One git runner and registry loader. Fix the root STATUS, ROADMAP, README and the versioning "Agent workflow". Register or migrate `docs/superpowers/`. Carried here by ruling R-A16 and later reviews: refuse to land while the latest GitHub `contracts` run on `main` failed when it can be read without credentials, and document branch protection as the author's step (TOOL-1, third fix); run `test-architecture-scanners.sh` and `test-version-contract.py` in the landing's `pre_guards` when a unit changes `docs/projects.toml` or `scripts/` (TOOL-13, second fix); probe only the toolchains a project's build declares in the registry; give the input path matchers one owner in `production_artifact.py`; leave halted projects out of the consumers `agent-context.py` prints. (P-20: TOOL-5, TOOL-6, TOOL-7, TOOL-8, TOOL-9, TOOL-10, TOOL-11, TOOL-12, TOOL-14, TOOL-15, TOOL-16, TOOL-17, TOOL-18, TOOL-19, TOOL-20) | [evidence](../../evidence/2026-09-26-tooling-integrity.md) | None |
| HALT-SHELL | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/HALT-SHELL.numstat.tsv) | 22 files, +691/-54 | Halt the Celestina shell | [evidence](../../evidence/2026-09-28-halt-shell.md) | None |
| AUD-1-G | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-G.numstat.tsv) | 8 files, +159/-2 | Make each registered artifact path belong to one project | [evidence](../../evidence/2026-09-29-one-owner-per-artifact.md) | None |
| AUD-1-H | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-H.numstat.tsv) | 8 files, +254/-3 | Fix merging main into a stacked branch through the hooks | [evidence](../../evidence/2026-09-29-merged-in-inventories.md) | None |
| AUD-1-I | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-I.numstat.tsv) | 8 files, +888/-0 | Add the radius guard the 2026-09-30 design specifies: a bounded QML scanner that refuses text or a glyph closer to a rounded corner than `cornerInset`, a nested token radius that is not concentric with its parent, and a numeric margin literal inside a rounded surface, with a shrink-only per-project baseline registered as a shared ratchet so each application lowers it in the unit that pays the debt | [evidence](../../evidence/2026-09-30-radius-guard.md) | None |
| AUD-1-J | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-J.numstat.tsv) | 14 files, +799/-16 | Add the prune of every build tree to its registered artifacts after each landing deploys | [evidence](../../evidence/2026-10-06-prune-build-trees.md) | None |
| AUD-1-K | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-K.numstat.tsv) | 11 files, +256/-217 | Keep the release cache when pruning the build trees | [evidence](../../evidence/2026-10-06-prune-keeps-release-cache.md) | None |
| AUD-1-L | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-L.numstat.tsv) | 14 files, +464/-65 | Add a development mode that loads the QML from the source tree | [evidence](../../evidence/2026-10-06-qml-dev-mode.md) | None |
| AUD-1-M | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-M.numstat.tsv) | 10 files, +1319/-0 | Add the glass-canvas guard: every registered application's `Main.qml` binds `color: CelestinaTheme.clear` and contains a `CelestinaBackdrop`, counted per project against `scripts/glass-canvas-baseline.tsv` (shared ratchet), run by the architecture contract and CI. | [evidence](../../evidence/2026-10-07-glass-canvas-contract.md) | None |
| AUD-1-N | `suite:` | done | [inventory](../../inventories/2026-09-26-monorepo-hardening/AUD-1-N.numstat.tsv) | 5 files, +85/-1 | Sort the QML sources before the batched qmllint invocation so the warning ratchet no longer depends on directory order, and re-measure Siderita's row (239 to 241) under that order. | [evidence](../../evidence/2026-10-08-qmllint-order.md) | None |

`AUD-1-E` lands as `suite-bug` bumping Magnetita and Magnetita Android;
`scripts/land-unit.py` refuses a `suite-bug`, so the author records both
bumps by hand, as the landing contract says. `AUD-1-B` lands as
`suite-maintenance` unless its Hematita token fix changes what a person sees,
in which case it becomes `suite-bug` bumping Hematita (ruling R-A6). This plan
records intent; it grants no authority.
