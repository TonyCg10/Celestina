# Evidence: development QML from the source tree

- **Date:** 2026-10-06
- **Scope:** AUD-1-L — suite
- **Environment:** CachyOS, Qt 6.11, Rust stable, Python 3.14
- **Artifact:** the five applications are rebuilt, verified and deployed by
  the landing, since each `main.rs` changes

## Request

The author asked on 2026-10-06 for a way to see a small QML change, such as a
colour, without a build. Each application compiles its QML module into the
binary (`QmlModule` in `build.rs`) and loads `Main.qml` from `qrc:`, so the
installed application only shows a QML change after a release build.

## Design

- `scripts/qml-dev.sh APP [--binary PATH] [-- ARGUMENTS...]` lays the source
  QML out as an import tree under `$XDG_RUNTIME_DIR/celestina-qml-dev/APP/`:
  the generated module's `qmldir` without its `prefer` line, its
  `plugin.qmltypes`, and `qml` linked to `APP/qml`. It starts the binary,
  by default the installed one, with `CELESTINA_QML_DEV_IMPORT` (the tree)
  and `CELESTINA_QML_DEV_MAIN` (the file URL of the tree's `Main.qml`).
- Each application's `main.rs` puts `CELESTINA_QML_DEV_IMPORT` first among the
  engine's import paths with `addImportPath` and loads
  `CELESTINA_QML_DEV_MAIN` instead of its `qrc:` `Main.qml` when they are set;
  otherwise it loads what it always did. The tree layout is known only to the
  script; the applications read two variables and no shared Rust owner is
  added, since the only crate all five link that could hold it,
  `celestina-core`, is Qt-free by charter and Magnetita does not link it.
- The module is located by `scripts/qmllint-cxxqt.sh`, whose discovery
  (`locate_module`) is extracted into a function with a `--print-module`
  entry, so the lint and the development tree read the same module.

Two simpler variants were tried first and rejected on evidence:
`QML_IMPORT_PATH` alone leaves every type resolved from `qrc:` (7 089 module
loads from `qrc:/qt/qml/org/celestina/siderita/qml` in the import trace),
because `Main.qml` lives in the compiled module; loading `Main.qml` from disk
with `QML_IMPORT_PATH` still resolved the module from `qrc:/qt/qml`, which
precedes the environment's paths. Only `addImportPath` puts the tree first.

## Procedure

```sh
sh scripts/test-qml-dev.sh
sh scripts/test-qmllint-target.sh
sh scripts/qmllint-cxxqt.sh siderita
cargo clippy --locked --all-targets -- -D warnings   # in each of the five applications
cargo test --locked                                  # in each of the five applications
QT_QPA_PLATFORM=offscreen QML_IMPORT_TRACE=1 QT_FORCE_STDERR_LOGGING=1 \
    timeout 8 sh scripts/qml-dev.sh siderita --binary <session target>/release/siderita
QT_QPA_PLATFORM=offscreen QML_IMPORT_TRACE=1 QT_FORCE_STDERR_LOGGING=1 \
    timeout 6 <session target>/release/siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
bash scripts/test-commit-scope.sh
sh scripts/test-production-artifacts.sh
```

## Result

- `test-qml-dev.sh`: the tree mirrors the module with the source QML linked
  and no `prefer` line, the binary receives both variables and its
  arguments, a second run replaces the tree, and the script refuses a
  missing module, an unknown application, a missing binary and bad usage.
- `test-qmllint-target.sh` passes, and the real Siderita lint through the
  extracted function reports `OK — org.celestina.siderita (239 non-fatal
  baseline warning(s))`, its recorded row.
- Clippy with `-D warnings` exits 0 for all five applications; their tests
  pass: Siderita 144, Grafita 13, Hematita 80, Fluorita 71, Magnetita 24.
- Siderita through `qml-dev.sh`: no type is loaded from
  `qrc:/qt/qml/org/celestina/siderita/qml`, and `CelestinaTheme.qml` resolves
  644 times from the development tree; no QML error. A probe line added to
  `components/chrome/TopBar.qml` in the source tree (then removed) printed
  at start-up without a build. Without the variables the same binary loads
  every type from `qrc:` as before (7 089 loads, none from the tree).
- An incremental release build of Siderita after a `main.rs` change took
  about 35 s with the release cache that AUD-1-K now keeps.
- Every guard above exits 0.

## Limits

Only Siderita was run through the development tree; the other four share the
same nine lines and were compiled, linted and tested. A visible check on the
author's session, starting an application with `scripts/qml-dev.sh` and
changing a colour, is left to the author.

## Landing

- **Base revision:** `7794550d9b1ab9de623e3120ffe5354b0f2b3d15`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:4cdfba87f5d7f574e4146a177cd5f316e9f11fd41eeb3ad54bf133d79de22174, verification_fingerprint sha256:2f0c459e9bf885a0cab7a7d49101d8e9b15bd203a68e9ab0f2c710eaa509936a; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5f0b4c070652d1f87bc0b4c162e82c12512cad88ee3c1cefaef59f06a144b122, verification_fingerprint sha256:171dd706eb834bab01877924d5a10899a2356c0ee401d2de805e5bcd6fcb7d8b; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ddd3d8f84ee843f01519142e1b00353aab09ca776057b3ec41d0ed3c851f2ab0, verification_fingerprint sha256:d0567b067e64fe81068f5c030913de269cc252b1dc3b73b09b9c2c0721c91671; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4ece5e76fcfefd5e88c63811f0f413391eabcfc1f0bee89d1fe5ba7740f0635a, verification_fingerprint sha256:a543d3c815440b6f8722bf91a6b8d76eb6738d7c80c1a8aafcb44d9299a4baec; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:c185758da616995f85adecf96cdbab049bd97d1bd4eca4b7e6e363854aa6df16; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9bb3dfad1d2abff7ea77659d88e94cc35da5edba6738373f1e1d68bfa88fb0d0, verification_fingerprint sha256:9f724cb8d7e64cc0a806aa64cf49b1f2ec94de583fe044ce6e7640a03f7ad46e; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5e286fa2720f57fa7e45a3fe0fb9465f812b847a0b58936c34b49ef360d06627, verification_fingerprint sha256:6bde20b658341e20ad25706e6fc5fff6c5a5e68a4a033076ec8fdc8df95cedbb; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:aaf5f52d364bd04d97b0f560f9989645e7695b8e183a4ac832d57b6b78f6b1f9, verification_fingerprint sha256:62f7b6aba37e54bc6c32d0bf88c1275c9ade8de680bce9dd5be8626c8bb500ce
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
