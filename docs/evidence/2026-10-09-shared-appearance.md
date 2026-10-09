# Evidence: shared appearance

- **Date:** 2026-10-09
- **Scope:** CONV-1-B — suite
- **Environment:** CachyOS, rustc 1.98.1, Qt 6.12.0, Python 3.14.7; debug and release builds
  in the shared session Cargo target
- **Artifact:** not applicable

## Change

- New crate `celestina-rs/crates/celestina-settings` (no Qt), registered as a
  workspace member and as the component prefix `celestina-settings` of
  `celestina-rs`, and added to the production inputs of Siderita, Grafita,
  Fluorita, Hematita, Magnetita and Cuprita. It owns
  `~/.config/celestina/appearance.toml`: `TextScale` (`compact`, `normal`,
  `large`, `larger`; factors 0.9, 1.0, 1.15, 1.3), `Appearance`
  (`reduced_motion`, `text_scale`; default `false`, `normal`), `path`,
  `load` (defaults on a missing, unreadable or malformed file, logged, the
  file left alone; bounded read through `atomic_file::read_bounded`),
  `save` (line-based rewrite of the two root-table keys through
  `atomic_file::replace`, every other line kept byte for byte, a trailing
  comment on a known line kept, the file's own line ending (LF or CRLF)
  used; a classifier follows multi-line arrays and `"""`/`'''` strings so
  only a real `[table]` or `[[array]]` header ends the root table and bounds
  where missing keys go),
  `env_forces_reduced_motion` (`CELESTINA_REDUCED_MOTION` still forces
  reduced motion on) and `watch` (`notify-debouncer-full` 0.5, 300 ms, on
  the directory so a rename-over is seen, calling back only when the value
  changes; the handle stops the watcher on drop; the directory is created
  by `watch` only, since inotify needs it to exist) and `follow` (a thread
  of its own that starts `watch`, then delivers `load`, and stops on drop of
  its `Follower`). No TOML library: the
  workspace has none, and a parse/serialise round trip would not keep
  comments and unknown keys verbatim.
- `CelestinaTheme` gains `property real textScale: 1.0`; the nine font roles
  are `Math.round(font<Role>Base * textScale)` over nine unscaled base
  constants. Layout tokens, `rowHeight` and radii are unchanged;
  `reducedMotion` keeps its semantics. A new style type,
  `CelestinaAppearance`, binds the theme's two inputs from two plain values
  (it takes no controller), so the binding has one owner instead of six
  copies; registered in `qmldir`, `CMakeLists.txt`, DESIGN §5.4 and §6.1,
  README, and linked into each application's `qml/` and `build.rs`.
- Each application: the six `CELESTINA_REDUCED_MOTION` reads (five
  `main.rs` initial properties, Cuprita's controller) are gone, with the
  windows' `required property bool reducedMotion` and the
  `Component.onCompleted` assignments. Each adapter seeds its two values
  with a synchronous, bounded `load()` in `Default`, so the theme is right
  before the QML that reads it loads (no first frame at the defaults), and
  calls `celestina_settings::follow` with a closure that queues each value
  through cxx-qt `Threading`; it keeps only its bridge, `apply` and that
  call. The object exposes `appearanceReducedMotion` (bool) and `appearanceTextScale` (real),
  and `Main.qml` hands them to `CelestinaAppearance`. Cuprita carries them
  on its existing window-wide `CupritaController`. Siderita, Grafita,
  Fluorita, Hematita and Magnetita get a narrow `src/appearance.rs` QObject
  (`<App>Appearance`): Siderita's controller is a ratcheted legacy
  coordinator (`lines` baseline 746) and the other four have no window-wide
  controller, so the properties were not added to an unrelated domain
  object. The follower stops when the object goes; Cuprita's singleton
  lives as long as the engine, so its follower lives until exit.
- Cuprita's existing smoke report switch (`CUPRITA_SMOKE_REPORT`) adds
  `textScale` and `fontBody` to its report and prints
  `cuprita-appearance: textScale=… fontBody=…` at each change of the theme.

## Procedure

```sh
cd celestina-rs && cargo test -p celestina-settings
# celestina-style, in a scratch build directory (never the production one):
cmake -S celestina-style -B <scratch> -DBUILD_TESTING=ON && cmake --build <scratch>
ctest --test-dir <scratch>; cmake --build <scratch> --target all_qmllint
bash celestina-style/scripts/check-style-contract.sh
# in each of siderita grafita fluorita hematita magnetita cuprita:
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo build --release --locked
# from the root, for each application:
sh <app>/scripts/smoke.sh --binary .cargo-target/release/<app>
bash scripts/qmllint-cxxqt.sh <app>
sh siderita/scripts/qml-tests.sh; sh cuprita/scripts/qml-tests.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-production-artifacts.py
python3 scripts/test-land-unit.py
```

Live demonstration: the release `cuprita` built from this tree, with its
shipped `Main.qml`, ran offscreen on a private bus with a scratch
configuration home (the author's `~/.config` untouched), started with
`text_scale = "large"`; after 4 s the file was rewritten in place with
`"larger"`, after 1 s more replaced by rename with `"compact"`:

```sh
S=<scratch>; mkdir -p $S/config/celestina $S/run
printf 'text_scale = "large"\n' > $S/config/celestina/appearance.toml
XDG_CONFIG_HOME=$S/config XDG_RUNTIME_DIR=$S/run CUPRITA_FAKE=1 \
CUPRITA_SMOKE_REPORT=1 QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  timeout 10 dbus-run-session -- .cargo-target/release/cuprita > $S/out.log 2>&1 &
sleep 4; printf 'text_scale = "larger"\n' > $S/config/celestina/appearance.toml
sleep 1; printf 'text_scale = "compact"\n' > $S/config/celestina/appearance.toml.new
mv $S/config/celestina/appearance.toml.new $S/config/celestina/appearance.toml
grep -E 'cuprita-(appearance|smoke)' $S/out.log
```

```text
qml: cuprita-appearance: textScale=1.15 fontBody=15
qml: cuprita-smoke: networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50 textScale=1.15 fontBody=15
qml: cuprita-appearance: textScale=1.3 fontBody=17
qml: cuprita-appearance: textScale=0.9 fontBody=12
```

The first line is the binding replacing the theme's own default while the
window is created, before the report: the seeded value is there from the
start, and each later change arrives within the debounce.

## Result

- **Exit:** every command above exits 0.
- **Tests added:** crate 7 integration (`tests/appearance.rs`: path,
  defaults, round trip, unknown key and comment kept, malformed file left
  alone, environment override, factor table), 5 watcher and follower
  (`tests/watch.rs`: one callback within 2 s and none for a same-value
  rewrite, deletion answers the defaults, two writes inside the debounce
  give one callback with the last value, a missing directory is created and
  followed, `follow`'s first delivery equals `load()` and a change follows),
  11 unit (comments and literal strings, table keys, insertion before the
  first table, repeated key, trailing comment kept, CRLF kept, multi-line
  array, multi-line string, `[[array]]` header, `key="value"` without
  spaces, an unknown string holding `=`, `#` and `[`); `celestina-style/tests/tst_textscale.qml`
  (bases, every role at 1.3 equals `Math.round(base * 1.3)` with
  `fontBody` 17, layout tokens fixed, `CelestinaAppearance` feeds the theme);
  `siderita/tests/qml/tst_appearance.qml` and
  `cuprita/tests/qml/tst_appearance.qml` (a queued change over a stub
  adapter reaches the theme).
- **Observed:** qmllint rows unchanged (siderita 222, grafita 38, fluorita
  56, hematita 26, magnetita 38, cuprita 9). A first version kept the bases
  in a nested `QtObject`, which added nine `missing-property` warnings per
  application; plain base properties removed them.

## Limits

- Grafita, Fluorita, Hematita and Magnetita have no QML test harness, so
  their wiring is proven by the build, the smoke and the shared
  `CelestinaAppearance` test, not by a per-application QML test.
- Each adapter reads the file once on the Qt thread when it is created: a
  bounded read (64 KiB) of a local two-line file, accepted so the first
  frame honours reduced motion and the text scale.
- `CELESTINA_REDUCED_MOTION` set while Cuprita saves (CONV-1-C) would make
  `load` answer `true`; the file itself is not changed by the override.
- Grafita, Hematita and Cuprita gain `notify` and its platform crates in
  their lock files.
- How the larger type reads in each window is the author's to judge on a
  real session.

## Follow-up

CONV-1-C.

## Landing

- **Base revision:** `efc08f29d7764f38163ad8c8d46fe74bd367fd8f`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3cbc7ec492a3526eb55411fa8a554bbb8b91b8abc0cf945d1dca8bba615e22a7, verification_fingerprint sha256:ec82f2bb1ed32d77ca3b2aa966d2a456e9b4d6c97bac528bc4d5cab785eea153; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1bb654e785383f2824fe496d61b92b9e4faa7a809d81a207687780ca61db15ed, verification_fingerprint sha256:44a2c1bad70abe5690e08ecc71b8eb6ce3eb03ba422b7ff6de48154203d47669; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:58a1321092c7e869beeb1cbf36d80fb6a94791d5ada4e3e258cff570e6f5cf05, verification_fingerprint sha256:44d4309a126e38d8ea653d68c6000e997f261514c87c6cba727c7c06d039b6ed; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3ff0499711296cf36079b0cba94c47396eb21af2392f45e44fd0ebd79e616349, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2daac9ec5f3121a824f99a712a7df52e40bf887a5554309f9a463c88ea9ca5f0, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:88f40e795252ffd74d172d335e6eb5368209fd5bf228dd3bb4950c7353db43bb, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:42fd873bd5f04baab899de479bdeb9d3316d24f0fabcb5cb5ed33dae22de3f6e, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ca068cdf578f4b1785b6d0f8a5be009dabc6a21dbff21a0bb509331d8015170c, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:82fa41379fc092a76705db707fd4447ba2f95aa24dc139e3efb2ab2193110bd8, verification_fingerprint sha256:6b679c198c3c1a39a41fac71a896ffed78cd9199df1a85f2978c44c1e85bac2d
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh
