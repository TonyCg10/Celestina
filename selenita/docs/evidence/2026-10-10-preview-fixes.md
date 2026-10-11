# Evidence: the preview's sharpness, the focused monitor and `Grabaciones` (SEL-3-A)

- **Date:** 2026-10-10
- **Scope:** SEL-3-A of
  [SEL-3's plan](../plans/active/2026-10-10-sel-3-preview-fixes.md):
  `celestina-rs/crates/selenita-core` (`niri.rs`, `names.rs`,
  `history.rs`, their tests and the `niri-focused-output.json` fixture) and
  `selenita/` (`PreviewWindow.qml`, `CaptureCard.qml`, `Main.qml`,
  `ActivationRoute.qml`, the capture worker, the backends, the controller,
  the recording worker, the QML tests and stub, the smoke, the documents)
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0
  (`QT_QPA_PLATFORM=offscreen`)
- **Artifact:** the `selenita` release binary in the shared Cargo target
  (`CELESTINA.worktrees/.cargo-target/release/selenita`), not installed

No capture, recording or window was made on the author's session, and niri
was not asked anything: every check ran offscreen over `SELENITA_FAKE=1`,
the QML stand-ins, a fake niri socket in the scratch folder and fixtures.

## What the author found

On 2026-10-10, the first live use of the 1.1.0 preview:

1. The preview looked poor. `PreviewWindow.qml` loaded the picture with
   `sourceSize.width: previewWidth * 2` (480 px) and no smoothing, and the
   scene graph shrank that again into a frame of 224 px; a capture of three
   monitors (9600×2160) became jagged and blurred.
2. The screen key binding (`selenita --screenshot screen`, also
   `Selenita1.Capture("screen")`) took every output, where a key pressed on
   one monitor means that monitor.
3. Recordings landed in the videos folder's `Recordings`; the author wants
   `Grabaciones`, the design's own name, beside the pictures folder's
   «Capturas».

The preview also opened tiled and focused. The cause was niri's rule, not
Selenita: its `title="^Vista previa$"` never matched, because Qt on Wayland
appends the application's display name to a window's title, so the
preview's real title is «Vista previa — Selenita». The coordinator
already corrected the author's configuration and the suite's documents; this
unit corrects the rule as Selenita's README states it (`title="^Vista
previa"`, with the reason the `$` must not be there).

## Change

- **The preview's picture** (`PreviewWindow.qml`): `sourceSize` is
  `pictureSize`, the frame's size (the window's size less `frameMargin`,
  `spaceSm`, on each side) times `Screen.devicePixelRatio`, both
  dimensions rounded up; with `PreserveAspectCrop` Qt decodes at the
  smallest size that covers the frame and keeps the picture's shape.
  `smooth: true` and `mipmap: true`. The poster of a recording is the same
  `Image`, so it follows the same rule. The size comes from the window's
  declared size, not the frame's laid-out one: a first version bound to
  the frame read `-16×-16` before the window was laid out (an invalid
  `sourceSize` decodes the whole picture), which the new QML test caught.
- **The focused monitor** (`selenita_core::niri`): `focused_output(socket)`
  sends `"FocusedOutput"` over `$NIRI_SOCKET` as the window target's
  requests do, and `parse_focused_output` reads what `niri msg --json
  focused-output` prints; `null` is the new `NiriError::NoFocusedOutput`,
  an output without a name or a logical rectangle is `Protocol`. The
  per-output parsing is one function (`output_from`) that `outputs` and
  `focused_output` share. The fixture is the `DP-1` entry of the existing
  `niri-outputs.json` (the same object shape, serial already redacted).
- **The key binding's capture** (`capture.rs`, `backend.rs`,
  `controller.rs`, QML): `Request.output` is a `ScreenOutput`:
  `Chosen(name)` (the card: «Todas» is the empty name) or
  `Focused { fallback }` (a key binding). A screen capture with `Focused`
  asks the backend's new `focused_output()` before the delay (as the
  window target notes its window first) and hands the name to
  `grim -o`; when niri cannot answer it logs
  `selenita: no focused output (<error>); capturing <fallback>` and takes
  the card's choice. A window or region capture never asks. The
  controller's new invokable `captureFromBinding` queues the same request
  as `capture` with `Focused`; `ActivationRoute` calls the host's
  `startBindingCapture()`, which `Main.qml` routes to
  `CaptureCard.captureFromBinding()`. The window's «Capturar», Enter and
  the smoke still call `capture`. The fake backend reports `FAKE-1`
  focused (or nothing, for a test) and names a screen's output in its
  `grab` line, which the smoke reads.
- **`Grabaciones` and `Capturas`** (`selenita_core::names`):
  `RECORDINGS_FOLDER` is `Grabaciones`; the new `CAPTURES_FOLDER`
  (`Capturas`), `captures_dir_in(pictures)` and `captures_dir()` sit beside
  `recordings_dir_in(videos)` and `recordings_dir()` (the coordinator's
  second addendum: Fluorita spelled «Capturas» itself because the crate had
  no constant, and switches to it after this landing). The
  file is marked `language-contract: product-copy`: the two folder names
  are names on disk a person reads and other applications look for, and
  they are its only Spanish literals. Everywhere Selenita resolves a folder
  takes it from there: the recording worker's `Places::resolve`, the
  capture worker's destination (`captures_dir_in`, where it used to join
  the window's text) and `Adopt`'s `Folders`. The window's visible text
  stays product copy: `Main.qml` still binds `qsTr("Capturas")` to the
  controller's `folderName`, which only the card's switch label reads
  («Guardar en la carpeta %1»); the capture `Request` and the `Adopt` job
  no longer carry it. Selenita no longer makes
  `Recordings`; nothing is moved (the coordinator moves the author's files
  after the landing).
- **The `Adopt` rule as the single owner** (`selenita_core::history`):
  `adopted(path, Folders, now)` and `Folders { captures, recordings }`
  keep their names and signatures (Fluorita's FLU-P2-A builds `Folders`
  itself); their documentation now says they are the one rule Selenita's
  `Adopt` and Fluorita's fallback apply. The new owned
  `FolderPaths::inside(pictures, videos)` applies `names`'s two folder
  names to the caller's pictures and videos folders and lends `Folders`
  through `folders()`. The capture worker's `Job::Adopt` takes only the
  key now and uses `FolderPaths::inside` over the pictures folder and the
  recording worker's `videos_folder()`; the controller no longer hands
  it the captures folder's name.

Canonical owners: the niri request and its parser live in `selenita-core`
beside `FocusedWindow`; the choice between the card's output and the
focused one is the capture worker's (`ScreenOutput`), decided by which
entry point the controller took; QML only routes the key binding to its
invokable. Searched for another focused-output reader (`rg -n
"FocusedOutput|focused_output|focused-output"` outside the halted shell):
none in the suite. Searched for other owners of the folder names (`rg -n
"Capturas|Recordings|RECORDINGS_FOLDER"`): Selenita's `Main.qml` binding
(removed), Fluorita's own `CAPTURES_FOLDER` and its tests (FLU-P2-A's to
reconcile), and historical evidence.

## Procedure

```sh
# Crate (the niri tests first, failing on the missing parser)
cd celestina-rs && cargo test -p selenita-core --test niri
# QML: the new preview test first failed on the laid-out frame (-16x-16)
sh selenita/scripts/qml-tests.sh
# Fluorita still builds and passes against the changed crate (no Fluorita file changed)
cd fluorita && cargo test --locked
# The unit's checks
cd selenita && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && cargo test && cargo build --release --locked
cd celestina-rs && cargo fmt --all --check && cargo test -p selenita-core && cargo clippy -p selenita-core --all-targets --locked -- -D warnings
sh selenita/scripts/qml-tests.sh   # three times
sh selenita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
git diff --name-only HEAD; git ls-files --others --exclude-standard \
  | python3 scripts/commit_scope.py --check "selenita-maintenance: Fix the preview's sharpness, capture the focused monitor and record into Grabaciones"
```

## Result

- **Exit:** 0 for every check listed.
- **Crate:** 61 tests pass (56 before): the focused output from the
  fixture (`DP-1` at 1920,0 2560×1440), `null` as `NoFocusedOutput`,
  garbage, a nameless and a disabled output as `Protocol`, the
  `"FocusedOutput"` request line over a fake socket; `CAPTURES_FOLDER` and
  `captures_dir_in`; `RECORDINGS_FOLDER` is `Grabaciones` and not
  `Recordings`; `FolderPaths::inside` names «Capturas» and `Grabaciones`,
  adopts a capture and a recording there and refuses one in the old
  `Recordings` and one loose in the pictures folder; the earlier `Adopt`
  test runs on `Grabaciones`. Clippy clean.
- **Application:** 50 tests pass (46 before): a key binding's screen
  capture takes `FAKE-1` whatever the card chose while the card's own
  capture keeps every output or its output; without a focused output the
  card's choice is taken; a region capture never asks niri; the
  controller maps a key binding to `Focused` with the card's output as
  fallback; `Adopt` joins from «Capturas» and `Grabaciones` and refuses
  the old `Recordings`; the recording worker's test checks that no
  `Recordings` folder is made. Clippy clean.
- **Fluorita:** 114 tests pass against the changed crate (its own
  `Folders` and paths are untouched).
- **QML:** 66 test functions pass three times in a row (64 before): the
  preview's picture is smooth, mipmapped, cropped and decoded at the
  frame's size times the pixel ratio for a square capture (224×224 at
  ratio 1) and again for a 16:9 recording's poster once the window has
  shrunk (224×124); a key binding's `screen` goes to
  `captureFromBinding` while the window's own capture goes to `capture`
  and the card's output stays chosen; the binding test for `window` now
  expects `captureFromBinding`, and `folderName` is still the window's
  `qsTr("Capturas")`.
- **Smoke:** OK. The plain launch's capture logs `grab screen (every
  output)`, the `--screenshot screen` launch's `grab screen (FAKE-1)`; the
  fake MP4 is in the scratch videos folder's `Grabaciones` in both report
  runs and in the close run, and no `Recordings` is made.
- **qmllint:** selenita stays at 1 (the baseline `CelestinaIcons` note).

## Limits

- Nothing here shows the preview's sharpness on a real screen, at a real
  scale, or a real `FocusedOutput` answer and `grim -o` capture under
  niri, or the corrected rule placing the preview: VAL-SEL-FIXES.
- Offscreen the pixel ratio is 1, so the test proves the size follows the
  frame and the ratio property, not a ratio above 1.
- Qt reports the window's new height before it emits the change, so the
  decoded size follows a shape change a moment later (the test waits for
  it); the picture may be decoded twice when a new result changes the
  shape.
- `scripts/complete-production.sh` is the landing's, not this session's.

## Follow-up

- VAL-SEL-FIXES in [VALIDATION.md](../../VALIDATION.md).
- The coordinator moves the author's recordings from `Recordings` to
  `Grabaciones` after the landing.
- Fluorita may take `names::CAPTURES_FOLDER` and `FolderPaths::inside`
  instead of its own constant (FLU-P2-A or later).

## Landing

- **Base revision:** `1eed320941c17db775ec4113be3155a46071a037`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4314cb980436fdae08ce03a2fedf1b45857913e732af8cf13f6a43097fd02005, verification_fingerprint sha256:6036733dd8097d96c97d021c0933b55656d75f7df33d1737cec9b9853a3410c2; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c1cb87e6a626a51f5f462591de8addde5d5804b1bf304704d935eaaa6cd1a040, verification_fingerprint sha256:296a25b45901747d3db1fbe0dcbcd8060d4328f995aa5810742eb78dcd65fc0d; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:04e7b359161cff9dc0955c1fb17c3d669bdccaf39f945399953c0a6df1c5d501, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh
