# Evidence: the style's tokens as one data file generated into QML and Rust

- **Date:** 2026-10-15
- **Scope:** CEL-1-C, the whole unit: both commits of the tokens work (the
  data, generator, theme and crate; then the qmllint ratchet rows) and the
  commit that switches the shell's theme (the 2026-10-15 section below) —
  suite (`celestina-style/tokens.toml`,
  `celestina-style/scripts/gen-tokens.py`, `celestina-style/scripts/test-gen-tokens.py`,
  `celestina-style/CelestinaDesignTokens.qml`, `celestina-style/CelestinaTheme.qml`,
  `celestina-rs/crates/celestina-tokens`, the style and Rust verifies, the
  guards that read the theme's values, the eight CXX-Qt consumers'
  registration, `docs/projects.toml`), branch `unit/suite/CEL-1-C` on
  `d6c78b1c`. The first commit's own checks are the sections down to
  "Follow-up"; the shell's `theme.rs` switch is the unit's last commit.
- **Environment:** CachyOS, Python 3.14.8, Rust 1.97.1, CMake 4.4.4, Qt 6.12.0
  (`/usr/lib/qt6/bin/qmllint`); QML tests offscreen
- **Artifact:** not built; no production entry ran in the session

## Procedure

### The data and the generator

`celestina-style/tokens.toml` holds 253 tokens, each with a one-line comment
directly above it: `[ref]` 21, `[scheme.dark]` 88, `[radius]` 8, `[type]` 16
(with `[type.scales]`), `[spacing]` 11, `[motion]` 14, `[haze]` 5, `[glass]` 6,
`[ambient]` 2, `[shadow]` 10, `[backdrop]` 1, `[compositing]` 2, `[opacity]`
11, `[accent]` 11, `[icon]` 14, `[size]` 9, `[border]` 2, `[control]` 22. The
values were listed from `CelestinaTheme.qml` by a throwaway script (every
`readonly property` with a literal value, the `ColorScheme` role list, and
each `schemeDark` role resolved with the theme's arithmetic: linear mix of
the 0..1 channels, each channel rounded half-up to 8 bits). One role is new:
`controlPressed = "#38ffffff"`, the pressed plate one white step above
`controlHover`.

`gen-tokens.py` (standard library only) writes `CelestinaDesignTokens.qml` and
`celestina-rs/crates/celestina-tokens/src/generated.rs`; `--check` writes
into a temporary directory and exits 1 with a unified diff per stale output.
`CelestinaTheme.qml` binds every former literal to `CelestinaDesignTokens` and
keeps its API, relations, enums, functions and recipes.

### Test-first

| Command | Exit | Output |
|---|---|---|
| `python3 celestina-style/scripts/test-gen-tokens.py` (before `gen-tokens.py` existed) | 1 | `FileNotFoundError: … celestina-style/scripts/gen-tokens.py` |
| `python3 celestina-style/scripts/test-gen-tokens.py` (first generator) | 1 | 9 tests, 2 failures: `every_key_needs_its_comment` (the missing-table check ran before the comment check; reordered) and `every_scheme_role_of_celestina_theme_is_a_key` (the theme had no `controlPressed` yet) |
| `python3 celestina-style/scripts/test-gen-tokens.py` (reordered) | 1 | 9 tests, 1 failure: `every_scheme_role_of_celestina_theme_is_a_key`, first differing element `controlPressed` |
| `python3 celestina-style/scripts/test-gen-tokens.py` (role added to the theme) | 0 | Ran 9 tests, OK |
| `python3 celestina-style/scripts/test-gen-tokens.py` (a tenth test, written with its rule, after review) | 0 | Ran 10 tests, OK |

The ten tests: the scheme keys equal the `ColorScheme` roles in order; hex
and table colours parse; QML colours are 8-bit literals (`{ color =
"#050608", alpha = 0.70 }` → `"#b3050608"`); Rust keeps `a: 22.0 / 255.0`,
`a: 1.0` and `a: 0.7`; curves gain `1, 1` in QML; every derived role equals
its recipe recomputed from `[ref]`, `[accent]` and `[opacity]`; a key without
a comment is refused; two tokens that flatten to one QML name are refused; `--check` passes on fresh outputs and fails, naming
only the changed file, on a changed byte and on a missing file; two runs give
identical bytes.

### Guards, before and after

| Command | Before (`d6c78b1c`) | After |
|---|---|---|
| `bash celestina-style/scripts/check-style-contract.sh` | 0, QML visual contract: OK | 0, QML visual contract: OK |
| `python3 celestina-style/scripts/check-contrast-contract.py` | 0, Contrast contract: OK | 0, Contrast contract: OK |
| `python3 scripts/check-sealed-colours.py` | 0, OK (4 colour(s)) | 0, OK (4 colour(s)) |
| `bash scripts/check-architecture-contract.sh` | — | 0, Architecture contract: OK (radius contract OK at the recorded ratchet) |

The contrast guard now reads `CelestinaDesignTokens.qml`, the scheme roles and
the `ref` palette each from its own generated block, and reads `accentLink`,
`focusRing`, `accentHover`, `accentPressed` and `glyphDirectory` as the
resolved colours they are. A scratch wrapper printed every ratio before and
after: the same 133 checks, 34 ratios moved by the 8-bit rounding of the
derived roles (largest move +0.032; `primary button/pressed` 4.9765 → 4.9948;
`sheet/folder` 3.6602 → 3.6467), and the smallest margin over a floor is
0.018 both times.

Other readers of the theme's values: `scripts/check-sealed-colours.py`
resolves `CelestinaDesignTokens.<path>` bindings through the generated file;
`scripts/radius_contract.py` resolves them for its integer tokens (new test
in `scripts/test-radius-contract.py`); `scripts/architecture_scanners.py
qml-style-contract` takes `--token-data`, which `check-style-contract.sh`
passes, and exempts that file like the theme (new fixture case in
`scripts/test-architecture-scanners.sh`); `celestina-style/scripts/glass-reference.py`
reads its four colours from the generated file (its literal reads checked
with Pillow stubbed; the PNGs were not regenerated).

### Checks

| Command | Exit | Output |
|---|---|---|
| `python3 celestina-style/scripts/test-gen-tokens.py` | 0 | Ran 10 tests, OK |
| `python3 celestina-style/scripts/gen-tokens.py --check` | 0 | gen-tokens: OK (both outputs match tokens.toml) |
| `python3 scripts/test-radius-contract.py` | 0 | Ran 25 tests, OK |
| `bash scripts/test-architecture-scanners.sh` | 0 | Architecture fixtures: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (12 owners) |
| `cargo fmt --all --check` (in `celestina-rs/`) | 0 | no diff |
| `cargo clippy --locked -p celestina-tokens --all-targets -- -D warnings` | 0 | no warning |
| `cargo test --locked -p celestina-tokens` | 0 | 3 passed |
| `cmake -S celestina-style -B celestina-style/build` | 0 | configured |
| `cmake --build celestina-style/build --target all_qmllint` | 0 | 8 warnings, all in `CelestinaLineGutter.qml`; the same 8 on `d6c78b1c` |
| `cmake --build celestina-style/build` then `ctest --test-dir celestina-style/build --output-on-failure` | 0 | 1/1 passed; the runner reports 205 passed, `CelestinaTokenData` 3 of them (the name before the 2026-10-16 rename) |
| `sh celestina-style/scripts/smoke-production.sh` | 0 | gallery alive 8 s on the built module |

## Result

- **Exit:** 0 for every command above
- **Observed:** both outputs regenerate byte for byte from `tokens.toml`; every
  `ColorScheme` role of the theme equals its generated colour through the
  scheme and the flat property (`tst_tokendata.qml`); `focusRing` is `#6cabff`,
  `glassTint` `#b3050608`, `controlPressed` `#38ffffff`; the Rust `DARK` carries
  the bytes and alpha spellings of the shell's current `theme.rs` table

## Limits

- No application was built. The CXX-Qt qmllint ratchet
  (`scripts/qmllint-cxxqt.sh`, `scripts/qmllint-baseline.tsv`) was simulated
  against each application's newest release module in the shared Cargo
  target with `CelestinaTokenData` (the name then) added to its `qmldir`: in each of the seven
  applications with such a module, the count rises by one, the
  `not declared as singleton in qmldir but using pragma Singleton` false
  positive Qt 6.12 already reports for `CelestinaIcons.qml`. Hematita has no
  release module there and was not measured. `scripts/qmllint-baseline.tsv`
  was not changed by the first commit; the second commit raised it (the
  controller's ruling below).
- Simulated counts (baseline / HEAD-sim / with change / delta): calcita 1/1/2/+1, cuprita 9/9/10/+1, fluorita 56/69/70/+1, grafita 38/38/39/+1, magnetita 38/42/43/+1, selenita 1/1/2/+1, siderita 220/226/227/+1, hematita 26/—/—/not measured; the fluorita, magnetita and siderita HEAD-sim counts differ from their rows because the newest module in the shared target was built from another tree (stale), so only the delta is read from them.
- Controller ruling, 2026-10-15: every application row of `scripts/qmllint-baseline.tsv` rises by one in the unit's second commit, as a correction of the measurement for this Qt false positive (the calcita/EXT-1-A precedent); Hematita's +1 is assumed and the landing's own count confirms it.
- `glass-reference.py`'s images were not rendered again; its values are
  unchanged.
- At the first commit the shell did not read the crate; the unit's last commit
  does (the 2026-10-15 section below).

## Follow-up

None for the ratchet: the rows were raised by the controller's ruling above; the landing's app verifies confirm the counts.

## 2026-10-15: the shell reads the crate (CEL-1-C, last commit)

Scope: `celestina/crates/celestina-paint/src/theme.rs` (one source file),
`celestina/crates/celestina-paint/Cargo.toml` (the `celestina-tokens` path
dependency), `celestina/Cargo.lock`, and this record. Branch
`unit/suite/CEL-1-C` on `a4f42b2b`. `celestina/src/app.rs` is unchanged:
`celestina-settings` has no `scheme` key yet, so `Theme::dark()` keeps its name
and is `Theme::from(&celestina_tokens::DARK)`; there is no `by_name` call.
`celestina-rs/` is untouched (including its `Cargo.lock`).

The theme is `impl From<&celestina_tokens::Scheme> for Theme`: each of the 15
roles reads its `Scheme` field (`TextHi` from `text`, `TextLo` from
`text_muted`, the rest by name), `GlassFallback` follows `canvas` (the QML
theme's `glassFallback: canvas`, a relation and not a role), and the scalars
come from `haze::GRAIN`, `radius::*`, `spacing::*`, `type_scale::*` and
`motion::*`. `DARK`, `SCALARS`, `Scalars`, `parse_row`, `hex_pair`, `hex_digit`,
`dark_palette` and the two tests that existed for the table
(`dark_table_rows_parse`, `dark_table_covers_every_role`) are removed; the
existing `dark_matches_design_md`, `every_role_has_a_colour` and `scaled_*`
tests are unchanged and pass, which pins every value to the old table's bytes.

### Test-first

The path dependency was added first and the lock recorded it, so that the
test's failure is the missing conversion and not a missing crate.

| Command (in `celestina/`) | Exit | Output |
|---|---|---|
| `cargo build -p celestina-paint` (once, without `--locked`) | 0 | `Adding celestina-tokens v0.1.0`; `Finished`; the lock gains the `celestina-tokens` package and one dependency line of `celestina-paint` (+5 lines, nothing else) |
| `cargo test -p celestina-paint --locked theme` (test `theme_from_tokens_matches_the_generated_dark_scheme` written, no `From` yet) | 101 | `error[E0308]: mismatched types` at `Theme::from(&DARK)`: `expected Theme, found &Scheme`; `could not compile celestina-paint (lib test)` |
| `cargo test -p celestina-paint --locked theme` (after the switch) | 0 | 6 passed: `every_role_has_a_colour`, `scaled_multiplies_only_the_type_ladder`, `scaled_never_compounds`, `scaled_ignores_a_nonsense_factor`, `theme_from_tokens_matches_the_generated_dark_scheme`, `dark_matches_design_md` |

The new test builds `Theme::from(&celestina_tokens::DARK)`, spot-checks accent
`#3e91ff`, card `#0b0c10`, the glass tint as the canvas at alpha 0.70, grain
0.15 and motion 100/200/350/150/500, and compares `t.color(role)` of every
`Role` with the mapped `DARK` field, so a swapped assignment fails.

### Checks

| Command | Exit | Output |
|---|---|---|
| `cargo fmt --all --check` (in `celestina/`) | 0 | no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (in `celestina/`) | 0 | no warning |
| `cargo test --workspace --locked` (in `celestina/`) | 0 | 232 passed, 0 failed (61 + 3 + 47 + 2 + 70 + 9 + 40); the unit B run was 233, minus the two table tests, plus the new one |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (12 owners) |
| `rg -n 'SCALARS\|parse_row\|hex_pair\|hex_digit\|dark_palette' celestina/crates/celestina-paint/src/theme.rs` | 1 | no match: no helper is left |

Diffstat of the three source paths before the commit: `theme.rs` +112/-179,
`celestina-paint/Cargo.toml` +4/-0, `celestina/Cargo.lock` +5/-0.

### Limits

- The shell was not built in release, run or deployed; no production entry ran
  in the session. The landing builds and verifies the artifact.
- The scheme is still chosen in code (`DARK`); the `scheme` setting and the
  `Scheme::by_name` fallback wait for the `celestina-settings` key.

## 2026-10-16: the first release build found a registration order (CEL-1-C, fix)

The first landing build ran the Siderita smoke on the release binary and the
theme read `undefined` from the token data, while the 237 `qmltestrunner`
cases of that landing run passed: the style tests import the CMake module
`CelestinaStyle`, whose compiled registration declares the singletons, not the
qrc of a CXX-Qt application. With `qt.qml.import` logging the observed
difference is that `CelestinaTheme.qml`, which imports only QtQuick, resolved
the token data through its own directory as a plain file type (`TYPE/URL`)
instead of the module's singleton (`TYPE/URL-SINGLETON`), and `Main.qml` did
not resolve the name at all (no importer named it). The proposed mechanism is
that Qt registers a qmldir file singleton by URL when an importer resolves its
name, and that the importer resolves names in alphabetical order, so the theme
compiled before `CelestinaTokenData` (`CelestinaTheme` sorts first). Only the
observation and the fix are proven; Main also resolves `CelestinaAppearance`
and `CelestinaBackdrop` (which bind the theme) before the new name, so the
ordering explanation is not fully established.

The fix renames the singleton to `CelestinaDesignTokens` and each application's
`Main.qml` names it once (`designTokensRegistered`). Values, the generator output and
the theme's bindings are otherwise unchanged. Checked after the rename: the
Siderita release binary was rebuilt and `siderita/scripts/smoke.sh` passed
(exit 0, no QML errors); `gen-tokens.py --check`, `test-gen-tokens.py`,
`check-style-contract.sh`, `check-sealed-colours.py`, the contrast, language,
architecture and documentation guards exit 0. The style `ctest` and the other
seven applications run in the landing's verification, which is the proof for them.

## Landing

- **Base revision:** `d6c78b1c4b2c85989d05f42c96125b966c1d20be`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e3ff3e745c13ac9af799aa6def7de3a8bd0925f93775a81dce733c6a67bc6abb, verification_fingerprint sha256:a6f31bd3505f0b115dbe0c15b9bf85407164bd6414b6060c284263100bd27472; celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0810aa272e5ba64ca25e014c71afd25636cd510bc5352bbb00e796d1e0244069, verification_fingerprint sha256:7863ba031810a0067952fc65d4e82abd3353b190eeca93c9edead23a9280a220; celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:4ea29bf7d081e6b0289aa8616c33fc272214b7058dd8cc2f9e06ef0542435b6d, verification_fingerprint sha256:ffe02560bbcc1003b001fc4e776c8f389698c16a98a1740b1077d4ef1bbed893; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3169f8a8be482849b1a12218152eba825630626db7764be49df250c402de522f, verification_fingerprint sha256:06dfaa103ba501af12953996348620aa07c5db1c5f718404e5a7e2b0a835c4e1; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:58754d264b3d1d719f01337e35f9a0ea0909dfc50373dfaf013728a874c71a13, verification_fingerprint sha256:001c655c46bae61390803bd4e80624e52bc03dff1389485ec83159c209b1d57f; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:cb0bac4484179a885ea4e002c23e0ccda7f37e9da2982c1a094994275f3013ad, verification_fingerprint sha256:398245ca097ba60854b9a5f10718005c33ef274c452cb7f7cf159a4ef6896005; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b062e3aa5a719d0124b32783fc5b0001b90775df00ff8cd49ea61b913a09eef1, verification_fingerprint sha256:8c23a19f4fc1a2cd87ca83280f9c1849db4cf85902e43e6af2d36fd9858590ce; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:12c66a65e4aaddbb7bd1141e3610b36756694115c9d8dda7fa6c96ab6f0ca5a9, verification_fingerprint sha256:e1a9a7a07676c5603e1d953be1d3ea1023687191a2e028e530290030e55c8b82; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:decea6242b46c3dff5f0551f2305f9f79db3cdbd1ba5a374f804332914bb48bf, verification_fingerprint sha256:24229b85b8fb1b504b2d429eeba685db69c4b4afb85886830b76046c64e0368c; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8a73dd70ffd6bbf49ef50e57eba3984f82d4048df1139c3bc199c6a3ed34cb92, verification_fingerprint sha256:f6cb94bc35e1e33557649d8ff90570663c0b45a0d462d0e7473c6b2e86d9a399; calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b41c471bda9be9836196f645681e867cd88ca5dc70cdfca74432e3387b528ab9, verification_fingerprint sha256:d51dfa67ce594b17b4507b36fccdd1b64d9e81cc3df86448f012a4b44fcc63e6; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7c30edaf1987ffccc3ff9c738a7c953fc0a7edf6808772bfb09dce005cb794ca, verification_fingerprint sha256:63ce2c1a7ca9e13dfc13fa7e0681726c5646d8b190e5f3c4e91d81bdb182e4e6
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
