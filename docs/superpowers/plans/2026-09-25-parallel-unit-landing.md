# Parallel Unit Landing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A unit prepared in one session lands on `main` in arrival order, with one build at most and none when its product's inputs did not move, while other sessions keep working in their own worktrees.

**Architecture:** Sessions work in `git worktree`s outside the repository (`scripts/worktree.sh`) sharing one Cargo target directory, and `production_artifact.py` refuses production runs there. Landing is a tool run from the canonical checkout (`scripts/land-unit.py`, orchestration) over pure functions (`scripts/landing.py`: ledger merge, ratchet merge, lockfile rule, inventory generation, state): rebase onto `origin/main`, semantic merge of the hot files, version bump against the current `main`, the one production run `production_artifact.py check` demands, inventory and ledger closure, the hooks' own guard chain, one typed commit, fast-forward and push, retry when `main` moved. No guard, hook, inventory format or contract changes.

**Tech Stack:** POSIX `sh`, Python 3.11+ (stdlib only: `tomllib`, `subprocess`, `hashlib`, `unittest`), Git, Cargo (`cargo metadata --offline`), the existing `scripts/*.py` modules.

**Spec:** [docs/superpowers/specs/2026-09-25-parallel-unit-landing-design.md](../specs/2026-09-25-parallel-unit-landing-design.md)

## Global Constraints

- **One suite plan, `LND-1`, three units, all `suite-maintenance`, no product version moves.** `PRD-1-A` was delivered in `36ed52a` but its plan is still under `docs/plans/active/`, and the root roadmap admits one active checkpoint, so the administrative unit `PRD-1-B` archives it in the same suite commit as `LND-1-A`, exactly as `LNG-1-B` landed with `PRD-1-A` (see `docs/plans/archive/2026-08-04-spanish-product-copy.md` and `docs/evidence/2026-08-05-spanish-product-copy-archive.md`).
- **Nothing in a guard changes.** `check-staged-units.py`, `commit_scope.py`, `documentation_contract.py`, the hooks, the inventory format and `versioning.md` stay as they are. `landing.py` may import `documentation_contract` and `project_registry` from the worktree; it is a tool, not a guard, so it does not need the HEAD-only rule loading that `check-staged-units.py` uses.
- **Python:** stdlib only; no `unwrap`-style shortcuts: every subprocess failure becomes a `LandingError` with the command and stderr; no bare `except`; no TODO/FIXME. Diagnostics and comments in English. Every new Python file is scanned by `check-language-contract.py`; every new script must keep `bash scripts/check-architecture-contract.sh` green.
- **Shell:** POSIX `sh`, `set -eu`, the style of `scripts/production-common.sh`; no bashisms in `worktree.sh`.
- **Tests:** `unittest` in `scripts/test-land-unit.py` with a fixture repository built per test in a temporary directory, plus a bare `origin`; the fake production entries follow `scripts/test-production-artifacts.py` (`write_entry_script`), the fixture registry follows `scripts/test-staged-units.sh`. The shell wrappers `scripts/test-land-unit.sh` and `scripts/test-worktree.sh` mirror `scripts/test-production-artifacts.sh`. The spec's §8 lists a shell-built fixture; building it in Python is the same fixture with less duplication and is the deviation this plan records.
- **Builds:** none of the three units changes a deployable app, so no `complete-production.sh` runs during this plan. The fake entries in the tests are the only "builds".
- **Commit subjects:** `suite-maintenance: <Imperative>` with a recognized verb (`Add`, `Archive`, `Record`, `Move`). Trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- **Units and inventories:** `LND-1-A` (with `PRD-1-B`) is inventoried by hand per `docs/templates/plan.md` and verified with `python3 scripts/check-staged-units.py INVENTORY...`; `LND-1-B` may use `landing.py`'s generator from the worktree; `LND-1-C` is landed by `land-unit.py` itself. Never `git stash`. Never commit without the author's word; leave the unit staged and report. Preserve any dirty file that is not this unit's.
- **Paths outside the repository.** The session worktrees and the landing worktree live at `<repository parent>/<repository basename>.worktrees/`; the guard walks only the repository, so nothing needs an ignore rule. (`documentation_contract.excluded_path` already tolerates `.claude/worktrees/`, so a native Claude worktree does not break the guard either, but only `worktree.sh` writes the shared-cache config.)

## Review Focus

Inputs the spec implies but no task's tests exercise unless listed here; each gets a test in the task that owns the code.

1. **A branch whose ledger row is already `done` with an inventory link** (a session sealed by hand): the tool must stop at preflight saying the row must be `active` or `done` without a link, not land a second inventory. Task 5.
2. **A branch that edits a tracked inventory** (any `*.numstat.tsv` that exists on `origin/main`): stop at preflight; inventories are immutable. Task 5.
3. **Interruption between the detached checkout and the commit** (Ctrl-C during the fake build): `main` untouched, state file present, `--abort` returns the canonical checkout to `main` and removes the landing worktree. Task 5.
4. **Evidence record missing or not under the owner's `docs/evidence/`:** stop at preflight, before any build. Task 5.
5. **Dirty canonical checkout, or not on `main`:** stop at preflight with the `git status` lines in the message. Task 5.

---

## File structure

| Path | Responsibility |
|---|---|
| `docs/plans/active/2026-09-25-seal-at-landing.md` (new) | the `LND-1` plan and ledger |
| `docs/plans/archive/2026-08-05-desktop-entry-registration.md` (moved) | `PRD-1` archived by `PRD-1-B` |
| `docs/evidence/2026-09-25-desktop-entry-registration-archive.md` (new) | `PRD-1-B` evidence |
| `docs/inventories/2026-08-05-desktop-entry-registration/PRD-1-B.numstat.tsv`, `docs/inventories/2026-09-25-seal-at-landing/LND-1-{A,B,C}.numstat.tsv` (new) | immutable unit inventories |
| `docs/evidence/2026-09-25-seal-at-landing{,-tool,-documents}.md` (new) | `LND-1-A/B/C` evidence |
| `ROADMAP.md`, `STATUS.md`, `docs/plans/active/README.md`, `docs/plans/archive/README.md` | checkpoint `LND-1`, focus, plan indexes |
| `scripts/production_artifact.py` | refuse `run-build`, `run-verification`, `status` inside a session worktree |
| `scripts/test-production-artifacts.py` | the refusal's tests |
| `scripts/worktree.sh` (new) | `open PROJECT UNIT`, `close PROJECT UNIT` |
| `scripts/test-worktree.sh` (new) | its tests |
| `scripts/landing.py` (new) | pure functions: unit discovery, ledger merge, ratchet merge, lockfile rule, inventory, diffstat, ledger closure, evidence section, landing state |
| `scripts/land-unit.py` (new) | orchestration: preflight, unbump, rebase, version, checkout, build, seal, guards, commit, push, `--continue`, `--abort` |
| `scripts/test-land-unit.py`, `scripts/test-land-unit.sh` (new) | unit tests of `landing.py`, integration tests of `land-unit.py` on fixture repositories |
| `.github/workflows/contracts.yml` | run the two new test scripts in `Commit scope` |
| `docs/decisions/0011-seal-at-landing.md` (new), `docs/decisions/README.md` | the decision |
| `docs/contracts/landing.md` (new), `docs/projects.toml` (`suite.shared_rules`) | the canonical landing contract, printed by `agent-context.py` |
| `AGENTS.md`, `CONTRIBUTING.md`, `docs/governance/change-policy.md`, `docs/templates/plan.md`, `docs/README.md` | closure becomes "request the landing" |

---

### Task 0: Open `LND-1` and archive `PRD-1` — `PRD-1-B` and the plan of `LND-1-A` (`suite:`)

**Files:**
- Create: `docs/plans/active/2026-09-25-seal-at-landing.md`
- Move: `docs/plans/active/2026-08-05-desktop-entry-registration.md` → `docs/plans/archive/2026-08-05-desktop-entry-registration.md`
- Create: `docs/evidence/2026-09-25-desktop-entry-registration-archive.md`
- Modify: `ROADMAP.md` (`Active implementation checkpoint`, `PRD-1` section link, new `LND-1` section before `## Project implementation fronts`), `STATUS.md` (`Current focus`, `Implementation checkpoint`, a `## Active cross-project work` paragraph), `docs/plans/active/README.md`, `docs/plans/archive/README.md`

**Interfaces:** documents only. Produces the ledger rows `LND-1-A`, `LND-1-B`, `LND-1-C` that Tasks 1–6 close.

- [ ] **Step 1: Write the `LND-1` plan** from `docs/templates/plan.md` with this header and ledger:

```markdown
# LND-1 — seal at landing

- **Opened:** 2026-09-25
- **Plan ID:** seal-at-landing
- **Status:** active
- **Scope:** suite
- **Implementation checkpoint:** LND-1
- **Author-validation checkpoint:** none
```

Hypothesis: "The closure of a unit is a function of the commit that will be its parent, so producing it at landing time removes every reason a second session has to wait for the first." Tangible outcome, scope, exclusions and build order copied from spec §1, §9. Implementation exit:

```sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-production-artifacts.sh
bash scripts/test-worktree.sh
bash scripts/test-land-unit.sh
python3 scripts/check-staged-units.py
```

Ledger (open form; `Files / areas` names stable paths):

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LND-1-A | `suite:` | active | `scripts/worktree.sh`, `scripts/test-worktree.sh`, `scripts/production_artifact.py` (`main`), `scripts/test-production-artifacts.py`, this plan, root roadmap/status/plan indexes | — | Add the session worktree entry with its shared Cargo cache and refuse production runs inside a session worktree | `bash scripts/test-worktree.sh && bash scripts/test-production-artifacts.sh` | None |
| LND-1-B | `suite:` | planned | `scripts/landing.py`, `scripts/land-unit.py`, `scripts/test-land-unit.py`, `scripts/test-land-unit.sh`, `.github/workflows/contracts.yml` | — | Add the landing tool that rebases, merges the hot files, bumps, builds only when the artifact is stale, seals, guards, commits and pushes one unit | `bash scripts/test-land-unit.sh` | None |
| LND-1-C | `suite:` | planned | `docs/decisions/0011-seal-at-landing.md`, `docs/contracts/landing.md`, `docs/projects.toml`, `AGENTS.md`, `CONTRIBUTING.md`, `docs/governance/change-policy.md`, `docs/templates/plan.md`, `docs/README.md`, `docs/decisions/README.md` | — | Record the decision and move closure from hand-computed inventories to the landing | `sh scripts/check-documentation-contract.sh && python3 scripts/agent-context.py scripts` | None |

Add a `## Paired suite transition` paragraph naming `PRD-1-B` as the unit that archives `PRD-1` in the same commit as `LND-1-A`, mirroring the paragraph in the `PRD-1` plan.

- [ ] **Step 2: Archive `PRD-1`.** `git mv` the plan to `docs/plans/archive/`, set `Status: done`, add after `Plan ID`:

```markdown
- **Closed:** 2026-09-25
- **Successor:** LND-1, which archived this plan; its unit landed in `36ed52a`
```

Append the `PRD-1-B` row at the top of its ledger, open form: `| PRD-1-B | `suite:` | active | this plan (both endpoints), `docs/plans/active/README.md`, `docs/plans/archive/README.md`, `ROADMAP.md`, `STATUS.md` | — | Archive the delivered desktop-entry plan through its own administrative unit and reconcile the suite roadmap, status and plan indexes | `sh scripts/check-documentation-contract.sh` | None |`. Fix the plan's relative links (`../../inventories/…` stays valid from `archive/`; `../../../celestina/VALIDATION.md` stays valid).

- [ ] **Step 3: Write the `PRD-1-B` evidence** at `docs/evidence/2026-09-25-desktop-entry-registration-archive.md`, modeled line by line on `docs/evidence/2026-08-05-spanish-product-copy-archive.md` (date, scope `PRD-1-B`, procedure = the four guard commands, result = both endpoints of the transition, observed facts: `PRD-1-A` landed in `36ed52a`; `LND-1-A` is a separate unit in the same commit).

- [ ] **Step 4: Reconcile the root documents.** `ROADMAP.md`: `Active implementation checkpoint: LND-1`; in the `PRD-1` section replace `[the active PRD-1 plan](docs/plans/active/...)` by the archive link; add before `## Project implementation fronts`:

```markdown
## LND-1 — Seal at landing

**Hypothesis:** the closure of a unit is a function of the commit that will be
its parent, so producing it at landing time removes every reason a second
session has to wait for the first.

**Tangible outcome:** sessions work in their own worktrees and never build
production there; `scripts/land-unit.py` lands one unit from the canonical
checkout with one build at most, and none when the product's inputs did not
move; no guard changes.

- [ ] Add the session worktree entry with a shared Cargo cache and refuse
      production runs inside a session worktree.
- [ ] Add the landing tool with its fixture tests and CI step.
- [ ] Record the decision and move closure from hand-computed inventories to
      the landing.

The build order, exclusions and ledger are in
[the active plan](docs/plans/active/2026-09-25-seal-at-landing.md).
```

`STATUS.md`: `Current focus: LND-1, landing units prepared in parallel sessions`, `Implementation checkpoint: LND-1`, and replace the `## Active cross-project work` paragraph (which still describes LNG-1) with two sentences on LND-1 linking the plan; move the LNG-1 paragraph under `## Completed cross-project work`. `docs/plans/active/README.md`: name `LND-1` as the active suite plan and say `PRD-1` closed on 2026-09-25. `docs/plans/archive/README.md`: add the `PRD-1` row if the index lists plans (mirror how `LNG-1` is listed).

- [ ] **Step 5: Run the documentation guard.**

Run: `sh scripts/check-documentation-contract.sh`
Expected: `Documentation contract: OK` (the `SID-G7-D` erratum lines are expected output, not errors).

- [ ] **Step 6: Stage and report.** `git add` exactly the paths above. Do not write inventories yet; Task 2 closes `LND-1-A` and `PRD-1-B` together.

---

### Task 1: `production_artifact.py` refuses production runs in a session worktree — part of `LND-1-A`

**Files:**
- Modify: `scripts/production_artifact.py` (`main`, after `project_contract(...)`; a new `session_worktree_marker(root: Path) -> Path | None`)
- Test: `scripts/test-production-artifacts.py`

**Interfaces:**
- Produces: `WORKTREE_MARKER = ".celestina-worktree"` (module constant) and `session_worktree_marker(root) -> Path | None`, returning `root / WORKTREE_MARKER` when it is a regular file. Task 3's `worktree.sh` writes that file; Task 4's `land-unit.py` never runs in a directory that has it.

- [ ] **Step 1: Write the failing tests** in `scripts/test-production-artifacts.py`:

```python
def test_session_worktree_refuses_build_verify_and_status(self) -> None:
    (self.root / ".celestina-worktree").write_text("project = \"demo\"\n", encoding="utf-8")
    for command in ("run-build", "run-verification", "status"):
        process = self.run_tool(command, "demo", check=False)
        self.assertEqual(process.returncode, 1)
        self.assertIn(
            "this is a session worktree; production runs happen at landing (scripts/land-unit.py)",
            process.stderr,
        )
    self.assertFalse((self.root / ".fixture-demo-build-ran").exists())

def test_session_worktree_still_answers_check(self) -> None:
    self.run_build("demo")
    (self.root / ".celestina-worktree").write_text("project = \"demo\"\n", encoding="utf-8")
    process = self.run_tool("check", "demo", check=False)
    self.assertEqual(process.returncode, 0)
```

Read `run_tool`'s existing signature first and pass `check=False` the way its other callers do.

- [ ] **Step 2: Run them to verify they fail.**

Run: `python3 scripts/test-production-artifacts.py -k session_worktree -v`
Expected: FAIL, `returncode` 0 instead of 1 (the marker is ignored today).

- [ ] **Step 3: Implement.** Add `WORKTREE_MARKER` and `session_worktree_marker(root: Path) -> Path | None`; in `main`, after `project_contract`, when `args.command in {"run-build", "run-verification", "status"}` and the marker exists, raise `ContractError("this is a session worktree; production runs happen at landing (scripts/land-unit.py)")`, which the existing handler prints as `production-artifact: …` and returns 1.

- [ ] **Step 4: Run the whole artifact suite.**

Run: `bash scripts/test-production-artifacts.sh`
Expected: PASS, including the two new tests.

- [ ] **Step 5: Stage** `scripts/production_artifact.py` and `scripts/test-production-artifacts.py`; report.

---

### Task 2: `scripts/worktree.sh` and its test; close `LND-1-A` + `PRD-1-B` — `LND-1-A` (`suite:`)

**Files:**
- Create: `scripts/worktree.sh`, `scripts/test-worktree.sh`
- Create: `docs/evidence/2026-09-25-seal-at-landing.md`, the two inventories

**Interfaces:**
- Produces: `worktree.sh open PROJECT UNIT` → branch `unit/<project>/<unit>`, directory `<parent>/<basename>.worktrees/<project>-<unit>/`, files `.cargo/config.toml` (`[build]\ntarget-dir = "<parent>/<basename>.worktrees/.cargo-target"`) and `.celestina-worktree` (`project = "<project>"\nunit = "<unit>"\ncanonical = "<absolute repository path>"`). `worktree.sh close PROJECT UNIT`. Exit 2 on usage, 1 on refusal, with one English line on stderr. Task 4 reads the marker's keys.

- [ ] **Step 1: Write `scripts/test-worktree.sh`** (POSIX, temporary directory, trap cleanup) with a fixture: a repository `repo/` containing `docs/projects.toml` with the suite and one project `app` (copy the registry block from `scripts/test-staged-units.sh`), one commit, pushed to a bare `origin`, remote named `origin`. Cases, each ending in `printf 'ok %s\n'`:
  1. `open app APP-1` from inside `repo/`: `git -C repo worktree list` shows `repo.worktrees/app-APP-1` on `unit/app/APP-1`; the marker has the three keys; `.cargo/config.toml` names `repo.worktrees/.cargo-target`; `git -C repo.worktrees/app-APP-1 rev-parse HEAD` equals `origin/main`.
  2. `open app APP-1` again: exit 1, stderr contains `already exists`.
  3. `open nope APP-2`: exit 1, stderr contains `unregistered project`.
  4. `close app APP-1` after a commit in the worktree that is not on `origin/main`: exit 1, stderr contains `not on origin/main`; after `git -C repo.worktrees/app-APP-1 push origin HEAD:main` (fixture only), `close app APP-1` exits 0 and the worktree and branch are gone.
  5. `open suite LND-9`: accepted (`suite` is a valid project id).

- [ ] **Step 2: Run it to verify it fails.**

Run: `sh scripts/test-worktree.sh`
Expected: FAIL at case 1, `scripts/worktree.sh: not found`.

- [ ] **Step 3: Implement `scripts/worktree.sh`.** `set -eu`; resolve `repo_root=$(git rev-parse --show-toplevel)`; refuse when `.celestina-worktree` exists there (`run this from the canonical checkout`); validate the project id by `python3 - "$repo_root/docs/projects.toml" "$project"` reading the registry with `tomllib` (ids of `[[projects]]` plus `suite`); `git fetch origin main`; `git worktree add -b unit/<project>/<unit> <dir> origin/main`; write the two files with `printf`. `close`: refuse when `git rev-list origin/main..unit/<project>/<unit>` is non-empty; `git worktree remove <dir>`; `git branch -D`. Usage line: `usage: scripts/worktree.sh open|close PROJECT UNIT`.

- [ ] **Step 4: Run the test.**

Run: `sh scripts/test-worktree.sh`
Expected: five `ok` lines, exit 0.

- [ ] **Step 5: Run the guards** the unit's exit names for this task.

Run: `bash scripts/check-architecture-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-documentation-contract.sh`
Expected: all green.

- [ ] **Step 6: Close `LND-1-A` and `PRD-1-B`.** Write `docs/evidence/2026-09-25-seal-at-landing.md` from `docs/templates/evidence.md` (procedure: the three guard commands plus `bash scripts/test-worktree.sh` and `bash scripts/test-production-artifacts.sh`; result: exits and the case count; limits: no real Cargo build ran, the shared cache is exercised by the author's first real worktree). Set both ledger rows to `done` with inventory links, exact diffstats and evidence links. Write the two inventories by hand per `docs/templates/plan.md` (`Base revision` = current `HEAD`; one `Pathspec` per path; `PRD-1-B` claims the deleted active plan, the archived plan, its evidence, its inventory, the two plan indexes, `ROADMAP.md` and `STATUS.md`; `LND-1-A` claims the scripts, the tests, the `LND-1` plan, its evidence and its inventory). Verify:

Run: `git add <exact paths> && python3 scripts/check-staged-units.py docs/inventories/2026-08-05-desktop-entry-registration/PRD-1-B.numstat.tsv docs/inventories/2026-09-25-seal-at-landing/LND-1-A.numstat.tsv && printf '%s\n' $(git diff --cached --name-only) | .githooks/commit-msg --check 'suite-maintenance: Add the session worktree entry and archive the desktop entry plan'`
Expected: `staged-unit: …` OK and scope exit 0.

- [ ] **Step 7: Report; commit only on the author's word.** Subject: `suite-maintenance: Add the session worktree entry and archive the desktop entry plan`.

---

### Task 3: `scripts/landing.py` — the pure functions — part of `LND-1-B`

**Files:**
- Create: `scripts/landing.py`
- Test: `scripts/test-land-unit.py` (class `LandingFunctions`)

**Interfaces:**
- Consumes: `documentation_contract.split_table_row`, `is_separator_row`, `normalized_status`, `markdown_headings`, `normalized_heading`, `extract_inline_links`; `project_registry.build_commit_scopes`, `path_allowed`.
- Produces (all in `landing.py`):
  - `class LandingError(RuntimeError)`; `class LandingStop(LandingError)` with attributes `step: str`, `path: str | None`.
  - `@dataclass(frozen=True) UnitRef(project_id: str, prefix: str, plan_path: str, unit: str, summary: str, evidence_path: str | None)`.
  - `discover_unit(root: Path, registry: dict, changed_paths: set[str], read_plan: Callable[[str], str | None]) -> UnitRef`: exactly one changed path under a registered `active_plans` directory; in it exactly one ledger row whose status is `active`, or `done` with no `.numstat.tsv` link in `Files / areas`; the row's `Commit prefix` cell is `` `<prefix>:` `` and the prefix is registered; `summary` = the `Intended change` cell; `evidence_path` = the target of the first `Automated evidence` link ending in `.md`, or `None`. Raises `LandingStop(step="preflight")` otherwise, naming the count found.
  - `scope_violations(prefix: str, registry: dict, changed_paths: set[str]) -> list[str]`: paths `path_allowed` rejects for `build_commit_scopes(registry)[prefix]`.
  - `merge_plan(main_text: str, branch_text: str, unit: str) -> str`: `main_text` with the ledger row whose `Unit` cell equals `unit` replaced by the branch's row; when `main` has no such row, the branch's row is inserted after the row that precedes it in the branch when that row exists in `main`, else directly after the separator row. Raises `LandingStop` when either text has no `Change and commit ledger` table.
  - `merge_ratchet(base_text: str, main_text: str, branch_text: str) -> str`: rows are non-comment, non-empty lines split on tabs; the integer cell is the value, the remaining cells the key; a key absent in `main` or in the branch that was present in `base` is dropped; a key present in both takes `min`; a key only in the branch is appended; comments, blank lines and order follow `main`. Raises `LandingStop` when a row has no integer cell or more than one.
  - `lockfile_upgrades(main_lock: str, merged_lock: str) -> list[str]`: names of `[[package]]` blocks present in both whose `version` or `checksum` differ (the branch may add packages, never move an existing one).
  - `numstat_rows(root: Path, base: str, paths: Iterable[str]) -> list[InventoryRow]` with `InventoryRow(added: str, deleted: str, content: str, path: str)`: tracked-at-base paths via `git diff --numstat --no-renames <base> -- <path>` (`-`/`-` for binary, `0`/`0` when Git prints nothing for a mode-only change), new paths via `git diff --no-index --numstat /dev/null <path>`, `content` = SHA-256 hex of the final bytes (symlinks: link-target bytes), or `deleted` when the path no longer exists.
  - `render_inventory(unit: str, base: str, rows: list[InventoryRow], inventory_path: str) -> str`: the exact layout of `docs/inventories/2026-08-05-desktop-entry-registration/PRD-1-A.numstat.tsv` (title, `Base revision`, one `Pathspec` per row path including the inventory, `Calculation`, `Hashes`, blank line, header, rows sorted by path, the `self` row whose `added` equals the rendered file's total line count and `deleted` is `0`).
  - `diffstat(rows: list[InventoryRow]) -> str`: `N files, +X/-Y`, binary rows counted in `N` and contributing zero.
  - `close_ledger_row(plan_text: str, unit: str, inventory_link: str, diffstat: str, evidence_link: str) -> str`: sets `Status` to `done`, `Files / areas` to `[inventory](<inventory_link>)`, `Diffstat`, `Automated evidence` to `[evidence](<evidence_link>)`; links are relative to the plan's directory (`posixpath.relpath`).
  - `landing_section(base: str, check_output: str, build: str | None) -> str`: a `## Landing` Markdown section with `Base revision`, the `check` line, and either `Build: complete-production.sh exit 0, manifest git_revision <sha>` or `Build: artifact current; no build`.
  - `@dataclass LandState(step, branch, kind, summary, project_id, unit, attempts)` with `dump() -> str` (simple `key = "value"` TOML) and `LandState.load(text) -> LandState` via `tomllib`.

- [ ] **Step 1: Write the failing tests** (`class LandingFunctions(unittest.TestCase)`) on literal inputs, one method per function:
  - `test_discover_unit_requires_exactly_one_open_row`: a plan text with one `active` row → `UnitRef` fields exact; the same plan with two open rows, with zero, with a `done` row that has a link, and with an unregistered prefix → `LandingStop` whose message names the case.
  - `test_merge_plan_replaces_own_row_and_keeps_main`: main has rows `X-A` (done) and `X-B` (planned); branch turned `X-B` into `done` and edited the prose; result has main's prose, `X-A` unchanged, `X-B` from the branch. Then main without `X-B` and branch with `X-B` after `X-A` → inserted after `X-A`; main without `X-A` either → inserted after the separator.
  - `test_merge_ratchet_takes_lower_and_drops_removed`: base rows `a 10`, `b 20`, `c 30`; main lowered `a` to 8 and removed `c`; branch lowered `a` to 9, `b` to 15, added `d 5` → `a 8`, `b 15`, `d 5`, no `c`, comments from main. Both ratchet layouts (`lines\tpath\tN` and `N\tpath`).
  - `test_lockfile_upgrades_names_moved_packages`: two lock texts where `serde` changed version and `newdep` was added → `["serde"]`.
  - `test_numstat_rows_and_render_inventory`: a temporary repository with one commit, then a modified tracked file, a new file, a deleted file, a new binary file → rows exact; `render_inventory` output re-parsed: `self` row `added` equals `len(text.splitlines())`, `Pathspec` set equals row paths.
  - `test_diffstat_ignores_binary_lines`: rows `3/1`, `-/-`, `0/0` → `3 files, +3/-1`.
  - `test_close_ledger_row_writes_relative_links`.
  - `test_landing_section_two_shapes`.
  - `test_land_state_round_trip`.

- [ ] **Step 2: Run to verify they fail.**

Run: `python3 scripts/test-land-unit.py LandingFunctions -v`
Expected: `ModuleNotFoundError: landing`.

- [ ] **Step 3: Implement `scripts/landing.py`** with the signatures above. Ledger parsing: locate the `Change and commit ledger` heading with `markdown_headings`/`normalized_heading`, read the header row with `split_table_row`, index cells by the normalized column names of `documentation_contract.LEDGER_COLUMNS`. Ratchet merge keeps the file's comment lines verbatim from `main`.

- [ ] **Step 4: Run to verify they pass.**

Run: `python3 scripts/test-land-unit.py LandingFunctions -v`
Expected: all PASS.

- [ ] **Step 5: Stage** `scripts/landing.py` and `scripts/test-land-unit.py`; report.

---

### Task 4: `scripts/land-unit.py` — orchestration — part of `LND-1-B`

**Files:**
- Create: `scripts/land-unit.py`, `scripts/test-land-unit.sh`
- Modify: `.github/workflows/contracts.yml` (`Commit scope` step)
- Test: `scripts/test-land-unit.py` (class `LandUnitFixture` helpers only in this task; cases in Task 5)

**Interfaces:**
- Consumes: everything Task 3 produces; `version_tool.py bump/check` as subprocesses; `production_artifact.py check` and the registered `complete_script`/`verify_script` as subprocesses; `WORKTREE_MARKER` from Task 1.
- Produces: the CLI `land-unit.py BRANCH --kind KIND [--summary TEXT]`, `land-unit.py --continue`, `land-unit.py --abort`; environment overrides `LAND_UNIT_GIT` (default `git`), `LAND_UNIT_CARGO` (default `cargo`) and `LAND_UNIT_PYTHON` (default `sys.executable`) so the fixture can substitute recorders; exit 0 landed, 1 stopped or failed, 2 usage. Functions, each one step of spec §5.1, taking `(ctx: LandContext)` where `LandContext(root: Path, landing_dir: Path, registry: dict, state: LandState, unit: UnitRef)`: `preflight`, `unbump`, `rebase`, `bump_version`, `checkout_canonical`, `build_if_stale`, `seal`, `run_guards`, `commit_and_push`. `STEPS` is the ordered tuple of their names; `--continue` resumes at `state.step`.

- [ ] **Step 1: Write the fixture helper** `class LandUnitFixture(unittest.TestCase)` in `scripts/test-land-unit.py` whose `setUp` builds, in a temporary directory: `origin.git` (bare); `repo/` cloned from it with `user.name/email`, `main` checked out; `docs/projects.toml` with `[suite]` (`allow_all_commit_paths = true`, `shared_rules = []` is not needed by the tool), `[commit_policy]` with `workspace_manifests = ["ws/Cargo.lock"]` and `shared_ratchet_files = ["scripts/architecture-baseline.tsv"]`, `[version_policy]` as in the real registry, one `[[projects]]` `app` (`commit_prefix = "app"`, `version_source = { kind = "cargo-package", path = "app/Cargo.toml", package = "app" }`, `version_mirrors = [{ kind = "cargo-lock", path = "app/Cargo.lock", package = "app" }]`, `commit_roots = ["app/"]`, `include_workspace_manifests = true`, `deployable = true`, `build_script`/`verify_script`/`deploy_script`/`status_script`/`complete_script` = `app/scripts/*-production.sh`, `artifact_manifest = "app/target/production-artifact.toml"`, `artifact_paths = ["app/target/release/app"]`, `production_inputs = ["app/Cargo.toml", "app/src"]`, `verification_inputs = ["app/tests"]`, `active_plans = "app/docs/plans/active"`, plus the document fields the registry parser requires, pointing at small files); copies of the real `scripts/project_registry.py`, `documentation_contract.py`, `version_contract.py`, `version_tool.py`, `production_artifact.py`, `landing.py`, `land-unit.py`; fake guard scripts at `scripts/check-staged-units.py`, `scripts/commit_scope.py`, `scripts/check-architecture-contract.sh`, `scripts/check-language-contract.py`, `scripts/check-documentation-contract.sh` that append their name to `.guards-ran` and exit 0 unless `LAND_FIXTURE_FAIL_GUARD` names them; a fake `complete-production.sh` that runs the real `production_artifact.py run-build` and `run-verification` with fixture entry scripts (reuse `write_entry_script` from `test-production-artifacts.py` by import) and appends to `.builds-ran`; a fake `cargo` on `PATH` that records `metadata --offline` calls; `docs/version-history.tsv` with the `app 0.1.0 baseline` row; `app/Cargo.toml` (`version = "0.1.0"`), `app/Cargo.lock`, `app/src/main.rs`, `app/docs/plans/active/2026-09-25-fixture.md` with a ledger containing `FX-A` (`planned`), `app/docs/evidence/`, `scripts/architecture-baseline.tsv` with `lines\tapp/src/main.rs\t10`. Commit, push `main`. Helpers: `open_branch(unit)` (worktree at `repo.worktrees/app-<unit>` on `unit/app/<unit>`), `write_unit(worktree, unit, *, bump=False, touch_ratchet=None, edit_status=False)` that edits `app/src/main.rs`, sets the row to `active` with an evidence link, writes the evidence file, commits; `advance_main(...)` that lands an unrelated or a same-product change directly on `origin/main`; `land(branch, kind, *env)` running `land-unit.py` from `repo/` and returning the `CompletedProcess`.

- [ ] **Step 2: Write one smoke test** `test_fast_forward_landing` (case 1 of spec §8): after `open_branch("FX-A")`, `write_unit`, `land("unit/app/FX-A", "bug")`: exit 0; `origin/main` has one new commit whose subject is `app-bug: <Intended change>`; its parent equals the base recorded in `app/docs/inventories/2026-09-25-fixture/FX-A.numstat.tsv`; the ledger row is `done` with the inventory link and `N files, +X/-Y` matching `git show --numstat`; the evidence ends with a `## Landing` section; `app/Cargo.toml` says `0.1.1` and the history has the `app 0.1.1 bug FX-A` row; `.guards-ran` lists the five guards in order; `.builds-ran` has one line (inputs changed, so the fake build ran).

- [ ] **Step 3: Run to verify it fails.**

Run: `python3 scripts/test-land-unit.py LandUnitFixture.test_fast_forward_landing -v`
Expected: FAIL, `land-unit.py` not found.

- [ ] **Step 4: Implement `scripts/land-unit.py`.** Argument parsing per the CLI above. `preflight`: `git status --porcelain` empty and branch `main`, else `LandingStop("preflight", …)` with the status lines; `git fetch origin main`; `changed_paths` = `git diff --name-only --no-renames origin/main...BRANCH`; `discover_unit`; `scope_violations` empty (suite prefix: `allow_all`); every changed `*.numstat.tsv` must not exist on `origin/main`; the evidence path must exist on the branch under `<owner>/docs/evidence/`; write `LandState` to `<landing_dir>/.land-state.toml`. `unbump`: version sources, mirrors and the history file from the registry; if the branch changed any, `git checkout origin/main -- <paths>` on the landing worktree and commit `fixup! strip session bump`, print the warning. `rebase`: `git worktree add <landing_dir> BRANCH` (or reuse on `--continue`), `git rebase --no-autosquash origin/main`; on conflict, for each conflicted path that is the discovered plan, a `shared_ratchet_files` entry, or a lockfile (`Cargo.lock` basename) apply Task 3's merge (`git show :1:`, `:2:`, `:3:` for base/ours/theirs, noting that during a rebase `ours` is `main` and `theirs` is the branch), for a lockfile run `<cargo> metadata --offline --format-version 1` in that file's directory after taking `main`'s copy, then `lockfile_upgrades` must be empty; `git add` and `git rebase --continue`; any other conflicted path → `LandingStop("rebase", path)`. `bump_version`: for `bug|milestone|release`, `<python> scripts/version_tool.py bump <owner> <kind> --unit <unit> --summary <summary>` with `--root <landing_dir>`; refuse `suite` with a bump; commit `fixup! bump`. `checkout_canonical`: `git checkout --detach <landing tip>` in `root`. `build_if_stale`: `<python> scripts/production_artifact.py check <project> --require-verified`; on failure run `complete_script` (deployable) or `verify_script` (not deployable) from `root`; record the outcome in the state. `seal`: compute `changed_paths` against `origin/main` on the detached tree plus the inventory path; append `landing_section` to the evidence; `numstat_rows`, `render_inventory`, `close_ledger_row`; write files; `git add -A` restricted to `changed_paths`. `run_guards`: the five commands of spec §5.1 step 8 with `--check <subject>` for `commit_scope.py`; any non-zero exit → `LandingStop("guard", …)` with stdout+stderr. `commit_and_push`: `git commit -m <subject>` (the hooks run), `git checkout main`, `git merge --ff-only <sha>`, `git push -u origin main`; on rejection, `git reset --hard origin/main`, `state.attempts += 1`, if `< 3` restart at `rebase` from the branch's unsealed tip (the landing worktree's branch is reset to `BRANCH` and the fixups are re-applied), else `LandingStop("push")`. `--abort`: `git checkout main` in `root`, `git worktree remove --force <landing_dir>`, delete the state file. Every `git` call goes through one `run(ctx, *args)` that raises `LandingError` with stderr.

- [ ] **Step 5: Run the smoke test.**

Run: `python3 scripts/test-land-unit.py LandUnitFixture.test_fast_forward_landing -v`
Expected: PASS.

- [ ] **Step 6: Write `scripts/test-land-unit.sh`** (`python3 "$suite_root/scripts/test-land-unit.py"`, like `test-production-artifacts.sh`) and add to `.github/workflows/contracts.yml` under `Commit scope`:

```yaml
          bash scripts/test-worktree.sh
          bash scripts/test-land-unit.sh
```

- [ ] **Step 7: Stage** `scripts/land-unit.py`, `scripts/test-land-unit.py`, `scripts/test-land-unit.sh`, `.github/workflows/contracts.yml`; report.

---

### Task 5: The landing cases; close `LND-1-B` — `LND-1-B` (`suite:`)

**Files:**
- Test: `scripts/test-land-unit.py` (class `LandUnitFixture`, remaining cases)
- Modify: `scripts/land-unit.py` where a case exposes a gap
- Create: `docs/evidence/2026-09-25-seal-at-landing-tool.md`, `docs/inventories/2026-09-25-seal-at-landing/LND-1-B.numstat.tsv`

**Interfaces:** consumes Task 4's fixture helpers and CLI; produces no new names.

- [ ] **Step 1: Write the cases**, one method each, with the assertions the spec's §8 names:
  - `test_other_product_advanced_needs_no_build`: `advance_main` changes a file outside `production_inputs` (for instance `docs/notes.md`); land `FX-A` as `maintenance` → exit 0, `.builds-ran` empty, no conflict.
  - `test_same_product_advanced_rebumps_and_merges`: `advance_main` lands `FX-Z` as `app-bug` (0.1.0 → 0.1.1) lowering the ratchet row to 9 and adding a `done` row `FX-Z`; the branch lowered the same row to 8 and has `FX-A`; land `FX-A` as `bug` → `0.1.2`, history rows `0.1.1` then `0.1.2`, ratchet row `8`, both ledger rows present, `.builds-ran` has one line.
  - `test_branch_bump_is_stripped_and_warned`: `write_unit(bump=True)` (the branch already bumped to 0.1.1 and appended a row); land as `bug` → the landed version is `0.1.1` computed at landing, one history row, stderr contains `the session bumped`.
  - `test_plan_archived_on_main_stops`: `advance_main` moves the plan to `app/docs/plans/archive/`; land → exit 1, stderr names the plan path, `origin/main` unchanged.
  - `test_prose_conflict_stops_and_continue_lands`: `advance_main` edits the line of `app/STATUS.md` the branch also edits; land → exit 1, stderr names `app/STATUS.md`, `origin/main` unchanged, `.land-state.toml` exists; resolve in `repo.worktrees/.landing/`, `git add`, `git rebase --continue` there; `land --continue` → exit 0.
  - `test_lockfile_needing_network_stops`: the fake `cargo` exits 101 with `network` when `LAND_FIXTURE_CARGO_OFFLINE_FAILS=1` and both sides changed `app/Cargo.lock` → exit 1, stderr names `app/Cargo.lock`.
  - `test_push_rejected_retries_then_stops`: a `pre-receive` hook on `origin.git` that rejects the first push and lets the second through → exit 0, one landed commit whose parent is the `main` after the first rejection; a hook that always rejects → exit 1 after four pushes (assert on the hook's counter file), `main` in `repo/` back on `origin/main`.
  - `test_review_focus_preflight_stops`: five sub-cases with `subTest`: the row already `done` with a link; a branch editing an inventory that exists on `origin/main`; evidence link missing; dirty canonical checkout; canonical checkout on another branch → each exit 1 with the named reason, `.builds-ran` empty, no landing worktree created.
  - `test_interrupted_build_abort_restores_main`: the fake build sleeps and the test kills `land-unit.py` (`LAND_FIXTURE_BUILD_BEHAVIOR=hang`, `Popen`, `terminate` after `.builds-ran` appears) → `repo/` detached, state file present, `origin/main` unchanged; `land --abort` → `repo/` on `main`, landing worktree gone, state file gone.
  - `test_guard_failure_stops_before_commit`: `LAND_FIXTURE_FAIL_GUARD=check-language-contract.py` → exit 1, stderr contains the guard's name, no commit on the landing branch, `origin/main` unchanged.

- [ ] **Step 2: Run them.**

Run: `python3 scripts/test-land-unit.py LandUnitFixture -v`
Expected: some FAIL; fix `land-unit.py` until every case passes without weakening an assertion. Each fix is one function of §5.1, never a special case keyed on a fixture name.

- [ ] **Step 3: Run the wrapper and the guards.**

Run: `bash scripts/test-land-unit.sh && bash scripts/check-architecture-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-documentation-contract.sh`
Expected: all green.

- [ ] **Step 4: Close `LND-1-B`.** Evidence `docs/evidence/2026-09-25-seal-at-landing-tool.md` (procedure: the wrapper and the guards; result: the case count and the fixture's fake builds run per case; limits: no real `cargo`, no real hooks in the fixture, the first real landing is `LND-1-C`). Ledger row `done`. Inventory generated from the session worktree with:

```sh
python3 - <<'EOF'
import sys; sys.path.insert(0, "scripts")
import landing, pathlib, subprocess
# paths = the unit's changed paths + plan + evidence + inventory; base = origin/main
EOF
```

or by hand per the template; either way verify with `python3 scripts/check-staged-units.py docs/inventories/2026-09-25-seal-at-landing/LND-1-B.numstat.tsv` after `git add` of exactly the unit's paths, and the subject `suite-maintenance: Add the landing tool that seals a unit on the current main` with `.githooks/commit-msg --check`.

- [ ] **Step 5: Report; commit only on the author's word.**

---

### Task 6: Decision, contract and document amendments; landed by the tool — `LND-1-C` (`suite:`)

**Files:**
- Create: `docs/decisions/0011-seal-at-landing.md`, `docs/contracts/landing.md`
- Modify: `docs/decisions/README.md` (row 0011), `docs/projects.toml` (`suite.shared_rules` gains `"docs/contracts/landing.md"` after `"docs/contracts/versioning.md"`), `AGENTS.md` (`## Mandatory preflight` item 5, `## Documents and two delivery lanes` inventory paragraphs, `## Git and commits`), `CONTRIBUTING.md` (`## Closing a unit`, `## Git and commits`), `docs/governance/change-policy.md` (`## Exact immutable inventories`, `## Commit enforcement`), `docs/templates/plan.md` (the "Before setting a unit to `done`" paragraph), `docs/README.md` (source-of-truth table row)
- Create: `docs/evidence/2026-09-25-seal-at-landing-documents.md`

**Interfaces:** documents only. `agent-context.py` prints `docs/contracts/landing.md` once it is registered; the documentation guard requires the registered path to exist.

- [ ] **Step 1: Write ADR 0011** from `docs/templates/decision.md`, `Status: accepted`, with the Context, Decision and Consequences of spec §7, plus `Revisit when`: "landing needs a merge commit, a second active checkpoint per project, or the artifact contract stops deciding currency from registered inputs". Add the index row `| [0011](0011-seal-at-landing.md) | accepted | A unit is sealed, bumped and built at landing, on the commit that will be its parent |`.

- [ ] **Step 2: Write `docs/contracts/landing.md`**: sections `Session worktrees` (§4 of the spec: layout, the two files, the refusal, what a session does and does not do), `Landing` (§5.1 steps as a numbered list, with the exact commands), `Hot files` (§5.2 rules), `Stops and resumption` (§6), `What the tool never does` (§6). Link ADR 0011, ADR 0003, ADR 0004 and `production-artifacts.md`. Register it in `suite.shared_rules`.

- [ ] **Step 3: Amend the workflow documents.** `AGENTS.md` preflight item 5 becomes "Declare or update the ledger unit before editing planned milestone work, in a session worktree opened with `scripts/worktree.sh`." In the inventory paragraph, after "The base is the `HEAD` immediately before the unit…", add one sentence: "`scripts/land-unit.py` computes the inventory, the ledger closure, the version transition and the production run on the current `main`; see `docs/contracts/landing.md` (write it as a Markdown link in `AGENTS.md`)." In `## Git and commits`, replace step 6 with "run `python3 scripts/land-unit.py <branch> --kind <kind>` from the canonical checkout, which runs the version, staged-inventory and commit-scope guards itself". `CONTRIBUTING.md` `## Closing a unit`: keep the description of what the inventory contains; replace steps 2–4 with "request the landing; `scripts/land-unit.py` computes the inventory, the diffstat and the links". `change-policy.md`: one sentence in `## Exact immutable inventories` pointing at the landing contract; `## Commit enforcement` unchanged. `docs/templates/plan.md`: the paragraph starting "Before setting a unit to `done`, calculate tracked paths…" becomes "Before setting a unit to `done`, run its exit and request the landing; `scripts/land-unit.py` calculates tracked paths…" keeping the format description. `docs/README.md`: add `| How does a finished unit reach \`main\`? | [contracts/landing.md](contracts/landing.md) |` after the inventories row.

- [ ] **Step 4: Verify the documents.**

Run: `sh scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && python3 scripts/agent-context.py scripts | grep -c landing.md`
Expected: `Documentation contract: OK`, language OK, `1`.

- [ ] **Step 5: Prepare `LND-1-C` for the tool.** In the session worktree: set the ledger row to `active` with the evidence link `../../evidence/2026-09-25-seal-at-landing-documents.md`; write that evidence (procedure: the three commands of Step 4; result: their exits; limits: none). Commit the work on the branch as ordinary commits. Do **not** write the inventory, bump anything or set the row to `done`.

- [ ] **Step 6: Land it, on the author's word**, from the canonical checkout:

Run: `python3 scripts/land-unit.py unit/suite/LND-1-C --kind maintenance`
Expected: exit 0; `git log -1 --format=%s` on `main` is `suite-maintenance: Record the decision and move closure to the landing`; the evidence ends with `## Landing` … `Build: artifact current; no build` (no registered production input changed); `sh scripts/check-documentation-contract.sh` is OK on the landed tree.

- [ ] **Step 7: Close the checkpoint.** After landing, `LND-1`'s three rows are `done`; the root roadmap's three boxes become `[x]` and `STATUS.md`'s focus moves on — those two edits, the `Closed`/`Successor` lines and the move to `archive/` are a later administrative unit `LND-1-D`, opened only when the author names the next suite checkpoint, exactly as `PRD-1-B` did here.

---

## Self-review

- **Spec coverage.** §4 → Tasks 1–2; §5.1 → Task 4; §5.2 → Task 3 (`merge_plan`, `merge_ratchet`, `lockfile_upgrades`) and Task 4 (rebase resolution); §6 → Task 4 (`--continue`, `--abort`, state) and Task 5 (interruption, abort); §7 → Task 6; §8 → Tasks 1, 2, 3, 5 and the CI line in Task 4; §9 → Task 0 (PRD-1-B, LND-1 plan) and the unit closures in Tasks 2, 5, 6.
- **Deviations from the spec, recorded here:** the fixture is built in Python (`test-land-unit.py`) and the shell file is a wrapper; the lockfile rule is stated as "no existing package moves" (`lockfile_upgrades`) because "differs outside the packages the branch's manifests changed" is not computable from a lockfile; `LND-1-A` and `PRD-1-B` land together as spec §9 says.
- **Type consistency.** `InventoryRow(added, deleted, content, path)` is used by `numstat_rows`, `render_inventory` and `diffstat`; `UnitRef.evidence_path` feeds preflight and `seal`; `LandState.step` values are the names in `STEPS`; `LandingStop(step, path)` is what every stop raises and what `--continue` reads.
- **Review Focus.** All five inputs have a sub-case in `test_review_focus_preflight_stops` or `test_interrupted_build_abort_restores_main`.
- **Proportion.** The plan names signatures, assertions and commands; the only code blocks are tests' assertions, one YAML fragment and the document fragments the spec fixes.
