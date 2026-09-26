# Evidence: the repository tooling and governance audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): `scripts/`, `.githooks/`, `.github/workflows/`, `docs/projects.toml` and the governance documents, on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit TOOL: repository tooling and governance".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
python3 scripts/audit-version-commits.py
python3 scripts/check-staged-units.py
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
sh scripts/test-commit-scope.sh
sh scripts/test-documentation-contract.sh
bash scripts/test-land-unit.sh
python3 scripts/test-land-unit.py
bash scripts/test-production-artifacts.sh
python3 scripts/test-production-artifacts.py
sh scripts/test-production-common.sh
sh scripts/test-qmllint-target.sh
sh scripts/test-staged-units.sh
bash scripts/test-worktree.sh
python3 scripts/test-language-contract.py
```

The probes marked "demonstrated" ran in a throwaway clone under the
scratchpad, with `core.hooksPath .githooks` and a scratch bare
repository as `origin`: one normal commit through the hooks, the
partial-staging, merge and unstaged-guard cases of TOOL-5 and TOOL-6,
and one documentation-only `scripts/land-unit.py` landing. The clone
was deleted afterwards. The GitHub `contracts` runs 94 to 97 were read
for TOOL-1.

## Result

- **Exit:** the four guards, `audit-version-commits.py`, `check-staged-units.py` and every fixture suite exited 0 except `test-architecture-scanners.sh` and `test-version-contract.py`, which failed (TOOL-1); timings are in the table below.
- **Observed:** 22 findings: 1 Critical, 10 Important, 11 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Audited on `9d022dd` (the checkout's branch `claude/festive-albattani-2uk3tn`, which equals `origin/main`). This was a read-only audit. The probes marked "demonstrated" ran in a throwaway clone under the scratchpad whose `origin` was a scratch bare repository. That clone has been deleted. Nothing in the real repository was modified.

### Executive summary

1. **Healthy:** the guard design is careful. Ratchets use HEAD rules to judge INDEX data and fail closed. Inventories are exact. `production_artifact.py` seals only after a supervised child succeeds. Most scripts have positive and negative fixtures.
2. **Healthy:** the new landing tool worked end to end against the real hooks and guards. A documentation-only `suite:` unit landed in one sealed commit in 102 s, and the documentation contract passed on the result (demonstrated).
3. **Top risk:** CI has been red on `main` since 2026-09-25 (GitHub `contracts` runs 94–97). `test-architecture-scanners.sh` and `test-version-contract.py` both fail because Hematita was never added to the lists they check. Step 1 fails, so every later CI step is skipped, including documentation, language, version audit, commit-scope replay and landing tests. CI currently enforces nothing.
4. **Top risk:** production fingerprints come only from hand-listed `production_inputs`. Magnetita, Grafita, Fluorita and Hematita omit Cargo path dependencies they really link, and Magnetita Android lists no inputs at all. `check` and `status` report "current" over stale bytes, and the landing skips rebuilding those apps, even when their own prefix changed the code.
5. **Top risk:** the hooks do not do what the governance text says they do. Partial staging and automatic merges both get past the documentation contract (demonstrated). The hooks also execute the worktree copies of the guard scripts, so an unstaged edit switched off all commit enforcement (demonstrated).
6. **Performance:** the documentation guard takes about 36 s. The file walk (`os.walk`) accounts for only 0.6 s; about 37 s goes to re-verifying 240 immutable historical inventories through roughly 16,000 `git` subprocesses. It runs in every pre-commit and twice per landing. `audit-version-commits.py` takes 65 s and grows with history.
7. **Landing risks on a real repository:**
   - It deploys before its final guards, commit and push, and `--abort` does not roll the deployment back.
   - Any change to a shared verification input re-verifies all 9 projects and redeploys 6 apps. Every ratchet decrease counts, and it needs Gradle, Qt and CMake toolchains.
   - Its tests replace every guard and hook with a double.
8. `qmllint-cxxqt.sh` picks the newest QML module under a Cargo target directory that every session and app shares. It can lint one app against another app's types.
9. The style guard never scans `hematita/qml`, and two real visual-contract violations are hidden there.
10. The root STATUS, ROADMAP, README, the versioning contract and AGENTS.md still describe a registry of 6–7 products. It now has 9 projects and 8 versioned owners.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| TOOL-1 | Critical | Tests | `scripts/test-version-contract.py:234`, `celestina-style/scripts/check-style-contract.sh:16` | CI `contracts` job red on main since 2026-09-25; every later step skipped | S | `suite:` |
| TOOL-2 | Important | QML and accessibility | `celestina-style/scripts/check-style-contract.sh:16,39,51` | Style guard never scans `hematita/qml`; 2 violations hidden | S | `suite:` (guard) + `hematita:` (fixes) |
| TOOL-3 | Important | Correctness | `docs/projects.toml:222,288,323,360` | `production_inputs` omit linked Cargo path dependencies; stale artifacts report "current" and the landing skips the rebuild | M | `suite:` |
| TOOL-4 | Important | Correctness | `docs/projects.toml:235-256` | Magnetita Android declares no production or verification inputs; its APK manifest never goes stale | S | `suite:` |
| TOOL-5 | Important | Correctness | `.githooks/pre-commit:13` | Partial staging and automatic merges get past the documentation contract (demonstrated) | M | `suite:` |
| TOOL-6 | Important | Correctness | `.githooks/commit-msg:12`, `docs/governance/change-policy.md:64`, `AGENTS.md:216` | Hooks run the worktree guard scripts; an unstaged edit disabled enforcement (demonstrated), contradicting the governance text | M | `suite:` |
| TOOL-7 | Important | Performance | `scripts/documentation_contract.py:1853,1861` | 36 s guard; 94% is re-verifying immutable inventories with ~16k `git` calls | M | `suite:` |
| TOOL-8 | Important | Correctness | `scripts/land-unit.py:654-712,791` | Landing deploys before final guards, commit and push; failure or `--abort` leaves unlanded bytes installed | M | `suite:` |
| TOOL-9 | Important | Performance | `scripts/production_artifact.py:309-330` | Any shared-input change (every ratchet decrease, any registry edit) re-verifies 9 projects, redeploys 6 and needs every toolchain | M | `suite:` |
| TOOL-10 | Important | Correctness | `scripts/qmllint-cxxqt.sh:120-126` | Newest-by-mtime QML module in a shared target dir can be another app's; the documented session fast check needs a release build sessions cannot run | S | `suite:` |
| TOOL-11 | Important | Correctness | `scripts/check-language-contract.py:70,74` | Language exemption markers accepted in any file type; `product-copy` used in about 20 Markdown files to exempt development prose | S | `suite:` |
| TOOL-12 | Minor | Tests | `scripts/test-land-unit.py:679-693` | Landing tests replace every guard and hook with doubles; no project-path landing ever ran for real | M | `suite:` |
| TOOL-13 | Minor | Tests | `.github/workflows/contracts.yml:34-51` | `test-language-contract.py`, `test-production-artifacts.sh`, `test-production-common.sh` never run in CI; landing runs no `test-*` | S | `suite:` |
| TOOL-14 | Minor | Correctness | `scripts/documentation_contract.py:2675` | An erratum excuses every error of an inventory, not the recorded defect; `--quiet` still prints about 60 lines | S | `suite:` |
| TOOL-15 | Minor | Documentation truth | `STATUS.md:78,104-108`, `ROADMAP.md:137,166-176`, `README.md:14-24`, `docs/contracts/versioning.md:93`, `AGENTS.md:85` | Stale counts and project lists; archived plan labelled "active" | S | `suite:` |
| TOOL-16 | Minor | Documentation truth | `docs/contracts/versioning.md:61-77` | "Agent workflow" tells a session to bump and complete production, contradicting AGENTS.md and the landing | S | `suite:` |
| TOOL-17 | Minor | Documentation truth | `docs/projects.toml:347`, `scripts/land-unit.py:8` | `docs/superpowers/{plans,specs}` is an unregistered second plan and spec tree that registry and code depend on | S | `suite:` |
| TOOL-18 | Minor | Architecture and reuse | `scripts/commit_scope.py:370,419`; `scripts/check-language-contract.py:126,158` | 7 `git` runners, about 11 registry loaders, 4 language-baseline parsers, a copy-pasted evidence scan | M | `suite:` |
| TOOL-19 | Minor | Correctness | `scripts/land-unit.py:698`, `scripts/worktree.sh:95,178` | No post-build re-check in the landing; no timeouts on network `git`; `close -D` can drop post-landing commits | S | `suite:` |
| TOOL-20 | Minor | Performance | `scripts/audit-version-commits.py:200-215` | Full-history replay (65 s, 441 commits) on every CI push, one `git` process per blob | S | `suite:` |
| TOOL-21 | Minor | Architecture and reuse | `scripts/agent-context.py` | Context for a shared crate names only its owning consumer (`siderita-ops` omits Fluorita and Hematita) | S | `suite:` |
| TOOL-22 | Minor | Correctness | `scripts/production_artifact.py:618-640` | Toolchain is recorded but never compared; a system Qt or rustc upgrade does not stale an artifact | S | `suite:` |

### Guard and test timings

Measured on this container.

| Command | Result | Time |
|---|---|---|
| `bash scripts/check-architecture-contract.sh` | OK | 4.7 s |
| `python3 scripts/check-language-contract.py` | OK (148 ratcheted) | 1.5 s |
| `sh scripts/check-documentation-contract.sh` | OK (plus ~60 erratum lines) | **36.1 s** |
| `python3 scripts/version_tool.py check` | OK (8 owners) | 0.1 s |
| `python3 scripts/audit-version-commits.py` | OK (441 commits) | **65.6 s** |
| `python3 scripts/check-staged-units.py` | OK | 0.1 s |
| `test-architecture-scanners.sh` | **FAIL** | 11.2 s |
| `test-version-contract.py` | **FAIL** | 0.8 s |
| `test-commit-scope.sh` | OK | 47.0 s |
| `test-documentation-contract.sh` | OK | 14.5 s |
| `test-land-unit.sh` / `test-land-unit.py` (same suite, twice) | OK | 61.9 s / 49.8 s |
| `test-production-artifacts.sh` / `.py` | OK | 10.0 s / 7.2 s |
| `test-production-common.sh` | OK | 1.3 s |
| `test-qmllint-target.sh` | OK | 0.2 s |
| `test-staged-units.sh` | OK | 3.9 s |
| `test-worktree.sh` | OK | 0.8 s |
| `test-language-contract.py` | OK | 1.1 s |
| One normal `git commit` through the hooks (demonstrated) | — | 38 s |
| One docs-only `land-unit.py` landing (demonstrated) | landed | 102 s |

---

### TOOL-1: CI `contracts` job red on main; every later step skipped (Critical, Tests)

**Evidence**

- `bash scripts/test-architecture-scanners.sh` exits 1:
  `ERROR: check-style-contract.sh: an input list omits hematita/qml -> if ! find siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style \` (three times).
- `python3 scripts/test-version-contract.py` fails `test_current_repository_static_contract`: `Items in the first set but not the second: 'hematita'`. The expected set at `scripts/test-version-contract.py:234-244` is hard-coded to seven owners.
- GitHub Actions (`TonyCg10/Celestina`, workflow `contracts.yml`, branch `main`):
  - runs 94 (`17b801f`), 95 (`93e572a`) and 97 (`9d022dd`) are all `conclusion: failure` and finish in about 20 s;
  - the last success is run 93 on 2026-09-19.
- `contracts.yml:24-27` runs `test-architecture-scanners.sh` first. Because it fails, the steps "Documentation, language, versions" and "Commit scope" never run: no documentation contract, no language contract, no `audit-version-commits.py`, no history scope replay and no landing tests.

**Why it matters**

The workflow README calls CI the backstop for commits that skipped the local hooks. For at least four pushes it has enforced nothing. The landing and the hooks never run `test-*`, so registry-coupled fixture tests rot silently. TOOL-5 shows a documentation-red merge that only CI would have caught.

**Fix**

- Add `hematita/qml` to the style guard (or derive the roots, see TOOL-2).
- Derive the expected owner set in `test-version-contract.py` from the registry, or assert only invariants.
- Treat a red `contracts` run as blocking. Branch protection can do this, or the landing can refuse when the latest `main` run failed.

**Effort / prefix:** S, `suite:`

### TOOL-2: style guard never scans Hematita; two violations hidden (Important, QML and accessibility)

**Evidence**

`celestina-style/scripts/check-style-contract.sh:16,39,51` list the QML roots by hand (`siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style`). `hematita/qml` exists since `9ecc72e` (2026-09-21, 90 commits ago).

Running the same scanner on it: `python3 scripts/architecture_scanners.py qml-style-contract celestina-style/CelestinaTheme.qml hematita/qml` reports

- `hematita/qml/components/HistoryGraph.qml:99`: `fillColor: Qt.rgba(graph.trace.r, graph.trace.g, graph.trace.b,` (local colour transformation)
- `hematita/qml/components/PathCrumbs.qml:61`: `? CelestinaTheme.weightDemiBold : Font.Normal` (direct font weight)

**Why it matters**

AGENTS.md requires colours and typography to come from `CelestinaTheme`. Every new app gets through the visual contract until someone edits this list, and the architecture guard deliberately avoids hand lists for exactly this reason.

**Fix**

- Build the root list from `docs/projects.toml` (`source_roots` ending in `/qml`), as `check-architecture-contract.sh` already does.
- Add theme tokens for the two Hematita cases, or add a `CelestinaTheme` helper for a trace with alpha.

**Effort / prefix:** S, `suite:` for the guard; the Hematita fixes need `hematita:`.

### TOOL-3: production inputs omit linked Cargo path dependencies (Important, Correctness)

**Evidence**

I compared each app's Cargo path-dependency closure (walking `Cargo.toml` path and workspace dependencies) against its `production_inputs`. These crates are linked but not declared:

- **magnetita** (`docs/projects.toml:222`):
  - `fluorita-engine` and `fluorita-qt`: direct dependencies at `magnetita/Cargo.toml:28,42`;
  - `fluorita-core`;
  - `magnetita-proto` and `magnetita-link`: dependencies of the sealed `magnetitad`, at `celestina-rs/crates/magnetitad/Cargo.toml:17-18`;
  - `siderita-ops`: via `fluorita-engine`, at `celestina-rs/crates/fluorita-engine/Cargo.toml:29`.
- **grafita** (`:288`): `celestina-core`, through `grafita-core` (`celestina-core.workspace = true`).
- **fluorita** (`:323`): `siderita-ops`, a direct dependency at `fluorita/Cargo.toml:36`.
- **hematita** (`:360`): `celestina-core` and `siderita-ops`, direct dependencies at `hematita/Cargo.toml:26,38`.

`production_fingerprint` (`scripts/production_artifact.py:262-271`) hashes only the declared patterns. `affected_projects` (`scripts/landing.py:359-409`) only matches declared patterns.

Concrete case: a `magnetita:` unit changes `celestina-rs/crates/magnetita-link/`, which is inside Magnetita's own `commit_roots`.

1. The owner check still passes, so `check --require-verified` exits 0.
2. The landing runs nothing, and the installed `magnetitad` does not contain the fix.
3. `status-production.sh` then reports "current and verified".

**Why it matters**

The artifact contract promises that the author's binary contains the verified bytes of every completed bug. This breaks that promise without any warning.

**Fix**

- Derive the Rust inputs from `cargo metadata --format-version 1 --offline` (every `path`-sourced package's `manifest_path` directory) at fingerprint time.
- Or add a guard to `check-architecture-contract.sh` that fails when a path dependency is missing from `production_inputs`.

**Effort / prefix:** M, `suite:`

### TOOL-4: Magnetita Android has no inputs (Important, Correctness)

**Evidence**

- The `[[projects]] id = "magnetita-android"` table (`docs/projects.toml:235-256`) has neither `production_inputs` nor `verification_inputs`.
- `magnetita-android/scripts/build-production.sh` does seal through `production_artifact.py run-build`.
- `production_input_patterns` therefore returns only `build_script`.
- Gradle's own inputs (`magnetita-android/app/build.gradle.kts:21-23`) cover `magnetita-mobile`, `magnetita-link` and `magnetita-proto`. None of these, nor `app/src`, reach the fingerprint.

**Why it matters**

After one build, `check` and `status` say the APK is current forever. The landing never rebuilds it for an `AND-*` bug, and this project has already shipped `magnetita-android-bug` commits.

**Fix**

- Declare `magnetita-android/app/src`, the Gradle files, `magnetita-android/scripts/build-native.sh` and the three crates as production inputs, and declare the test directories as verification inputs.
- Make the documentation or architecture guard reject a buildable project whose `production_inputs` is empty.

**Effort / prefix:** S, `suite:`

### TOOL-5: partial staging and automatic merges get past the documentation contract (Important, Correctness)

**Evidence (demonstrated in the scratch clone with `core.hooksPath .githooks`)**

- **Partial staging:**
  1. `printf '\nSee [missing](does-not-exist.md).\n' >> docs/README.md; git add docs/README.md`.
  2. Restore the worktree copy, then `git commit -m "suite-maintenance: Add a partially staged link"`.
  3. Result: exit 0 after 38 s. The committed tree fails `docs/README.md: line 87: broken local link`.
  - Cause: `.githooks/pre-commit:13` runs `check-documentation-contract.sh`, which walks the worktree (`iter_repository_files`), not the index.
- **Merge:**
  1. On a side branch, commit the same broken link with `--no-verify`.
  2. On `main`, run `git merge --no-ff side`.
  3. Result: exit 0. Git runs `pre-merge-commit` for automatic merges, not `pre-commit`, and the repository has no `pre-merge-commit`. `commit_scope.validate_merge` checks ratchets, language and inventories, but not documentation.
  - The same merge with Spanish in `AGENTS.md` was correctly rejected by `commit-msg`.
- A fast-forward `git merge` or `git pull` runs no hook at all. With CI red (TOOL-1), nothing re-checks it.

**Why it matters**

A documentation-red revision reaches `main` by a normal workflow. The pre-commit comment itself notes that "five commits reached published main while it was failing".

**Fix**

- Run the documentation contract against the index: `git checkout-index -a --prefix=<tmp>/`, or build the file list from `git ls-files -s` plus `:path` blobs.
- Add `.githooks/pre-merge-commit` that execs `pre-commit`.
- Refuse `git commit` while partially staged plan or evidence files exist.

**Effort / prefix:** M, `suite:`

### TOOL-6: hooks execute worktree guard scripts, contradicting the governance text (Important, Correctness)

**Evidence**

- `.githooks/commit-msg:12` runs `exec python3 "$root/scripts/commit_scope.py" "$@"`.
- `.githooks/pre-commit` runs the worktree `check-language-contract.py`, `documentation_contract.py` and `check-staged-units.py`.
- Only helper modules (`project_registry.py`, the scanners, `version_contract.py`, parts of `documentation_contract.py`) are loaded from `HEAD`.
- Demonstrated:
  1. Overwrite (unstaged) `scripts/commit_scope.py` and `scripts/check-language-contract.py` with `sys.exit(0)`.
  2. Stage a Spanish line in `AGENTS.md`.
  3. `git commit -m "not even a valid subject"` succeeds (commit `c501f53` in the scratch clone).
- The governance text claims otherwise:
  - `docs/governance/change-policy.md:64`: "so neither INDEX nor worktree rule modules execute";
  - `AGENTS.md:216`: "staged or unstaged rule modules never execute in the current hook";
  - `docs/standards/architecture.md` says the same.

**Why it matters**

The documents describe an integrity property the hooks do not have. It also fails by accident: an agent that is mid-edit on `commit_scope.py`, with a bug that exits 0, silently disables its own gate.

**Fix**

- Have the hook wrappers extract `HEAD:scripts/` (`git archive HEAD scripts | tar -x -C "$tmp"`) and exec from there, falling back to the worktree only when `HEAD` lacks the file (first adoption).
- Otherwise, correct the three documents.

**Effort / prefix:** M, `suite:`

### TOOL-7: documentation guard spends 94% of its time re-verifying immutable history (Important, Performance)

**Evidence**

`python3 -m cProfile scripts/documentation_contract.py --quiet` takes 39.6 s in total:

- `check_active_plans` → `check_ledger` → `check_done_ledger_row` → `check_inventory_against_git`: 240 calls, 36.8 s cumulative;
- `git_command`: 15,991 subprocess calls, 37.3 s;
- `commit_path_bytes` (`documentation_contract.py:1853`): runs a `cat-file -e` plus a `show` per path, 4,740 calls, 20.2 s;
- the repository walk (`iter_repository_files`) and link checks: 0.6 s in total.

**Why it matters**

The guard runs in every `pre-commit` (38 s per commit, demonstrated). It runs twice per landing (`run_guards` and the commit hook), and it grows linearly with every landed unit. Tracked inventories are immutable, so this re-proves the same facts every time.

**Fix**

- Batch blob reads through one `git cat-file --batch` process.
- Memoize the verdict per (inventory blob SHA, endpoint commit) in a cache under `.git/`.
- Or in hook mode verify only inventories that are new versus `HEAD`, leaving the full history replay to CI.

**Effort / prefix:** M, `suite:`

### TOOL-8: the landing deploys before its final guards, commit and push (Important, Correctness)

**Evidence**

- `build_if_stale` (`scripts/land-unit.py:714`) runs each affected deployable project's `complete_script`, which is build, verify, deploy and status. For the verify-only path it runs `verify_script`, `deploy_script` and `status_script` (`check_and_build`, `:654-712`).
- The documentation contract and `check-staged-units.py` run only afterwards, in `run_guards` (`:791`). The hooks then run in `seal_commit`, and the push can be rejected up to four times.
- `abort` (`:950-972`) restores `main` and the checkout, but not the installed prefix.

**Why it matters**

A unit that fails the documentation guard, a hook, or a lost push race is left installed in the author's normal prefix, although `main` does not contain it. The next landing of another unit leaves that binary untouched when its own inputs are unaffected, so the installed bytes match no revision.

**Fix**

- Split the production run: build and verify before the seal, deploy and status only after the push succeeds.
- Or run `documentation_contract.py` in `pre_guards` as well.
- Record in the state file what was deployed, so `--abort` can report or redeploy the previous manifest.

**Effort / prefix:** M, `suite:`

### TOOL-9: shared verification inputs fan out to every project and toolchain (Important, Performance)

**Evidence**

- `verification_input_patterns` (`scripts/production_artifact.py:309-330`) adds these to every project's verification fingerprint:
  - `docs/projects.toml`;
  - `scripts/architecture-baseline.tsv` and `scripts/qmllint-baseline.tsv`;
  - `scripts/check-architecture-contract.sh` and `scripts/architecture_scanners.py`;
  - `celestina-style/scripts/check-style-contract.sh`, among others.
- `affected_projects` marks every project whose verification patterns match.
- `shared_ratchet_files` exists so that each project unit lowers `architecture-baseline.tsv` in the same commit.

As a result, any unit that lowers a ratchet row or edits the registry re-runs `verify_script` for all 9 projects and `deploy_script` and `status_script` for all 6 deployables. That includes `magnetita-android/scripts/verify-production.sh` (`./gradlew testDebugUnitTest lintRelease`). A project whose manifest is missing on the machine goes down the full build path (`check` fails with "missing …").

**Why it matters**

A small Siderita refactor that earns a ratchet decrease cannot land without the Android SDK, and it redeploys every app. The ratchet rule (lower it in the same commit) collides with the landing's cost model.

**Fix**

Scope the shared inputs:

- the architecture baseline only through the rows for paths the project owns;
- `docs/projects.toml` only through the project's own table, hashed canonically.

Alternatively, let the landing skip verification for projects whose relevant rows did not change.

**Effort / prefix:** M, `suite:`

### TOOL-10: `qmllint-cxxqt.sh` can lint against another app's module (Important, Correctness)

**Evidence**

- `scripts/qmllint-cxxqt.sh:120-126` does `find "$(target_directory …)/release/build" -path '*/out/qt-build-utils/qml_modules' … | sort -nr | sed -n '1s/^[^ ]* //p'`, which takes the newest module by mtime.
- `worktree.sh` points every session and every application at one `…/.worktrees/.cargo-target`. With Siderita, Grafita and Fluorita release builds there, the newest module wins whichever app is being linted. The `uri` then comes from that module, so Siderita's QML is linted against Grafita's `plugin.qmltypes`.
- `docs/contracts/landing.md` lists `qmllint-cxxqt.sh` among a session's fast checks. But it needs a release module ("run build-production.sh"), and `production_artifact.py` refuses `run-build` in a session worktree. So in a session it either fails or reads a stale or foreign module.
- `scripts/test-qmllint-target.sh` covers only a single-module target.

**Fix**

- Select the module whose URI matches the app. The app's `build.rs` declares the URI, or filter `release/build/<crate-name>-*/`.
- Fail on more than one candidate.
- Either state that sessions do not run qmllint, or allow a non-production release build in the session target.

**Effort / prefix:** S, `suite:`

### TOOL-11: language exemption markers accepted anywhere (Important, Correctness)

**Evidence**

- `scripts/check-language-contract.py:70` returns `[]` for any file whose first ten lines contain `language-contract: allow-non-english`.
- Line 74 enables literal-stripping for any non-QML suffix that carries `language-contract: product-copy`.
- `docs/standards/language.md` limits `product-copy` to "a Rust or C++ file" and `allow-non-english` to fixtures that test international input.
- In a scratch evaluation, `suspicious_lines` returns `[]` for a Spanish `.md` paragraph once either marker is prepended.
- In the checkout, 16 Markdown development records carry `product-copy` in their heads, for example:
  - `docs/superpowers/plans/2026-09-21-hematita-h1-foundation.md:1` (4 suspicious lines hidden);
  - `docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md` (6 hidden);
  - several `siderita/docs/evidence/*.md`.
- Nothing prevents a canonical document (`is_canonical`) from carrying `allow-non-english`.

**Why it matters**

The ratchet can be emptied without translating anything. This is not the ratcheted debt: it is an exemption that grows without any record.

**Fix**

- Honour `product-copy` only for `.rs`, `.cpp`, `.cc`, `.h` and `.hpp`, and `allow-non-english` only under fixture or test paths.
- Never honour either marker for `is_canonical` paths.
- Migrate the Markdown records through a declared scanner migration: stage the scanner, then add the "Resolved language debt" evidence.

**Effort / prefix:** S, `suite:`

### TOOL-12: landing tests use doubles for every guard and hook (Minor, Tests)

**Evidence**

`scripts/test-land-unit.py:679-693` replaces `check-staged-units.py`, `commit_scope.py`, `check-language-contract.py`, `check-architecture-contract.sh` and `check-documentation-contract.sh` with doubles that "record the call and pass unless told to fail". The hooks are record-only (`:905`).

No real landing has happened on `main`: no evidence record other than `docs/contracts/landing.md` contains `## Landing`, and LND-1-F was sealed by hand.

The real-guard trial run for this audit landed a docs-only suite unit successfully. The project path (owner check, verify-only, deploy, version bump plus the real `commit-msg` version transition) is still untested against the real guards.

**Fix**

Add one integration test in a temporary clone of the real `scripts/` and registry, like the trial above, for a `suite` unit and for a versioned fixture project with trivial production entries.

**Effort / prefix:** M, `suite:`

### TOOL-13: CI coverage gaps (Minor, Tests)

**Evidence**

- `contracts.yml:34-51` never runs `scripts/test-language-contract.py`, `scripts/test-production-artifacts.sh` or `scripts/test-production-common.sh`.
- The last two run only inside project `verify-production.sh`, and nothing runs `test-language-contract.py`.
- The landing's guard chain runs no `test-*`, which is how TOOL-1 went unnoticed locally.
- Documented and accepted gaps: merges are not audited in CI; intermediate commits are not re-checked; `--history-scope-only` reads today's registry. The first becomes live if TOOL-5 merges happen.

**Fix**

- Add the three tests to the "Commit scope" step.
- Run `test-architecture-scanners.sh` and `test-version-contract.py` in `pre_guards` when a unit changes `docs/projects.toml` or `scripts/`.

**Effort / prefix:** S, `suite:`

### TOOL-14: errata over-excuse; `--quiet` is noisy (Minor, Correctness)

**Evidence**

- `partition_errata` (`scripts/documentation_contract.py:2675`) excuses every error whose label is the listed inventory (`if label in errata`), not the recorded "Pathspec written on one line" defect. A different, new error in that record would be swallowed too.
- `.githooks/pre-commit` passes `--quiet`, yet every commit prints about 60 `erratum (…)` lines to stderr.

**Fix**

- Key each erratum by (path, error-message digest), or match the recorded class of errors.
- Under `--quiet`, print one summary line per erratum.

**Effort / prefix:** S, `suite:`

### TOOL-15: stale root documents (Minor, Documentation truth)

**Evidence**

- `STATUS.md:78`: "entries complete for all seven projects". The registry has 9.
- `STATUS.md:104-108` still reports GOV-2's "existing manifests are intentionally stale" (2026-08-03) as the suite's evidence.
- `ROADMAP.md:137` labels an archived plan "`[active plan](docs/plans/archive/2026-08-04-spanish-product-copy.md)`".
- `ROADMAP.md:166-176`: the "Project implementation fronts" table omits Hematita and Magnetita Android.
- `README.md:14-24` omits Magnetita Android.
- `docs/contracts/versioning.md:93`: "The six top-level products currently versioned…". `version_tool.py check` reports 8 owners, including Magnetita Android and Hematita.
- `AGENTS.md:85`: "Siderita may consume narrow Grafita and Fluorita domain/seams". `docs/standards/architecture.md:21` also allows `hematita-core::usage`.
- `docs/README.md`'s Governance list omits `contracts/landing.md`.

**Fix**

Update these lists. Replace counts with pointers to `docs/projects.toml`, as `STATUS.md` already recommends.

**Effort / prefix:** S, `suite:`

### TOOL-16: versioning contract contradicts the landing (Minor, Documentation truth)

**Evidence**

- `docs/contracts/versioning.md:61-77` ("Agent workflow") step 3: "update the version before the canonical production build. Use the helper…". Step 4 says to run `complete-production.sh`.
- `AGENTS.md` ("a session never bumps"), `docs/contracts/landing.md` (`unbump` drops session bumps with a warning) and `production_artifact.py` (refuses builds in sessions) all say the opposite.
- The trailing sentence "Through land-unit.py the landing performs steps 3 to 5" does not remove the imperative.

**Fix:** Rewrite the section as "What the landing does", with the manual commands reserved for the hand-sealed exceptions such as archiving.

**Effort / prefix:** S, `suite:`

### TOOL-17: unregistered `docs/superpowers/` plan and spec tree (Minor, Documentation truth)

**Evidence**

- `docs/superpowers/plans/*.md` and `docs/superpowers/specs/*.md` form a second plan and spec system. It is absent from the `docs/README.md` source-of-truth map.
- It is still load-bearing: `docs/projects.toml:347` registers `docs/superpowers/specs/2026-09-21-hematita-design.md` as a Hematita context document, and `scripts/land-unit.py:8` cites a spec there as its behavioural source.
- `docs/governance/documentation.md` says "Do not create a second documentation tree when a canonical owner already exists."

**Fix**

- Register it (decisions, discussions or plans), or migrate its accepted content into `docs/decisions/` and project plans.
- Make `land-unit.py` cite `docs/contracts/landing.md`.

**Effort / prefix:** S, `suite:`

### TOOL-18: duplicated helpers (Minor, Architecture and reuse)

**Evidence**

- Distinct `git` runners:
  - `audit-version-commits.py:32 git`;
  - `check-staged-units.py:43 git`;
  - `commit_scope.py:224 git_output` plus `:259 git_blob`;
  - `documentation_contract.py:1587 git_command`;
  - `landing.py:703 git_run`;
  - `land-unit.py run`;
  - `production_artifact.py:76 git_state`.
- About 11 independent `docs/projects.toml` loaders with different validation (for example `production_artifact.load_registry` silently keeps the last duplicate `id`), plus an inline Python loader in `worktree.sh`.
- The language baseline is parsed by `check-language-contract.py:126 read_baseline`, `:158 read_historical_baseline` (which accepts duplicates and zero counts that `read_baseline` rejects) and `commit_scope.parse_language_ratchet`.
- `commit_scope.py:370` and `:419` repeat the same evidence-root scan almost line for line.

**Fix**

- One `repo_git.py` (runner, blob, batch reader).
- One registry loader in `project_registry.py`, loaded from HEAD where required.
- One ratchet parser per file format.
- Fold the two `has_staged_*` functions into `has_staged_field(marker)`.

**Effort / prefix:** M, `suite:`

### TOOL-19: landing and worktree robustness (Minor, Correctness)

**Evidence**

- `check_and_build` (`scripts/land-unit.py:698`) reads the manifest after the entries but never re-runs `check --require-verified`. The recorded fingerprints are therefore not proven current.
- No subprocess anywhere has a timeout. `git fetch` and `git push` in `land-unit.py`, and `fetch` in `worktree.sh:95`, can hang indefinitely on a stalled remote.
- `worktree.sh close` (`:178`, `branch --quiet -D`) deletes the branch once any inventory for the unit exists on `origin/main`. Commits added to the branch after its landing are discarded.
- `landing.git_run` uses `text=True`. A non-UTF-8 path raises `UnicodeDecodeError`, which is not a `LandingError`, so the user sees a traceback.

**Fix**

- Re-check after the build.
- Add timeouts to network `git` calls.
- In `close`, refuse when the branch tip is newer than the landed commit's date, or compare trees.
- Decode with `surrogateescape`.

**Effort / prefix:** S, `suite:`

### TOOL-20: the version audit replays all history on every push (Minor, Performance)

**Evidence**

`audit-version-commits.py` audits 441 commits in 65.6 s. For each commit it reads 2 registries, the subject, `diff-tree`, and each version blob through its own `git show` (`:200-215`). Cost grows linearly on every CI push.

**Fix**

- Audit only `github.event.before..HEAD` in CI, with a full replay on a schedule.
- Use `git cat-file --batch` for the blobs.

**Effort / prefix:** S, `suite:`

### TOOL-21: agent context omits a shared crate's other consumers (Minor, Architecture and reuse)

**Evidence**

`python3 scripts/agent-context.py celestina-rs/crates/siderita-ops` prints the celestina-rs and Siderita documents only. Fluorita and Hematita also link `siderita-ops` (`fluorita/Cargo.toml:36`, `hematita/Cargo.toml:38`). An agent changing it is never shown those consumers' contracts.

The root cause is the same as TOOL-3: the registry holds ownership, not the dependency graph.

**Fix:** Derive consumers from `cargo metadata` and print their context as well.

**Effort / prefix:** S, `suite:`

### TOOL-22: toolchain drift never invalidates an artifact (Minor, Correctness)

**Evidence**

- `run_build` records `toolchain(root)` (cargo, rustc, cmake, cxx, qt). `validate_manifest` (`scripts/production_artifact.py:618-640`) never compares it.
- The C++ and Qt apps have no pinned toolchain file among their inputs.

**Why it matters**

A system Qt upgrade (for example, a distribution update) leaves every artifact "current and verified" although it links against a different Qt.

**Fix:** Compare the recorded `qt` and `cxx` probes, and surface a mismatch as a verification error so the artifact is re-verified.

**Effort / prefix:** S, `suite:`

## Limits

- No production build, verification with a real toolchain or deploy
  ran, and no project-path landing ran; the one trial landing was
  documentation-only, in a scratch clone.
- The timings are this container's; a normal workstation differs.
- Merges are not audited in CI, intermediate commits are not
  re-checked, and `--history-scope-only` reads today's registry; these
  are documented and accepted gaps (TOOL-13), not re-audited here.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-1 | `AUD-1-B` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | TOOL-1, TOOL-2, TOOL-13 |
| P-2 | `AUD-1-D` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | TOOL-3, TOOL-4, TOOL-21, TOOL-22 |
| P-20 | `AUD-1-F` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | TOOL-5, TOOL-6, TOOL-7, TOOL-8, TOOL-9, TOOL-10, TOOL-11, TOOL-12, TOOL-14, TOOL-15, TOOL-16, TOOL-17, TOOL-18, TOOL-19, TOOL-20 |

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Ordered by value divided by effort.

1. **`suite:` Restore green CI and cover Hematita** (TOOL-1, TOOL-2 guard half, TOOL-13). Derive the style-guard roots and the expected version owners from the registry, add the three missing tests to `contracts.yml`, and run the registry-coupled fixture tests in `pre_guards` when `scripts/` or `docs/projects.toml` change. Then land a follow-up `hematita:` unit for the two visual violations.
2. **`suite:` Derive production inputs from Cargo and register Magnetita Android's inputs** (TOOL-3, TOOL-4, TOOL-21, TOOL-22). Fingerprint the `cargo metadata` path-package closure, add a guard against empty or incomplete inputs, and compare the toolchain probes.
3. **`suite:` Make the hooks judge the index with committed rules** (TOOL-5, TOOL-6, TOOL-14). Run the guards from an extracted `HEAD:scripts`, run the documentation contract over the index, add `pre-merge-commit`, and scope the errata.
4. **`suite:` Speed up the documentation guard and the version audit** (TOOL-7, TOOL-20). Memoize the verification of immutable inventories, batch blob reads, and audit only the pushed range in CI.
5. **`suite:` Make landing production safe and proportional** (TOOL-8, TOOL-9, TOOL-19). Deploy only after the push, scope shared verification inputs per project, re-check after the build, add network timeouts, and make `close` safe.
6. **`suite:` Fix qmllint module selection and the language markers** (TOOL-10, TOOL-11). Pick the app's own QML module, and restrict the exemption markers by suffix and path, with a declared scanner migration.
7. **`suite:` Add real-guard landing tests and consolidate helpers** (TOOL-12, TOOL-18). Add an integration landing test on a clone of the real `scripts/`, and add a shared `git` runner, registry loader and ratchet parser.
8. **`suite:` Correct the governance documents** (TOOL-15, TOOL-16, TOOL-17). Update the project lists and counts, rewrite the versioning "Agent workflow", and register or migrate `docs/superpowers/`.
