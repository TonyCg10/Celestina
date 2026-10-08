# Evidence: 2026-10-07 renaming and the format backend for removable drives

- **Date:** 2026-10-07
- **Scope:** `SID-H1-O` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11, offscreen QML tests; no device was renamed or formatted
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo fmt --all --check)
(cd siderita && cargo clippy --all-targets --locked -- -D warnings)
(cd siderita && cargo test)
(cd siderita && cargo build --release --locked)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

The UDisks2 calls that change a device (`SetLabel`, `Block.Format`,
`PartitionTable.CreatePartitionAndFormat`) were not run by any test or during
development. The unit tests cover the pure rules in `src/drives.rs`: the label
limits per file system, the partition table by size (MBR up to 2 TiB, GPT
above), the MBR type codes and GPT type GUIDs, `/proc/swaps` parsing, and the
system-drive decision over a fake block tree (system mount points, a swap
partition, a swap file, a root inside LUKS on LVM). The QML test
`tst_sidebar_volume_row.qml` drives the extracted `SidebarVolumeRow` against a
stub controller; `tst_sidebar_menus.qml` gained the device-menu entries.

A review round followed (same commands). Every drive action now names its
volume by device node: the listing is sorted by name and rebuilt on every
hotplug, so a row position could point at another drive by the time a rename
or a format was confirmed. The controller finds the volume by device when the
call arrives, the worker checks that the UDisks2 object still is that device,
and the sidebar keeps the rename (and what was typed) on the device across a
rebuild. The same round makes FAT always FAT32 (`mkfs-args` `-F 32`, refused
below 64 MiB), stores FAT labels in capitals and inside code page 850, starts
the whole-disk partition at 1 MiB, refuses an unreadable or zero size,
treats a drive holding a member of a mounted multi-device filesystem (same
`IdUUID`) as system, re-checks `HintSystem`/`HintIgnore` before acting,
formats a hybrid ISO block (filesystem and partition table at once) as a whole
disk, and lets one drive operation run at a time in the process. New tests:
`whole_disk_of`, the multi-device case, `/usr`, `/var` and `/boot/efi/`,
FAT32 sizes and arguments, CP850, the operation slot, lookup by device after
a reorder and after removal, and two QML cases that rebuild the list in the
middle of a rename.

## Result

- **Exit, first round:** fmt, clippy, the Rust tests (156 passed, 1
  ignored), the release build, the QML tests (183 passed, 0 failed), the
  smoke run, qmllint (239 baseline warnings, unchanged), the architecture
  contract, the language contract and the documentation contract exited 0.
- **Exit, review round:** fmt, clippy, the Rust tests (163 passed, 1
  ignored), the release build, the QML tests (186 passed, 0 failed), the
  smoke run, qmllint (239 baseline warnings, unchanged), the architecture
  contract, the language contract and the documentation contract exited 0.

## Limits

- Renaming and formatting a real stick, and the polkit prompts, are left to
  the author (`VAL-SID-18`). Whether UDisks2 renames a mounted exFAT stick in
  place depends on its version and exfatprogs; the fallback unmounts, renames
  and mounts again.
- `Formatear…` stays hidden (`formatAvailable: false`) until the dialog of
  `SID-H1-P`.

## Follow-up

- `SID-H1-P`: the format dialog.

## Landing

- **Base revision:** `7a06b94888e676a0f91c09df886e7a0475f82de6`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c334fdd822b0ea040c5f40e40c2de7acf17b7f364143bf835b313eab477908b3, verification_fingerprint sha256:10d2af947b66b2698121d3a7f3bccfdb6cf5ee081be0915243e7327851475978
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
