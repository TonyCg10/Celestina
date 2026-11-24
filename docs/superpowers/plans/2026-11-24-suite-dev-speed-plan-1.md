# Suite development speed — plan 1: open the plan (U00) and bound the build cache (U01)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the development-speed work a place in the repository, then make session builds lighter and stop the session build cache and landed worktrees from piling up.

**Architecture:** Two deliveries, one after the other. U00 is one hand commit on `main` (documents only): it archives the finished RSH-1 plan and opens the suite plan `dev-speed`. U01 is one ordinary ledger unit, `DEV-1-A`: `worktree.sh open` writes a lighter dev profile, `build_trees.py` learns to trim the shared session target above a cap, `worktree.sh close` calls it instead of deleting the cache, and `land-unit.py` closes the landed unit's session worktree.

**Tech Stack:** POSIX `sh` (`scripts/worktree.sh`, fixture tests in `scripts/test-worktree.sh`), Python 3 standard library (`scripts/build_trees.py`, `scripts/land-unit.py`, `unittest` fixtures), Cargo config profiles, GitHub Actions YAML.

**Spec:** `docs/superpowers/specs/2026-11-24-suite-dev-speed-design.md` (draft today at `/home/toni/CODIGO/2026-11-24-suite-dev-speed-design.md`; U00 moves it into the repository). U02, U03 and U04 of the spec get their own plans, written just before each runs.

## Global Constraints

- Repository text is English; the author is addressed in Spanish.
- Never edit the canonical checkout `/home/toni/CODIGO/CELESTINA`. Work in a session worktree opened from it with `scripts/worktree.sh open suite <UNIT>`.
- Session commits use `suite-maintenance: <Imperative> …`; the first word must be in `IMPERATIVE_VERBS` of `scripts/commit_scope.py` (`Add`, `Fix`, `Remove`, `Update` are safe). Check with `echo <one changed path> | python3 scripts/commit_scope.py --check "suite-maintenance: <subject>"` (it reads paths from stdin; never run it without a pipe).
- A unit changes an existing plan file only in its own ledger row. Never write a `## Landing` section in evidence; the landing appends it.
- A ledger row's `Automated evidence` cell is a Markdown link relative to the plan file. A row not yet landed uses `—` in `Files / areas` and `Diffstat`.
- The session never runs `scripts/land-unit.py`, `git push`, a merge into `main`, or any `*-production.sh`. It hands the exact command to the coordinating session, which asks the author.
- Never run two smokes at once and nothing against the author's live session. This plan runs no smoke.
- Session Cargo builds for measurements use a target under `<worktree>/target/`, never a directory beside the worktrees.
- Cap constant: `SESSION_TARGET_CAP_GIB = 40`. Dev profile: `debug = "line-tables-only"` for workspace members, `debug = false` for every dependency.
- None of `scripts/worktree.sh`, `scripts/land-unit.py`, `scripts/build_trees.py` or `.github/workflows/*` is a verification input (`verification_input_patterns`, `scripts/production_artifact.py:509-549`), so the U01 landing rebuilds and re-verifies no project.

## Review Focus

1. **Trim while another session is compiling.** Removing artifacts under a running `cargo` corrupts that build; the trim must skip when Cargo's lock on the profile is held (Task 3).
2. **Auto-close of a unit another branch is stacked on.** The dependent branch must still land after its dependency's branch is gone (Task 5).
3. **Auto-close refusal.** Uncommitted session work, or a commit made after the landing, must survive and the landing must still exit 0 (Task 5).
4. **A target that is not a directory, or a symlink.** `trim_session_target` must never follow a symlink out of the target (Task 3).
5. **Session worktrees opened before this unit.** They keep the old `.cargo/config.toml`; `close` must still accept them and must remove a `target/` directory left inside (Task 4).

---

## U00 — open the `dev-speed` plan (hand commit on `main`, no ledger unit)

The landing refuses a unit that moves a plan it is not the owner row of and a unit that touches two active plans, so archiving RSH-1 and opening its successor is a hand commit, as `d286dabc` and `5ccf48d7` were. The session prepares the commit on a branch; the author fast-forwards `main` to it.

**Precondition:** `unit/suite/CEL-TRACKS-OPEN` has landed (it edits the root roadmap and needs a row in the RSH-1 ledger). Check: `git -C /home/toni/CODIGO/CELESTINA branch --list 'unit/suite/CEL-TRACKS-OPEN'` prints nothing, or the coordinating session says to proceed. If it has not landed, stop and report.

### Task 1: Archive RSH-1 and open DEV-1

**Files:**
- Move: `docs/plans/active/2026-10-14-rust-shell.md` → `docs/plans/archive/2026-10-14-rust-shell.md` (`git mv`; add `- **Closed:** 2026-11-24` and `- **Successor:** DEV-1, [the development speed plan](../active/2026-11-24-dev-speed.md)` after `Plan ID`; set `- **Status:** done`; nothing else changes)
- Create: `docs/plans/active/2026-11-24-dev-speed.md` (from `docs/templates/plan.md`)
- Create: `docs/decisions/0013-development-speed.md` (shape of `docs/decisions/0012-suite-conventions.md`; **Status:** accepted; **Date:** 2026-11-24)
- Create: `docs/superpowers/specs/2026-11-24-suite-dev-speed-design.md` (copy of the draft; **Status:** approved 2026-11-24; drop the "Draft location" line)
- Create: `docs/superpowers/plans/2026-11-24-suite-dev-speed-plan-1.md` (this plan)
- Modify: `ROADMAP.md` (line 4 `Active implementation checkpoint: DEV-1`; a `## DEV-1 — Development speed` section before `## Project implementation fronts`; in the RSH-1 section the link text "the active plan" becomes "the archived plan" with the archive path)
- Modify: `STATUS.md` ("Active cross-project work": DEV-1 is active with a link to the plan; the RSH-1 paragraph moves to the top of "Completed cross-project work" and says the plan is archived)
- Modify: `docs/plans/active/README.md`, `docs/plans/archive/README.md`, `docs/decisions/README.md` (their lists)
- Modify: every other tracked file whose link to `plans/active/2026-10-14-rust-shell.md` the documentation guard reports as broken; change the path only

**Interfaces:**
- Produces: plan file `docs/plans/active/2026-11-24-dev-speed.md` with `Plan ID: dev-speed`, `Scope: suite`, `Implementation checkpoint: DEV-1`, `Author-validation checkpoint: none`, and this ledger (template columns), which Task 2 onwards rely on:

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| DEV-1-A | `suite:` | planned | — | — | Add a lighter session dev profile, a cap on the session Cargo target and the closing of a landed unit's worktree | — | None |
| DEV-1-B | `suite:` | planned | — | — | Add tiers to the agent preflight and a budget for its first tier | — | None |
| DEV-1-C | `suite:` | planned | — | — | Add the light lane, the review cap and the sensitive-path list | — | None |

  The smoke unit (U03 of the spec) belongs to the shell, and a suite plan carries only `suite:` rows: it gets its row in a `celestina` plan when its own plan is written. Say so in this plan's **Exclusions**.

- [ ] **Step 1:** From the canonical checkout: `scripts/worktree.sh open suite DEV-1-OPEN`; work only in the printed directory.
- [ ] **Step 2:** Write the plan file. Hypothesis: "A session can read about 40 KB instead of 373 KB before acting, and build against a cache that stays under 40 GiB, without any landing verifying less than it does today." Tangible outcome, scope, exclusions and build order come from the spec's sections 2 and 4 (U01, U02, U04; U03 named as the shell's). Implementation exit: every row `done`, the `now` budget test green, the session target under the cap after a landing.
- [ ] **Step 3:** Write ADR 0013: context (the spec's section 1 measurements), decision (the spec's section 3: four units, medium cut with a short evidence record and the full smoke at the landing; it amends the standing-tracks sentence "with the repository's rules unchanged"), consequences (the spec's anti-goals).
- [ ] **Step 4:** Make the moves and edits in **Files**. The `DEV-1` roadmap section has the hypothesis and outcome of Step 2 and one unchecked box per row (`DEV-1-A`, `DEV-1-B`, `DEV-1-C`).
- [ ] **Step 5:** Run the guards from the worktree root. Expected: each exits 0.

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
```

  If the documentation guard refuses the new plan because its roadmap section, checkpoint or row statuses do not match what it expects, fix the documents to what the guard names; do not change the guard.
- [ ] **Step 6:** Commit everything as one commit: `suite-maintenance: Add the development speed plan and archive the RSH-1 plan`. The hooks must pass without `--no-verify`.
- [ ] **Step 7:** Report the branch (`unit/suite/DEV-1-OPEN`) and its commit to the coordinating session with these author commands, and stop:

```sh
git -C /home/toni/CODIGO/CELESTINA merge --ff-only unit/suite/DEV-1-OPEN
git -C /home/toni/CODIGO/CELESTINA push origin main
scripts/worktree.sh close suite DEV-1-OPEN
```

---

## U01 — `DEV-1-A`: build cache and disk

Open after U00 is on `origin/main`: `scripts/worktree.sh open suite DEV-1-A`. First edit: in `docs/plans/active/2026-11-24-dev-speed.md`, set the `DEV-1-A` row to `active` and its evidence cell to `[evidence](../../evidence/2026-11-24-dev-speed-cache.md)`; create that file from `docs/templates/evidence.md`.

### Task 2: Baseline measurements

**Files:**
- Create: `docs/evidence/2026-11-24-dev-speed-cache.md`

**Interfaces:**
- Produces: a table `Measure | Before | After` in the evidence record with the rows below; Task 6 fills `After`.

- [ ] **Step 1:** In the worktree, with the default profile and a private target, measure and record wall-clock seconds and size:

```sh
cd celestina
export CARGO_TARGET_DIR="$PWD/../target/baseline-before"
/usr/bin/time -f '%e s' cargo test --workspace --locked --no-run      # row: cold test build
touch src/lib.rs
/usr/bin/time -f '%e s' cargo test --workspace --locked --no-run      # row: rebuild after touching src/lib.rs
du -sh "$CARGO_TARGET_DIR"                                            # row: target size
ls -l "$CARGO_TARGET_DIR"/debug/deps/celestina-* | sort -k5 -n | tail -1   # row: largest test binary
```

- [ ] **Step 2:** Confirm the linker. Run `readelf -p .comment "$CARGO_TARGET_DIR"/debug/deps/celestina-<hash>` on that largest binary and record the line naming the linker. Expected: it contains `LLD`. If it does not, record what it says, tell the coordinating session, and stop: the spec assumed `rust-lld`.
- [ ] **Step 3:** `rm -rf "$CARGO_TARGET_DIR"`; record environment (`rustc --version`, `nproc`, date) in the evidence.

### Task 3: `trim_session_target` in `build_trees.py`

**Files:**
- Modify: `scripts/build_trees.py`
- Test: `scripts/test-build-trees.py`

**Interfaces:**
- Produces:
  - `SESSION_TARGET_CAP_GIB = 40`
  - `trim_session_target(target: Path, cap_bytes: int, *, dry_run: bool = False) -> PruneReport` (`removed` holds paths relative to `target`)
  - CLI: `build_trees.py trim-session-target TARGET [--cap-gib N] [--dry-run]`; exit 0 also when nothing is removed or the lock is held; prints `build-trees: session target <size>, under the cap of <N> GiB` or one `removed <path>` line each and `build-trees: freed <size>`, or `build-trees: session target is in use; not trimmed`. This subcommand reads no registry and does not refuse inside a session worktree.

Behaviour, in order:
1. `target` missing, not a directory, or a symlink: empty report.
2. `tree_size(target) <= cap_bytes`: empty report.
3. For the profile directory `target/debug` (skip if absent or a symlink): take Cargo's lock without waiting, `fcntl.flock(open(target/debug/.cargo-lock), LOCK_EX | LOCK_NB)`; if it is held, return an empty report with `report.busy = True` (add `busy: bool = False` to `PruneReport`). A missing lock file means no build is running.
4. Member crates are the names of the entries of `debug/incremental`, cut at the last `-` and with `-` read as `_`. Remove: `debug/incremental`, `debug/examples`, every regular file directly in `debug` except `.cargo-lock`, and in each of `debug/deps`, `debug/build` and `debug/.fingerprint` every entry whose name, after an optional leading `lib`, cut at the last `-` and with `-` read as `_`, is a member crate.
5. If the tree is still over the cap, remove `debug` and `release` whole.

- [ ] **Step 1: Write the failing tests** in `scripts/test-build-trees.py`, class `TrimSessionTarget`, each on a `tempfile.TemporaryDirectory` with small files and `cap_bytes` chosen around their total:
  - `test_under_the_cap_nothing_is_removed`: report empty, every file still there.
  - `test_over_the_cap_member_artifacts_go_and_dependencies_stay`: fixture has `debug/incremental/my_app-1abc/x`, `debug/deps/my_app-9f.d`, `debug/deps/libmy_app-9f.rlib`, `debug/deps/libserde-77.rlib`, `debug/.fingerprint/my-app-9f/x`, `debug/.fingerprint/serde-77/x`, `debug/build/my-app-9f/out`, `debug/examples/demo`, `debug/my-app`, `debug/.cargo-lock`; after the call only `debug/deps/libserde-77.rlib`, `debug/.fingerprint/serde-77/x` and `debug/.cargo-lock` remain, and `report.freed_bytes` equals the removed files' sizes.
  - `test_still_over_the_cap_removes_the_profiles`: one large `debug/deps/libserde-77.rlib` and `release/deps/libserde-88.rlib`; both profile directories are gone.
  - `test_a_held_cargo_lock_skips_the_trim`: the test holds `flock(LOCK_EX)` on `debug/.cargo-lock` from another open file; report is empty with `busy` true, nothing removed.
  - `test_a_symlinked_target_is_left_alone`: `target` is a symlink to a directory over the cap; nothing in the real directory is removed.
  - `test_dry_run_removes_nothing`: report lists the paths, files remain.
  - `test_the_command_line_prints_what_it_freed`: run the script with `trim-session-target <dir> --cap-gib 0`; exit 0, stdout has `build-trees: freed`.
- [ ] **Step 2:** Run `python3 scripts/test-build-trees.py`. Expected: the new tests fail (`AttributeError: trim_session_target`), the existing ones pass.
- [ ] **Step 3:** Implement `trim_session_target` and the subcommand in `scripts/build_trees.py`, reusing `tree_size`, `remove`, `real_directory` and `human_size`. Update the module docstring's usage line.
- [ ] **Step 4:** Run `python3 scripts/test-build-trees.py`. Expected: all pass.
- [ ] **Step 5:** Commit: `suite-maintenance: Add a cap trim for the session Cargo target`.

### Task 4: `worktree.sh` — dev profile, `target/` inside the worktree, trim at close

**Files:**
- Modify: `scripts/worktree.sh` (header comment lines 4-27; the `open` block that writes `.cargo/config.toml`, lines 173-175; the `close` tail, lines 221-241)
- Test: `scripts/test-worktree.sh`

**Interfaces:**
- Consumes: `build_trees.py trim-session-target TARGET` from Task 3.
- Produces: the file `open` writes, exactly:

```toml
[build]
target-dir = "<worktrees>/.cargo-target"

[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false
```

  and `close` behaviour: removes `<unit_dir>/target` (ignored by the root `.gitignore`, so it is not in the status check) before `git worktree remove`; never deletes `<worktrees>/.cargo-target`; after deleting the branch runs `python3 "$script_dir/build_trees.py" trim-session-target "$worktrees/.cargo-target"`, passing its stdout through; a non-zero exit prints `worktree: warning: the session Cargo target was not trimmed` on stderr and `close` still exits 0.

- [ ] **Step 1: Write the failing tests** in `scripts/test-worktree.sh`, in its existing style:
  - after `open`, `.cargo/config.toml` contains the lines `[profile.dev]`, `debug = "line-tables-only"`, `[profile.dev.package."*"]` and `debug = false` (`grep -Fx` each; the existing `target-dir` check stays);
  - `close` of a worktree holding `target/private/x` succeeds and the directory is gone;
  - replace the existing assertion that the last `close` removes `.cargo-target` (around lines 128-140): `"$worktrees/.cargo-target/debug/cache.rlib"` still exists after the last `close`;
  - `close` of a worktree whose `.cargo/config.toml` is the old two-line file succeeds.
- [ ] **Step 2:** Run `bash scripts/test-worktree.sh`. Expected: FAIL at the first new assertion.
- [ ] **Step 3:** Implement the three changes in `scripts/worktree.sh` and rewrite the header comment's `open` and `close` paragraphs to say what they now do.
- [ ] **Step 4:** Run `bash scripts/test-worktree.sh`. Expected: exits 0.
- [ ] **Step 5:** Commit: `suite-maintenance: Add the session dev profile and trim the shared target at close`.

### Task 5: `land-unit.py` closes the landed unit's session worktree

**Files:**
- Modify: `scripts/land-unit.py` (`prune`, lines 1288-1304; module docstring's environment list near line 18)
- Test: `scripts/test-land-unit.py`

**Interfaces:**
- Consumes: `scripts/worktree.sh close PROJECT UNIT` from Task 4.
- Produces: `close_session(ctx: LandContext) -> None`, called as the last line of `prune`, after `remove_landing(ctx)`. It parses `ctx.state.branch`: only a branch of the exact form `unit/<project>/<unit>` is closed; anything else returns silently. It runs `sh <worktree.sh> close <project> <unit>` with `cwd=ctx.root`, where `<worktree.sh>` is `program("LAND_UNIT_WORKTREE", str(Path(__file__).resolve().parent / "worktree.sh"))`. Exit 0 prints `land-unit: closed the session worktree of <unit>`; any other exit prints, through `say`, `warning: the session worktree stays: <stderr, stripped>` and returns. It never raises.

- [ ] **Step 1: Write the failing tests** in `scripts/test-land-unit.py`, beside `test_a_landing_prunes_the_debug_builds` (line 3968), using the fixture's own helpers (`open_branch`, `write_unit`, `land`, `assert_landed`). If `open_branch` places the worktree somewhere other than `<repo>.worktrees/app-<UNIT>`, add a helper that opens it there, with the marker file, as `worktree.sh open` does:
  - `test_a_landing_closes_the_session_worktree`: after `land("unit/app/FX-A", "--kind", "maintenance")`, the landing is accepted, stdout has `land-unit: closed the session worktree of FX-A`, the worktree directory does not exist and `refs/heads/unit/app/FX-A` is gone.
  - `test_a_refused_close_only_warns`: an untracked file `notes.txt` is written in the session worktree after its last commit; the landing is accepted (exit 0), stderr or stdout has `warning: the session worktree stays`, and both the directory and `notes.txt` exist.
  - `test_a_branch_outside_the_unit_namespace_is_not_closed`: stub `LAND_UNIT_WORKTREE` with a script that writes a marker file when run; land a branch whose name does not match `unit/<project>/<unit>` if the fixture allows one, otherwise call `close_session` directly with such a state; the marker file is absent.
  - Extend `test_stacked_branch_lands_after_its_dependency` (line 3481): assert the dependency's branch no longer exists before the stacked branch is landed, and that the stacked landing is still accepted.
- [ ] **Step 2:** Run `python3 scripts/test-land-unit.py -k close -k stacked_branch_lands`. Expected: the three new tests fail; note whether the stacked one fails.
- [ ] **Step 3:** Implement `close_session` and the call in `prune`; add `LAND_UNIT_WORKTREE` to the docstring's environment list. If the stacked test fails because the landing needs the dependency's branch after it landed, do not weaken `close`: report it to the coordinating session with the failing assertion and stop.
- [ ] **Step 4:** Run `bash scripts/test-land-unit.sh` (the whole file; several minutes, run it in the background and wait on its exit status). Expected: all pass. Tests that inspect a session worktree after a landing must now look before it, or assert its absence.
- [ ] **Step 5:** Commit: `suite-maintenance: Add the closing of the session worktree to the landing`.

### Task 6: CI profile, contract text, after-measurements

**Files:**
- Modify: `.github/workflows/celestina.yml`, `.github/workflows/celestina-rs.yml` (job-level `env:` with `CARGO_PROFILE_DEV_DEBUG: line-tables-only` and a one-line comment naming the session profile it mirrors)
- Modify: `docs/contracts/landing.md` (the path table at lines 24-29; the two-files paragraph at lines 57-64; the `close` paragraph at lines 76-84; step 12 `prune` at lines 403-410)
- Modify: `docs/contracts/production-artifacts.md` ("Pruning after deploy": one paragraph on the session target's cap)
- Modify: `docs/evidence/2026-11-24-dev-speed-cache.md`

**Interfaces:**
- Consumes: the behaviours of Tasks 3-5, described exactly as their Interfaces blocks state them.

- [ ] **Step 1:** Edit `landing.md`: the dev profile in the written config; "a session that needs a target of its own uses `<worktree>/target/<name>`; `close` removes it; no session creates a directory beside the worktrees"; `close` no longer removes `.cargo-target` and trims it above 40 GiB (what is removed, that a running build skips the trim); step 12 ends by closing the landed unit's session worktree, a refusal only warns, and a unit that changes `land-unit.py` itself lands with the old script, so its own worktree is closed by hand.
- [ ] **Step 2:** Add the `env:` block to both workflows.
- [ ] **Step 3:** Repeat Task 2 Step 1 with `CARGO_TARGET_DIR="$PWD/../target/baseline-after"` and the three profile lines of Task 4 appended to this worktree's own `.cargo/config.toml` (it was opened by the old script). Fill the `After` column; delete the target. Record in **Limits** that the time of a whole landing was not measured here and is taken at this unit's landing by the coordinating session.
- [ ] **Step 4:** Run, from the worktree root. Expected: each exits 0.

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/test-build-trees.py
bash scripts/test-worktree.sh
bash scripts/test-land-unit.sh
```

- [ ] **Step 5:** Record those commands and exits in the evidence. Commit: `suite-maintenance: Update the landing contract and CI for the session profile and cache cap`.
- [ ] **Step 6:** Report to the coordinating session, and stop: the branch, the four commits, the before/after table, anything not done or not checked, and the author's commands:

```sh
cd /home/toni/CODIGO/CELESTINA
time python3 scripts/land-unit.py unit/suite/DEV-1-A --kind maintenance --summary "Add a lighter session dev profile, a cap on the session Cargo target and the closing of a landed unit's worktree"
scripts/worktree.sh close suite DEV-1-A
```

## Chunks and models (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| U00.1 · Archive RSH-1, open DEV-1 (Task 1) | Sonnet 5.5 | medium | Documents against a strict guard; no code |
| U01.1 · Baseline and after-measurements, CI, contract text (Tasks 2, 6) | Sonnet 5.5 | medium | Commands and prose with exact values given |
| U01.2 · `trim_session_target` (Task 3) | Sonnet 5.5 | medium | New function with seven pinned tests |
| U01.3 · `worktree.sh` (Task 4) | Sonnet 5.5 | medium | Small shell change in a tested script |
| U01.4 · `land-unit.py` auto-close (Task 5) | Opus 5.5 | high | The landing tool; a 4 500-line fixture file; the stacked-branch case |
| U01.R · Final review | Fable 5.2 | high | Different from every implementer; `/code-review`, at most two rounds |

Tasks 3, 4 and 5 touch different files and run one after another in the same worktree (each consumes the previous one's interface); Task 6 runs last.
