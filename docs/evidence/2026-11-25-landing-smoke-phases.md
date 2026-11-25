# Evidence: land a series unit with only its smoke phases (DEV-2-A)

- **Date:** 2026-11-25
- **Scope:** ordinary unit on sensitive paths (the landing scripts under `scripts/`, `celestina/scripts/`, `docs/contracts/`, `docs/standards/`, an `AGENTS.md`); `DEV-2-A`: `--smoke` in `scripts/land-unit.py` and `scripts/landing.py`, the scope file in `celestina/scripts/verify-production.sh`, their tests in `scripts/test-land-unit.py`, the series rule, the design and plan, the DEV-2 plan opened and closed here, the root roadmap and status
- **Environment:** worktree `suite-DEV-2-A` on `985ce162`
- **Artifact:** not built here; the shell's verification inputs change (its verify script), so its next landing verifies it

## Procedure

```sh
sh -n celestina/scripts/verify-production.sh
sh celestina/scripts/smoke.sh --list
python3 scripts/test-land-unit.py -k smoke
sh scripts/test-land-unit.sh
sh scripts/test-production-artifacts.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-11-25-landing-smoke-phases/DEV-2-A.numstat.tsv
```

## Result

- **Exit:** every command exits 0. `test-land-unit.py -k smoke`: 10 tests, among them fixture landings that land with `--smoke osd` (scope `osd`, the evidence names the phases), verify again a current artifact whose scope is `osd` when the landing has no `--smoke` even with a stray `CELESTINA_SMOKE_ONLY=osd` in the environment (scope back to `full`), accept it for another unit with `--smoke bar`, stop at `build_if_stale` on an unknown phase before any entry runs, and accept a phase the unit itself adds. The whole landing suite: 126 tests
- **Review:** 1 round (Sonnet, `/code-review` by hand); fixed: phase ids were checked against main's driver in `preflight` (a unit adding a phase could not name it, one removing a phase failed after the build), now on the tip at `build_if_stale`; a rewrapped stop lost its path; the contract's wording on an empty id; plan text drifted from the code; deferred: a resume test, and unsetting a stray `CELESTINA_SMOKE_ONLY` in `complete-production.sh`

## Limits

- The shell's real `verify-production.sh` did not run here (a session never runs the production entries); the landing tests run doubles that write the scope file as it does. Its first real run is the shell's next landing.
- An artifact verified with `--smoke` is recorded `verified` in its manifest and deployed by that landing, and `complete-production.sh` run by hand accepts it; only `land-unit.py` reads the scope. A `CELESTINA_SMOKE_ONLY` left in the author's shell narrows a hand-run `verify-production.sh` too; the scope file records it, so the next landing verifies again.
- No test resumes a landing with `--continue` after a stop in `build_if_stale`; the state round-trip test covers the field.
- Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`: a born-archived plan has no open row for `land-unit.py`. The commit must reach `main` by fast-forward with parent `985ce162`.

## Follow-up

- The repeated release build, clippy and tests of a stacked pair (about 8 of the 20 minutes of CEL-I-004).
