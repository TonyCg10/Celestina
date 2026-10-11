# Evidence: pausing a recording by segments (SEL-4-B)

- **Date:** 2026-10-11
- **Scope:** SEL-4-B of
  [SEL-4's plan](../plans/active/2026-10-10-sel-4-recording-controls.md):
  `celestina-rs/crates/selenita-core` (`record.rs`'s `Paused` state, the
  new `segments.rs`, their tests) and `selenita/` (the recording worker,
  the recorders moved to `recorder.rs`, `portal.rs`'s `Session::remote`,
  the controller, `main.rs`, `RecordingBar.qml`, `RecordingCard.qml`,
  `components/RecordingClock.qml`, `Main.qml`'s smoke timers, the QML
  tests and stub, the smoke, the documents), with SEL-4-A's two review
  minors
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0
  (`QT_QPA_PLATFORM=offscreen`), FFmpeg n9.0.2 (`ffmpeg` 2:9.0.2-2.1,
  installed with libmpv)
- **Artifact:** the `selenita` release binary in the shared Cargo target
  (`CELESTINA.worktrees/.cargo-target/release/selenita`), not installed

No recording, capture, portal session or window was made on the author's
session: every check ran offscreen over `SELENITA_FAKE=1`, the QML
stand-ins and scratch folders. The only real tool run is `ffmpeg` on clips
it generates itself (`lavfi` `testsrc`) in a scratch folder, to prove the
concat list and the join's argv.

## Change

- **The state machine** (`selenita_core::record`): `State::Paused`
  (`"paused"`), `Event::Pause` and `Event::Resume`. `Recording ⇄ Paused`;
  both `Stop` to `Stopping` and fail to `Failed`; a pause is refused in
  any other state, a resume anywhere but `Paused`. `is_busy` includes
  `Paused`; the new `is_under_way` is `Recording | Paused`.
- **The segments** (`selenita_core::segments`): `segment_file(base, n)`
  puts `-part<n>` before the hidden file's extension; `part_name(name, n)`
  is the published name of segment `n` after a failed join, `<name>
  (n).mp4`, numbered from 1; `concat_list` writes one `file '<path>'`
  line per segment with ffmpeg's own quoting (`'` as `'\''`), the bytes
  otherwise kept, refusing no segment, a relative path (the demuxer
  would read it from the list's folder) or a line break; `join_argv` is
  `ffmpeg -hide_banner -nostdin -y -f concat -safe 0 -i <list> -c copy
  <out>` (round 1 dropped `-movflags +faststart`, see below);
  `join_deadline(bytes)` is 60 s plus 10 s per GiB begun, at most 5 min;
  `list_file(runtime, tag)` is
  `runtime/selenita/join-<tag>.txt`.
- **The portal** (`portal.rs`): `Session::remote()` calls
  `OpenPipeWireRemote` on the open session and sets close-on-exec;
  `open_monitor` uses it for the first remote, so the resume's remote is
  the same call on the same session (no `SelectSources`, no `Start`, no
  chooser).
- **The recorders** (`recorder.rs`, moved out of `record.rs` with the
  probe): `stop` became `finish` (the child is finished, the session
  kept); `release` closes the session; `reopen` gives the next segment's
  remote (fresh when the first had one, `None` when it had none, an error
  when the session is gone); `join` runs `ffmpeg` under
  `join_deadline` of the segments' size through the tool resolver; `kill`
  (round 1) kills the child and keeps the session. The fake's `reopen_fails` and
  `join_fails` drive the failure tests; its join reads the list, checks
  it is exactly `concat_list(segments)` and writes the segments' bytes in
  order. **Why a byte join and not real tiny MP4s:** the fakes never run
  a tool (`SELENITA_FAKE=1` routes every external tool to a fake, and the
  smoke must not need GStreamer, whose `mp4mux` this host lacked when
  recording was built); what a byte join cannot prove (the list's syntax
  and the argv against a real demuxer) is proved in `selenita-core`'s
  test on real files instead.
- **The worker** (`record.rs`): `Active` keeps the hidden file (the
  join's output), the first segment's pipeline, the finished segments,
  the current one, when it began and the time recorded. A start writes
  segment 1. `pause` finishes the child and keeps a segment with
  something in it, reports `Paused(recorded)` then the state. `resume`
  reopens, starts the next child with the same pipeline (sound and
  encoder included) into the next segment, reports `Started(now −
  recorded)` then the state. `stop`, from recording or paused, finishes
  the child if one runs and `wrap_up`s: the session released, one segment
  published as before, more joined (the list written privately with
  `atomic_file::replace_private`, removed after) and the joined file
  published, the segments removed; a failed join (or no runtime folder)
  publishes each segment as `part_name` with `RecordError::Join` as the
  notice. A current segment lost (its child did not finish it, died after
  a resume, or never streamed) is removed; with nothing finished before
  the recording fails as it did, else the earlier segments are published
  and the notice is `Partial` (or `Resume` when the resume itself
  failed). `discard` works from paused and removes every segment and the
  join's output. `quit` stops a paused recording too and returns every
  entry for the history file. The loop is now `Session::turn`: it polls
  the child and the stop file while recording or paused, and a
  `Job::Discard` skips the child's exit check, so a discard queued in the
  turn a child ended on its own wins (SEL-4-A minor 1); a quit in that
  turn still ends the worker (before, it was dropped and the quit waited
  for its deadline). `main` waits `QUIT_DEADLINE` (`STOP_DEADLINE +
  JOIN_MAX` + 15 s, round 1).
- **The Qt seam**: `recordingRecorded` (the time held while paused,
  milliseconds), `pauseRecording` and `resumeRecording`; the stop, pause,
  resume and discard invokables share one `ask_recorder`.
- **QML**: `RecordingClock` takes `active` and `paused` and holds
  `recordingRecorded` while paused (no tick then). The bar shows while
  recording or paused, its first action is «Pausar»/«Reanudar» (Tonal,
  `Qt.NoFocus`, as wide as its wider label so the pill keeps its size),
  the dot stops pulsing while paused, the time is muted and the pill's
  accessible description says «En pausa». The card shows a paused
  recording's held time with «en pausa» and its «Detener» stops it.
- **VAL-SEL-BAR** (SEL-4-A minor 2): the bar never takes the focus, so the
  Orca step reads it with flat review; the pause steps are added.

## Review round 1

The coordinator's review of dd6cc8fa found, and this amend fixes:

1. **`stopping` came too late** (`record.rs` `stop`): the state stayed
   `recording` while the last segment finished, so a second press of the
   stop queued a toggle that started a new recording, and a «Descartar»
   on the still-visible bar reached an idle worker and was dropped while
   the recording was published. `stop` now steps `Stop` before
   `close_segment`, and `wrap_up` steps only from recording or paused
   (`the_stop_says_stopping_before_the_last_segment_finishes`, which
   checks the state inside the fake's finish). The bar hides at
   `stopping` and the card reads «Terminando el archivo…» with its clock
   at 0 (QML `test_the_finish_and_the_join_read_as_stopping`). A
   «Descartar» asked meanwhile is honoured: `Worker::discard` sets
   `Signals::discard` before queuing its job, and the stop looks at it
   after the finish and after the join, removing everything and saying
   `Discarded` (`a_discard_asked_during_the_finish_drops_everything`,
   `a_discard_asked_during_the_join_drops_everything`). A toggle carries
   the instant it was asked, and one asked before the last stop returned
   is dropped (`a_toggle_asked_while_the_stop_ran_is_dropped`).
2. **A pause right after a resume hung 10 s and ended the recording**
   (`close_segment`): a segment still empty is killed at once
   (`Recorder::kill`, the session kept) and dropped with no time counted,
   and the recording pauses (or stops) with what it had; a segment whose
   child fails at a pause is dropped and, with earlier segments, the
   recording stays paused with `SegmentLost` as the notice
   (`a_pause_before_the_first_buffer_stays_paused_with_the_earlier_segments`,
   `a_pause_whose_child_fails_stays_paused_with_the_earlier_segments`).
3. **Rows lost at the quit**: once the window has quit (`record::quit`
   sets `Signals::quitting` before its job; `Session::quit` and the quit's
   turn set it too) every published entry goes to `Places::history`
   instead of the gone Qt thread
   (`entries_after_the_quit_reach_the_history_file`: the quit's own
   paused recording, a child that ended in the quit's turn, a stop whose
   publish came after the close). The history keeps one row per path, so
   no row is doubled.
4. **Publish failures**: a successful join whose publish fails falls back
   to the segments one by one; a file that cannot be published is never
   removed, it stays under its hidden name and
   `RecordError::Unpublished` names it in the notice
   (`files_that_cannot_be_published_stay_and_are_named`, a read-only
   recordings folder).
5. **Deadlines**: `+faststart` is dropped (the published file is played
   and sent whole, as the single-segment file `mp4mux` writes, and moving
   the index rewrote the whole file a second time); the join's deadline
   scales with the segments' size (60 s + 10 s per GiB, 5 min at most,
   past which the segments are published one by one); `QUIT_DEADLINE` is
   the finish, the longest join and 15 s for the publish and the history.
6. **Nits**: `paused` in `a_launch_is_over_when_nothing_shows_or_runs`;
   this record is renamed to its real date, 2026-10-11.

## Review round 2

The re-review of d9e633f9 found, and this amend fixes:

1. **The empty-file test of round 1 was wrong**: `filesink` buffers
   64 KiB by default, so a segment with frames flowing (a still screen,
   no sound) could be 0 bytes on disk for seconds; a pause then dropped
   those seconds, and a stop of a short first segment failed with
   `NoFile`. Both parts of the ruling are in: (a) the pipeline writes its
   file unbuffered, `filesink buffer-mode=2 location=…`
   (`selenita_core::record::FILESINK_UNBUFFERED`, the argv test updated);
   (b) the size before the finish never decides: `close_segment` always
   calls `Recorder::finish(out)`, which interrupts (SIGINT, EOS) and
   polls; only a child still running after `EOS_GRACE` (1 s) with `out`
   still empty is killed (`NoSignal`) and its segment dropped, a pause
   then staying paused, and a stop of a first segment failing with the
   no-signal notice; otherwise it gets `STOP_DEADLINE` as before.
   `Recorder::kill` is gone. Tests:
   `a_segment_written_only_at_its_eos_is_kept_by_a_pause_and_a_stop` (a
   fake that writes only at its EOS: kept by a pause, joined, and a short
   first segment stopped at once is published),
   `a_child_that_never_reached_its_eos_is_killed_after_the_grace_and_dropped`
   and `a_first_segment_that_never_reached_its_eos_fails_as_no_signal`
   (a fake that never exits), and on the real recorder with `sh` children
   (`recorder::tests`): a child that writes only on SIGINT is waited for
   and kept, a child that ignores SIGINT with nothing written is killed
   between 1 s and 3 s.
2. **The name during the quit wait**: `activation::release()` gives up
   `org.celestina.Selenita` (`ReleaseName` on the owner's connection) as
   soon as the event loop has ended, before `record::quit` waits; `main`
   calls it first. Verified by the smoke's new private-bus run (a
   `dbus-daemon` with no service folders, so nothing is activated): the
   window closes while a fake recording is paused, the log says the name
   was released before the quit wait, and the recording is published.
3. **Deferred**, recorded in STATUS's known issues: the history file's two
   writers at quit (the recording worker's load, append and save against
   the capture worker's in-memory copy).

## Procedure

```sh
cd celestina-rs && cargo fmt --all --check && cargo test -p selenita-core \
  && cargo clippy -p selenita-core --all-targets -- -D warnings
cd selenita && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings \
  && cargo test && cargo build --release --locked
cd fluorita && cargo check --locked     # selenita-core's other consumer
sh selenita/scripts/qml-tests.sh        # three times
sh selenita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
(git diff --name-only HEAD; git ls-files --others --exclude-standard) \
  | python3 scripts/commit_scope.py --check "selenita-maintenance: Add pause and resume to recordings by joining segments"
```

## Result

- **Exit:** 0 for every check listed.
- **After review round 1:** crate 80 tests, application 78, QML 86 three
  times in a row; smoke, qmllint (1), the contracts and Fluorita's check
  as below.
- **After review round 2:** crate 80 tests, application 82, QML 86 three
  times in a row; the smoke with its private-bus run; qmllint (1), the
  contracts and Fluorita's check as below.
- **Crate:** 79 tests pass (71 before), 80 after round 1 (the join's
  deadline): pause and resume both ways, stop
  and failure from paused, every pause or resume out of turn refused;
  segment names, part names from 1, the list's order and quoting (a
  space, a quote, bytes that are not UTF-8), the refusals, the argv, the
  list's place; and the real join: two one-second `mpeg4` clips in a
  folder named `it's a clip` join through `concat_list` and `join_argv`
  into one file `ffprobe` measures at 2.0 s. Clippy clean.
- **Application:** 70 tests pass (58 before), 78 after round 1 (the eight
  tests named above), 12 new in the first version: a pause then a stop
  publishes the one segment without a join (`prepare, start, finish,
  release`); two segments joined into one file and one entry with the
  calls `prepare, start, finish, reopen, start, finish, release, join`,
  no hidden file and no list left, both pipelines the same (sound on),
  the second origin moved back by the recorded time and the pause
  reports growing; three segments stopped while recording joined in
  order; a discard while paused removing both segments; a quit while
  paused publishing the joined file; a failed resume publishing the
  first segment with the `Resume` notice and no second child; a child
  that dies after a resume keeping the first segment with `Partial`; a
  failed join publishing `… (1).mp4` and `… (2).mp4` with the `Join`
  notice; no runtime folder publishing them one by one; pause and resume
  out of turn refused (and silent through the worker); the stop file
  stopping a paused recording; a discard queued with a child that ended
  on its own winning (no entry, no file), and a stop queued with it
  dropped while the child's file is published once. The watchdog test
  now also sees `Stopping` before `Failed`. Clippy clean. Fluorita
  checks against the crate.
- **QML:** 85 passes three times in a row (80 before), 86 after round 1: «Pausar» and
  «Reanudar» take turns with the bar shown, the dot still at full
  opacity and «En pausa» described while paused; the clock holds `0:05`
  through 1.3 s of pause and goes on from there; the pill and the button
  keep their width through a pause; «Detener» and «Descartar» act from
  paused; three actions in order; the card holds a paused time with «en
  pausa» and stops.
- **Smoke:** OK. In both report runs the bar logs `recording=recording`
  at 1.65 s and `recording=paused` at 1.95 s, the fake logs `join 2
  segments`, and the one published MP4 is `fake mp4 finishedfake mp4
  finished` (both segments, in order), with no hidden segment and no
  list left; the close run, whose window closes while the recording is
  paused, publishes the one segment and both history lines; the
  discard and `--pick-output` runs are unchanged.
- **qmllint:** selenita stays at 1 (the baseline `CelestinaIcons` note).

## Limits

- Nothing here records through a real portal: the fresh remote of an
  open session, a second `gst-launch-1.0` on the same node, a node that
  disappears during a pause, and the joined file's playback, picture
  and sound at the joins are VAL-SEL-BAR's.
- The real join is proved on `mpeg4` clips `ffmpeg` makes itself, not on
  the pipeline's H.264 and AAC; the segments of one recording share
  their codecs, which is what `-c copy` needs.
- A pause and a resume each take a child's finish and start, so the
  segment boundaries are not frame-exact.
- `scripts/complete-production.sh` is the landing's, not this session's.

## Follow-up

- VAL-SEL-BAR in [VALIDATION.md](../../VALIDATION.md).
- The suite follow-up from SEL-4-A stands: `scripts/activation_contract.py`'s
  reason for `org.celestina.Selenita1` should list `RecordFocused`.

## Landing

- **Base revision:** `43b05d1da9067fd00813c31caebc39d0627f236d`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:48fe16d9e09173f34802a3069f387f53289b3ec6a6bbd88159f87c3e16a35a97, verification_fingerprint sha256:c8ba469b7be4a5e67e02b153c9ef91270b75e25078150a99f4e639fb5281039b; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b03ab640410b4bd56ddd896d0ba427d10b39914a20e307751b38ff6a6db3f2a6, verification_fingerprint sha256:76727a57a8386d4ea9d0f38a4b73d9afe6430bcbfb05245afc3c4248eb2c36fe; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:6ed04cd7b23f08e2db3b8d07d5afcf768908a1a4e42d363e3c8b41dc54d4983f, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh
