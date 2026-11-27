# Evidence: icons for the shell's launcher tabs and calculator (STYLE-ICONS-2)

- **Date:** 2026-11-26
- **Scope:** `STYLE-ICONS-2`: `icons/rotate-ccw-clock.svg`,
  `icons/arrow-down-a-z.svg`, `icons/globe.svg`, `icons/calculator.svg`; their
  entries in `CMakeLists.txt`, `icons.qrc` and `CelestinaIcons.qml`; four names
  more in `tests/tst_iconcatalog.qml`
- **Environment:** worktree `celestina-style-STYLE-ICONS-2`; Qt from the host
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the author approved, on 2026-11-26, the launcher preview that
  uses these glyphs (a private artifact)
- **Name note:** Lucide 1.27.0 renamed `history` to `rotate-ccw-clock`; the
  catalogue keeps the upstream name `rotate-ccw-clock`

## Procedure

Each glyph is the Lucide 1.27.0 file
(`https://raw.githubusercontent.com/lucide-icons/lucide/1.27.0/icons/<name>.svg`)
with the catalogue's recipe: `currentColor` becomes `#fff` and
`stroke-width="2"` becomes `2.5`. The same recipe applied to upstream
`power.svg` reproduces the vendored `icons/power.svg` byte for byte.

```sh
bash celestina-style/scripts/check-style-contract.sh
cmake -S celestina-style -B target/style-test -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build target/style-test
QT_QPA_PLATFORM=offscreen ctest --test-dir target/style-test --output-on-failure
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every command exits 0. `check-style-contract.sh` 0 (contrast and
  QML visual contracts OK); configure and build 0; CTest 1/1 (the
  `celestina-style-modal-test` binary runs the `IconCatalog` cases, which name
  the four glyphs); `check-architecture-contract.sh` 0; `check-documentation-contract.sh`
  0 (its erratum lines concern other units' inventories and are not failures);
  `check-language-contract.py` 0 (128 legacy files ratcheted)
- **Review:** 1 round (Opus, the controller, a model other than the Haiku implementer): each glyph checked against its Lucide 1.27.0 upstream with the recipe, and the four registrations; nothing to fix

## Limits

- The glyphs are not drawn by any consumer yet; the shell's launcher tabs and
  calculator entry draw them.

## Follow-up

None.

## Landing

- **Base revision:** `ccf142d8c4f3a11d8b4b9aebeda5b1fa419195f7`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:306985abc449374caeb187c134aaabac80ba07d620c505c125edd538582a9def, verification_fingerprint sha256:39543979b810fd6f6e8f0e377f2e2d5a58192ffb857c4a34ba90d4b7b7c4968b; celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:f31480de97660c72898bb558c1614f6ae5683347be3bc72f783f4dbc05655af9, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
