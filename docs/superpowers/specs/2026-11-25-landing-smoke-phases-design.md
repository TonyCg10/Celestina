# Landing a unit of a series with only its smoke phases — design

- **Date:** 2026-11-25
- **Author's request:** a unit that is one of a larger series of improvements
  (CEL-I-004 within the shell's standing tracks) must not run the whole smoke
  at its landing; the whole smoke runs at the end of the series. The author
  chose: a mid-series landing runs only the phases its unit declares, and the
  phases are declared on the landing command.

## Why

The CEL-I-004 pair took about 20 minutes: the cuprita landing and then the
shell landing each ran the shell's whole `verify-production.sh`, ending in the
whole smoke (about 4 minutes each). One phase takes 5–44 s.

## Goals

- `land-unit.py --smoke PHASE[,PHASE…]` lands a unit whose shell verification
  runs only those smoke phases. Without the option the landing runs the whole
  smoke, as today.
- The shell records which smoke its last verification ran. A landing without
  `--smoke` never accepts an artifact verified with a partial smoke: it runs
  the verification again (the verify script alone when the bytes are current).
- Unknown phase names stop the landing before any build.
- The landing's evidence section says which phases ran, or "full".
- The rule in the repository: a unit of a series lands with `--smoke`; the last
  unit of the series, and any unit outside a series, lands without it. A series
  that is abandoned closes with one landing or `complete-production.sh` without
  the option.

## Non-goals

- The repeated release build, clippy and tests of a stacked pair (about 8 of
  the 20 minutes). A separate unit afterwards.
- Choosing phases automatically from the changed files.
- Projects other than the shell: their verification has no phases. With
  `--smoke`, they verify as today.

## Anti-goals

- A partially verified artifact that looks fully verified to a later landing
  or to `check --require-verified` with the full requirement.
- The author-run `verify-production.sh` or `complete-production.sh` changing
  behaviour: both stay full unless the option reaches them from the landing.

## Approaches

- **On the command (chosen by the author):** an option of `land-unit.py`;
  smallest change; relies on the command being right, and the agent writes it.
- **In the ledger row:** durable and reviewed before landing, but changes the
  ledger format and the documentation guard. Not chosen.

## How (decided)

- `land-unit.py` takes `--smoke` (comma-separated phase ids), validates them
  against `celestina/scripts/smoke.sh --list`, stores them in its landing state
  (so `--continue` keeps them), and passes them to the verification through
  one environment variable, `CELESTINA_SMOKE_ONLY`, that
  `production_artifact.py run-verification` already forwards.
- `celestina/scripts/verify-production.sh` runs `smoke.sh --only <phases>`
  when the variable is set, the whole smoke otherwise.
- After the smoke passes, the verify script writes the scope it ran (`full` or
  the ids) to `celestina/target/release/smoke-scope`; it removes the file
  before its first step. A missing file reads as `full`. The landing reads it
  when the artifact check passes. (Implementation call: the design first put
  this in the manifest, but `scripts/production_artifact.py` is a
  verification input of every project, so changing it would re-verify the
  whole suite.)
- `docs/standards/verification.md` and `docs/contracts/landing.md` state the
  series rule.

## Left to the builder

- The variable's name, the error text, and the shape of the tests in
  `scripts/test-land-unit.py` and `scripts/test-production-artifacts.py`.
- Hosting: the suite has one active plan, TRK-1, whose scope excludes code.
  The plan says how this unit is hosted.
