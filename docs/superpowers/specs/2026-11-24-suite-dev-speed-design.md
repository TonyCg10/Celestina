# Suite development speed: design

- **Date:** 2026-11-24
- **Status:** approved 2026-11-24
- **Scope:** the Celestina monorepo's development process (scripts, agent
  documents, the shell's smoke) and the author's global agent instructions

## 1. Intent

Each unit costs too much wall-clock time and too many agent tokens for reasons
that are not the unit's own work. The author wants development to stop taking
this long, without giving up what makes a landing trustworthy.

Measured on 2026-11-24:

- The shared session Cargo target held 407 GiB (970 copies of the shell's
  375 MB test binary, 181 GiB of incremental state); three private targets
  held 74 GiB more. `worktree.sh close` removes the shared target only when
  the last session worktree goes, and 53 worktrees were never closed. Cleaned
  by hand that day (243 GiB freed on disk).
- `scripts/agent-context.py celestina/src` orders 22 documents, 373 KB
  (about 95 000 tokens), read "completely" by every session and subagent.
  `celestina/VALIDATION.md` alone is 82 KB of author-only manual checks.
- The root `AGENTS.md` is 438 lines, 24.7 KB.
- `celestina/scripts/smoke.sh` is one 2 165-line file, 22 nested niri runs in
  series, 2 min 18 s to 3 min 35 s; a rest-frame doubt reruns all of it, up
  to three times.
- Past sessions record review rounds 4, 5, 6 and up to 10 on single units.
- Since 2026-10-01, 87 of 190 commits are `maintenance`; evidence documents
  reach 137 KB for one unit.

Not measured: the time of one full landing, cold and incremental build times.
The first unit records them as the baseline.

## 2. Goals, non-goals, anti-goals

**Goals**

- A session reads about 40 KB before it may act on the shell, not 373 KB,
  and that figure cannot grow back unnoticed.
- The session build cache never exceeds a fixed cap and landed worktrees
  close themselves.
- While developing, a session runs the smoke phases of the area it touched;
  a rest-frame doubt reruns one phase, not the script.
- Small `maintenance` and `bug` units are written up in a short evidence
  record from a template, take one review round, and need no design or task
  plan document.
- No unit takes more than two review rounds, three on sensitive paths.

**Non-goals**

- Changing production artifacts, their verification, version bumps,
  inventories, commit scope or hook integrity rules. Every landing still runs
  the full smoke through `verify-production.sh`.
- Deleting or rewriting historical plans, specs, evidence or inventories.
- Trimming the applications' `STATUS.md` files (hematita's is 42 KB); the
  budget below records them as they are.
- The applications' own smokes (about 8 s each) and their Qt builds.
- Parallel smoke runs: rest-frame counting needs the host to itself.

**Anti-goals**

- A lighter lane that a sensitive change (the lock, polkit, PAM, the guards
  and landing code) can use.
- A shorter preflight that hides a rule an agent then breaks; every moved rule
  stays reachable from a one-line pointer with its trigger.
- A landing that verifies less than today.

## 3. Approaches

**A. One unit that changes everything.** It mixes script behaviour, shell
test code and documents, and one failure holds all of it back. Rejected.

**B. Four independent units, cheapest first (chosen).** Build and disk, then
preflight, then smoke phases, then ceremony. Each lands alone and pays back
before the next starts.

**C. Ceremony first.** The largest saving per unit, but it would land through
the heaviest path. Rejected.

For the ceremony the author chose, on 2026-11-24, the **medium cut** over "no
cut" (review cap and one plan system only) and "strong cut" (no inventory and
no release build for small units). As first described, the medium cut dropped
the evidence document and ran a partial smoke at the landing. The repository
makes both expensive: the landing's `seal` step writes its `## Landing`
section into the evidence record and stacked merges read it
(`scripts/land-unit.py:1133-1149`), and the smoke runs inside the verify
script whose result seals the artifact (`verify-production.sh:93`), so a
partial smoke would mean a "partly verified" artifact. This design therefore
keeps a **short** evidence record and the full smoke at the landing, and takes
the saving where it is: document length, review rounds, and no design or plan
documents. The light lane then needs no change to any guard or to the landing.

This amends one sentence of the standing-tracks design of the same day
("each entry is one ordinary unit with the repository's rules unchanged"): a
track entry that qualifies uses the light lane.

## 4. The units

Planning labels `U00`–`U04` follow the author's global convention; ledger
unit ids are the planner's call. Task plans go to `docs/superpowers/plans/`,
as `docs/governance/documentation.md` requires.

### U00 — Open the plan (suite, administrative)

The suite roadmap is `active` on RSH-1 and allows one active plan
(`AGENTS.md:284-286`); `STATUS.md:55-58` says RSH-1 stays active until an
administrative unit archives it and that new cross-project work needs an
accepted decision. So the first unit: archives the RSH-1 plan (the shell is
complete and the author's session runs it since the cutover), records the
decision that promotes this work, opens the suite plan `dev-speed` with its
ledger, switches the root roadmap's active checkpoint, and adds this design.
It lands after the in-flight unit that opens the standing tracks
(`unit/suite/CEL-TRACKS-OPEN`), which also edits the root roadmap.

### U01 — Build cache and disk (suite)

- **Baseline first:** record cold `cargo test --workspace --no-run`, a
  one-line-edit rebuild of the same, and the phases of one real landing, in
  the unit's evidence. Repeat after the change.
- **Dev profile for sessions only.** `worktree.sh open` already writes the
  untracked `.cargo/config.toml` of each session worktree; it also writes
  `[profile.dev] debug = "line-tables-only"` and
  `[profile.dev.package."*"] debug = false`. Manifests are not touched: they
  are production inputs. CI sets the same through `CARGO_PROFILE_DEV_*`
  environment variables in `celestina.yml` and `celestina-rs.yml`. No linker
  change: the pinned 1.97.1 is expected to link with `rust-lld` already; the
  baseline task confirms it and, if it does not, the unit stops and reports.
- **Private targets live inside the worktree, under `target/`.** A session
  that needs its own target uses `<worktree>/target/private`: the guards
  already skip a directory named `target`
  (`scripts/documentation_contract.py:205-209`), and `worktree.sh` excludes
  `/target/` as it excludes `.cargo/config.toml`. Nothing creates
  `.cargo-target-<name>` beside the worktrees again; the shell's local
  `AGENTS.md` says so where it describes the private-target smoke.
- **A landing closes its session worktree.** After the `prune` step,
  `land-unit.py` runs `worktree.sh close PROJECT UNIT` for the landed unit.
  A refusal (uncommitted changes, commits the landing never saw) is printed
  and does not fail the landing.
- **A cap replaces the last-close deletion.** `worktree.sh close` no longer
  deletes the whole shared target when the last worktree goes
  (`scripts/worktree.sh:236-241`): with auto-close that would rebuild every
  dependency after each landing. Instead, at every close, when
  `<worktrees>/.cargo-target` exceeds a cap (default 40 GiB, one constant),
  `debug/incremental` and every artifact of a workspace member crate are
  removed; third-party dependency artifacts stay. Owner:
  `scripts/build_trees.py`.

### U02 — Preflight in tiers (suite)

- **Two tiers in the registry.** Every document `agent-context.py` can print
  gets a tier in `docs/projects.toml`: `now`, or `when` with a one-line
  trigger ("before a landing", "when the unit changes QML or tokens", "when
  adding a manual check", "when the unit touches the lock"). The default
  text output prints the `now` list, then the `when` list with triggers.
  `--full` prints today's order. `--json` keeps its shape, a plain list of
  paths, because `scripts/test-documentation-contract.sh` consumes it; it
  lists the `now` tier by default and everything with `--full`.
- **What is `now`:** root and local `AGENTS.md`, the owner's `STATUS.md` and
  `docs/standards/language.md`. An active plan ledger is `when: working on a
  unit of this plan`, and then the session reads the plan's header and its
  own unit's row, not every row. `VALIDATION.md` is `when: adding or changing
  a manual check`.
- **Trim the root `AGENTS.md` to about 12 KB.** The Qt halt history and the
  salvage record move verbatim to `docs/history/` with one pointer line; the
  inventory, hook and production-artifact mechanics that
  `docs/contracts/{landing,production-artifacts,versioning}.md` already own
  shrink to pointers with triggers. "Mandatory preflight" is reworded: read
  the `now` tier completely; read a `when` document when its trigger applies,
  before acting on that matter.
- **A budget that cannot regrow.** A baseline file in the repository's
  ratchet style records the `now` size of each registered project; a test
  fails when one grows past its row. The shell's row starts at or under
  40 KB; the applications' rows start where their `STATUS.md` puts them.

### U03 — Smoke in phases (celestina)

- **One file per phase.** `celestina/scripts/smoke.sh` becomes a driver plus
  `celestina/scripts/smoke/common.sh` and one script per phase (base,
  launcher, bar, wallpaper, energy, glance, clipboard, connectivity, osd,
  phone, media, overview, polkit, cutover, lock-artifacts; the planner fixes
  the list from the file). Checks move unchanged.
  `celestina/scripts/smoke/` is added to the shell's `verification_inputs`
  in `docs/projects.toml`.
- **Selection.** `smoke.sh --binary PATH` runs everything, as today and as
  `verify-production.sh` calls it. `--only A,B` runs those phases; `--list`
  prints them. The nest lock and signal handling stay in the driver.
- **Retry one phase.** An inconclusive rest-frame check reruns that phase, up
  to three attempts, instead of restarting the script.
- **Proof:** the full smoke passes three times in a row; the number of
  assertions executed is the same before and after; each phase passes alone.

### U04 — Ceremony (suite documents, plus the author's global instructions)

No script or guard changes.

- **Sensitive paths**, one list in `docs/standards/verification.md`:
  `celestina/crates/celestina-lock`, `celestina/crates/celestina-lock-verify`,
  `celestina-rs/crates/celestina-lock-proto`, `celestina/src/lock`,
  `celestina/src/polkit`, `celestina/src/providers/polkit`,
  `celestina/contrib/pam.d`, the `*-production.sh` scripts of any project,
  root `scripts/*.py` and `scripts/*.sh`, `.githooks/`, `.github/`, and
  `docs/projects.toml`. Ratchet baselines (`scripts/*-baseline.tsv`) are not
  sensitive.
- **Light lane.** A unit qualifies when its kind is `maintenance` or `bug`,
  it changes at most 150 lines outside documents and lockfiles, and it
  touches no sensitive path. A light unit: has no design or task plan
  document (its ledger row describes it); runs in the session without
  subagents; writes its evidence from a new short template in
  `docs/templates/` (commands, results, limits; no narrative and no review
  history); takes one review round. Its ledger row, inventory, version rule,
  build, verification and landing are the ordinary ones.
- **Review cap.** At most two rounds for any unit (review, fixes, one
  re-review of the fixes only); three on sensitive paths; one in the light
  lane. What is left and is not a defect in the unit's own goal becomes a
  backlog entry, not another round.
- **One place per document kind.** Designs and task plans stay in
  `docs/superpowers/{specs,plans}`; ledgers stay in `docs/plans/active`. The
  author's global `CLAUDE.md` is edited so that a repository with its own
  plan scheme wins over the `docs/plans/U<nn>` convention, the light lane and
  the review cap are stated there, and the `/deslop` step, a skill that is
  not installed, is replaced by `/simplify`.
- **Outside the repository, by the author:** disabling unused plugins. Not a
  unit.

## 5. Constraints

- Repository text is English; the author is addressed in Spanish.
- Every unit: session worktree from `scripts/worktree.sh`, ledger row,
  landing with `scripts/land-unit.py` on the author's authorisation; the
  canonical checkout is never edited.
- Guards run from `HEAD`: a unit that changes a guard cannot rely on its own
  new rule in the same commit.
- A change to a shared script can mark every registered project for
  re-verification at the landing (`docs/contracts/landing.md:300-307`); the
  planner checks which of `worktree.sh`, `land-unit.py`, `build_trees.py` and
  `agent-context.py` are in that set and says what each landing will rebuild.
- Two smokes never run at once; nothing is run against the author's live
  session.
- The global `CLAUDE.md` is outside the repository and is edited directly,
  with the author's approval of the exact text.

## 6. Left to the builder

- Ledger unit ids and whether U01 is one or two ledger units.
- The exact cap mechanism (how member-crate artifacts are named and found).
- The final phase list and how shared state between phases is handled.
- The wording of each trigger and of the trimmed `AGENTS.md`.
- Whether tiers sit in the suite table or in each project's table of
  `docs/projects.toml`, by what re-verifies least.

## 7. Calls made by Claude, for the author to check

- The medium cut keeps a short evidence record and the full smoke at the
  landing (section 3).
- Dev profile through the worktree's Cargo config, not the manifests.
- No `mold`.
- 40 GiB cap; about 40 KB for the shell's `now` tier; 150-line light-lane
  limit; the sensitive-path list.
- Ledgers leave the `now` tier.
- Splitting the smoke into one file per phase.
- `/simplify` in place of `/deslop`.
- RSH-1 is archived to make room for the `dev-speed` plan.
