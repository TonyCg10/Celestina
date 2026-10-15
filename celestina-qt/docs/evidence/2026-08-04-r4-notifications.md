# Evidence: R4 notifications

- **Date:** 2026-08-04
- **Scope:** R4-A through R4-D of [the R4 plan](../plans/archive/2026-08-04-r4-notifications.md)
- **Environment:** Arch Linux checkout at `76fcb87ca74075c492f7c20ffd021c47671d0052` with the uncommitted R4 batch; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1, dbus-daemon present
- **Artifact:** `celestina/build/production-artifact.toml`, `verified`, built from the declared version 0.3.0 and deployed to `~/.local`

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py bump celestina milestone --unit R4-A \
    --summary "Serve the session notification checkpoint"
python3 scripts/version_tool.py check
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
(cd celestina && cargo fmt --all --check)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && cargo test --locked)
celestina/scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
celestina/scripts/complete-production.sh
```

The two-process check was also exercised by hand against a private bus before
it became a test, and against the author's live session to confirm the refusal
path.

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 102 passed, 19 of them the notification state machine |
| shell helper tests | 18 passed |
| `notification_server` integration test | passed in 3.4 s against a private `dbus-daemon` |
| CTest | 13/13, including both surface-placement cases |
| QuickTest (offscreen) | 30 cases, 7 of them notifications |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| QML lint, visual and contrast guards | OK |
| Architecture, documentation and language contracts | OK |
| Version contract | OK; celestina 0.2.0 → 0.3.0 |
| `complete-production.sh` | built once, verified those bytes, deployed to `~/.local`; the session was not activated |

## Observed facts

- On the author's live session, where Noctalia (PID 1338) owns
  `org.freedesktop.Notifications`, the helper logs that another server owns the
  name and withdraws its provider. Nothing was taken from the running server.
- On a private bus the helper claims the name, and `Notify` with Magnetita's
  exact arguments returns id 1; a second `Notify` carrying `replaces_id = 1`
  returns 1 again, and the panel is handed the replacement's body rather than
  what it replaced.
- `GetServerInformation` answers `('Celestina', 'celestina', '0.3.0', '1.2')`
  and `GetCapabilities` omits `body-markup` and `persistence`, which this
  server does not do.
- `CloseNotification` moves the notification out of the toast list and into
  history rather than dropping it.
- A second helper started on the same private bus publishes its other providers
  and never publishes a notifications provider, watched across three seconds of
  frames rather than judged on one.

## Limits

- The integration test skips itself, with a printed reason, on a machine
  without `dbus-daemon`. It did not skip here.
- Everything above is compilation, isolated behaviour, an offscreen start and a
  private bus. None of it is evidence about how a toast looks on a compositor,
  the handover from Noctalia's server, phone notifications arriving over the
  air, or assistive-technology behaviour. Those remain `VAL-R4`.
- The unread indicator and the notification centre were exercised offscreen
  through their properties, never through a real pointer or a real screen
  reader.

## Follow-up

- `VAL-R4` is deferred until the author runs it against the deployed bundle.
- R5 opens its own plan; the roadmap is idle until then.
