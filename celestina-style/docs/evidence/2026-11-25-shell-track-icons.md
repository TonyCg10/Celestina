# Evidence: icons for the shell's session menu and Island tabs (STYLE-ICONS-1)

- **Date:** 2026-11-25
- **Scope:** `STYLE-ICONS-1`: `icons/calendar.svg`, `icons/lock.svg`,
  `icons/log-out.svg`, `icons/moon.svg`; their entries in `CMakeLists.txt`,
  `icons.qrc` and `CelestinaIcons.qml`; four names more in
  `tests/tst_iconcatalog.qml`
- **Environment:** worktree `celestina-style-STYLE-ICONS-1`; Qt from the host
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the author approved, on 2026-11-25, the preview of the session
  menu (variant A), the icon tabs (variant A) and the per-app mute button
  (variant A), which use these glyphs (a design canvas private to the author)

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

- **Exit:** every command exits 0; CTest 1/1 (the icon catalogue test names the four glyphs)
- **Review:** 1 round (Sonnet, high reasoning); fixed: alphabetical placement in `CMakeLists.txt` and `icons.qrc`, the status line, a long line in the plans index; deferred: none

## Limits

- The glyphs are not drawn by any consumer yet; the shell's `CEL-I-006` and
  `CEL-D-001` draw them.

## Follow-up

None.

## Landing

- **Base revision:** `101f2da8f6e5f83536c51d5a270eb7d2b280e29d`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; the toolchain changed since the build; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:94e48ad3bc35dcc99fe6902b39f9943a6a2d2254e8f00f9cb71827e1cbd24d8c, verification_fingerprint sha256:d67d27055c5e304bb91270e0162ab06ade491e33e377712d4f8bfc61b33bb75b; celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:5033f97c55ca9847e9ff8edeef7c4d57595b516f0b69a15cc914f4c2775ecfef, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
