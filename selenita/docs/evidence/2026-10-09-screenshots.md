# Evidence: screenshots, delay, destinations and the history (SEL-1-A)

- **Date:** 2026-10-09
- **Scope:** SEL-1-A of
  [SEL-1's plan](../plans/active/2026-10-09-sel-1-foundation.md):
  `celestina-rs/crates/selenita-core` (geometry, targets, file names, the
  history, the niri client, the tool argv and the deadline runner) and
  `selenita/` (the capture worker, the real and fake backends, the
  controller, `org.celestina.Selenita1`, the `--screenshot` flag, the capture
  and history cards)
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0, niri 26.04
- **Artifact:** the `selenita` release binary in the shared Cargo target, not
  installed

No real capture was taken and `slurp` was never run on the session: every
run below is offscreen over `SELENITA_FAKE=1` or the QML stand-ins. The real
capture is VAL-SEL-SHOT in [VALIDATION.md](../../VALIDATION.md).

## Procedure

Run from the worktree root unless a line says otherwise; every command
exited 0.

```sh
cd celestina-rs && cargo fmt -p selenita-core --check && cargo test -p selenita-core
cd celestina-rs && cargo clippy -p selenita-core --all-targets --locked -- -D warnings
cd celestina-rs && cargo test -p celestina-core --features activation
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

The niri fixtures were captured once, read-only, and redacted (the window
title, the pid and the monitors' serials; the outputs keep two modes each):

```sh
niri msg --json focused-window   # exit 0 → tests/fixtures/niri-focused-window.json
niri msg --json outputs          # exit 0 → tests/fixtures/niri-outputs.json
```

The focused window as niri 26.04 reported it (title redacted):

```json
{"id":2,"title":"A window title","app_id":"com.anthropic.Claude","pid":4242,
 "workspace_id":1,"is_focused":true,"is_floating":false,"is_urgent":false,
 "layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[942.0,1010.0],
 "window_size":[942,1010],"tile_pos_in_workspace_view":null,
 "window_offset_in_tile":[0.0,0.0]},
 "focus_timestamp":{"secs":28522,"nanos":944035349}}
```

A read-only exchange on `$NIRI_SOCKET` (`"Outputs"` and `"FocusedWindow"`)
confirmed the reply shape the client unwraps, `{"Ok":{"<Request>":…}}` on
one line.

## Result

- **selenita-core:** 29 tests. Geometry round trip in `x,y wxh` and refusals;
  the target words; the delay set; the file name
  `Captura 2026-10-09 14.32.05.png` from a fixed wall clock, the ` (2)`/` (3)`
  collision suffix and `first_free`; `user-dirs.dirs` parsing; the history's
  save/load, prune of a vanished file, the cap of 100, damaged lines; the
  niri parsers over both fixtures, a floating window's position, `null`,
  reply unwrapping and a one-line exchange with a fake socket server; the
  argv of `grim`, `slurp`, `wl-copy` and niri's `screenshot-window`, Qt
  colours as slurp's `#RRGGBBAA`, the stub-folder seam; the runner's output,
  stdin, typed failure, missing program and the deadline killing
  `sh -c 'sleep 5'` within 2 s.
- **selenita:** 18 tests. The capture plan over the fake backend (a screen
  capture lands in `Capturas` and the saved history; a 3 s delay counts
  3, 2, 1, 0 after the hide settle; a clipboard-only capture leaves no file
  and no entry; no destination is refused before anything runs; a cancelled
  region is not an error; a taken name is numbered and never replaced;
  delete trashes the file and forgets the line; a vanished file is forgotten
  when used); the flags (`--screenshot` with each word and `=` form, a
  missing or wrong target, `--record`/`--stop` reserved, paths ignored);
  `Capture` takes the three words only and `ToggleRecording` is reserved;
  the activation adapter and the fake switch as before.
- **QML:** (before fix round 1) 20 passed, 0 failed, in each of five runs. `tst_capture_card.qml`:
  the three target buttons set the target, the delay choice, the two
  switches (the button disables with neither), the output row only with
  several outputs, a capture over the stub yields a history row whose
  thumbnail loads, the countdown text. `tst_history.qml`: the empty line,
  rows latest first with their details, the four actions reach the
  controller with the row's id. `tst_main.qml`: a key binding's
  `captureRequested` chooses the target and captures, the window hides and
  returns on the controller's signals, plus the skeleton's three.
- **Smoke:** the binary offscreen for 8 s with `SELENITA_FAKE=1`, a scratch
  `XDG_PICTURES_DIR` and no session bus; one fake capture through the report
  switch; `selenita-smoke: cards=3 history=1 fake=true textScale=1
  fontBody=13`; one PNG in the scratch `Capturas` and one line in the
  scratch history.
- **qmllint:** 1 warning, the baseline row (the shared `CelestinaIcons.qml`).

## Findings

- niri 26.04 gives a tiled window no position
  (`tile_pos_in_workspace_view: null`), so `grim -g` cannot take it. The
  window target goes through `niri msg action screenshot-window --path`,
  which also copies the picture to the clipboard whatever the switch says.
  Recorded in the README and STATUS; VAL-SEL-SHOT checks the result.
- Serving `org.celestina.Selenita1` on the activation connection needs
  `Owner::connection()` in `celestina-core`, and the delete's
  `siderita-ops` needs a `production_inputs` entry in `docs/projects.toml`;
  both are suite files and land with the scanner's allowlist line in
  `scripts/activation_contract.py`, in a suite commit before this unit.

## Fix round 1 (review of `ae0aa698`)

- **Window meant:** the worker reads niri's `Windows` (each with its
  `focus_timestamp`) before Selenita steps aside and takes the latest
  focused window that is not Selenita (`niri::pick_window`, tested over a
  third fixture, `niri msg --json windows`, captured read-only with titles
  and pids redacted), by id through `screenshot-window --id`. The window
  steps aside only after that read (`Report::HideWindow`).
- **Fresh `--screenshot`:** the window starts hidden (`launchCapture`), takes
  the capture, then shows. The smoke runs twice: a plain launch
  (`selenita-smoke-start: shown=true`) and `--screenshot screen` with no bus
  (`shown=false` at start), both ending `cards=3 history=1 shown=true`.
- **Settle:** only when the target may show Selenita's output
  (`target::needs_settle`, tested; `niri msg --json workspaces` captured
  read-only as the fourth fixture).
- **Window and clipboard:** with «Ventana» the clipboard switch is disabled
  and on, with the hint `niri copia la ventana al portapapeles`; niri's copy
  counts as a destination and `wl-copy` is not run again.
- **Minors:** `wl-copy` runs through `runner::run_quiet` (no output pipes;
  a test forks a sleeper and returns at once); slurp's status 1 is a cancel
  only with an empty or "selection cancelled" stderr; `user-dirs.dirs` is
  read as bytes (a non-UTF-8 folder test); the history's lossy read is
  explained (every field, the path key included, is ASCII); `first_free`
  is removed.
- Re-run, all exit 0: crate tests (34), clippy, selenita fmt/clippy/test
  (21)/release build, qml-tests 21/21 in five runs, both smoke runs,
  qmllint (1, the baseline), the activation, architecture, language and
  documentation contracts.

## Limits

- Nothing here touched the session's screen, clipboard, Fluorita or
  Siderita: the fake backend writes a 1×1 PNG and prints the other actions on
  stderr, and the QML stand-in answers a capture with a fixture row.
- The real backend's tool runs, niri's window action, `ShowItems`, the trash
  and the key bindings on the live compositor are VAL-SEL-SHOT.
- The window's hide and return were exercised only offscreen, where the
  compositor's timing does not apply; the 350 ms settle before the picture
  is an estimate VAL-SEL-SHOT confirms.

## Landing

- **Base revision:** `42f9c3ffa6f82d0f623b64450fffed4043edb303`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8d6635cc95bf123d038826cc9a0b240e72f3b6b2130fce7b865985465a8f01e7, verification_fingerprint sha256:514c6e0aff7a00eaa41032a68c4921383c182adf7b9f6bf4748c9b54f6cb63ed; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:89b4df3bfd1b028ffedf7da207097ff58a6d3b2db0844091904994b175ff798f, verification_fingerprint sha256:87eb1e3376ad084befca33fb9d320f09b368e50f1fbc245defe74e1f7c5bea84; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:79c423c6a4ca391828d09e8961bac67bd8e737bfd0b154f71bfa2aefdbfa6718, verification_fingerprint sha256:4e4c9f4bbfe41cd41690fa242748446d59112ca070feb53af29f48f8dc373f81; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:eea6dc535938ff3fb4f9a6dc3bba8951f1ff350d4ee9a8feb5e6a4ae7f0433c2, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0f378e8ae0141caf33d8d49b486ba5537b6e646b4f67fa6a1799f77c7101eee4, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f8a0169043ebe191eccb006399ff53aa4cf7425d8f57e09d123ba5f163863f39, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b2dc8e23a122858977e0fdcb773f6d55e627374c2637e737e7417a0e7735324e, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:fe3358c40d4fe4823c4c28914dee2c8222abd9a2978e1ca0f79c354c94dc7730, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c110237dee529179d3c8d021be03a0b0d0b6aec2ae58d4d463ea7e03e2239ada, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:51e0ecafeeac77597170062cf6ba01221ddcd61b062376450b54b871b1abc95a, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh
