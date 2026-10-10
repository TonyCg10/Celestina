# Evidence: the capture preview's interfaces between Selenita and Fluorita

- **Date:** 2026-10-10
- **Scope:** PRV-1-A — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable; the unit changes documents and the activation
  scanner's allowlist only, no registered production or verification input

## Change

- [ADR 0012](../decisions/0012-suite-conventions.md#follow-ups) gains the
  `PRV-1 (2026-10-10)` follow-up: Fluorita serves `org.celestina.Fluorita1`
  at `celestina_core::activation::object_path(&FLUORITA)` with `Edit(s key)`,
  refusing a key that does not decode or names anything but a regular file
  with `org.freedesktop.DBus.Error.InvalidArgs`; Selenita's
  `org.celestina.Selenita1` gains `Adopt(s key)`, which adds a regular file in
  its pictures `Capturas` or videos `Recordings` folder to the history (`.png`
  a screenshot, `.mp4` a recording) and ignores anything else without error.
  Both take one `celestina_core::pathkey` key and are served beside the
  shared interface on the connection that owns the name. This is the contract
  `SEL-2-A` and `FLU-P1-A` implement.
- `scripts/activation_contract.py` allowlists `org.celestina.Fluorita1` in
  `fluorita/src/activation.rs` (served) and `selenita/src/preview.rs`
  (client), and `org.celestina.Selenita1` in `fluorita/src/adopt.rs`
  (client), each with its reason; the reason of the existing
  (`selenita/src/activation.rs`, `org.celestina.Selenita1`) entry names
  `Adopt`.
- `scripts/test-activation-contract.py` gains
  `test_the_capture_preview_interfaces_are_allowed_in_their_files_only`: the
  three new (file, literal) pairs pass, and `org.celestina.Fluorita1` in
  `grafita/src/x.rs` is reported. The test does not pin the allowlist's size,
  so no other case changed.
- The root `ROADMAP.md` is `active` on `PRV-1` with its five units, `STATUS.md`
  and the active-plans index name the new suite plan, which records `PRV-1-A`
  and `PRV-1-E` and names the application units with their ledgers.

## Procedure

The scanner test was written first and run before the allowlist entries
existed:

```sh
python3 scripts/test-activation-contract.py
```

Exit 1: `FAILED (failures=1)`; the new case reported
`fluorita/src/activation.rs:1`, `fluorita/src/adopt.rs:1` and
`selenita/src/preview.rs:1` as literals outside the shared owner. After the
allowlist entries:

```sh
python3 scripts/test-activation-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
git diff --name-only --cached | python3 scripts/commit_scope.py --check "suite-maintenance: Fix the capture preview's interfaces between Selenita and Fluorita"
```

## Result

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-activation-contract.py` | 0 | `Ran 14 tests`, `OK` |
| `bash scripts/check-architecture-contract.sh` | 0 | `Activation contract: OK`, `Architecture contract: OK` |
| `python3 scripts/check-language-contract.py` | 0 | `Language contract: OK (139 legacy file(s) ratcheted)` |
| `bash scripts/check-documentation-contract.sh` | 0 | `Documentation contract: OK` |
| `python3 scripts/commit_scope.py --check …` | 0 | no output: the subject and the staged paths are in the `suite` scope |

## Limits

- No Rust or QML changes here: the methods exist only as a contract until
  `SEL-2-A` and `FLU-P1-A` serve and call them, each with its own tests.
- Nothing runs live; the author's checks are the application units' VAL
  entries.

## Landing

- **Base revision:** `dfda1326c0658f49aabeb8c936ce0b3a52fcb791`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
