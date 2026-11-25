# Suite development speed — plan 3: light units and the review cap (U04, `DEV-1-C`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Write down, where a session reads it before starting, when a unit is light, what a light unit skips, and how many review rounds any unit may take.

**Architecture:** Documents only. `docs/standards/verification.md` owns three new sections; the root `AGENTS.md` and the registry's reading trigger point at them so a session meets them when it sizes a unit; a short evidence template is added. No script, guard or hook changes. The author's global `CLAUDE.md` and the standing-tracks draft, both outside the repository, are edited by the coordinating session with the author's approval of the exact text.

**Tech Stack:** Markdown, one TOML string.

**Spec:** `docs/superpowers/specs/2026-11-24-suite-dev-speed-design.md`, section 4 "U04". Ledger: `docs/plans/active/2026-11-24-dev-speed.md`, row `DEV-1-C`.

## Global Constraints

- Repository text is English; the author is addressed in Spanish.
- Never edit the canonical checkout. Open the unit from it: `scripts/worktree.sh open suite DEV-1-C`.
- First edit: set the `DEV-1-C` row to `active` with `[evidence](../../evidence/2026-11-25-dev-speed-light-units.md)`; the plan file changes only in that row. Copy this plan to `docs/superpowers/plans/2026-11-24-suite-dev-speed-plan-3.md`.
- Session commits: `suite-maintenance: <Add|Fix|Remove|Update> …`.
- The session never runs `scripts/land-unit.py`, `git push` or a merge into `main`.
- In `docs/projects.toml` change only the one trigger string named below, in `[suite.reading_triggers]`.
- The word is **light unit**, never "light lane": `CONTRIBUTING.md` already uses "two delivery lanes" for implementation and author validation.
- Root `AGENTS.md` must stay at or under 16 384 bytes (14 127 today).

## Review Focus

1. **A sensitive change dressed as light.** The three conditions are joined by "all"; the list of sensitive paths is closed and literal.
2. **A unit that stops qualifying mid-work.** The text must say what happens: it becomes ordinary, and the session says so.
3. **A session that never opens `verification.md`.** The pointer in `AGENTS.md` (always read) and the trigger must both name sizing a unit.
4. **Rounds read as a target.** The numbers are caps; nothing requires reaching them.
5. **The short template failing the guard.** It must keep the metadata and sections `check_evidence` requires (`Date`, `Scope`, `Environment`, `Artifact`; `Procedure`, `Result`, `Limits`).

---

### Task 1: The three sections, the pointer, the template

**Files:**
- Modify: `docs/standards/verification.md` (append three sections after "Evidence wording")
- Create: `docs/templates/evidence-light.md`
- Modify: `docs/templates/README.md` (one list entry)
- Modify: `docs/governance/documentation.md` ("Evidence": one sentence)
- Modify: `AGENTS.md` ("Minimum evidence": one paragraph)
- Modify: `CONTRIBUTING.md` (one sentence under "Plans and monorepo ledger")
- Modify: `docs/projects.toml` (one trigger string)
- Create: `docs/evidence/2026-11-25-dev-speed-light-units.md` (from the new short template: this unit is its first user, though it is not itself light)

**Interfaces:**
- Produces, appended to `docs/standards/verification.md`, exactly:

````markdown
## Sensitive paths

A change is sensitive when it touches any of these:

- `celestina/crates/celestina-lock`, `celestina/crates/celestina-lock-verify`,
  `celestina-rs/crates/celestina-lock-proto`, `celestina/src/lock`;
- `celestina/src/polkit`, `celestina/src/providers/polkit`,
  `celestina/contrib/pam.d`;
- a `*-production.sh` script of any project;
- `scripts/*.py` and `scripts/*.sh` at the repository root, `.githooks/`,
  `.github/`;
- `docs/projects.toml`.

The ratchet baselines (`scripts/*-baseline.tsv`) are not sensitive.

## Light units

A unit is light when all three hold:

1. its kind is `maintenance` or `bug`;
2. it changes at most 150 lines, added plus deleted, outside documents and
   lockfiles (`git diff --numstat origin/main...`, leaving out `*.md`, `*.tsv`
   and `Cargo.lock`);
3. it touches no sensitive path.

The session sizes the unit before it starts and writes `light unit` in the
`Scope` line of its evidence. A unit that stops qualifying while it is being
worked on becomes an ordinary unit from that point: the session says so and
follows the ordinary rules for what remains.

A light unit:

- has no design and no task-plan document; its ledger row describes it;
- is implemented by the session itself, without subagents;
- records its evidence from
  [the short template](../templates/evidence-light.md): at most 40 lines,
  with commands, exits and limits, and no narrative or review history;
- takes one review round.

Everything else is ordinary: the ledger row, the inventory, the version rule,
and the landing's build, verification and full smoke.

## Review rounds

A round is one independent review of the unit's diff, followed by its fixes.
These are caps, not targets:

| Unit | Rounds at most |
|---|---|
| Light | 1 |
| Ordinary | 2; the second reads only the fixes of the first |
| Touches a sensitive path | 3 |

After the last round, a finding that is a defect in what the unit set out to
do is fixed and checked by the session, with no further reviewer. Any other
finding becomes a backlog entry in the owner's plan or a line under
`Follow-up` in the evidence. More rounds need the author's request.

Evidence records what ran, not the review: one line gives the rounds, the
findings fixed and the findings deferred.
````

- Produces `docs/templates/evidence-light.md`, exactly:

````markdown
# Evidence: topic

- **Date:** YYYY-MM-DD
- **Scope:** light unit; unit id and paths
- **Environment:** toolchain or session facts that matter
- **Artifact:** exact binary/manifest or `not applicable`

## Procedure

```sh
exact command
```

## Result

- **Exit:** one line per command
- **Review:** rounds, findings fixed, findings deferred

## Limits

- What this does not prove; checks not run and why.

## Follow-up

Deferred findings, or `None`.

Keep the whole record under 40 lines. No narrative and no review history.
````

- Produces in `AGENTS.md`, as a new last paragraph of "Minimum evidence":

```markdown
Size the unit before starting it. A small `maintenance` or `bug` unit off the
sensitive paths is a light unit and skips the design, the task plan and the
long evidence record, and every unit has a cap on review rounds. Both rules
are in [docs/standards/verification.md](docs/standards/verification.md).
```

- Produces in `docs/projects.toml`: the trigger of `docs/standards/verification.md` becomes `"before claiming that a change works, and when sizing a unit or a review"`.
- Produces in `docs/governance/documentation.md`, end of "Evidence": `A light unit uses the short template; see [the verification standard](../standards/verification.md#light-units).`
- Produces in `CONTRIBUTING.md`, end of the first paragraph of "Plans and monorepo ledger": `A light unit needs the ledger row and no other plan document; see [the verification standard](docs/standards/verification.md#light-units).`

- [ ] **Step 1:** Open the worktree, set the row, copy this plan, create the evidence record from the new template's shape.
- [ ] **Step 2:** Make the edits in **Interfaces**, byte for byte where a block is given.
- [ ] **Step 3:** Run, from the worktree root. Expected: the size is at most 16384, the second command prints the new trigger beside `docs/standards/verification.md`, and each guard exits 0.

```sh
wc -c AGENTS.md
python3 scripts/agent-context.py celestina/src | grep verification
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-preflight-budget.py
```

- [ ] **Step 4:** Fill the evidence (those commands and exits; **Limits:** the rules are text, no guard enforces a light unit's conditions or the round cap; no unit has run under them yet). Commit: `suite-maintenance: Add light units, the review cap and the sensitive-path list`.
- [ ] **Step 5:** One review round by a different model: does the text satisfy the spec's U04 and this plan's Review Focus, and does any sentence contradict `AGENTS.md`, `CONTRIBUTING.md` or `docs/contracts/landing.md`. `docs/projects.toml` is a sensitive path, so the cap for this unit is three; one is expected.
- [ ] **Step 6:** Hand the author the command:

```sh
cd /home/toni/CODIGO/CELESTINA
time python3 scripts/land-unit.py unit/suite/DEV-1-C --kind maintenance --summary "Add the light lane, the review cap and the sensitive-path list"
```

### Task 2: Outside the repository (coordinating session, after the landing)

**Files:**
- Modify: `/home/toni/.config/claude/CLAUDE.md` (the author approves the exact text first; proposal at `/home/toni/CODIGO/CLAUDE-global-propuesto.md`)
- Modify: `/home/toni/CODIGO/2026-11-24-celestina-standing-tracks-design.md` line 37: "with the repository's rules unchanged" becomes "under the repository's rules; an entry that qualifies as a light unit (`docs/standards/verification.md`) runs as one"

- [ ] **Step 1:** Replace the global file with the approved text.
- [ ] **Step 2:** Edit the standing-tracks draft sentence. Tell the author that its plan 1 still names the RSH-1 ledger for its administrative unit, which is now the `dev-speed` ledger.

## Chunks and models (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| U04.1 · Documents (Task 1) | the coordinating session (Fable 5.2) | medium | Exact text is given; one worktree, eight files; small unit, run in the planning session |
| U04.R · Review | Opus 5.5 | high | Different from the implementer; one round |
| U04.2 · Global `CLAUDE.md` and the draft (Task 2) | the coordinating session | low | Two files outside the repository, text approved by the author |
