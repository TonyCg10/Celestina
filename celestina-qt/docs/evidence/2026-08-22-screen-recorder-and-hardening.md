# REC-1 delivery — recorder, slider fidelity, night-light reapply, workspace identity, DDC safety

- **Date:** 2026-08-22
- **Scope:** Celestina unit `REC-1`
- **Artifact:** see the exact inventory at
  `docs/inventories/2026-08-22-screen-recorder-and-hardening/REC-1.numstat.tsv`
- **Environment:** `ctest --test-dir celestina/build`, `cargo test`/`clippy`/
  `fmt` on the author's own machine; the canonical production build and verify
  ran clean and the bundle was activated on the live session (three monitors:
  `DP-1`, `DP-2`, `HDMI-A-1`), which the author drove directly throughout
- **Plan:** [screen recorder and hardening](../plans/archive/2026-08-22-screen-recorder-and-hardening.md)
- **Validation:** the author's own live exercise across the whole session,
  confirmed directly as working

## Procedure

### REC-1-A — screen recording

`gpu-screen-recorder` was probed directly first (`--list-monitors`,
`--list-capture-options`) against the live three-monitor session, then a
timed 4 s capture of `DP-2` was taken and inspected with `ffprobe` (h264,
1920x1080, 30 fps, clean close on SIGINT) before any shell code was written.
The toolbox's "Grabar una pantalla…" row raises the session's own screen
chooser rather than assuming an output, because the first version wrongly
recorded only the output the toolbox menu happened to be opened on — the
author caught this directly and it was corrected before landing. The chooser
component gained parametrized words (`headline`/`prompt`/`confirmText`) so
recording and screen-sharing can share it, with the window title held fixed
(`tests/tst_outputchooser.qml::test_the_words_change_but_the_window_title_does_not`)
because niri's window rule floats the dialog by matching that exact title.
`tests/tst_capturemenu.qml` proves starting a recording asks the panel rather
than the provider directly, and that stopping goes straight to the provider
without asking anything. The author confirmed a real recording landed at
a file named after the recorded output, inside a `Recordings` folder in
the session's own videos directory.

### REC-1-B/C — level rows and round steps

The author recorded and reviewed a video of a volume drag: the fill never
tracked the pointer and periodically snapped backward. Frame extraction
(`ffmpeg` to a contact sheet) showed the numeric reading itself regressing
(52% → 36%) mid-drag, which narrowed the defect to state, not animation
timing. Two causes were found and fixed, each with its own regression:
`AudioMenu`/`BrightnessMenu` rebuilding the touched row out from under the
drag on every provider frame (`tests/tst_audiomenu.qml::test_a_frame_does_not_rebuild_the_row_being_touched`,
`::test_a_frame_does_not_lose_what_the_row_asked_for`), and `LevelRow`
believing any reading — including the provider's own unrelated poll — was an
answer to what it had just asked (`tests/tst_levelrow.qml`, six cases
covering the pacing, an inexact reading, a drag's newest-target-wins
collapsing, and a wheel notch chaining from what is shown). The round-step
behavior followed once the author asked for it directly: a notch from 22
reaches 25 or 20, proved both in the shell core
(`session::tests::a_step_lands_on_a_round_number`) and at the QML row
(`tst_levelrow.qml::test_a_notch_lands_on_a_round_number`) and the control
centre's night-light wheel
(`tst_controlcentre.qml::test_the_wheel_asks_for_whole_hundreds_of_kelvin`,
which also caught and fixed a stale-`asked` guard that would have refused a
legitimate repeat ask at the range's edge).

### REC-1-D — night-light warmth

The author reported the temperature slider changing correctly but freezing
the desktop briefly on each change. The cause was the existing full 19-frame
neutral-to-warm transition sweep running on every warmth change, not only on
switch-on; the fix settles every lit output to the new warmth in one gamma
commit instead, proved by
`nightlight::tests::many_warmth_changes_collapse_into_one_reapplication`.

### REC-1-E — workspace identity

The author reported the sixth workspace showing as a foreign monitor's group
capsule on `HDMI-A-1` and `DP-2` but not on `DP-1`, where a workspace *named*
"6" actually lives. Niri's unnamed trailing spare was carrying its display
index as its identity when the adapter queried the cross-monitor homes
memory, so all three monitors' spares answered to wherever the named
workspace "6" belonged. `niri_adapter::tests::an_unnamed_spare_does_not_borrow_a_named_workspaces_home`
reproduces the exact three-monitor, one-declared-home shape and confirms the
spare now reports its own output.

### REC-1-F — DDC session safety

Two live freezes during this session (23:55 and 00:52) were followed by
`amdgpu` recovery on wake; the session's own diagnostics for the second
showed seven provider-helper starts in two seconds, several concurrent, each
running `ddcutil detect` — the overlap shape the 2026-08-05 static audit had
already named and left open. A session-wide `flock` around every `ddcutil`
child (`brightness::tests::two_ddc_operations_never_overlap_and_an_overlap_would_be_recorded`,
extended to prove a concurrent second operation is refused, not merely
counted) now serializes every conversation this suite starts, and a host
that cannot confirm the session bus is exclusively its own withholds its
provider helper's automatic probe rather than gamble on it.

## Result

`ctest --test-dir celestina/build`: 25/25. `cargo fmt --check`, `cargo clippy
--all-targets --locked -- -D warnings`, and the full Rust test suite (including
the new cases above) pass across every crate touched. `qmllint-production.sh`
passes. The canonical production build and verify pass against the exact
deployed bundle; the author activated it in place of Noctalia and exercised
every area above directly on the live three-monitor session, reporting the
whole batch working.

## Limits

Every case above is headless or offscreen except the author's own live
exercise, which is real but unrepeatable evidence: it happened once, on this
session's own three monitors, and is not filed as a `VAL-*` checkpoint. The
DDC lease is proven to refuse a concurrent operation under a synthetic
`AtomicBool` standing in for another process's shutdown signal, not under a
second real `celestina-provider-adapter` process; that is the harder claim and
remains to be exercised live, deliberately, the next time two shells start at
once.
