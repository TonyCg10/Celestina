# REC-1 — Screen recording joins the toolbox, and three live defects close

- **Opened:** 2026-08-22
- **Closed:** 2026-08-22
- **Plan ID:** screen-recorder-and-hardening
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** REC-1
- **Author-validation checkpoint:** none opened; the author drove the deployed
  bundle directly and confirmed it live in the same session
- **Predecessor:** none
- **Successor:** none

## Hypothesis

The author asked for a screen-recording tool in the toolbox to capture bugs
with, and while building and exercising it against the live three-monitor
session, four more defects surfaced that the same live testing could name
precisely enough to fix in the same pass: a dragged audio/brightness slider
that periodically snapped back to a stale reading instead of tracking the
pointer; a night-light warmth change that did not reapply until the light was
next switched off and on; a wheel notch on either that landed on an arbitrary
number instead of the round one a person turning a wheel means; and a sixth,
unnamed workspace on two of three monitors wrongly drawn as a foreign
monitor's group capsule. A concurrent audit of the session's own diagnostics,
triggered by two unexplained live freezes, found the residual DDC-overlap
window the 2026-08-05 audit had already named but left open.

## Tangible outcome

The toolbox offers a recording action that asks which screen through the
session's existing screen chooser and saves to `~/Vídeos/Recordings`, with a
second panel control that appears only while recording and stops it. Level
rows (volume, microphone, per-application levels, monitor brightness) track a
drag continuously and never show a reading older than what was last asked for;
a wheel notch always lands on a multiple of the row's own step. Night-light
warmth reapplies to every lit output the instant it changes, without a full
transition sweep, and gets the same round-number wheel behavior. A workspace
niri creates without a name — its trailing per-monitor spare — never borrows a
named workspace's home from the shared memory that groups monitor strips.
Every `ddcutil` conversation this suite starts, from any process, is
serialized through one session-wide lease; a host that could not confirm it is
the session's only shell withholds its own automatic DDC probe rather than
risk a second one colliding with a probe from a shell it cannot see.

## Scope

- **REC-1-A — screen recording.** `provider_adapter/recorder.rs` wraps
  `gpu-screen-recorder`, one recording at a time, output named by the
  session's own chooser (`OutputChooser.qml`, reused rather than duplicated —
  its words and its window title are now parameters, the title held fixed
  because a niri window rule matches it to float the dialog). Destination is
  `xdg-user-dir VIDEOS`/`Recordings`, created if absent. `CaptureMenu.qml`
  raises the question instead of answering it; a second panel button, visible
  only while recording, stops it from any output's bar. The toolbox row now
  needs a `providerSource` the way every other indicator row already does,
  which is a fixture change in `tests/surfacemanager_test.cpp` and (recorded
  under SURF-1-D's own inventory, since that unit already claims the file)
  `tests/indicatormenu_test.cpp`.
- **REC-1-B — level rows track the pointer.** `LevelRow.qml`'s pacing used to
  conflate two different things: how fast a request may leave (one in flight,
  newest position wins) and what the row shows (which sprang back to any
  reading that was not an exact echo of the last ask, including the
  provider's own unrelated poll arriving mid-drag). They are now separate: the
  shown level holds what was asked until an exact answer or a 1.5 s settle
  confirms otherwise. `AudioMenu.qml` and `BrightnessMenu.qml` additionally
  fed their `Repeater`s a fresh JavaScript array on every provider frame,
  which rebuilds every delegate — destroying the very row a drag was touching
  mid-gesture. Both now repeat by count and read their own entry, so a frame
  changes identity only when an application or a monitor actually comes or
  goes. `LevelChange::applied_to` (`celestina-shell-core::session`) also
  gained a `Set` fast path (`absolute()`) so a dragged slider's absolute
  target no longer costs a `wpctl` read-before-write per pixel.
- **REC-1-C — a step lands on a multiple, not an offset.** Both the session
  core's `LevelChange::Step` and `LevelRow`'s own wheel handler used to add
  the step to wherever the level happened to be. They now round to the next
  or previous multiple of the step size in the wheel's direction, so a level
  left on 22 by something else reaches 25 or 20, and that stray value is
  spent once rather than carried through every future notch. The control
  centre's night-light warmth control gained the same rule at its own
  hundred-kelvin step, plus a wheel it did not have before.
- **REC-1-D — night-light warmth reapplies without a sweep, and a stale
  `asked` no longer refuses a repeat.** The worker only ever reapplied warmth
  on the next full on/off transition; a coalescing flag now asks it to settle
  every lit output to the new warmth in one commit, no nineteen-frame sweep,
  the instant the setting changes. The control centre's own `asked` guard
  could get stuck holding a value the reading had already confirmed once,
  refusing to ask for that exact value again later; it now clears itself the
  moment the reading matches it.
- **REC-1-E — an unnamed workspace has no identity to look up.** Niri's
  trailing per-monitor spare carries no name; the adapter displayed its
  index as a label ("6") for the panel to speak, and then used that same
  display label to query the cross-monitor homes memory — which answered
  with whatever monitor a *named* workspace "6" belonged to. The adapter now
  keeps the compositor's real name (or its absence) apart from the display
  label: only a real name may be looked up in, or teach, the homes memory.
- **REC-1-F — one session-wide DDC lease, and NoBus withholds the automatic
  probe.** `provider_adapter/brightness.rs` gained an advisory `flock` on
  `$XDG_RUNTIME_DIR/celestina-ddc.lock`, held for the duration of every
  `ddcutil` child this suite starts, from any process — serializing a
  development nest against the real session as well as a fast restart loop
  against itself. `main.cpp`'s `ShellService::Attachment::NoBus` path — a
  host that could not even ask the session bus whether it is alone — now
  starts its provider helper with `CELESTINA_DDC=0`, withholding the
  automatic detect a host in that state cannot prove is safe. The restart
  backoff (`ShellProvidersClient`) also stopped resetting to its initial
  250 ms on the first valid frame, which a dying helper can still emit; it
  now requires 30 s of continuous uptime first. This closes the residual
  overlap window the 2026-08-05 static audit named and left open, found
  live: the session's own diagnostics recorded seven provider-helper starts
  in two seconds during an unexplained freeze, several concurrent, each
  running its own `ddcutil detect`.

## Exclusions

- No change to the recording tool's encoder, container or quality options;
  `gpu-screen-recorder`'s own defaults are used throughout.
- No change to DDC's user-facing `CELESTINA_DDC` override, which still turns
  the feature off entirely; `AutomaticDdc::Withheld` is a distinct, stronger
  host-decided state that a working session never enters.
- No change to the homes memory's declaration/learning contract in
  `celestina-shell-core::workspace_groups`; REC-1-E keeps an unnamed
  workspace out of it rather than changing what it means to be named.
- The `SessionHold`-class residual noted in `brightness.rs` — a SIGKILLed
  helper releases the lease while its orphaned `ddcutil` child may still be
  finishing — is documented, not closed; closing it needs the child's pid,
  which the lease file does not currently carry.

## Build order

Delivered as one continuous session against the live three-monitor host: the
recording tool first, then the slider and night-light defects the author's own
use of the new toolbox surfaced immediately, then the workspace-group defect
the author reported directly, then the DDC hardening once two live freezes and
their diagnostics pointed at the residual overlap window. Each area has its
own headless regression; all six areas share one production build, verify and
deploy cycle and one live activation.

## Implementation exit

- `ctest --test-dir celestina/build` passes all registered suites, including
  new cases for the recorder's output-picker wiring, the level-row pacing and
  round-step behavior (`tst_levelrow.qml`), the audio menu's frame-survives-a-
  drag behavior (`tst_audiomenu.qml`), the night-light wheel and stale-`asked`
  guard (`tst_controlcentre.qml`), and the unnamed-spare identity fix
  (`niri_adapter.rs`).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and the full
  Rust test suite pass across `celestina-shell-core` and the provider adapter,
  including a DDC-lease test that proves a second, concurrent operation is
  refused rather than run.
- `qmllint-production.sh` and the common architecture guard pass.
- The canonical production build, verify and deploy pass against the exact
  bundle, activated on the live session in place of Noctalia and driven
  directly by the author across every area above.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| REC-1 | `celestina:` | done | [exact inventory](../../inventories/2026-08-22-screen-recorder-and-hardening/REC-1.numstat.tsv) | Screen recording, slider/step fidelity, night-light live reapply, workspace-group identity and session-wide DDC safety | 36 files, +2673/-176 | [delivery evidence](../../evidence/2026-08-22-screen-recorder-and-hardening.md) | author's live exercise, 2026-08-22 |
