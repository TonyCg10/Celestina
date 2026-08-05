# Evidence: live validation remediation

- **Date:** 2026-08-05
- **Scope:** `LVR-1-A` of [the remediation plan](../plans/archive/2026-08-04-live-validation-remediation.md)
- **Environment:** Arch Linux checkout at `e170be5824cd8c975b6dc1e32a53117dd9d4647e` with the uncommitted LVR-1 batch; Rust/Cargo 1.97.1, GCC 16.1.1, CMake 4.4.2, Qt 6.11.1, dbus-daemon present
- **Artifact:** `celestina/build/production-artifact.toml`, `verified`, built from the declared version 0.6.1 and deployed to `~/.local`
- **Answers:** the failures recorded in [the live run](2026-08-04-live-validation-failures.md)

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py bump celestina bug --unit LVR-1-A \
    --summary "Fix the live validation failures"
python3 scripts/version_tool.py check
(cd celestina && cargo fmt --all --check)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && cargo test --locked)
(cd celestina-rs && cargo clippy --locked -p celestina-shell-core --all-targets -- -D warnings)
(cd celestina-rs && cargo test --locked -p celestina-shell-core)
celestina/scripts/qmllint-production.sh
ctest --test-dir celestina/build --output-on-failure
celestina/scripts/complete-production.sh
celestina/scripts/verify-production.sh
celestina/scripts/verify-production.sh
celestina/scripts/status-production.sh
```

The media hypothesis was measured rather than assumed, with `playerctl` timed
directly and the helper's own published frame captured.

## Result

| Check | Result |
|---|---|
| `celestina-shell-core` tests | 151 passed |
| shell helper tests | 25 unit plus 3 integration, all passed |
| CTest | 13/13; `celestina-provider-states` now 12 cases, QuickTest 75 |
| Rust format and Clippy (`-D warnings`) | clean in both workspaces |
| QML lint, visual and contrast guards | OK |
| Architecture, documentation and language contracts | OK |
| Version contract | OK; celestina 0.6.0 → 0.6.1 |
| `complete-production.sh` | built once, verified those bytes, deployed to `~/.local`; the session was not activated |
| Repeated canonical verification | two immediate additional runs passed on the same incremental tree after the [CelestinaStyle writer-ordering correction](../../../celestina-style/docs/evidence/2026-08-05-qmllint-ordering.md) |
| `status-production.sh` | current and verified; seven artifacts installed, including the desktop entry |

## What each failure was, and what it is now

**Notifications emptied the bar.** Three boundaries, each fixed where it
belongs. The helper published an `actions` array *inside* each notification
row; the host takes one level of structure, so the whole frame was refused.
Actions now travel as a flat sibling list, each row naming the notification it
belongs to, capped at 32 — the actions are kept, not dropped. The host then
answered an unreadable frame by clearing every provider's confirmed state, the
same thing it does when the helper dies; that decision is now a named
`FrameEffect` rule, an unreadable frame is dropped and changes nothing, and
only real helper loss still clears. `AudioLevel.qml` evaluated an
`Accessible` binding that reached into an absent reading, which is what threw
on every missed frame; the spoken text is now a named property guarded by
`hasReading`.

**Media never appeared. The recorded hypothesis was wrong.** `playerctl`
answers in 3-5 ms, not near the 750 ms deadline, and the captured helper frame
carried a valid player with a non-empty `nowPlaying`. The provider was
publishing correctly all along. `WorkspaceStrip` was allowed to claim the
entire flank — it grows with the focused window's title — and `PanelFlank`
clips, so `SysMon` and `MediaMini` were pushed out of the bar without a
word. The strip now takes only what its neighbours do not need. **No timeout
was changed**, because no measurement justified it.

**The emptied clipboard could not be closed.** The list owning Escape was
`visible: entries.length > 0`, so `Vaciar` removed the only thing listening.
Focus now falls back to the card and returns to the list when entries reappear.
Deleting was reachable only by a key or a right-click, neither visible; each row
carries a delete button that is Tab-reachable and named for assistive
technology, with the keyboard and context-menu paths untouched.

**Startup diagnostics.** `Accessible` was attached to the wallpaper's root
`Window`, which Qt rejects; it hangs on an `Item` now. The portal could not
find application information for `celestina`, so `celestina.desktop` exists,
is registered in `docs/projects.toml` as both a production input and a sealed
artifact, is deployed, and is checked by both `status-production.sh` and
`activate-production.sh`. Its `StartupWMClass` and file name match the host's
`setDesktopFileName("celestina")`.

**Product copy.** Every exposed surface is Spanish, including the panel title
that was still English. The one test asserting that copy carries the
`allow-non-english` marker, because it checks the exact text a screen reader
is handed.

## Regressions added

- `an_offered_action_is_published_flat_and_still_names_its_notification`: a
  real `Notify` with one action, through the helper, asserting the action
  survives, names its notification, and that nothing published nests a list.
- `acceptsTheShapeTheNotificationProviderActuallyPublishes`: that exact shape
  through the **C++ host decoder**, not the helper's JSON.
- `anUnreadableFrameLeavesEveryOtherProvidersReadingAlone`: audio, network and
  sysmon readings survive a malformed frame, while helper loss still clears.
- `PanelFlank`: a window title long enough to overflow the flank, proving a
  valid `MediaMini` keeps its width.
- `AudioLevel`: the absent reading, where no accessible binding may reach into
  `reading.volume`.
- `NotificationJoin`: the QML join, including that an action naming another
  notification is never borrowed.
- `ClipboardEntryRow`: real left and right pointer presses plus a real press on
  the visible delete button, proving the row cannot intercept the button's
  pixels whether the row is current or merely hovered.
- `ClipboardOverlay`: the keyboard survives the history emptying and returns
  when it refills.

## Limits

- Nothing here was validated on a live session. Celestina was not activated,
  Noctalia was not stopped, and no Niri configuration was read or written.
- The private-bus integration test covers the producer and the helper; the C++
  decoder is covered by its own test against the same literal shape. No single
  automated test spans one running helper *and* the Qt host.
- The clipboard delete button's pointer path is driven by Qt Quick Test. Its
  keyboard reachability is asserted by construction — `activeFocusOnTab` and
  an accessible name — because an offscreen test cannot drive Tab into a
  layer-shell surface.
- `VAL-R1-01`, `VAL-R2-02`, `VAL-R4`, `VAL-SHELL-03` and `VAL-COPY-01`
  remain **failed** until the author runs them again. This record says the
  corrections are ready, not that they were seen working.

## Follow-up

- The author reruns the five cases above against the deployed 0.6.1 bundle.
- The checks that were never reached in the first run — notification
  replacement, actions, do-not-disturb, history, DPMS, hotplug, tray
  activation — remain open in their own entries.
