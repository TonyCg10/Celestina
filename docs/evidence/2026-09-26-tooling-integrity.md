# Evidence: tooling integrity, speed and governance documents

- **Date:** 2026-09-28
- **Scope:** `AUD-1-F` (program P-20) of the [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md): TOOL-5, TOOL-6, TOOL-7, TOOL-8, TOOL-9, TOOL-10, TOOL-11, TOOL-12, TOOL-14, TOOL-15, TOOL-16, TOOL-17, TOOL-18, TOOL-19, TOOL-20 of the [tooling audit](2026-09-26-monorepo-audit-tooling.md), and the items carried to the unit: TOOL-1's third fix, TOOL-13's second fix, per-project toolchain probes, one owner for the input path matchers, and halted projects left out of the consumers `agent-context.py` prints
- **Environment:** session worktree `unit/suite/AUD-1-F` on base `b510455` (AUD-1-B, AUD-1-C, AUD-1-D and HALT-SHELL merged), Linux container, Python 3.11.15, Git 2.43.0, rustc 1.94.1; no Qt, CMake application build, libmpv, Android SDK or Gradle, and no production entry may run in a session worktree
- **Artifact:** the landing builds it; the unit changes shared verification inputs (`scripts/production_artifact.py`, `scripts/complete-production.py`, `scripts/qmllint-cxxqt.sh`, `docs/projects.toml`), so every registered project that is not halted re-verifies once at landing

## Procedure

Every suite the unit changes, the three guards, the version tools, and the
RED runs, which replace one changed script by its `HEAD` copy (`git show
HEAD:<path>`) or run the new fixture against the old hooks, then restore it:

```sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
python3 scripts/audit-version-commits.py
python3 scripts/audit-version-commits.py --since HEAD~5
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
sh scripts/test-commit-scope.sh
sh scripts/test-staged-units.sh
sh scripts/test-documentation-contract.sh
bash scripts/test-land-unit.sh
sh scripts/test-production-artifacts.sh
sh scripts/test-worktree.sh
sh scripts/test-qmllint-target.sh
python3 scripts/test-language-contract.py
```

The documentation guard was timed on the whole worktree and in hook mode,
which checks the index out into a temporary directory and runs the contract
there, exactly as `scripts/git_hooks.py` does. The batched Git reads were
compared with the per-path reads they replace over every tracked inventory.
The old and the new language scanner were both run over the final tree and
their per-file counts compared.

## Result

- **Exit:** every command above exits 0 on the final tree; `test-land-unit.sh` runs 82 tests, `test-language-contract.py` 17, and the architecture, documentation and language guards report OK (148 legacy files ratcheted, unchanged).
- **Observed:** RED before, GREEN after, for each fixture the unit adds:

| Finding | Fixture | RED (old code) | GREEN |
|---|---|---|---|
| TOOL-5, TOOL-6 | `test-staged-units.sh` hook repository: a broken link staged then removed from the worktree; a worktree-only broken link; an automatic merge that brings a broken link; worktree guards replaced by `sys.exit(0)`; a clean merge | the old hooks accepted the partially staged link, refused the commit for the unstaged worktree link, let the merge in (no `pre-merge-commit`), and were disabled by the no-op guards | all five behave as designed |
| TOOL-6 | `test-commit-scope.sh`: `commit-msg` with a no-op worktree `commit_scope.py` and an invalid subject | not run against the old hook (the RED is the staged-units case above) | refused by HEAD's guard; a valid subject passes |
| TOOL-14 | `test-documentation-contract.sh` errata case | the old `partition_errata` excused a stale SHA-256 error of the listed inventory | only the `unbounded-pathspec` errors are excused, an unknown class and a stale entry are errors, `--quiet` prints one line |
| TOOL-9 | `test-land-unit.py` `test_affected_projects_read_only_their_slice_of_shared_inputs`; `test-production-artifacts.py` `test_verification_reads_only_the_projects_slice_of_shared_inputs` | the old fingerprint went stale for another project's ratchet row (`check` exit 1) | only the owning project's slice counts |
| TOOL-8 | `test_deploy_runs_after_the_push_and_resumes`, `test_abort_after_the_push_reports_the_pending_deploy`, `test_hook_failure_stops_without_commit` | the old landing ran `deploy-production.sh` inside `build_if_stale`, before the guards, the hooks and the push | a stopped landing deploys nothing; the deploy follows the push, resumes with `--continue`, and `--abort` names what to deploy |
| TOOL-19 | `test_stale_artifact_after_the_build_stops`; `test-worktree.sh` close after a post-landing commit | the old `close` deleted the branch with the late commit (exit 0) | the landing stops at `build_if_stale`; `close` refuses |
| TOOL-1 (third fix) | `test_red_contracts_on_main_stops_the_preflight`, `test_unreadable_contracts_status_only_warns` | no check existed | a red run stops `preflight`; `--accept-red-ci` lands; an unreadable status warns |
| TOOL-13 (second fix) | `test_same_product_advanced_rebumps_and_merges` | `pre_guards` ran no fixture test | a unit that changes `scripts/` runs `test-architecture-scanners.sh` and `test-version-contract.py` first |
| TOOL-10 | `test-qmllint-target.sh` cases 4 and 5 | the old script linted against the newest module, `org.other` | the app's own `org.example` module is used; without it the lint stops |
| TOOL-11 | `test-language-contract.py` marker fixtures | four of the five new tests fail on the old scanner | all pass |
| TOOL-12 | `RealGuardLanding` in `test-land-unit.py` | no such test existed | a documentation-only `suite` unit lands in a clone through the real guards and hooks in about 43 s |
| probes | `test_only_the_declared_toolchains_are_compared` | the old manifest recorded `cmake`, `cxx` and `qt` for a pure-Rust project | only the declared `rust` probes are recorded and compared |
| halted consumers | `test-documentation-contract.sh` halted consumer | the old `agent-context.py` listed the halted app as a consumer | it is left out |

- **Observed:** timings on this container:

| Command | Before | After |
|---|---|---|
| `sh scripts/check-documentation-contract.sh --quiet` (whole worktree) | 44.0 s | 3.2 s |
| documentation contract in hook mode (over the index) | 47.1 s for the whole pre-commit hook | 3.3 s, plus 1.4 s to check the index out |
| whole `pre-commit` hook | 47.1 s | 7.6 s |
| `python3 scripts/audit-version-commits.py` (456 commits) | 150.5 s (65.6 s in the audit) | 7.5 s |
| `python3 scripts/audit-version-commits.py --since HEAD~5` | not available | 2.0 s |

- **Observed:** the batched reads equal the per-path reads: over all 254 tracked inventories, the newest commit touching each (`git log -1 -- PATH`), each endpoint's numstat against its parent and each endpoint's subject are identical.
- **Observed:** the language scanner measures the same debt before and after the change: over the final tree the old and new scanners report identical per-file counts (148 files), because the Markdown records that relied on the `product-copy` marker cite product copy only in code, once one line of `docs/superpowers/specs/2026-09-22-siderita-bottom-chrome-design.md` quotes a button label as a code span instead of in plain quotes. The markers left in those records are inert; removing them is a follow-up that changes no count.
- **Resolved language debt:** `scripts/check-language-contract.py`

The field above declares that the scanner changed in this unit. No
`language-baseline.tsv` row moves, by design: the hooks judge a commit with
the rules committed in its parent, so a scanner change that moved a row could
not land together with the move (AGENTS.md: a semantic rule change first
lands compatible behaviour).

Per finding:

- **TOOL-5, TOOL-6:** `.githooks/committed-rules.sh` extracts `HEAD:scripts` and runs `scripts/git_hooks.py`, which runs every guard from that copy; the language and documentation contracts run over the index, checked out into a temporary directory with Git pointed at a private copy of the index. `.githooks/pre-merge-commit` runs them for an automatic merge. `commit-msg` called with an option stays the scope tool the tests and the landing use. Partially staged plans are judged by their staged bytes, so no separate refusal was added.
- **TOOL-7:** the historical inventory replay reads blobs and commits through one `git cat-file --batch`, the index and status once, the newest commit of every inventory in one `git log`, and every endpoint's numstat in one `git log --numstat`; the per-run memo replaces about 16,000 processes with 79 and one batch process. No cache survives between runs, so CI and the hook see the same answer.
- **TOOL-8:** `build_if_stale` builds and verifies; the new `deploy` step runs the deploy and status entries after the push; `--abort` before the push says nothing was deployed and after it names what to deploy.
- **TOOL-9:** `docs/projects.toml`, `scripts/architecture-baseline.tsv` and `scripts/qmllint-baseline.tsv` count per project in the verification fingerprint and in the landing's matching.
- **TOOL-10:** `qmllint-cxxqt.sh` takes the URI from `build.rs` and the module of the app's own package; the landing contract says sessions do not run it.
- **TOOL-11:** `product-copy` counts only in Rust and C++ sources, `allow-non-english` never in a document, neither in a canonical path; a Markdown record outside the canonical paths may cite product copy as a string literal inside a closed fenced block or an inline code span that does not cross a blank line (narrowed in fix round 1).
- **TOOL-12:** `RealGuardLanding`, for a `suite` unit. The versioned fixture project the audit also proposed is not added (see Limits).
- **TOOL-14:** each erratum names its defect class; `--quiet` prints one line per erratum.
- **TOOL-15:** root `STATUS.md`, `ROADMAP.md`, `README.md`, `docs/contracts/versioning.md`, `AGENTS.md` (Hematita among Siderita's consumed seams) and the `docs/README.md` governance list now match the registry or point to it.
- **TOOL-16:** the versioning "Agent workflow" became "What the landing does".
- **TOOL-17:** `docs/superpowers/` is registered in `docs/README.md` and `docs/governance/documentation.md` as the working tree of session plans and specs; `land-unit.py` cites `docs/contracts/landing.md`.
- **TOOL-18:** `scripts/repo_git.py` is the Git runner and batch reader of the documentation contract, the version audit, the language contract, the staged-unit and commit-scope guards, `landing.py` and `production_artifact.py`; `project_registry.parse_registry`/`load_registry` read the registry for the documentation contract, `production_artifact.py` (which now refuses a duplicate `id`), `complete-production.py`, `audit-version-commits.py` and `worktree.sh`; `parse_baseline` in the language contract is the one language-ratchet parser, which `commit_scope.py` reads from HEAD; `has_staged_field` replaces the two copied evidence scans.
- **TOOL-19:** the post-build `check --require-verified`, fetch and push timeouts in `land-unit.py` and `worktree.sh`, `surrogateescape` decoding, and a `close` that keeps a branch with commits made after its landing.
- **TOOL-20:** `audit-version-commits.py --since REV` and batched reads; `contracts.yml` audits the pushed range and replays all history weekly.
- **TOOL-1 (third fix):** `preflight` refuses while the latest `contracts` run on `main` failed, read without credentials from GitHub's public API; the landing contract documents branch protection as the author's step.
- **TOOL-13 (second fix):** `pre_guards` runs the two registry-coupled fixture tests when a unit changes `docs/projects.toml` or `scripts/`.
- **Probes:** `toolchains` in each registry table selects the probes; the shell's table is left untouched (halted), so it keeps every probe.
- **Matchers:** `path_reaches`, `pattern_covers` and `inputs_changed` in `production_artifact.py` are the one owner; `landing.py` and `agent-context.py` call them.

## Limits

- No production build, verification, deployment or landing of a project unit
  ran: Qt, CMake application builds, libmpv and the Android SDK are absent,
  and a session worktree refuses production runs. Every artifact's
  verification fingerprint changes with this unit (the fingerprint contract
  and shared verification inputs changed), so the landing re-verifies every
  registered project that is not halted once; the author's machine must have
  their toolchains.
- `qmllint-cxxqt.sh` ran only against the fixture module; its real selection
  runs inside each application's `verify-production.sh` at landing.
- The real-guard landing test covers a documentation-only `suite` unit. A
  versioned project landing through the real guards needs a buildable fixture
  project in the real registry, which the real architecture guard would have to
  accept; the project path stays covered by the doubled fixtures.
- The GitHub status read was exercised against a local file and against the
  public API of this repository by hand; it needs no credentials only while the
  repository is public. Branch protection is the author's step on GitHub.
- The first commit that carries the new hooks is judged by the worktree's
  runner with HEAD's guards, which is also what the landing of this unit does.
- `land-unit.py` keeps its own process runner for Git so that `LAND_UNIT_GIT`
  and the tests' Git wrapper keep working; `landing.py` uses `repo_git.py`.
  `architecture_scanners.py` keeps its own registry parsing, because it is
  loaded as a HEAD rule module and by `runpy` without the scripts directory on
  its path.

## Follow-up

- Remove the now inert language markers from the Markdown records under
  `docs/superpowers/` and `siderita/docs/evidence/`; no count changes.
- Enable GitHub branch protection on `main` requiring `suite-contracts`
  (author).
- A versioned fixture project for the real-guard landing test, if the author
  wants the project path proven end to end.

## Fix round 1

The review of 2026-09-28 found one High, one Medium and five Low items; each
is fixed on the branch, with the reviewer's probes turned into tests:

- **F1 (High):** after a lost push race, or an abort before the push and a new
  landing, `build_if_stale` found the first attempt's artifact current and the
  deploy list empty, so the unit landed and nothing was installed. Every
  affected deployable project now waits for the `deploy` step whether or not a
  build ran, since deploying verified bytes again is harmless, and the Landing
  section gains a `Deploy` line. New `test_abort_before_the_push_then_reland_deploys`;
  `test_push_rejected_retries_then_stops`, `test_push_offline_keeps_main_and_resumes`
  and `test_other_product_advanced_needs_no_build` now assert the deploy. RED on
  the previous `land-unit.py`: three of those tests fail.
- **F2 (Medium):** the Markdown citation exemption is scoped: never in a
  canonical path, only literals inside a closed fenced block or an inline code
  span that does not cross a blank line; a `qsTr()` call in prose and the text
  after an unclosed fence are scanned. The reviewer's five shapes are
  `test_a_citation_does_not_exempt_prose_around_it` (RED: 8 of its subtests
  fail on the previous scanner). The current tree still needs the scoped
  exemption for the plans and specs under `docs/superpowers/` and one Siderita
  evidence record; the old and new scanners still report identical per-file
  counts (148 files). `docs/README.md` and `docs/standards/language.md` say so.
- **F3 (Low):** the shared QML module's registry slice holds every project's
  `id`, `kind`, `path` and `source_roots`, which the style guard reads;
  `test_the_shared_qml_module_reads_every_qml_root`.
- **F4 (Low):** `--accept-red-ci` is kept in the landing state (`red_ci`) and
  written as a `CI` line of the Landing section;
  `test_red_contracts_on_main_stops_the_preflight` asserts it.
- **F5 (Low):** an `http.client.HTTPException`, such as `IncompleteRead`, is a
  warning like any unreadable status; `test_truncated_contracts_status_only_warns`
  (RED: the previous code raised).
- **F6 (Low):** AGENTS.md states the one-time worktree fallback, as the change
  policy does.
- **F7 (Low):** `RealGuardLanding` no longer skips: with no active suite plan
  its setup opens a fixture plan and names its checkpoint in the root
  ROADMAP. That path was checked against the documentation contract on a copy
  of the valid fixture; the landing itself still uses the active suite plan
  while one exists.

Commands, all exit 0 after the round: the three guards, `version_tool.py
check`, `audit-version-commits.py`, `test-land-unit.sh`,
`test-language-contract.py`, `test-production-artifacts.sh`,
`test-documentation-contract.sh` and `test-staged-units.sh`.

## Landing

- **Base revision:** `a64dc43d69107e0ec8ef01d031b79a7725928bc6`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: the toolchain changed since the build; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:a07296319641a7bb878fddba20ba6faa4c0bbda5e9a3377366b2145606354c2d, verification_fingerprint sha256:bf0382b006c48074ba5ac7d9fcf3bb71d708852ec3f0d3e9e5530ff2a766112e; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4ac5523bc556c02252a6b668fe8af67ce2291a6738b3eb92285034765622b5d8, verification_fingerprint sha256:c156fc9477e719e78cb28931bbfc9eacc73e38aed1d638cbd6f6cc64497cd64b; siderita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:41961220fa5cf9dc774e80fac2444fd1a638d1e05abdada292cc2169eb7f42e3, verification_fingerprint sha256:8065598d2adb0646a70fa97837e1d7d32ef3cdd761e8b3a356cbddaecfc4d0de; magnetita build: complete-production.sh exit 0, manifest source_fingerprint sha256:d449f2dac276377685eef183809ffe4d5c0bef23e8261bdfaabd03307c9a5a99, verification_fingerprint sha256:801ebe2922890002c4c8047f031976bd213cfe2b41baeba3a7a59e4818c7157f; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1a3ab6c6f14e4c17055fc0ce34a28982b54ee41f3862de330573bcaf96b1fead, verification_fingerprint sha256:c683ad765577629ef8a643cd8e30d086d99716662f751985a8ca6cb3120ce423; grafita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:e9efd5f1ca9ed03a3342f931649aad20f9a985c94047e5631aaa38e85a0b8a1f, verification_fingerprint sha256:6f0abd9586175e80fea346fceb73a033beff228dcf56ed5fd930cfa18fb08153; fluorita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:2d0c3004eb0d47d098bac6fb720efdd84b6ef735a639d80c4f4798307b1bf858, verification_fingerprint sha256:929424321d59b7342f212777a44068e3d312dc5f66316110074087e757eff1ee; hematita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:447b0da876e9635b1c0acc703416147ab76a569d39982d76d9bc4d5149eb9415, verification_fingerprint sha256:627a5ba9353520c02b60837a75459854d60278d1bbd3054e81082dd089f19c00
