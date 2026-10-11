# Evidence: the recording bar and the focused monitor without the dialog (SEL-4-A)

- **Date:** 2026-10-10
- **Scope:** SEL-4-A of
  [SEL-4's plan](../plans/archive/2026-10-10-sel-4-recording-controls.md):
  `celestina-rs/crates/selenita-core` (the new `pick.rs` and its tests)
  and `selenita/` (`RecordingBar.qml`, `components/RecordingClock.qml`,
  `RecordingCard.qml`, `Main.qml`, `components/ActivationRoute.qml`,
  `build.rs`, the recording worker, the controller, the activation, the
  flags, `main.rs`, the new `picker.rs`, the QML tests and stub, the smoke,
  the documents)
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0
  (`QT_QPA_PLATFORM=offscreen`)
- **Artifact:** the `selenita` release binary in the shared Cargo target
  (`CELESTINA.worktrees/.cargo-target/release/selenita`), not installed

No recording, capture or window was made on the author's session, niri
was not asked anything and neither the portal nor its configuration was
touched: every check ran offscreen over `SELENITA_FAKE=1`, the QML
stand-ins and scratch runtime folders.

## Change

- **The request** (`selenita_core::pick`): `request_file(runtime)` is
  `runtime/selenita/pick-output`. A `PickRequest` holds an output name
  (1–128 bytes, no whitespace or control character, so the chooser's
  line stays one line) and when it was written; its text is
  `output=<name>\nwritten=<ms since the epoch>\n`, unknown keys ignored, a
  repeated key refused. It is fresh for `FRESHNESS` (30 s) and never when
  it comes from the future. `write_request` makes the folder private
  (`xdg::ensure_private_dir`) and writes the file through
  `atomic_file::replace_private` (`0600`, atomic). `take_request` claims
  the file by renaming it to `pick-output.taken-<pid>` (two choosers
  never both take it), reads it bounded (512 bytes), removes it and says
  `Absent`, `Fresh`, `Stale` or `Invalid`. `clear_request` removes one
  nobody consumed. Typed `PickError` with sources.
- **The focused start** (`record.rs`): `record::Request` gains
  `focused`; `Places` gains `pick_request` (`None` without a runtime
  folder: the chooser reads only that one). Before the portal is asked,
  any start removes a request left over; a focused one then asks the
  recorder's niri (`Recorder::focused_output`: the real one over
  `$NIRI_SOCKET` with `selenita_core::niri::focused_output`, the fake
  `FAKE-1`) and writes the request, logging and going on without it when
  either fails. After the portal answered, a request still there is
  removed, so `R` never meets one. `Recorder::prepare` takes the
  request's path: the real portal ignores it (xdg-desktop-portal-wlr runs
  the chooser), the fake consumes it as `selenita --pick-output` would
  and logs which output its chooser took.
- **The discard** (`record.rs`): `Job::Discard` and `Session::discard`
  step `Recording → Stopping`, call `abandon` (a kill: the file is going,
  so nothing waits for the muxer's index; `Running::kill` reaps the
  child before the removal), remove the hidden file and step to `Idle`
  with `Report::Discarded`; no publish, no entry. A file that cannot be
  removed fails the session to `Idle` with `RecordError::Discard`, whose notice says the discarded recording
  could not be deleted. A discard with
  nothing recording says nothing, as a stop does.
- **The Qt seam**: the controller's `toggleFocusedRecording` and
  `discardRecording` invokables and the constant `smokeDiscard`; a
  `Discarded` report ends a key-binding launch (`finish_launch_if_over`).
  `RecordingAction::ToggleFocused` is `focused` on the Qt signal and
  `RecordFocused()` on `org.celestina.Selenita1` (a toggle, as
  `ToggleRecording()`); `--record-focused` sends it or becomes the
  instance, its main window hidden like `--record`'s.
- **The chooser** (`picker.rs`, `main.rs`, `flags.rs`): `main` parses the
  flags first and, for `--pick-output`, calls `picker::run()` before the
  platform theme, the bus claim, the settings and Qt. A fresh request
  prints `Monitor: <name>` and exits 0; otherwise (no runtime folder, no
  request, a stale or broken one, each logged) it `exec`s `/bin/sh -c`
  with `SELENITA_FALLBACK_CHOOSER` or `exec "$HOME/.local/bin/celestina"
  --pick-output`, keeping the standard streams, and exits 1 only when the
  shell cannot be started.
- **The bar** (`RecordingBar.qml`): a third top-level window, `qsTr("Grabando")`,
  `Qt.Window | Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus`,
  `transientParent: null`, transparent over a `radiusPill` pill in the
  glass tint; visible while `recordingState` is `recording` and the
  application is not closing (`hideNow()`, called by `Main.qml` when the
  main window closes and when a launch ends). The dot pulses like the
  card's (stopped under reduced motion, ending at full opacity); the time
  is a `RecordingClock` whose width is that of `00:00:00`, so the pill
  does not change width with the seconds; «Detener» (Primary) and
  «Descartar» (Destructive) are one `Row` with `Qt.NoFocus`, the pill's
  size following it. `onClosing` refuses a close while it shows.
- **The shared clock** (`components/RecordingClock.qml`): the card's
  elapsed-time logic moved out (origin from `recordingStartedAt`, reset
  on a new origin, `m:ss` with hours when there are some); `RecordingCard`
  keeps `elapsed` and `elapsedText()` over it, so its tests stand as they
  were. One change: it looks every 250 ms instead of every second. The
  bar's ticking test failed three runs in four with `0:02` where `0:01`
  was due: a coarse timer may fire a little early, so a tick just before
  the first second turned read `0:00` and the next, just after the
  second, `0:02`; the card skipped seconds the same way. The value only
  changes when the second does.

Canonical owners: the request's format and freshness live once in
`selenita-core` and both the worker (writing, clearing) and the chooser
(consuming) use them; the fake portal consumes with the same function.
The focused output comes from `niri::focused_output`, the screen key
binding's reader. The bar and the card share one clock. Searched for
another chooser or pick request in the suite (`rg -n
"pick-output|chooser_cmd|pick_output"` outside the halted shell and
Selenita): only dated evidence about the halted shell's own
`--pick-output`, the installed chooser this one falls back to; it is run,
never read or reused.

## Procedure

```sh
cd celestina-rs && cargo fmt --all --check && cargo test -p selenita-core \
  && cargo clippy -p selenita-core --all-targets -- -D warnings
cd selenita && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings \
  && cargo test && cargo build --release --locked
cd fluorita && cargo check --locked     # selenita-core's other consumer still builds
sh selenita/scripts/qml-tests.sh        # three times
sh selenita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
(git diff --name-only HEAD; git ls-files --others --exclude-standard) \
  | python3 scripts/commit_scope.py --check "selenita-maintenance: Add the recording bar and recording the focused monitor without the chooser"
```

## Result

- **Exit:** 0 for every check listed.
- **Crate:** 71 tests pass (61 before), 10 new in `tests/pick.rs`: the
  request's path; the text round trip, unknown keys and any order; a
  missing, repeated or unreadable key, an empty name or one with a space
  refused; names on one line (newline, tab, control, over 128 bytes
  refused); fresh at 0, 29 and 30 s, stale past 30 s and from the
  future; a written request private (`0700` folder, `0600` file),
  replaced by a second write, consumed once with no file left; a stale
  one removed; junk, bytes that are not UTF-8 and an oversized file
  removed as `Invalid`; `clear_request`; a bad name writes nothing.
  Clippy clean.
- **Application:** 58 tests pass (50 before): a discard kills (no
  `stop`), removes the hidden file, publishes nothing, walks
  `Preparing, Recording, Stopping, Idle` with `Discarded` and no
  `Finished`, refuses a second discard, and the next recording publishes;
  a hidden file that is a folder fails the discard with its notice and
  `Failed, Idle`; a focused start asks niri before the portal and the
  fake chooser takes `FAKE-1`, the request gone; without niri (or a
  runtime folder) it starts with no request; a plain start removes a
  fresh leftover before the portal (the chooser never sees it) and a
  request the portal never read is removed after; `--record-focused`
  and `--pick-output` parse; `RecordFocused` hands on `focused`; the
  fallback runs through `/bin/sh -c` with its output, status and the
  same standard input. Clippy clean. Fluorita checks against the crate.
- **QML:** 80 passes three times in a row (68 before); 10 new in
  `tst_recording_bar.qml` (the window niri matches: title, frameless, no
  focus, no transient parent; shown only in `recording`; «Detener» stops
  and keeps the row; «Descartar» discards with no row; one actions row
  the pill and window follow; the clock from the origin, reset, ticking,
  with hours; the width holding as the time grows; a compositor's close
  refused and `hideNow`; reduced motion stilling the dot at full
  opacity; Spanish names) and 2 in `tst_main.qml` (a `focused` request
  toggles `toggleFocusedRecording` with only the bar shown; closing the
  window takes the bar at once). `tst_keyboard.qml` gives the window the
  keyboard back after a recording starts (offscreen activates the bar).
- **Smoke:** OK. Both report runs log `selenita-smoke-bar:
  recording=recording bar=shown title=Grabando` at 2 s and `bar=hidden`
  in the report; the close run still ends with its recording finished;
  the `--record-focused` run logs `the chooser is asked for FAKE-1` and
  `the chooser took FAKE-1 without its dialog`, shows only the bar
  (`shown=false preview=hidden`), discards and ends by itself (rc 0)
  with no MP4 (hidden or not), no history line and no request left;
  `--pick-output` on a Qt platform that does not exist prints
  `Monitor: FAKE-1` for a fresh request and consumes it, and `Monitor:
  FAKE-2` from `SELENITA_FALLBACK_CHOOSER='echo Monitor: FAKE-2'` with no
  request and with a stale one (removed).
- **qmllint:** selenita stays at 1 (the baseline `CelestinaIcons` note);
  a first version's `id: recordingBar` shadowed `Main.qml`'s alias of the
  same name and raised it to 3, so the id is `recordingBarWindow`.

## Limits

- Nothing here shows the bar's place, focus and look under niri, a real
  `FocusedOutput` answer, xdg-desktop-portal-wlr running the chooser and
  casting the named output, or the fallback's dialog: VAL-SEL-BAR.
- A Cargo integration test cannot link in this package (CXX-Qt's build
  links its objects into every target), so the binary's `--pick-output`
  is exercised end to end by the smoke and its shell step by a unit test.
- Offscreen, Qt activates every window it shows and ignores
  `Qt.WindowDoesNotAcceptFocus`; on the session niri's rule keeps the
  focus.
- The default fallback is `"$HOME/.local/bin/celestina" --pick-output`,
  the chooser the portal ran before; it is never run here.
- `scripts/complete-production.sh` is the landing's, not this session's.

## Follow-up

- VAL-SEL-BAR in [VALIDATION.md](../../VALIDATION.md).
- The coordinator adds the bar's niri rule and the `V` binding, and sets
  the portal's `chooser_cmd`, after the landing.
- Suite: `scripts/activation_contract.py`'s reason for
  `org.celestina.Selenita1` should list `RecordFocused` beside
  `ToggleRecording` (prose only; the allowlist is unchanged).
- SEL-4-B: pausing by segments, a button in the bar's actions row.

## Landing

- **Base revision:** `7e2c8f0b3b947deb4e1a13889e5a7b96823b74ad`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b91845545bfcbaa05a01d947e1f83bf83520178f54b462507df9c9cafcd64457, verification_fingerprint sha256:a4744ef0d5ee34e4166b8b92008abb8435d7a4acda3189924a2641c43a02efca; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:54690b40a0e8abdc4707b4fb98dda088d60297c864552105cce80816fec990e2, verification_fingerprint sha256:bf15996c730f38826176f3f4c74ed136de60cabf75fd42030d7c87974d6dce68; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8d36f17928fdcbd5efcadebfd2e5a7781344e1ab56c498becd3f4231c852d331, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh
