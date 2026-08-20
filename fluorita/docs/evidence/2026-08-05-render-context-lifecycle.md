# Evidence: 2026-08-05 render-context lifecycle and engine truthfulness

- **Date:** 2026-08-05
- **Scope:** `F6-B`; plan
  [immersive-content](../plans/archive/2026-08-04-immersive-content.md); suite
  audit findings `FLU-C1`, `FLU-A1`–`FLU-A4`, `FLU-M2`–`FLU-M6`, `FLU-B3` from
  [`../../../docs/evidence/2026-08-05-static-suite-audit.md`](../../../docs/evidence/2026-08-05-static-suite-audit.md)
- **Environment:** source corrections with compilation and unit tests. No
  production build, no deployment, no window opened on the live session, and no
  version transition — the author asked for the corrections, not the delivery
- **Artifact:** none; no production build ran

## What was wrong

`MpvVideoItem::setHandle` inferred "no renderer exists" from
`!window() || !isVisible()` and, on that inference, answered `contextReleased`
itself. The inference is false: an item that is not drawing can still own a live
`mpv_render_context`. `PlayerSurface.qml` hides the video item whenever playback
is not confirmed — which includes the error state after a stream mpv cannot
open — so the release was reported while renderer and context were alive, and
`player.rs` went on to destroy the mpv core underneath a context libmpv requires
to be freed first. The same ordering was reachable two other ways: an activation
during an in-flight close, and application exit, where `PlayerRust` had no
`Drop` and the window accepted the close without waiting for the release.

## What changed

- `MpvVideoItem` counts renderer claims in a `QAtomicInt`, claimed when the
  renderer first synchronizes against a live handle and settled when it releases
  — including when context creation failed, because the player waits for exactly
  one answer per session and cannot tell the two cases apart. `setHandle` now
  answers only when that count is zero, which is the condition the visibility
  shortcut was standing in for.
- A context that fails to build reports `notifyContextFailed` instead of nothing,
  so the player publishes an error rather than sitting on "abriendo" for ever
  with no sound, no message and no timeout.
- `player.rs` holds an activation that arrives while a close is in flight and
  replays it from `surface_released`; `PlayerRust` implements `Drop` to stop and
  join its worker; `Main.qml` accepts the window close from the release rather
  than from `Qt.callLater`.
- `library/work.rs` polls in slices so cancellation is answered during the scan
  and between tag probes instead of after a 180 s or 15 s budget; a watcher
  refresh re-projects under the scope currently selected; a scan failure
  re-projects the catalogue already published instead of an empty one; and a
  touched file takes its source from `SourceSet::owner_of` rather than from
  whichever record happened to be first.
- `mpris.rs` emits `playback_status_changed`, `metadata_changed`,
  `volume_changed` and `Seeked`, so a spec consumer sees state change instead of
  polling for it.
- `folders.rs` receives with a timeout and checks its deadline without traffic,
  so an unanswering portal backend can no longer pin the thread that `Drop`
  joins.
- The accessible activation in `GalleryGrid.qml` and `MusicList.qml` passes the
  same four arguments as the pointer path, so an item opened from a screen
  reader keeps its filmstrip and kind.

## Procedure

```sh
cargo check                                          # in fluorita/
cargo test -p fluorita-core -p fluorita-engine        # in celestina-rs/
cargo fmt
```

## Result

| Command | Result |
|---|---|
| `cargo check` in `fluorita/` | passes; only Qt system-header warnings |
| `cargo test -p fluorita-core -p fluorita-engine` | 39 + 1 + 29 + 22 pass, 0 fail |
| `cargo fmt` | clean on the edited files |

## Limits

Compilation and unit tests cannot show that the mpv core outlives its render
context on real hardware, that a failed context now surfaces a visible error, or
that MPRIS reaches `playerctl`. Those need a real session and are recorded as
`VAL-FLU-TEARDOWN` in [`../../VALIDATION.md`](../../VALIDATION.md).

## Not in this unit

`FLU-M1`, the lossy Qt seam that makes non-UTF-8 names visible but not
operable, is untouched. Siderita carries the identical defect at the identical
boundary; fixing one alone would leave two divergent seams, so it belongs to a
shared decision and its own plan.
