# Evidence: R3 session verbs, OSD and held session states

- **Date:** 2026-08-04
- **Scope:** R3-A through R3-E of [the R3 plan](../plans/active/2026-08-03-r3-session-verbs.md)
- **Environment:** Arch Linux checkout at `928ea71105570b423ee3496e02900b437ac4baaf` with the uncommitted R3 batch and another session's unrelated uncommitted Grafita/Fluorita work; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1
- **Artifact:** `celestina/build/production-artifact.toml`, source fingerprint `sha256:60c5f3a4121ecad7e5bde0742759a1849747bd3886f3418268f23821e0882c0f`, built from the declared version 0.2.0

## Procedure

The registered production entry was run first, and it stopped before reaching
this project's own checks:

```sh
celestina/scripts/build-production.sh
celestina/scripts/verify-production.sh
```

Every remaining step of `verify-production.sh` was then run directly against
the same built bundle:

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/test-production-artifacts.sh
(cd celestina && cargo fmt --all --check)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && cargo test --all-targets --locked)
(cd celestina-rs && cargo fmt --all --check)
(cd celestina-rs && cargo clippy --locked -p celestina-shell-core --all-targets -- -D warnings)
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
celestina/scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
celestina/scripts/smoke-production.sh
ldd celestina/build/celestina | grep 'not found'
python3 scripts/version_tool.py check
```

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 84 passed |
| shell helper tests (`celestina` package) | 16 passed |
| CTest | 13/13 passed, including `celestina-session-requests`, `celestina-osd-readings` and 10 `celestina-shell-service` cases on a real session bus |
| QuickTest (offscreen) | 22 cases passed, including 6 `SessionOsd` cases |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| QML lint | OK against the generated module |
| Offscreen smoke | release host plus compiled style alive 8 s |
| Dynamic libraries | no missing objects |
| Documentation contract | OK |
| Version contract | OK, 6 owners; celestina 0.1.0 → 0.2.0 |
| Architecture contract | **failed on `grafita/qml/components/EditorScrollBar.qml`** |

## Observed facts

- The session command channel answers a device verb twice: `pending` when the
  shell forwards it, then `confirmed` or `failed`. `accepted` is never
  published as an outcome, and the confirmation comes from a later provider
  reading or, for a compositor action, from Niri's own answer.
- `lock` and `lock-and-suspend` are refused with `NotSupported` naming the
  missing locker, proved by `refusesToLockWhileNoLockerProviderExists` on a
  real bus.
- A held state — night light, caffeine — reports as off as soon as its child
  process is gone, proved by `a_holder_that_exited_is_not_a_held_state`, and a
  tool that cannot start is a refusal rather than a state claimed as on.

## Limits

- The architecture contract failure is **outside this batch**: it is an
  untracked file belonging to another session's Grafita work in the same
  checkout, which the suite-wide guard reaches before it reaches Celestina.
  No path of this batch is flagged by it, and nothing here weakens or works
  around that guard. Until it is green, `verify-production.sh` and therefore
  `complete-production.sh` cannot run to completion for this project, so this
  bundle is **built but not verified** by the registered runner and has not
  been deployed.
- The language contract also fails only on another session's Fluorita baseline
  rows.
- Everything above is compilation, isolated behaviour and an offscreen start.
  None of it is evidence about Wayland geometry, the OSD's real appearance,
  gamma actually warming, an idle inhibitor actually holding logind, monitors
  blanking, DDC hardware or AT-SPI. Those remain `VAL-R3` in
  [VALIDATION.md](../../VALIDATION.md) and are the author's to run.

## Follow-up

- Re-run `celestina/scripts/verify-production.sh` and then
  `celestina/scripts/complete-production.sh` once the foreign architecture
  failure leaves the checkout, and record the verified manifest.
- `VAL-R3` remains deferred until that run is green and the author authorizes
  each live mutation.
