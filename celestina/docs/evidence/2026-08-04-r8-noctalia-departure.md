# Evidence: R8 reversible Noctalia departure

- **Date:** 2026-08-04
- **Scope:** R8-A and R8-B of [the R8 plan](../plans/archive/2026-08-04-r8-noctalia-departure.md)
- **Environment:** Arch Linux checkout at `8e65ea1643dc4571b2c820b0f3c0ae0b4e5d7049` with the uncommitted R8 batch; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1
- **Artifact:** `celestina/build/production-artifact.toml`, `verified`, built from the declared version 0.6.0 and deployed to `~/.local`

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
celestina/scripts/handover-status.sh
celestina/scripts/handover-remove.sh --confirm
celestina/scripts/complete-production.sh
```

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 151 passed, six of them the handover model |
| shell helper tests | 25 passed |
| CTest | 13/13 |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| Architecture, documentation and language contracts | OK |
| Version contract | OK; celestina 0.5.0 → 0.6.0 |
| `handover-status.sh` | ran read-only, exit 2 — incomplete, which is a state rather than a failure |
| `handover-remove.sh --confirm` | **refused**, changed nothing |
| `complete-production.sh` | built once, verified those bytes, deployed to `~/.local`; the session was not activated |

## Observed facts

- The report lists eight responsibilities: six built and unrecorded, and two —
  screen lock and the polkit agent — that nothing in this shell provides. It
  names the validation each one needs rather than saying only that something is
  missing.
- `handover-remove.sh --confirm` was run and **refused**. Afterwards
  `$XDG_DATA_HOME/celestina` contained only `generated/`: no rollback file was
  written and no autostart entry was touched, because the refusal happens
  before either.
- The refusal survives even a hypothetical run where every validation has
  passed: the two unbuilt responsibilities still block it, proved by
  `removal_is_refused_while_anything_is_unbuilt_or_unseen`.
- The report now runs inside `verify-production.sh`, whose only claim about it
  is that it works and changes nothing; its exit 2 is read as a state, and only
  a higher code fails the build.
- No build, verification or completion script invokes the removal path.

## Limits

- **Nothing was removed, disabled or stopped.** Noctalia is running, its
  autostart is untouched, and no package manager was involved. The removal path
  was exercised only to prove it refuses.
- The report reads `VALIDATION.md` for statuses written as `passed`; a check
  the author performed but did not write down does not count, on purpose.
- Whether the eight responsibilities are the right eight is a judgement this
  evidence cannot settle. The list is the claim, and `VAL-R8` is where it meets
  a real day of use.

## Follow-up

- `VAL-R8` is deferred, and depends on `VAL-R3`, `VAL-R4`, `VAL-R5` and
  `VAL-R7` being run and recorded first.
- R6 stays conditional on SHELL-D2, and R8's Polkit and dock slices on SHELL-D3
  and SHELL-D4. The roadmap is idle.
