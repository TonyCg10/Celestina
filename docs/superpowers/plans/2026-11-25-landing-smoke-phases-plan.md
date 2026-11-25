# Landing a series unit with only its smoke phases — implementation plan

> **For agentic workers:** executed natively by the session that wrote it
> (superpowers:executing-plans). Steps use checkbox (`- [ ]`) syntax.

**Goal:** `land-unit.py --smoke ID[,ID…]` lands a mid-series unit whose shell
verification runs only those smoke phases; a landing without the option never
accepts a shell artifact verified with a partial smoke.

**Architecture:** the landing passes the phases to the shell's
`verify-production.sh` in one environment variable, which
`production_artifact.py run-verification` already forwards (it copies
`os.environ`). The verify script runs `smoke.sh --only` with them and, after the
smoke passes, writes the scope it ran to a file beside the release binary. The
landing reads that file to decide whether a current, verified shell artifact
still needs a full verification.

**Tech stack:** Python 3 (`scripts/land-unit.py`, `scripts/landing.py`,
`unittest` in `scripts/test-land-unit.py`), POSIX sh.

**Spec:** `~/CODIGO/2026-11-25-landing-smoke-phases-design.md` (approved
2026-11-25), committed with this unit under `docs/superpowers/specs/`.

## Global constraints

- Unit `DEV-2-A`, prefix `suite`, kind `maintenance`. It touches sensitive
  paths (`scripts/*.py`, `celestina/scripts/`): ordinary unit, at most 3 review
  rounds, at least 1.
- Do not change `scripts/production_artifact.py` or `docs/projects.toml`: both
  are verification inputs of every registered project, so a change would make
  every project re-verify. (This replaces the design's "the manifest gains
  `smoke`" with a scope file; same behaviour.)
- Without `--smoke`, every byte of today's behaviour stays: the author's manual
  `verify-production.sh` and `complete-production.sh` run the full smoke.
- Development truth in English; agents speak Spanish to the author.

## Decisions

- Variable: `CELESTINA_SMOKE_ONLY`, a comma-separated list of phase ids, empty
  or unset meaning the full smoke.
- Scope file: `celestina/target/release/smoke-scope`, one line, `full` or the
  ids. `verify-production.sh` removes it before its first step and writes it
  only after `smoke.sh` exits 0. A missing file on a verified artifact means
  `full` (every manifest verified before this unit ran the full smoke).
- Which project has phases: a constant in `scripts/landing.py`,
  `SMOKE_PHASES = {"celestina": ("celestina/scripts/smoke.sh",
  "celestina/target/release/smoke-scope")}`. The ids are validated against
  `celestina/scripts/smoke.sh --list` in `preflight`; projects without phases
  ignore the variable (the affected set is known only after the rebase).
  Implementation call: the ids are checked at the start of `build_if_stale`,
  on the rebased tip (a unit may add a phase), before any entry runs, not in
  `preflight`.
- `LandState` gains `smoke: str = ""`, so `--continue` keeps it. `--continue`
  and `--abort` refuse `--smoke`, like `--kind`.
- Evidence: the `Build` line of a phased project built or verified with
  `--smoke` ends with `smoke phases <ids> only; the last landing of the series
  runs the full smoke`; a re-verification of a partial scope says so in the
  `Check` line. (Implementation call: no new `landing_section` parameter.)
- Hosting: the suite's one active plan is TRK-1, whose scope excludes code, and
  `land-unit.py` needs an open row in an active plan. So `DEV-2` is a suite
  plan opened and closed in one hand-sealed commit (as VIS-1), sealed with
  `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat`, merged by fast-forward. The landing would also have
  re-verified the shell (its verify script changes); that now happens at the
  shell's next landing, which this unit's evidence says.

## Review focus

1. A partial verification followed by a landing without `--smoke` on the same
   bytes: must run `verify_script` alone, not accept the artifact.
2. A verification that fails after the scope file was removed: the next
   landing must not read a stale `full`.
3. `--continue` after a stop in `build_if_stale` keeps the phases.
4. A phase id that does not exist, or an empty item (`osd,`): refused in
   preflight, before any build.
5. A unit that affects no shell (a siderita unit landed with `--smoke`): the
   option changes nothing and the evidence line still appears only when the
   shell was verified with it.

---

### Task 1: the shell's verify script honours the variable

**Files:** modify `celestina/scripts/verify-production.sh`.

- [ ] Before `test-production-artifacts.sh`: `scope_file=$project_root/target/release/smoke-scope; rm -f "$scope_file"`.
- [ ] Replace the smoke line with: when `${CELESTINA_SMOKE_ONLY:-}` is
  non-empty, `smoke.sh --binary "$binary" --only "$CELESTINA_SMOKE_ONLY"`,
  else the current line; then `printf '%s\n' "${CELESTINA_SMOKE_ONLY:-full}" > "$scope_file"`.
- [ ] Check: `sh -n celestina/scripts/verify-production.sh` exits 0;
  `sh celestina/scripts/smoke.sh --list` lists the ids the tests use.

### Task 2: the landing takes `--smoke`

**Files:** modify `scripts/landing.py`, `scripts/land-unit.py`; test
`scripts/test-land-unit.py`.

**Interfaces (produce):**
- `landing.SMOKE_PHASES: dict[str, tuple[str, str]]`
- `landing.smoke_scope(root: Path, project_id: str) -> str | None` — `None`
  for a project without phases; else the file's stripped text, or `"full"`
  when it is missing.
- `landing.parse_smoke(value: str, known: list[str]) -> str` — the value
  unchanged; raises `LandingError` on an empty item, an unknown id or a repeat.
- `LandState.smoke: str = ""`; `landing_section(..., smoke: str | None = None)`.

- [ ] Write failing tests in `scripts/test-land-unit.py`:
  - `test_parse_smoke_refuses_unknown_empty_and_repeated_ids` (`"osd"` ok;
    `"osd,"`, `"nope"`, `"osd,osd"` raise);
  - `test_smoke_scope_reads_the_file_and_defaults_to_full` (no phases →
    `None`; missing → `"full"`; file `osd` → `"osd"`);
  - `test_land_state_round_trips_smoke_and_loads_older_states_without_it`;
  - `test_landing_section_names_the_smoke_phases`;
  - `test_parse_arguments_refuses_smoke_with_continue_or_abort`;
  - `test_check_and_build_reverifies_a_partially_smoked_artifact_without_smoke`
    and `test_check_and_build_accepts_it_with_smoke`, built on the fake
    registry of `test_affected_projects_puts_the_owner_first` (line ~1073),
    with a project id in a patched `SMOKE_PHASES`;
  - `test_the_verify_entry_receives_the_phases` (the fake verify script
    writes `$CELESTINA_SMOKE_ONLY` to a file).
- [ ] Run `python3 scripts/test-land-unit.py -k smoke` (or the module's own
  runner): the new tests fail.
- [ ] Implement: parse `--smoke` in `parse_arguments`; validate it in
  `preflight` against `sh celestina/scripts/smoke.sh --list`; store it in
  `LandState`; in `check_and_build`, when the check passes, the project has
  phases, `state.smoke` is empty and `smoke_scope` is not `"full"`, run the
  verify script alone; run every registered entry with
  `CELESTINA_SMOKE_ONLY=state.smoke` in its environment; pass `smoke` to
  `landing_section` when a phased project was built or verified with it.
- [ ] Run `bash scripts/test-land-unit.sh`: every test passes.

### Task 3: the rule and the unit's records

**Files:** `docs/standards/verification.md` (Agent lane item 4),
`docs/contracts/landing.md`, `CONTRIBUTING.md` only if it lists the landing's
options; `docs/plans/archive/2026-11-25-landing-smoke-phases.md` (DEV-2,
born archived), `docs/evidence/2026-11-25-landing-smoke-phases.md` (ordinary
template), `docs/superpowers/specs/` and `docs/superpowers/plans/` copies of
the design and this plan, root `ROADMAP.md` and `STATUS.md` entries, the
inventory `docs/inventories/2026-11-25-landing-smoke-phases/DEV-2-A.numstat.tsv`.

- [ ] The rule, in `landing.md` with one line in `verification.md`: a unit of
  a series lands with `--smoke` naming the phases it touches; the last unit
  of the series, and any unit outside a series, lands without it; an
  abandoned series closes with a landing without it or with
  `complete-production.sh`.
- [ ] `/simplify` over the diff in-session; `stop-slop` over the new prose.
- [ ] Guards: `bash scripts/check-documentation-contract.sh`,
  `python3 scripts/check-language-contract.py`,
  `bash scripts/check-architecture-contract.sh`, `bash scripts/test-land-unit.sh`,
  `python3 scripts/check-staged-units.py --root . <inventory>`: each exit 0.
- [ ] Review `DEV-2-A.R` (below), fix, seal, commit; the author merges by
  fast-forward, pushes and closes the worktree.

## Models (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Tasks 1–3 | this session (Opus) | as set | Small, touches the landing; done in-session, no subagents |
| DEV-2-A.R · final review | Sonnet | high | Different from the implementer; `/code-review` over the diff |
