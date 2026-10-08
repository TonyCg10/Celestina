# Evidence: 2026-10-08 the format dialog for removable drives

- **Date:** 2026-10-08
- **Scope:** `SID-H1-P` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11, offscreen QML tests; no device was formatted
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo fmt --all --check)
(cd siderita && cargo clippy --all-targets --locked -- -D warnings)
(cd siderita && cargo test --locked)
(cd siderita && cargo build --release --locked)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

No UDisks2 call that changes a device was run by any test or during
development. `FormatDialog.qml` is driven by `tst_format_dialog.qml` against a
stub controller: it opens on exFAT, quick format on, whole disk off and the
current name; FAT32 writes the typed name in capitals and a name too long for
it shows the reason under the field and disables the format button; the
whole-disk switch moves the warning from the partition to the drive device; the
format button hands the stub exactly the device and filesystem UUID captured on
opening (after a hotplug re-sorted the list) with the chosen file system, name
and switches; a stick unplugged, or replaced under the same device node, or a
drive operation already running disables it; Escape and the cancel button close
without a call; FAT32 below 64 MiB is refused. `tst_sidebar_menus.qml` now
expects the format entry on a removable volume. The Rust tests add `disk_of`
(the whole-disk block under a partition, under an unlocked LUKS volume, and a
stick without a table) and the UUID match the worker applies before a format.

## Result

- **Exit:** fmt, clippy, the Rust tests (165 passed, 1 ignored), the release
  build, the QML tests (195 passed, 0 failed), the smoke run, qmllint (239
  baseline warnings, unchanged), the architecture contract, the language
  contract and the documentation contract exited 0.

## Limits

- Formatting a real stick, the polkit prompt and the running notice during a
  zero-filled format are left to the author (`VAL-SID-18`). The progress shown
  is the indeterminate running notice; the UDisks2 job's progress is not read.
- The busy state the dialog reads is the tab's own; a drive operation started
  from another tab is refused by the controller's process-wide slot instead.

## Follow-up

- None.

## Landing

- **Base revision:** `9aff4955fff033acd26db79c1962f44219f798fc`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:de8c2a5094a009b4ae6255ba83d0eebe79f2d64800c2cd62044e18041f316a65, verification_fingerprint sha256:87a7efaa5b72a8eccba6ccdf63b5935e2bc380b93e91cec60a21eb4136c2a565
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
