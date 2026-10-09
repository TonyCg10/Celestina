# Evidence: the suite conventions recorded and the program closed

- **Date:** 2026-10-09
- **Scope:** CONV-1-F — suite
- **Environment:** CachyOS, rustc 1.98.1, Qt 6.12.0, Python 3.14.7; debug and
  release builds in the shared session Cargo target
- **Artifact:** not applicable (Cuprita's release binary built, not installed)

## Change

- [ADR 0012](../decisions/0012-suite-conventions.md) records the activation
  interface (`org.celestina.Application1`, claim first, `Open(as paths)` as
  path keys, the scanner), the appearance file (source of truth, the
  environment override at read time, the text scale on the nine font tokens
  only), open-with inside the suite (an explicit send only), drag-and-drop
  (`text/uri-list` through `file_uri::to_path`) and the decisions of spec §11;
  the decisions index lists it.
- The suite ledger: rows A, B, D and E are `done` from their landings; row
  `CONV-1-C` is removed, and the prose above the table says it was delivered
  as Cuprita's `CUP-1-I`, linking its inventory and Cuprita's plan (a `done`
  row here would need an inventory of its own); row F is `active` with this
  record.
- Root `ROADMAP.md`: the CONV-1 checklist is ticked for A–E; F's box is left
  for the landing. The roadmap stays `active` on `CONV-1`: archiving the plan
  and setting the roadmap idle is the author's hand commit after this unit
  lands, as `AUD-1-S` did for AUD-1.
- Root `README.md` gains "How the applications work together".
- Each application's `STATUS.md` (Siderita, Grafita, Fluorita, Hematita,
  Magnetita, Cuprita) opens its current truth with one paragraph on the
  conventions as adopted; Cuprita's header no longer calls `CUP-1-I` open.
- Author checks of spec §10 that were missing, added `pending`:
  `VAL-GRA-OPEN` and `VAL-FLU-OPEN` (a second file from Siderita into a
  running window) and `VAL-SID-SEND` (send to the phone from the menu).
  `VAL-F` (text size followed by every window) and the three `VAL-*-DROP`
  rows already existed; `VAL-F` gains a step for the fix below.
- Cuprita: `AppearanceStore` gains `read_stored`, which is
  `celestina_settings::load_stored()` for the file and the raw value for the
  fake; the section's read-modify-save starts from it, so a text-size change
  while `CELESTINA_REDUCED_MOTION` is set no longer saves
  `reduced_motion = true`. The page still shows `read()` (the override
  applied). The limit in
  [Cuprita's appearance evidence](../../cuprita/docs/evidence/2026-10-09-appearance.md)
  is struck and points here.

## Procedure

```sh
# in cuprita/:
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo build --release --locked
# from the root:
sh cuprita/scripts/qml-tests.sh
sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita
bash scripts/qmllint-cxxqt.sh cuprita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-land-unit.py
```

## Result

- **Exit:** every command above exits 0.
- **Tests added:** Cuprita `controller/appearance.rs` 1 unit
  (`a_forced_reduced_motion_is_never_saved`: a store that reads reduced
  motion forced on keeps `false` in the file after a text-size change).
- **Observed:** Cuprita unit 12 passed; QML 66 passed; smoke
  `appearance=normal textScale=1 fontBody=13`; qmllint Cuprita 9 (its row).

## Limits

- The fix is tested on the in-memory store; that `load_stored()` ignores the
  variable rests on `celestina-settings`' own tests (`CONV-1-D`).
- Nothing here was run by hand: the four live checks of spec §10 are the
  author's, pending in each application's `VALIDATION.md`.
- Only the activation convention has a guard (the CONV-1-A scanner); the
  appearance file, open-with and drop conventions have no scanner and rest on
  review and tests.
- The plan is not archived and the roadmap not set idle by this unit.

## Follow-up

The author's hand commit archiving
`docs/plans/active/2026-10-09-suite-conventions.md` once CONV-1-F lands.

## Landing

- **Base revision:** `48bd717c210ab2ae731d29d4ed2989b5544ba75d`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b171b0ea39ef0ee4a997dff1750a4d4711568c08a31e2131b3089a1b6fcf8062, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh
