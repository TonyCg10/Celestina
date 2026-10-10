# Evidence: screen recording through the portal and GStreamer (SEL-1-B)

- **Date:** 2026-10-09
- **Scope:** SEL-1-B of
  [SEL-1's plan](../plans/archive/2026-10-09-sel-1-foundation.md):
  `celestina-rs/crates/selenita-core` (the pipeline builder, the recording
  state machine, the stop file, the videos folder) and `selenita/` (the
  ScreenCast portal client, the recording worker with its real and fake
  recorders, the recording card, `--record` and `--stop`,
  `ToggleRecording()` and `StopRecording()` on `org.celestina.Selenita1`)
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0, niri
  26.04, GStreamer 1.28.7, `xdg-desktop-portal` with the ScreenCast portal
  version 4 routed to `wlr`
- **Artifact:** the `selenita` release binary in the shared Cargo target, not
  installed

No recording was made on the session and the portal was never asked for a
session: every run below is offscreen over `SELENITA_FAKE=1`, the QML
stand-ins or a `videotestsrc` pipeline to a scratch file. The real recording
is VAL-SEL-REC in [VALIDATION.md](../../VALIDATION.md).

## Spike (Step 1): the `gstreamer` crate or `gst-launch-1.0`

Throwaway crate in the session scratchpad (not in the tree),
`gstreamer = "0.25"` (`cargo search gstreamer`: 0.25.4 is current). Time
spent: about 15 minutes. The host was probed read-only with
`gst-inspect-1.0 --exists <element>` and `busctl --user introspect`; nothing
was installed.

| Measurement | Result |
| --- | --- |
| (a) The crate builds with the workspace toolchain | Yes: `gstreamer` 0.25.4 over `glib` 0.22.10 built in release in 10.7 s (58 crates in the closure), and the scratch binary ran `videotestsrc num-buffers=30 ! videoconvert ! x264enc ! fakesink` to EOS against GStreamer 1.28.7 (exit 0). |
| (b) The crate links beside cxx-qt in the `selenita` release binary | Not measured: (c) decides first and the plan's rule needs all three. |
| (c) `mp4mux` is found at runtime | **No.** `gst-inspect-1.0 --exists mp4mux` exits 1: `gst-plugins-good` is not installed on the host. Present: `x264enc`, `vah264enc` (a render node exists, `/dev/dri/renderD128`), `pipewiresrc`, `avenc_aac`, `opusenc`, `fdkaacenc`, `h264parse`, `videoconvert`, `audioconvert`, `queue`, `filesink`, `videotestsrc`, `audiotestsrc`. Absent: `mp4mux`, `pulsesrc`, `vaapih264enc`. |

The ScreenCast portal on the session (read-only introspection): version 4,
`AvailableSourceTypes` 1 (monitors only, as `wlr` serves it),
`AvailableCursorModes` 3 (hidden, embedded), methods `CreateSession`,
`SelectSources`, `Start`, `OpenPipeWireRemote`.

**Decision: `gst-launch-1.0 -e` as a child process.** The plan's rule for
the crate (builds, links beside cxx-qt, `mp4mux` found at runtime) fails on
the third point, and the child route has merits of its own: no new link
dependency in the window (the crate brings `glib`, `gio` and 56 others), the
recorder goes through the `SELENITA_TOOLS_DIR` seam like `grim` (a stub
named `gst-launch-1.0` records offscreen), `cargo test` and the QML tests
need no GStreamer at all, and `-e` turns the SIGINT the worker sends into an
EOS so `mp4mux` writes its index before the child exits. The pipeline
description is built in the crate against the element names and the
`videotestsrc` run to a scratch MP4 is left for the author once
`gst-plugins-good` is installed (the muxer is missing, so it could not run
here). The scratch crate was deleted after the spike.

## Procedure

Run from the worktree root unless a line says otherwise; every command
exited 0.

```sh
cd celestina-rs && cargo fmt -p selenita-core --check && cargo test -p selenita-core
cd celestina-rs && cargo clippy -p selenita-core --all-targets --locked -- -D warnings
cd selenita && cargo fmt --all --check
cd selenita && cargo clippy --all-targets --locked -- -D warnings
cd selenita && cargo test
cd selenita && cargo build --release --locked
sh selenita/scripts/qml-tests.sh          # five consecutive runs
sh selenita/scripts/smoke.sh --binary ../.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
python3 scripts/test-activation-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **selenita-core:** 45 tests (34 before). `record.rs`: the four pipeline
  combinations (`x264enc`/`vah264enc` × audio off/on) with `mp4mux name=mux`
  first, the video branch through `videoconvert`, the encoder, `h264parse`
  and a queue into `mux.`, the audio branch on the default sink's monitor
  (`stream.capture.sink=true`) through `audioconvert`, `audioresample` and
  `avenc_aac`; the output path as one quoted token (spaces, quotes and
  backslashes escaped); a source without a remote fd; the launcher argv
  (`gst-launch-1.0 -e -q …`) and the inspector argv (`--exists`); the
  encoder choice needing both a render node and the element; the state
  machine's happy path (Idle → Preparing → Recording → Stopping → Idle),
  the three failures into Failed and its two exits, and fifteen refused
  transitions; the stop file's path, touch and single take. `runner.rs`: a
  running child interrupted with SIGINT exits within the deadline, a child
  that ignores it is killed at the deadline, a missing launcher fails at
  start. `names.rs`: the videos folder reads like the pictures one.
- **selenita:** 30 tests (21 before). The recording session over the fake
  recorder (a recording walks Preparing, Recording, Stopping, Idle and
  lands as `Recording <stamp>.mp4` in the videos folder with the hidden file
  gone and a `Finished` entry of kind `recording`; a second start is busy
  and a stop without a recording is refused; a cancelled portal leaves the
  session idle without a file; a child that dies fails the recording and
  removes its file, and so does a stop that finds it gone; a child that
  ends with status 0 (EOS) has its file published; a quit while recording
  publishes the file and writes it to the history file, and a quit with
  nothing recording leaves nothing; the encoder follows the probe and the
  probe names the inspector, the launcher or the muxer when missing; every
  error has Spanish words); «Copiar» on a recording is refused while the
  other actions work; the flags (`--record`, `--stop`, the first flag
  winning); the bus interface's `ToggleRecording` and `StopRecording` hand
  their action on; the capture plan and the activation adapter as before.
- **QML:** 31 passed, 0 failed, in each of three runs (21 before).
  `tst_recording.qml`: the button starts and stops (its words, the red dot,
  the `m:ss` time), the elapsed time from the start (and `h:mm:ss`), the
  sound switch chosen before recording and locked during it, the transit
  states disabling the button, a missing muxer said in the card with the
  button disabled, the last recording's «Abrir en Fluorita» reaching the
  controller with its id. `tst_history.qml`: a recording row shows the film
  glyph and no still. `tst_main.qml`: a key binding's `toggle` and `stop`
  reach the controller and land a recording row, and the stem is bound.
- **Smoke:** both runs (plain, and `--screenshot screen` with no bus) take
  one fake capture, start a fake recording at 1.5 s and stop it at 2.5 s,
  and report `cards=3 history=2 recording=idle lastRecording=true
  shown=true fake=true textScale=1 fontBody=13`; one PNG in the scratch
  `Capturas`, one MP4 in the scratch videos folder, two lines in the
  scratch history.
- **qmllint:** 1 warning, the baseline row (the shared `CelestinaIcons.qml`).
- **Guards:** the activation, architecture, language and documentation
  contracts exit 0.

## Findings

- The ScreenCast portal's fd is handed to the child as its standard input
  (`pipewiresrc fd=0 path=<node>`): `Command` dups it onto 0 for that child
  alone and no `pre_exec` or unsafe call is needed. The bus delivers the
  descriptor without close-on-exec, so the worker sets `FD_CLOEXEC` on it
  right after `OpenPipeWireRemote` (`rustix::io::fcntl_setfd`); without that
  a `grim` or `wl-copy` the capture worker forks meanwhile would inherit the
  remote. When `OpenPipeWireRemote` fails the pipeline connects to the
  session's daemon on its own, where the wlr backend's node is visible.
- A history row cannot load an MP4 as an image: a recording row shows the
  film glyph instead of a still (found by the QML tests' warnings), and
  hides «Copiar», whose `wl-copy --type image/png` is for a picture; the
  worker refuses the action for a recording too (`NotAPicture`).
- Quitting the window while recording: `main` calls `record::quit` after
  the event loop ends, which sends the worker a `Quit`, closes any portal
  request it is waiting on (`portal::cancel_pending`, through the request's
  `Close` and a wake on the worker's channel) and waits up to the stop
  deadline; the worker stops the child, publishes the file and writes the
  entry to the history file directly (the Qt thread is gone), then ends.
- A child that exits 0 on its own (the source sent EOS) is a finished
  recording, published like a stopped one; any other early exit is a
  failure whose file is removed.
- Recordings land in the videos folder's root (`XDG_VIDEOS_DIR`), not in a
  «Grabaciones» subfolder as the captures do in «Capturas»: the design's
  §5.1 names "the videos folder" for the recording and only the pictures
  folder gets a subfolder; kept as written.
- `--stop` goes through its own `StopRecording()` rather than
  `ToggleRecording()`, which would start a recording when none runs; the
  interface literal and the activation scanner's allowlist are unchanged.
  With no instance on the bus `--stop` touches the stop file instead of
  starting a window.
- `celestina-rs/Cargo.lock` and `selenita/Cargo.lock` change only by the
  `rustix` edge of `selenita-core` (SIGINT through `kill_process`); the
  crate was already in both closures through zbus.

## Limits

- Nothing here asked the portal for a session, ran `gst-launch-1.0` or
  touched PipeWire: the fake recorder writes a short file, the real
  recorder was exercised only through its probe (`gst-inspect-1.0
  --exists`, exit 1 for `mp4mux` on this host) and the SIGINT path through
  `sh`. The pipeline's words (`pipewiresrc` with `fd=0`, the
  `stream.capture.sink` property, `vah264enc` after `videoconvert`) and the
  `-e` finish are VAL-SEL-REC's, as is the `videotestsrc` run to an MP4 the
  plan asked for once the muxer is installed.
- The portal's chooser deadline (180 s) and the muxer's stop deadline (10 s)
  are estimates.
- The window stays on screen while it records; a person who does not want
  it in the picture moves it off the recorded output. Closing it ends the
  recording (finished and published, see Findings), so the key bindings
  are the way to record without the window.
- A portal request that times out or is cancelled at quit leaves its
  helper thread parked on zbus's blocking signal iterator until the process
  ends (STATUS, Known issues).

## Landing

- **Base revision:** `0b84ad6cbd7e641176f2506aa3f882ef36d64513`
- **Check:** `production_artifact.py check selenita --require-verified` exit 0: artifact: selenita current; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 0: artifact: siderita current; `production_artifact.py check magnetita --require-verified` exit 0: artifact: magnetita current; `production_artifact.py check magnetita-android --require-verified` exit 0: artifact: magnetita-android current; `production_artifact.py check grafita --require-verified` exit 0: artifact: grafita current; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:93c2a81926270cc4e4fc90c74b470cae764d989c7c005003b5393f6d8c70f0d7, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:27e561bb973c5a81c7df3ff77197dfe78a06c39fd85439aa69ffbe083cdda7b9, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1936639d19dcbe5d8e4326b12450241e2571ac720a0ad033ab74a31f09e4aabf, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a51ac98df5d39106940c13b59a7ea314bc17b3b00ae3345f48ce93a4427bf600, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh
