# Evidence: the frame-accurate video trim through ffmpeg

- **Date:** 2026-10-10
- **Scope:** `FLU-P1-B` — `fluorita` (program `PRV-1`, design §6)
- **Environment:** CachyOS, Qt 6.12.0, `ffmpeg` n9.0.2 (`/usr/bin/ffmpeg`,
  with `libx264`, `h264_vaapi` and `aac`), a render node
  `/dev/dri/renderD128`, Rust toolchain of the workspace; a session worktree,
  offscreen only: no window was shown on the author's session
- **Artifact:** the session's release build at
  `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita`; the
  landing builds, verifies and deploys the registered binary

## Change

- `docs/decisions/0009-editing-without-an-encoder.md` gains its amendment:
  one operation, a video's duration trim, raster-class, re-encoded by an
  `ffmpeg` child process that is never linked; pictures keep every rule; the
  other video and audio edits stay refused.
- `fluorita-core` gains `trim` (pure): `Span { start, end }` with
  `Span::new(start, end, length)` (`start < end <= length`, at least
  `MIN_SPAN`, one frame at 30 fps) and `Span::framed(…, frame)` for the
  film's own frame, and `is_whole(length)`;
  `VideoEncoder::{Vaapi { device }, X264}` with `choose(render_node,
  has_h264_vaapi)`; `trim_argv(input, output, span, encoder)`, exactly
  `ffmpeg -hide_banner -nostdin -nostats -y -ss <start> -to <end> -i <input>` + the
  video tokens (`-vaapi_device <dev> -vf format=nv12,hwupload -c:v h264_vaapi
  -qp 20`, or `-c:v libx264 -pix_fmt yuv420p -crf 21 -preset veryfast`) +
  `-c:a aac -b:a 160k -sn -dn
  -movflags +faststart -progress pipe:1 <output>`, the times written to the
  millisecond and cut down, never rounded up (a handle on frame 31 at
  1.0333 s must not become 1.034 s and lose that frame); `Progress`, which
  folds `-progress` lines into a 0..1 share and the frame count; `Encoders`,
  which reads `-encoders`; and `TrimError`, typed, with `message_es` in a
  product-copy module of its own.
- `fluorita-engine` gains `land_file` beside `land`: the same landing order
  (copy beside; a replacement under a new name publishes then trashes; one
  under the original's name trashes then publishes, the result already an
  ordinary file under its hidden name) for a result another process wrote,
  which gives it the original's permission bits and syncs it first. A film is
  never read into memory to land it, and an edited picture and a trimmed film
  reach their names by one owner of that order.
- `fluorita/src/trim.rs` (new): `FluoritaTrim`, the QML-facing trim
  (`trimStart`, `trimEnd`, `lengthSeconds`, `minimumSeconds`, `edited`,
  `saving`, `progress`, `notice`, `savedKey`, `savedUrl`; `open`,
  `setLength`, `setSpan`, `saveTrim(replace)`, `cancel`, `close`). `edited`
  is the domain's answer: a span `Span::new` accepts that is not the whole
  film. A save runs on a worker of its own; progress and the outcome come
  back through the queue. `fluorita/src/trim/run.rs` is that worker:
  `choose_encoder` runs `ffmpeg -hide_banner -encoders` and looks for the
  first `/dev/dri/renderD*` (VA-API only with both, else `libx264`, else
  `EncoderMissing`); `run` spawns the child with a null standard input, reads
  `-progress pipe:1` on one thread and the last standard-error line on
  another (both bounded to 4 KiB a line), polls the child and the
  cancellation every 25 ms, kills the child and removes its file on a
  cancel, refuses a result the child reports no frame for, and only after
  exit 0 lands the hidden `.<name>.trim-<pid>-<n>.mp4` through `land_file` —
  «Guardar ambas» as the next free `name (editado).mp4` beside the original,
  then `adopt::request`; «Guardar solo la editada» under the original's name
  (as `.mp4`), the original to the Trash through `DesktopTrash`, the seam the
  picture editor uses. A landing failure is worded by the picture editor's
  own sentences (`editor::copy::failure`, now `pub(crate)`).
- `fluorita/src/editor.rs`: the opener's measurement decides a video too (the
  kind by name, then one `stat` on the opener's thread) and publishes
  `video` instead of a document; nothing of the picture editor is held for
  it.
- `fluorita/qml/components/TrimBar.qml` (new): the track, the kept span, the
  playhead and two handles from one `Repeater`; each handle is an
  `Accessible.Slider` named «Inicio del recorte» / «Final del recorte»,
  reached with Tab, moved by the pointer (from where it was gripped) or the
  arrows (a frame at 30 fps; Shift, a second; Home and End), and never
  closer to the other than `minimumGap`; colours are tokens and their
  animation is off under `reducedMotion`.
  `fluorita/qml/components/TrimSurface.qml` (new): the film (`MpvVideo`)
  over the times, the bar and the actions; moving a handle sets the span and
  seeks to it; play starts at the span's start and stops there again when
  the confirmed position reaches its end; the film is paused at its first
  confirmed state; «Guardar ambas» and «Guardar solo la editada» are enabled
  only for an `edited` span; while saving, a progress bar and «Cancelar»;
  leaving with a span chosen asks `EditCloseQuestion` (which gained a
  `message`, the video's being `qsTr("Este vídeo tiene cambios sin
  guardar.")`). The caption says `qsTr("Se conserva %1 · Vídeo nuevo")`:
  the class is raster and the surface says so.
  `fluorita/qml/EditWindow.qml`: `video` is now the editor's answer; the
  window holds a `FluoritaTrim` and a `FluoritaPlayer`, opens both for a
  video, shows a landed trim's result like a picture's (the film glyph
  instead of the picture, the same copy-only drag), and lets the player's
  render context go before it closes or shows the result.
- `fluorita/src/activation.rs`, the two minors deferred at `FLU-P1-A`'s
  review: a first `Edit` refused with `UnknownMethod`, `UnknownInterface` or
  `UnknownObject` (the owner has not served `Fluorita1` yet) now goes
  through `retry_edit` instead of exiting 1 (`first_answer`); `retry_edit`
  stops at a `Timeout` and returns it at once, instead of holding the launch
  for five hand-off timeouts against a hung owner.
- Harness and smoke: `fluorita/scripts/qml-tests.sh` publishes a stand-in
  `org.celestina.fluorita.render` (`tests/qml/render/MpvVideo.qml`) beside
  the `FluoritaTrim` and `FluoritaPlayer` stand-ins.
  `fluorita/scripts/smoke.sh` gains step 5c (`--edit` on a film: the QML
  loads, a playback thread runs off the GUI thread, the library is not
  scanned, nothing is written beside the film). On the way, the smoke's
  background `--edit` launch was found measuring the wrong process: `isolated
  … &` forked a shell that ran Fluorita as its child, so `$!` was the shell,
  step 5b read the shell's threads and its `kill` left Fluorita running
  (five such offscreen processes from earlier runs were still alive on this
  host). `isolated_background` execs Fluorita in the forked shell.
- The fixture `fluorita/tests/fixtures/three-seconds.mp4` (43 608 bytes: 90
  frames at 30 fps, 320 × 240 H.264 CRF 30, a sine as AAC, 3.000000 s) was
  written once by `fluorita/tests/fixtures/make-three-seconds.sh`.

## Decisions taken here

- **The audio is re-encoded to AAC 160 kbit/s, always.** The design allows a
  copy "when the cut allows it"; copying cuts on the audio's own packets,
  not on the chosen frame, and deciding when that is clean would need a
  probe of the stream. Re-encoding is the simplest frame-accurate choice.
- **The VA-API chain is the brief's, unchanged.** `-vaapi_device
  /dev/dri/renderD128 -vf format=nv12,hwupload -c:v h264_vaapi -qp 20`
  encoded the fixture on this host's driver offscreen, exit 0.
- **A trim writes MP4 with the main video and one audio stream, nothing
  else.** `ffmpeg`'s own stream selection already keeps one video and one
  audio and skips further audio tracks and subtitles it cannot carry
  (checked on a scratch MKV with two audio tracks and an SRT subtitle: the
  child exited 0 with one video and one audio stream); `-sn -dn` make that
  explicit, so a text subtitle MP4 could hold is not kept either. A film in
  another container, or with streams the result will not keep, gets a
  notice before it is saved. (The first round's claim that such a film
  makes the child fail was wrong: it succeeds and drops them.)
- **The landing order has one owner.** `fluorita-engine`'s `land_file` was
  added rather than repeating the order in the application; Siderita, the
  engine's other consumer, still builds (`cargo check --locked`, exit 0).

## Review round 1

The review's minors, each with its test:

- **M1, what a trim leaves out:** `fluorita_core::trim::{probe_argv,
  SourceFacts}` describe the film through `ffprobe` (same package as
  `ffmpeg`), run by `run::probe_source` on a worker when the trim opens,
  bounded (10 s deadline, 1 MiB, killed on close). `FluoritaTrim` publishes
  `containerNotice`, the picture editor's pattern and sentence
  (`editor::copy::container_change("mp4")`) for a film whose name is not
  `.mp4`, plus the number of streams left out; `TrimSurface` shows it under
  the span. Tests: `a_film_s_facts_give_its_frame_and_what_a_trim_leaves_out`
  (core), `the_probe_gives_the_film_s_frame_and_what_a_trim_leaves_out`
  (a real MKV with two audio tracks: one left out), the notice's words, and
  `test_what_a_trim_leaves_out_is_said_before_saving` (QML).
- **M2, VA-API fallback:** a VA-API run that fails or writes no frame,
  before anything is published, is retried once with `libx264` on the same
  span (`a_vaapi_run_that_fails_is_retried_once_with_x264`, over a stand-in
  `ffmpeg` that refuses `h264_vaapi`; an x264 failure is not retried).
- **M3/M4:** `-pix_fmt yuv420p` on the x264 path and `-nostats` for every
  run; the exact-argv tests name them (and `-sn -dn`).
- **M5:** the hidden name is `.<name>.trim-<pid>-<n>.mp4`, `n` a per-process
  counter, unique even for two names that share the 200 bytes kept. Tying
  the child to Fluorita's death would need `PR_SET_PDEATHSIG` in a
  `pre_exec` hook, which is `unsafe` and has no justified exception in
  Fluorita; the crash leftover is recorded in
  [STATUS.md](../../STATUS.md#known-issues) instead.
- **M6:** a handle between two frames may show one and cut from the next;
  recorded in STATUS and checked by `VAL-FLU-TRIM`.
- **M7:** the shortest span and the arrows' step are one frame of the
  film's own rate (`SourceFacts::frame`: `avg_frame_rate`, else
  `r_frame_rate`, believed between 1 and 1000 fps), `MIN_SPAN` the fallback
  (`a_span_holds_at_least_one_frame_of_the_film_s_own_rate`,
  `test_the_arrows_step_one_frame_of_the_film`).
- **MPRIS:** `FluoritaPlayer.announced` (true by default) gates
  `start_publishing` beside `previewing`; the edit window's player sets it
  false (`only_an_announced_player_that_is_not_previewing_reaches_mpris`,
  and the QML test reads `announced` on the window's player). The STATUS
  known issue is gone.
- **M8:** a film with no sound trims to a film with only video
  (`a_film_without_sound_trims_to_a_film_without_sound`); each handle's
  accessible description starts with the time it stands on; the README
  paragraph is rewrapped.

## Procedure

Tests were written first and seen failing: `cargo test -p fluorita-core
--test trim` (`could not find trim in fluorita_core`), `cargo test -p
fluorita-engine --lib landing` (`no land_file in landing`), the activation
and editor tests (`unresolved imports super::first_answer`,
`super::measure_video`), and `tst_trim.qml` (`TrimBar is not a type`). The
worker's tests were written with the worker in the same step and run once it
compiled. Then, from the worktree:

```sh
(cd fluorita && cargo fmt --all --check)
(cd fluorita && cargo clippy --all-targets --locked -- -D warnings)
(cd fluorita && cargo test --locked)
(cd fluorita && cargo build --release --locked)
(cd celestina-rs && cargo test -p fluorita-core)
(cd celestina-rs && cargo test -p fluorita-engine)
(cd celestina-rs && cargo clippy -p fluorita-core -p fluorita-engine --all-targets -- -D warnings)
(cd siderita && cargo check --locked)
sh fluorita/scripts/qml-tests.sh   # three times
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
python3 scripts/commit_scope.py --check "fluorita-maintenance: Add the frame-accurate video trim through ffmpeg"
```

The measured durations come from the worker's own tests against the real
`ffmpeg`, read back with `ffprobe -v error -show_entries format=duration`
(`cargo test --locked trim::run -- --nocapture`).

## Result

- **Exit:** every command above exits 0. Fluorita's crate: 114 tests (97
  before, 17 new: twelve for the worker, two for the activation minors, one
  for opening a video, one for the notice's words, one for the MPRIS gate);
  `fluorita-core`: 174 unit and 13 new integration tests
  (`tests/trim.rs`); `fluorita-engine`: 183 unit tests (6 new for
  `land_file`); QML: 29 of 29 (13 for the edit window, 16 for the trim),
  three runs out of three; the smoke passes with step 5c; `qmllint` stays at
  56 for `fluorita` (`TrimBar.qml` and `TrimSurface.qml` add none, and no
  `for` loop sits in a handler); the architecture, language, documentation
  and activation contracts pass.
- **Measured:** the fixture trimmed to [1.000 s, 2.000 s):
  - `libx264` (CRF 21, veryfast, yuv420p): **1.000000 s**, 30 frames;
  - VA-API (`h264_vaapi`, QP 20, `/dev/dri/renderD128`, the rule's choice on
    this host): **1.000000 s**, 30 frames;
  - «Guardar solo la editada» of [0.5 s, 2.5 s) with `libx264`: 2.000 s
    within one frame, the original byte for byte in a scratch Trash;
  - VA-API refused by a stand-in, retried with `libx264`: 1.0 s within one
    frame; a film without sound: 1.0 s within one frame, video only.
  Every run is within the ±0.034 s (one frame) the tests allow.
- **Observed:** the child's hidden file exists while it reports progress and
  takes its name only afterwards; the copy lands as `clip (editado).mp4` and
  is handed to adoption, a replacement is not; the last progress report is
  1.0; a child killed by a cancel 300 ms in (a stand-in that writes its
  output and sleeps) returns `Cancelled` at once and leaves only the
  original; a cancel on the first report leaves only the original; `PATH`
  emptied is `ToolMissing` for the encoder probe and the trim alike, with the
  original untouched; a file that is not a film is `Failed` with ffmpeg's
  last line (bounded) and no file left. In the window (stand-ins): a video
  key opens the trim and not the picture editor; the handles are named
  sliders, the start never passes the end by keyboard or by drag; moving a
  handle seeks to it; play seeks to the span's start and stops back at it at
  the end; a whole span disables both outcomes; each outcome reaches
  `saveTrim(replace)`; a landed result is shown, the player released first,
  and drags out as a copy; a save in flight shows its progress, holds the
  window, and is cancelled by «Cancelar»; leaving with a span asks the three
  answers, and the window goes only once the film let go of its surface.
  An offscreen `fluorita --edit film.mp4` stays up with a `fluorita-player`
  thread and no QML error.
- **Owner and reuse:** the span rule, the encoder rule and the argv are
  `fluorita-core`'s; the landing order is the engine's; the Trash is the
  engine's `DesktopTrash` over `siderita-ops`; the copy's name is
  `siderita_ops::next_available` with the editor's marker; adoption is
  `crate::adopt`; the words of a landing failure are the picture editor's.

## Limits

- No window was looked at and no real save was made from the interface: the
  QML tests run over stand-ins, and what a real save writes, trashes and
  adopts is proved by the worker's tests. Seeing the frame under a handle,
  play keeping to the span, the progress and the cancel, and a real trim of
  a Selenita recording are `VAL-FLU-TRIM`.
- When `ffprobe` cannot describe the film, the trim keeps `MIN_SPAN` (one
  frame at 30 fps) and says nothing of streams; a slower film can then
  produce a span with no frame, which the child reports and the trim
  refuses (`NoFrames`) rather than landing an empty file. A variable-rate
  recording's frame is its average.
- A save asks `ffmpeg -encoders` and lists `/dev/dri` each time (a few tens
  of milliseconds on the worker).
- A crash of Fluorita during a trim leaves the child running and its hidden
  file beside the original — see [STATUS.md](../../STATUS.md#known-issues).
- Five orphaned offscreen `fluorita --edit …/foto.png` processes from earlier
  smoke runs (FLU-P1-A's sessions and its landing) were left running on this
  host; this session's own were killed, the others were not touched.

## Follow-up

- `VAL-FLU-TRIM` in [VALIDATION.md](../../VALIDATION.md).

## Landing

- **Base revision:** `9efca5e8dd0e951c7ba638dd14b0f416e3319f5c`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7934d5ef0160e8a4f5de1cbdc4774756580a438b0d48c3616aab359bf522d14f, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cb652fb0f3e219ec6cf956f6b298df931ed0251e145ca10de37c6fb16ac8b6f6, verification_fingerprint sha256:090cc90a469b412f68e62fe5e1a00bc2da2eca4ab19720a6ceab4715f4eb94fe; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d8648a71cc67bfb35e3d757f5066c9d31b3d8d3d71131554c80cb3534a15191b, verification_fingerprint sha256:8ba69713fe6c03d8752a130f181abb479727ba67aeda498eb7b24f90d5996006; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5f7df4b7930b864382976c68285664dad4d630cbb86eb79777a548ede36943ca, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh
