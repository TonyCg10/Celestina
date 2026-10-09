# Evidence: CI contracts green again, with the style guard over Hematita

- **Date:** 2026-09-26
- **Scope:** `AUD-1-B` (program unit P-1) of the [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md), closing TOOL-1, TOOL-2 and TOOL-13 of the [tooling audit](2026-09-26-monorepo-audit-tooling.md) as far as the row's intended change reaches (see Limits); session worktree `Celestina.worktrees/suite-AUD-1-B`, branch `unit/suite/AUD-1-B`, forked from `origin/main` at `2a3c74f`
- **Environment:** Linux container (kernel 6.18) running as uid 0; Python 3.11.15, Git 2.43.0; no Qt 6 SDK, `qmllint`, CXX-Qt build, libmpv or Android SDK; no Wayland session or AT-SPI bus. The workflow run below used `GIT_CONFIG_GLOBAL=/dev/null`, so no personal Git identity or signing setting reached the fixtures, as on a fresh GitHub runner
- **Artifact:** the landing builds it. The unit changes `hematita/qml`, a Hematita production input, and `celestina-style/scripts/check-style-contract.sh`, a verification input shared by every registered project

## Procedure

Reproduce the two CI failures on the unchanged tree:

```sh
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
```

Change the guard and the fixtures, then run the widened style guard before
and after the two Hematita token fixes:

```sh
bash celestina-style/scripts/check-style-contract.sh
```

Prove the new fixtures fail on the defects they exist for, by temporary
mutation reverted before commit: restore the hand-listed style guard from
`HEAD` and run the scanner fixtures; then create an unregistered
`<root>/zzzfixture/qml/` and run them again.

Run every command of `.github/workflows/contracts.yml`, in its order, on the
committed tree. The first run was on `18f5f22`. The run below is on `63eb7a2`,
after the review fixes listed at the end of Observed facts:

```sh
export GIT_CONFIG_GLOBAL=/dev/null
ARCHITECTURE_COMPARE_REF=origin/main bash scripts/test-architecture-scanners.sh
ARCHITECTURE_COMPARE_REF=origin/main bash scripts/check-architecture-contract.sh
LANGUAGE_COMPARE_REF=origin/main bash scripts/test-documentation-contract.sh
LANGUAGE_COMPARE_REF=origin/main bash scripts/check-documentation-contract.sh
LANGUAGE_COMPARE_REF=origin/main python3 scripts/check-language-contract.py
LANGUAGE_COMPARE_REF=origin/main python3 scripts/test-version-contract.py
LANGUAGE_COMPARE_REF=origin/main python3 scripts/version_tool.py check
LANGUAGE_COMPARE_REF=origin/main python3 scripts/audit-version-commits.py
bash scripts/test-commit-scope.sh
bash scripts/test-staged-units.sh
bash scripts/test-worktree.sh
bash scripts/test-land-unit.sh
bash scripts/test-qmllint-target.sh
python3 scripts/test-language-contract.py
sh scripts/test-production-artifacts.sh
```

The last two lines are the new "Hermetic fixtures" step. They cover the
three scripts TOOL-13 names: `test-production-artifacts.sh` ends by
`exec`ing `test-production-common.sh`, so a separate line for it would run
those fixtures twice. The first run listed that script as a third command.
That double-counted it; none of the three had ever run in CI.

## Result

- **Exit (RED, unchanged tree `2a3c74f`):** `test-architecture-scanners.sh`
  exit 1 with `check-style-contract.sh: an input list omits hematita/qml`
  three times, once per hand-written list; `test-version-contract.py` exit 1,
  `test_current_repository_static_contract`:
  `Items in the first set but not the second: 'hematita'`. These are the two
  failures of GitHub `contracts` runs 94 to 97 that TOOL-1 records.
- **Exit (RED, widened guard before the Hematita fixes):**
  `check-style-contract.sh` exit 1, reporting
  `hematita/qml/components/HistoryGraph.qml:99` (`Qt.rgba(`, local colour
  transformation, found by the structural scanner and by the grep pattern)
  and `hematita/qml/components/PathCrumbs.qml:61` (`Font.Normal`, direct
  font weight, found by both). Nothing else in `hematita/qml` was reported.
- **Exit (GREEN, after the fixes):** `check-style-contract.sh` exit 0
  (`Contrast contract: OK`, `QML visual contract: OK`);
  `test-architecture-scanners.sh` exit 0; `test-version-contract.py` exit 0,
  22 tests.
- **Exit (mutations):** with the old hand-listed style guard the scanner
  fixtures exit 1: `check-style-contract.sh: hard-codes the registered
  project` for `celestina`, `celestina-style`, `siderita`, `magnetita`,
  `grafita` and `fluorita`, and `the style guard did not inspect a registered
  application` (the guard printed OK over a registry naming a sixth
  application). With an unregistered `zzzfixture/qml/` they exit 1:
  `zzzfixture/qml exists but no registered project owns it, so no guard
  inspects it`.
- **Exit (workflow on `63eb7a2`, every step in order):**

  | Step | Command | Exit | Time |
  |---|---|---|---|
  | Architecture and style | `test-architecture-scanners.sh` | 0 | 10 s |
  | Architecture and style | `check-architecture-contract.sh` | 0 | 5 s |
  | Documentation, language, versions | `test-documentation-contract.sh` | 0 | 12 s |
  | Documentation, language, versions | `check-documentation-contract.sh` | 0 | 39 s |
  | Documentation, language, versions | `check-language-contract.py` | 0 | 2 s |
  | Documentation, language, versions | `test-version-contract.py` | 0 | 0 s |
  | Documentation, language, versions | `version_tool.py check` | 0 | 0 s |
  | Documentation, language, versions | `audit-version-commits.py` | 0 | 69 s |
  | Commit scope | `test-commit-scope.sh` | 0 | 51 s |
  | Commit scope | `test-staged-units.sh` | 0 | 4 s |
  | Commit scope | `test-worktree.sh` | 0 | 1 s |
  | Commit scope | `test-land-unit.sh` | 0 | 46 s |
  | Commit scope | `test-qmllint-target.sh` | 0 | 0 s |
  | Hermetic fixtures (new) | `test-language-contract.py` | 0 | 1 s |
  | Hermetic fixtures (new) | `test-production-artifacts.sh`, which also runs `test-production-common.sh` | 0 | 9 s |

  The earlier run on `18f5f22`, with the old step layout, also exited 0
  everywhere. `audit-version-commits.py` covered 444 non-merge commits on
  that run; `version_tool.py check` reported 8 owners; `test-land-unit.sh`
  ran 56 tests. The production-artifact log contains
  `production-common fixtures: OK` exactly once.

### Observed facts

- **TOOL-2, the guard half.** `celestina-style/scripts/check-style-contract.sh`
  no longer names a project. It asks `scripts/architecture_scanners.py
  registry-qml-projects` for the QML roots, the same scanner command and the
  same `ARCHITECTURE_REGISTRY_FILE` override the architecture guard already
  uses, so both guards inspect one registry-derived set. The theme file is
  `<style root>/CelestinaTheme.qml`. The guard exits 1 when the registry
  cannot be read, declares no application or no style, or names a QML root
  that does not exist. Against the current registry the file set differs
  from the old one only by the 27 files under `hematita/qml`.
- **TOOL-2, the fix half.** Both replacements produce the same value as the
  literal they replace, so nothing a person sees changes and the unit stays
  `suite-maintenance` under ruling R-A6:
  - `HistoryGraph.qml`: `Qt.rgba(trace.r, trace.g, trace.b,
    accentSoftOpacity)` became `CelestinaTheme.withAlpha(graph.trace,
    CelestinaTheme.accentSoftOpacity)`. `withAlpha` is
    `Qt.rgba(value.r, value.g, value.b, clamp(alpha, 0, 1))` and
    `accentSoftOpacity` is `0.14`, inside the clamp. Hematita's
    `ResourceRow.qml` already calls `withAlpha` on a trace colour the same
    way.
  - `PathCrumbs.qml`: `Font.Normal` became `CelestinaTheme.weightRegular`,
    which the theme declares as `Font.Normal`.
  No theme token was added, so CelestinaStyle does not change.
- **TOOL-1.** `scripts/test-version-contract.py` derives the expected owners
  from the raw registry: every project that declares a `version_source`,
  unless it says `versioned = false`. It also asserts that the set is not
  empty and that the version history names exactly those owners. The old
  input-list check in `scripts/test-architecture-scanners.sh`, which only
  compared the style guard's lines against `siderita/qml`, is removed. In its
  place, the hard-coding check that covered the architecture guard now
  covers both guards and also catches a `find` list. A new check fails when
  a top-level `<dir>/qml/` exists that no registered project owns. The
  end-to-end `sextita` registry fixture now also runs the style guard,
  which must fail and name `sextita/qml`.
- **TOOL-13, the CI half.** `.github/workflows/contracts.yml` has a new
  "Hermetic fixtures" step that runs `test-language-contract.py` and
  `test-production-artifacts.sh`. The second ends by `exec`ing
  `test-production-common.sh`, so the three scripts the finding names all
  run, each once. They are hermetic: none needs Cargo, Qt or a personal Git
  identity, and the language fixtures set their own. Every step after the
  first carries `if: ${{ !cancelled() }}`, so a failing step no longer
  skips the ones after it; the job still fails. That skipping is the
  mechanism that let TOOL-1 hide every later check.
  `.github/workflows/README.md` describes the new step and the condition.
- **Review fixes (`63eb7a2`):**
  - The three tests left the `bash -e` chain of the "Commit scope" step for
    their own step, and the steps after the first no longer depend on the
    earlier ones succeeding.
  - The duplicate `test-production-common.sh` line is gone.
  - The style guard refuses a registry that registers more than one
    `qml-module` project, instead of keeping the last one as the theme
    owner. A new fixture in `test-architecture-scanners.sh` adds a second
    shared style to a copy of the registry and requires the refusal
    message. With the previous guard restored, that fixture exits 1 with
    `the style guard failed on a second shared style without saying so`.
  - `scripts/check-architecture-contract.sh` reads the same scanner rows
    and had the same last-one-wins assignment, so it now refuses a second
    `qml-module` project too, naming both roots. A sibling fixture requires
    a line starting `architecture: ERROR:` with the refusal, which tells it
    apart from the style guard's message printed later in the same run.
    With the previous architecture guard restored, that fixture fails with
    `the architecture guard failed on a second shared style without saying
    so`. After this change, `test-architecture-scanners.sh`,
    `check-architecture-contract.sh` (with `ARCHITECTURE_COMPARE_REF=origin/main`),
    `check-documentation-contract.sh` and `check-language-contract.py` exit 0.

## Limits

- **GitHub itself was not run.** The workflow ran locally with
  `origin/main` as the compare reference instead of `github.event.before`.
  Python here is 3.11; `ubuntu-latest` ships a newer one. "GitHub
  `contracts` green on the landed commit" can only be observed after the
  author lands and pushes.
- **No Qt.** Qt, `qmllint` and `hematita/scripts/verify-production.sh` were
  not available, so the two QML edits were not linted or rendered here.
  Hematita's `scripts/qmllint-baseline.tsv` row is 0 warnings, and
  `ResourceRow.qml` already uses the same `withAlpha` call on a `color`
  property inside that clean module; the landing's Hematita verification
  is the check.
- **Landing cost.** `hematita/qml` is a Hematita production input, so the
  landing runs Hematita's `complete-production.sh`. The style guard is in
  every project's shared verification inputs
  (`scripts/production_artifact.py`), so every other project whose artifact
  is current takes the verification-only path (TOOL-9).
- **Not closed here.** Two parts of the findings lie outside this row's
  intended change, and they stay open:
  - TOOL-1's third fix bullet: treat a red `contracts` run as blocking,
    through branch protection or a landing refusal. Branch protection is a
    GitHub setting outside the repository; a landing refusal would change
    `scripts/land-unit.py`.
  - TOOL-13's second fix bullet: run `test-architecture-scanners.sh` and
    `test-version-contract.py` in the landing's `pre_guards` when a unit
    changes `docs/projects.toml` or `scripts/`. That also changes
    `scripts/land-unit.py`, which `AUD-1-C` and `AUD-1-F` own.
  - TOOL-13's "documented and accepted gaps" (merges not audited in CI,
    intermediate commits not re-checked, `--history-scope-only` reading
    today's registry) are unchanged by design.

## Follow-up

- The two open fix bullets above (TOOL-1 bullet 3, TOOL-13 bullet 2) go to
  the row of `AUD-1-F` (P-20), which already owns `scripts/land-unit.py` and
  `.github/workflows/contracts.yml`. That row is edited on `main`, not on
  this branch. Branch protection is the author's decision.
- `STYLE-G7-N` (P-16) plans a pattern for a literal alpha passed to
  `withAlpha`. The Hematita call passes a token, so that pattern will not
  flag it. That unit edits the same guard file, so it rebases onto this one.

## Landing

- **Base revision:** `4eedb4075f895ee81d4c2a2fe5be2ab8540b580e`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:a07296319641a7bb878fddba20ba6faa4c0bbda5e9a3377366b2145606354c2d, verification_fingerprint sha256:1a3b2de39c95187e7e1a910c8545eb44ced0accd00f936eb66931a87eb667a08; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4ac5523bc556c02252a6b668fe8af67ce2291a6738b3eb92285034765622b5d8, verification_fingerprint sha256:3a9d45b9837425f12274edbe0a86c80466992d32b503a87d9b71877b60f05e28; siderita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:41961220fa5cf9dc774e80fac2444fd1a638d1e05abdada292cc2169eb7f42e3, verification_fingerprint sha256:18f8cea9b70135e07db03c75de8f43d084a6797495c33c0ae0b14040624fecd6; magnetita build: complete-production.sh exit 0, manifest source_fingerprint sha256:5bcadc527a911cac20f39372bf5991c0116655bb246225f307771f1dd5eebab5, verification_fingerprint sha256:571a87ac06514065454d9d01ad1bd6c98010ca807257f4f492e267bad96fd71c; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:9f4d1b9aa5339d08bb040f7de96d3804b3a8258fb61c54135bdae4fa3dcc3f5d, verification_fingerprint sha256:9098b47ba06e52b95aa31b76e6f505eaa47ad5dfbb7f6e9b9917a3c0f1423ea9; grafita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:d72431eb2e5d044437b424701d584d14733a7f63f5cd26141401b3d694b0f263, verification_fingerprint sha256:a0407f73b6ab27430dd1f53ea05b6ac5eca272e6381128833b78d107de8c4338; fluorita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:2f7a2eba3545ce64966cde62acd14a12f9d0b1b46ee05c5d2667fcf09360d601, verification_fingerprint sha256:0365e995211112feab8974c9d306f6a5b24bb6d91ed91bf22e0163027da4c214; hematita build: complete-production.sh exit 0, manifest source_fingerprint sha256:a8963057122183a981bbb055b04f92e69df3cd2fd91f65bb16571a23e5429e10, verification_fingerprint sha256:424b97a9c5fdbb46e1963d65343febf66a45bbdbd2e3da442539af788c80127a
