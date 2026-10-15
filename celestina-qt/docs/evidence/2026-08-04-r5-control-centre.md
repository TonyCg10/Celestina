# Evidence: R5 control centre, session menu, weather and calendar

- **Date:** 2026-08-04
- **Scope:** R5-A through R5-E of [the R5 plan](../plans/archive/2026-08-04-r5-control-centre.md)
- **Environment:** Arch Linux checkout at `7632ab810ac0df819f06f039d55bc45c5b87eaf9` with the uncommitted R5 batch; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1
- **Artifact:** `celestina/build/production-artifact.toml`, `verified`, built from the declared version 0.4.0 and deployed to `~/.local`

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py bump celestina milestone --unit R5-A \
    --summary "Add the control centre checkpoint"
python3 scripts/version_tool.py check
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
(cd celestina && cargo fmt --all --check)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && cargo test --locked)
celestina/scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
celestina/scripts/complete-production.sh
```

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 127 passed, including settings, weather and calendar |
| shell helper tests | 21 passed |
| `notification_server` integration test | passed against a private `dbus-daemon` |
| CTest | 13/13 |
| QuickTest (offscreen) | 43 cases, including `ControlCentre` and `SessionMenu` |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| QML lint, visual and contrast guards | OK |
| Architecture, documentation and language contracts | OK |
| Version contract | OK; celestina 0.3.0 → 0.4.0 |
| `complete-production.sh` | built once, verified those bytes, deployed to `~/.local`; the session was not activated |

## Observed facts

- A setting is in force only after its write is durable: `Store::apply` cannot
  be reached without handing back a `WriteOutcome`, and a `Failed` outcome
  leaves the previous value in force. Proved by
  `a_change_is_not_in_force_until_its_write_is`.
- A settings file this shell did not write — oversized, unreadable, or carrying
  a schema from the future — is not read, and the defaults are used without
  overwriting it.
- No control in the centre paints what it asked for: each switch rebinds itself
  to its provider's reading before sending, and a request's life is shown
  beside that reading. A result for a request the surface never made is
  ignored, proved by `test_an_answer_to_another_request_is_ignored`.
- The weather request carries a coordinate pair rounded to two decimals and
  nothing else — no identifier, no place name — proved by
  `the_request_carries_a_rounded_coordinate_and_nothing_else`.
- A weather reading stops being shown at the moment it stops being current, so
  a pending retry never leaves a stale temperature on screen.
- Month arithmetic is checked against known dates, including the 1900 and 2000
  leap exceptions, rather than against itself.
- `power-off`, `reboot`, `log-out` and `suspend` are typed one by one, and a
  near miss such as `shutdown` or `power-off-now` is refused rather than
  rounded to the nearest irreversible action. `suspend` is refused while no
  locker provider exists, for the same reason `lock` is.

## Limits

- Nothing here suspended, rebooted or powered off this machine, and no session
  was ended. The typed actions were exercised through their vocabulary, their
  refusals and their confirmation step, never by carrying one out.
- No real weather request was made during verification: the policy and the
  parser were tested against fixtures, and the provider asks nothing at all
  until a location is set.
- Everything above is compilation, isolated behaviour, an offscreen start and a
  private bus. Appearance, a real network or Bluetooth switch, a real location
  and assistive-technology behaviour remain `VAL-R5`.

## Follow-up

- `VAL-R5` is deferred until the author runs it against the deployed bundle.
- The roadmap is idle. R7 (session look) and R8 (Noctalia departure) are the
  remaining planned checkpoints; R6 and the dock stay conditional on their
  discussions.
