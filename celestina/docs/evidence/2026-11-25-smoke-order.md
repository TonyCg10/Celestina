# Evidence: the smoke runs next the phase whose window is open (SMOKE-ORDER)

- **Date:** 2026-11-25
- **Scope:** `SMOKE-ORDER`: `celestina/scripts/smoke.sh` (the order of the phases, `--order`, `--next-phase`), `celestina/tests/nest_run.rs`, `celestina/AGENTS.md`, `celestina/README.md`
- **Environment:** session worktree on `f71ed170`; the release binary built into a private target outside the repository; nested niri `v26.04-20-ge4062688`
- **Artifact:** not applicable (the landing builds and verifies the shell)

## Procedure

The model came first. Two full smokes of `f71ed170` ran with `date` and
`mkdir` wrapped to log each call, which gave every phase's start and every
minute wait inside it. Per phase, the work between its waits:

| Phase | Work (s), split at its waits | Windows (H) |
|---|---|---|
| notify | 4.4 | 48 |
| launcher | 6.4 | — |
| bar | 4.4 | 48 |
| wallpaper | 10.4 | 48 |
| energy | 6.4 | — |
| glance | 14.3, 14.3 | 40, 40 |
| clipboard | 30.2 | 22 |
| connectivity | 0.1, 11.0 | 48 |
| osd | 25.7 | — |
| phone | 0.1, 4.4, 12.4, 3.4 | 48, 48, 48 |
| media | 7.4, 9.3 | 48, 48 |
| overview | 7.9 | 34 |
| polkit | 5.4 | — |
| cutover | 19.1 | 26 |

The work adds up to 197 s. A simulation over the 60 start seconds, each
segment varied by up to 2 s, compared the orders:

| Order | Mean | 90th percentile | Worst |
|---|---|---|---|
| The fixed order of `f71ed170` | 247.9 s | 267.5 s | 285.2 s |
| The best fixed order found (local search) | 230.8 s | 252.6 s | 289.3 s |
| Next the phase that waits least, ties by duration | 220.6 s | 235.0 s | 274.0 s |
| Next the phase that waits least, ties by narrowest window (this unit) | 223.9 s | 237.2 s | 268.2 s |

The last rule needs no table of durations: each phase's window is read from
its own first wait.

```sh
cd celestina
cargo test --locked --test nest_run --test lock_scripts --test duty_files
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cd ..
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
```

## Result

- **Exit:** every command exits 0; `nest_run` 28 tests (3 new), `lock_scripts` 11, `duty_files` 8
- **Real runs:** three full smokes against the private release build, all `smoke: OK`: start second 5, 229.4 s; 54, 214.3 s; 25, 221.3 s. In each the driver chose the order the model chose for that start second. With the same start seconds the model gives the old fixed order 219.0 s and 230.0 s for the first two. `--order launcher,bar` ran exactly those (11 s, `smoke: OK (order: launcher,bar)`); `--only polkit,bar` ran bar then polkit (10 s, `smoke: OK (only: bar,polkit)`)
- **Review:** 1 round of the cap of 3 (Sonnet; a Fable reviewer stopped at its usage limit): 3 findings, 2 fixed (the second of `--next-phase` is one or two digits, so `008` is refused before the shell reads it as octal; the tests cover seconds 0, 9, 49 and 59), 1 does not apply: that a phase's work before its first wait is not counted. That work measured 0.0 to 0.1 s for every phase; the `sleep`s it named in `clipboard.sh` run inside the wrapper the phase generates, after the wait

## Limits

- The gain is an expectation over start seconds: a single run can be slower than a run of the fixed order that happened to start at a good second.
- The order differs between runs; a failure is repeated with the `--order` its run printed.
- The three full runs used the first commit of this unit; the fix after the review changes only how `--next-phase` reads its second, which a full run does not use.

## Follow-up

A rule that counts every wait of a phase, not only its first (`glance`, `phone` and `media` wait more than once), gives a mean of 215.4 s in the model, against 223.9 s here; it needs the work between each phase's waits as a table, which would age with every change to a phase. Not done.

## Landing

- **Base revision:** `f71ed170d22002e6ce79e1c41e9b2fc19008e48e`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:e39e41fd026c2ab518a284d55ce375e6dc6bbf5a124309f6201eb4be278ae64e, verification_fingerprint sha256:f0c17c671689126894dc28d424e02a4048eb41e95600c966b9b1ad1c0d5d44dd
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
