# Suite development speed — plan 2: preflight in tiers (U02, `DEV-1-B`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session reads a small `now` tier before acting and reads every other document only when its trigger applies; the `now` tier has a byte budget the guards enforce.

**Architecture:** `scripts/agent-context.py` keeps resolving the same documents and gives each a tier: `now`, or `when` with a one-line trigger. Triggers come from the document's source (README, roadmap, contract, plan…) and, for the shared rules, from a new table in the suite section of `docs/projects.toml`. A new module `scripts/preflight_budget.py` measures each project's `now` tier against `scripts/preflight-baseline.tsv`; `scripts/documentation_contract.py` calls it, so the hooks, the landing and CI enforce it with no change to hook plumbing. The root `AGENTS.md` loses its history and its duplicated mechanics.

**Tech Stack:** Python 3 standard library (`tomllib`, `unittest`), POSIX `sh` fixture tests, Markdown.

**Spec:** `docs/superpowers/specs/2026-11-24-suite-dev-speed-design.md`, section 4 "U02". Ledger: `docs/plans/active/2026-11-24-dev-speed.md`, row `DEV-1-B`.

## Global Constraints

- Repository text is English; the author is addressed in Spanish.
- Never edit the canonical checkout `/home/toni/CODIGO/CELESTINA`. Open the unit from it: `scripts/worktree.sh open suite DEV-1-B`.
- First edit in the worktree: set the `DEV-1-B` row to `active` with `[evidence](../../evidence/2026-11-24-dev-speed-preflight.md)` and create that record from `docs/templates/evidence.md`. The plan file changes only in that row. Copy this plan to `docs/superpowers/plans/2026-11-24-suite-dev-speed-plan-2.md` in the same commit.
- Session commits: `suite-maintenance: <Add|Fix|Remove|Update> …`; check a subject with `echo <path> | python3 scripts/commit_scope.py --check "<subject>"` (always piped).
- The session never runs `scripts/land-unit.py`, `git push`, a merge into `main`, a `*-production.sh` or a smoke.
- Do not edit `[commit_policy]`, any `[[projects]]` table, `scripts/check-architecture-contract.sh` or `scripts/production_artifact.py`: each is a verification input of every project and would re-verify the whole suite at the landing. New registry keys go in the `[suite]` table only.
- No rule is deleted. A sentence that leaves `AGENTS.md` must already exist, with the same meaning, in the document its pointer names; if it does not, move it there verbatim in the same commit.
- The heading `## Halted projects` and the rule paragraphs under it stay in `AGENTS.md`: `scripts/commit_scope.py` reads that section (`HALTED_SECTION`, line 42).
- Budget constants: floor `40960` bytes per project row; root `AGENTS.md` at most `16384` bytes.
- `unit/suite/LND-2-A` (unlanded) also edits `scripts/agent-context.py`, only its import list and `consumer_projects`. Do not touch those two places beyond what a task names.

## Review Focus

1. **A rule that vanished.** A sentence removed from `AGENTS.md` whose pointer target does not say it (Task 2 records a move table; the reviewer checks every row).
2. **A shared rule with no trigger.** Someone adds a document to `suite.shared_rules` and forgets its trigger: the tool must fail, not print it without one (Task 1).
3. **Consumers of a shared crate.** Touching a crate eight applications link must not put eight `STATUS.md` files in `now` (Task 1).
4. **A project with no budget row, or a deleted baseline.** A registered project missing from the baseline is an error; a repository with no baseline file at all (the fixtures) is not checked (Task 3).
5. **The halted-project notice.** It still comes first, in every output mode (Task 1).

---

### Task 1: Tiers in `agent-context.py`

**Files:**
- Modify: `scripts/agent-context.py`
- Modify: `docs/projects.toml` (`[suite]` only)
- Modify: `scripts/documentation_contract.py` (beside the `shared_rules` checks, lines 545-558)
- Test: `scripts/test-documentation-contract.sh`, fixtures under `scripts/fixtures/documentation/`

**Interfaces:**
- Produces, in `docs/projects.toml` `[suite]`:
  - `now_rules = ["docs/standards/language.md"]` (each entry must also be in `shared_rules`)
  - `[suite.reading_triggers]`, path → trigger, with exactly these entries:

    | Path | Trigger |
    |---|---|
    | `CONTRIBUTING.md` | before committing or asking for a landing |
    | `docs/README.md` | when you need to find where a kind of document lives |
    | `docs/governance/change-policy.md` | when the request's scope is unclear or the work would grow beyond it |
    | `docs/governance/documentation.md` | when creating, moving or closing a plan, an evidence record or an inventory |
    | `docs/standards/architecture.md` | when adding a module, a crate, a dependency or a call across projects |
    | `docs/standards/rust-cpp-qt-qml.md` | when writing Rust, C++, Qt or QML code |
    | `docs/standards/verification.md` | before claiming that a change works |
    | `docs/contracts/versioning.md` | before a landing of kind bug, milestone or release |
    | `docs/contracts/landing.md` | before opening a session worktree or asking for a landing |

- Produces, in `scripts/agent-context.py`:
  - `Entry = tuple[Path, str | None]` (document, trigger; `None` means `now`).
  - `resolve_context(root, raw_target) -> tuple[list[Entry], list[str], list[HaltedProject]]`, entries in today's order.
  - Tier by source: `now` = `suite.agents`, every physical `AGENTS.md`, every owner's and consumer's `agents`, the `status` of each **owner** (not consumer), and `suite.now_rules`. Everything else is `when`, with the trigger from `suite.reading_triggers[path]` when present, else the source default:

    | Source | Default trigger |
    |---|---|
    | `readme` | when you need how the product is used or how its tree is laid out |
    | `roadmap` | when choosing, scoping or closing a unit |
    | `validation` | when adding or changing a manual check |
    | `status` of a consumer | when the change alters behaviour this consumer relies on |
    | `context_documents` | when the unit changes what this document designs |
    | contract by scope | when the unit touches what this contract governs |
    | active plan | when working on a unit of this plan: read its header and your unit's row |

  - A `shared_rules` entry that is neither in `now_rules` nor a key of `reading_triggers` is an error: `suite.reading_triggers: no trigger for <path>`.
  - Command line: new flag `--full`. Text output without it:

    ```text
    # Read now, completely
    AGENTS.md
    celestina/AGENTS.md
    celestina/STATUS.md
    docs/standards/language.md

    # Read when its trigger applies, before acting on that matter
    CONTRIBUTING.md — before committing or asking for a landing
    ```

    With `--full`: today's output exactly (flat list, today's order). `--json`: an array of the `now` paths; `--json --full`: today's array. Halt notices keep their place (first line of text output; stderr with `--json`).
- Produces, in `scripts/documentation_contract.py`: `now_rules` must be a list of registered shared rules; every `reading_triggers` key must be an existing file inside the repository and every value a non-empty single line of at most 120 characters; every shared rule needs one of the two. Errors go through `self.error("docs/projects.toml", …)`.

- [ ] **Step 1: Write the failing tests** in `scripts/test-documentation-contract.sh`:
  - add `--full` to every existing `"$context"` call that compares against an `expected/*-context.txt` file or greps for a document that is now `when` (18 call sites; the expected files do not change);
  - the valid fixture's registry gets `now_rules` and `reading_triggers` for its own shared rules;
  - new: `"$context" --root "$valid" app/src/main.rs` equals a new `expected/app-context-tiers.txt` (both headings, the owner's `STATUS.md` under the first, one `—` trigger per line under the second);
  - new: `--json` output is a JSON array equal to the lines under the first heading;
  - new: for the consumed-crate case (the fixture near line 808), the consumer's `AGENTS.md` is under the first heading and its `STATUS.md` under the second with the consumer trigger;
  - new: for the halted-owner case, the first line is still the `HALTED:` notice;
  - new invalid fixtures `shared-rule-without-trigger`, `now-rule-not-shared`, `trigger-for-missing-file`: the checker exits non-zero and names the key.
- [ ] **Step 2:** Run `bash scripts/test-documentation-contract.sh`. Expected: FAIL (unknown argument `--full`).
- [ ] **Step 3:** Implement in `scripts/agent-context.py`: carry the source through `resolve_context` (it already adds documents source by source), `trigger_for(relative: str, source: str, registry) -> str | None`, the two output modes. Implement the registry checks in `documentation_contract.py`. Add the keys to `docs/projects.toml`.
- [ ] **Step 4:** Run `bash scripts/test-documentation-contract.sh` and `bash scripts/check-documentation-contract.sh`. Expected: both exit 0.
- [ ] **Step 5:** Record in the evidence the byte totals of `python3 scripts/agent-context.py --full celestina/src` and of the `now` tier (`--json`), summed with `wc -c`. Commit: `suite-maintenance: Add reading tiers to the agent context`.

### Task 2: Trim the root `AGENTS.md` and reword the preflight

**Files:**
- Modify: `AGENTS.md`
- Create: `docs/history/qt-shell-halt-and-salvage.md`
- Modify: `docs/history/README.md` (its list)
- Modify: `CONTRIBUTING.md` and every project `AGENTS.md` whose text tells an agent to read the whole context (`rg -n 'agent-context|completely|every document' CONTRIBUTING.md docs/README.md */AGENTS.md`)
- Modify: `docs/governance/documentation.md` or `docs/contracts/{landing,production-artifacts,versioning}.md`, only to receive a sentence that had no home (see Global Constraints)
- Modify: `docs/evidence/2026-11-24-dev-speed-preflight.md` (the move table)

**Interfaces:**
- Consumes: Task 1's output format and trigger wording.
- Produces: root `AGENTS.md` of at most 16 384 bytes, with these sections under their current headings:

  | Section (bytes today) | After |
  |---|---|
  | Repository language (1 014) | unchanged |
  | Halted projects (5 783) | the rule: first paragraph, the four bullets and "Only the author lifts…" stay; "History: the Qt/QML shell" and "Salvage record" move verbatim to `docs/history/qt-shell-halt-and-salvage.md`; one line points there |
  | Mandatory preflight (1 094) | rewritten, text below |
  | Authority and scope, Required technical level, Architecture direction, Reuse and modularity, Implementation invariants | unchanged |
  | Documents and two delivery lanes (6 008) | the list of document kinds and the paragraph "An implementation checkpoint closes…" stay; ledger, inventory, prefix, hook and ratchet mechanics become: "Ledger rows, inventories, commit prefixes, hooks and ratchets: `docs/governance/documentation.md` and `docs/contracts/landing.md`, before closing a unit or changing a guard." |
  | Production artifacts (1 965) | keep: the five entry names in one sentence, "A session never runs them in its worktree", "Do not run `clean`", and that completion is the exit of a deployable bug fix or milestone; the rest is a pointer to `docs/contracts/production-artifacts.md` and `docs/contracts/versioning.md` |
  | Minimum evidence, Git and commits | unchanged |

- Produces the new "Mandatory preflight" text:

  ```markdown
  ## Mandatory preflight

  Before acting:

  1. Run `python3 scripts/agent-context.py PATH` for the area you will touch.
     Read every document under "Read now" completely.
  2. Each document under "Read when" names its trigger. Read it completely
     when that trigger applies, before you act on that matter; not before.
     `--full` prints every document in the old reading order.
  3. When you work on a planned unit, read its plan's header and your unit's
     ledger row; the ledger is the cross-session hand-off and names the unit
     you may edit.
  4. Inspect `git status`, the real checkout, consumers, manifests, and guards.
     Documentation may be stale.
  5. Search first with `rg` or `rg --files`; reuse existing contracts instead of
     creating a parallel recipe.
  6. Declare or update the ledger unit before editing planned milestone work, in
     a session worktree opened with `scripts/worktree.sh`.
  ```

- [ ] **Step 1:** For each sentence that will leave `AGENTS.md`, find it in the pointer's target (`rg` on a distinctive phrase). Write a table in the evidence: `Moved text (first words) | Now lives in (file, heading) | Already there / moved verbatim`. No row may read "dropped".
- [ ] **Step 2:** Create the history file (title, one line saying it was moved from the root contract on 2026-11-24, then the two blocks byte for byte) and edit `AGENTS.md` and the files that receive sentences.
- [ ] **Step 3:** Align `CONTRIBUTING.md` and the project `AGENTS.md` files found by the `rg` above to the new wording ("the `now` tier", "when its trigger applies"); change only those sentences.
- [ ] **Step 4:** Run. Expected: the size is at most 16384 and each guard exits 0.

```sh
wc -c AGENTS.md
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-commit-scope.sh
```

- [ ] **Step 5:** Commit: `suite-maintenance: Update the root agent contract to the reading tiers and move its Qt history`.

### Task 3: The `now` budget

**Files:**
- Create: `scripts/preflight_budget.py`
- Create: `scripts/preflight-baseline.tsv`
- Create: `scripts/test-preflight-budget.py`
- Modify: `scripts/documentation_contract.py` (one call, after the registry checks)
- Modify: `.github/workflows/contracts.yml` (add `python3 scripts/test-preflight-budget.py` to the "Hermetic fixtures" step)
- Modify: `docs/governance/documentation.md` (one paragraph: what the budget measures and how to get back under it)

**Interfaces:**
- Consumes: `suite.now_rules` from Task 1; the trimmed `AGENTS.md` from Task 2.
- Produces, in `scripts/preflight_budget.py` (imports nothing from `documentation_contract`):
  - `BASELINE = "scripts/preflight-baseline.tsv"`, `ROOT_ROW = "root-agents"`
  - `now_documents(registry: dict, project_id: str) -> list[str]`: for `"suite"`, the suite's `agents`, `status` and `now_rules`; for a project, the suite's `agents`, the project's `agents` and `status`, and `now_rules`. Deduplicated, in that order.
  - `measure(root: Path, registry: dict) -> dict[str, int]`: bytes per row id, for `"suite"`, every project in `registry["projects"]` that is not halted, and `ROOT_ROW` (the size of the suite's `agents` alone). A missing file counts 0; the documentation contract already reports it.
  - `load_baseline(path: Path) -> dict[str, int]`: lines `id<TAB>bytes`; `#` comments and blank lines skipped; a malformed line raises `ValueError` naming the line number.
  - `check(root: Path, registry: dict) -> list[str]`: `[]` when `root / BASELINE` does not exist. Otherwise one message per problem: `preflight budget: <id> has no row in scripts/preflight-baseline.tsv`; `preflight budget: <id> reads <n> bytes before acting, over its limit of <m>; move closed work out of its STATUS.md into docs/history, or shorten its AGENTS.md`; a row whose id is neither a measured id nor `root-agents` is `preflight budget: unknown row <id>`.
  - CLI: `preflight_budget.py [--root DIR] measure` prints `id<TAB>bytes` for every row, sorted by id.
- Produces `scripts/preflight-baseline.tsv`: a two-line `#` header saying what a row is and that a row may only go down, then `root-agents<TAB>16384` and, for the suite and each project, `max(40960, measured rounded up to the next multiple of 4096)`, from `measure` run after Task 2.
- Produces, in `documentation_contract.py`: each message of `preflight_budget.check(root, registry)` reported through `self.error("scripts/preflight-baseline.tsv", message)`.

- [ ] **Step 1: Write the failing tests** in `scripts/test-preflight-budget.py` (`unittest`, each on a temporary directory with a tiny registry dict and files of chosen sizes):
  - `test_a_project_reads_the_suite_contract_its_own_contract_its_status_and_the_now_rules`: `now_documents` returns those four paths in order.
  - `test_the_suite_reads_its_contract_status_and_now_rules`.
  - `test_under_its_limit_passes`: `check` returns `[]`.
  - `test_over_its_limit_names_the_project_and_both_numbers`.
  - `test_a_project_without_a_row_is_an_error`.
  - `test_an_unknown_row_is_an_error`.
  - `test_a_halted_project_is_not_measured`: its id is absent from `measure` and a missing row for it is not an error.
  - `test_no_baseline_file_checks_nothing`.
  - `test_the_root_contract_has_its_own_row`: suite `agents` of 20 000 bytes against `root-agents<TAB>16384` is an error.
  - `test_a_malformed_baseline_line_names_its_number`.
  - `test_the_repository_is_within_its_budget`: `check` on the real checkout (root = parent of `scripts`) returns `[]`.
- [ ] **Step 2:** Run `python3 scripts/test-preflight-budget.py`. Expected: FAIL (`ModuleNotFoundError: preflight_budget`).
- [ ] **Step 3:** Implement the module. Generate the baseline with `measure` and the formula; record both columns (measured, limit) in the evidence.
- [ ] **Step 4:** Add the call in `documentation_contract.py`; add one invalid fixture `scripts/fixtures/documentation/invalid/preflight-over-budget/` (a baseline row smaller than the fixture's files) and its case in `scripts/test-documentation-contract.sh`, expecting the "over its limit" message.
- [ ] **Step 5:** Run. Expected: each exits 0.

```sh
python3 scripts/test-preflight-budget.py
bash scripts/test-documentation-contract.sh
bash scripts/check-documentation-contract.sh
```

- [ ] **Step 6:** Commit: `suite-maintenance: Add a byte budget for the first reading tier`.

### Task 4: Evidence, final guards, hand-over

**Files:**
- Modify: `docs/evidence/2026-11-24-dev-speed-preflight.md`

- [ ] **Step 1:** Fill the evidence: bytes of `--full` and of the `now` tier for `celestina/src`, `siderita/src`, `celestina-rs/crates/celestina-core` and `.`, before and after (the before column is the `--full` total, which is the list an agent had to read until now; the after column is the `now` tier); root `AGENTS.md` before (24 715) and after; the move table of Task 2; the measured/limit table of Task 3. **Limits:** no agent session was timed with the new tiers.
- [ ] **Step 2:** Run every guard and test, from the worktree root. Expected: each exits 0. `test-land-unit.sh` takes about 75 s.

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-preflight-budget.py
bash scripts/test-commit-scope.sh
bash scripts/test-land-unit.sh
```

- [ ] **Step 3:** Commit: `suite-maintenance: Add the evidence of the reading tiers`. Report to the coordinating session and stop: branch, commits, the tables, what was not done or not checked, and the author's command:

```sh
cd /home/toni/CODIGO/CELESTINA
time python3 scripts/land-unit.py unit/suite/DEV-1-B --kind maintenance --summary "Add tiers to the agent preflight and a budget for its first tier"
```

  The landing now closes the session worktree itself; this is its first real run, so report what it printed.

## Chunks and models (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| U02.1 · Tiers in `agent-context.py` (Task 1) | Sonnet 5.5 | medium | Pinned interface and output; many fixture edits |
| U02.2 · Trim `AGENTS.md` (Task 2) | Opus 5.5 | high | Judgement on every sentence; a lost rule is the unit's main risk |
| U02.3 · Budget (Task 3) | Sonnet 5.5 | medium | Small new module with eleven pinned tests |
| U02.4 · Evidence and guards (Task 4) | Sonnet 5.5 | low | Commands and tables |
| U02.R · Final review | Fable 5.2 | high | Different from the implementers; `/code-review`, at most two rounds |

Tasks run in order in one worktree: Task 2 uses Task 1's wording, Task 3 measures what Task 2 left.
