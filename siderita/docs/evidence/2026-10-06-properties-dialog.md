# Evidence: 2026-10-06 the properties dialog

- **Date:** 2026-10-06
- **Scope:** `SID-H1-G` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Rust 1.97, release profile tests
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

The author showed the dialog on a folder and asked for the free space of the
volume, fewer rows and a better layout. The design the author approved in
chat: a heading line with the kind and the path, one `Content` box with size,
free space (with a usage bar), modified and permissions with the owner; MIME,
access time and the path row leave.

```sh
cd siderita && cargo fmt --check
cargo test --release --locked --bin siderita properties::
cargo clippy --release --locked --bin siderita
cargo build --release --locked --bin siderita && target/release/siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** fmt clean; 7 `properties::` tests pass (two new: the root volume
  reports a bounded free space; a missing path has no volume); clippy clean
  for the crate (the Qt header noise is the C++ build's); every guard OK.
- **Observed:** `properties::volume` asks `rustix::fs::statvfs` for
  `f_bavail × f_frsize` and `f_blocks × f_frsize`; the controller publishes
  them as `prop_volume_free` and `prop_volume_total` in bytes, and the dialog
  formats them with the folder-usage `bytesText` and draws the used share as
  a `compLinearTrackHeight` track in `divider` with an `accent` fill.
  `prop_mime` and `prop_accessed` are removed from the controller, which keeps
  `controller.rs` at its 753-line baseline. `PropRow` rows are `rowHeight`
  tall at `fontBody` with tabular numerals and a `detail` slot under the
  value. The author relaunched the build on the session and judged it by eye.
- **Dependency:** `rustix` 1.1.4 (`std`, `fs`) added to `siderita/Cargo.toml`
  with its justification; `Cargo.lock` gains only the dependency edge, the
  crate was already in the closure through `hematita-core`.

- **qmllint:** the first landing stopped at `warnings grew from 257 to 262`:
  the dialog referred to its own `controller`, `panel` and `owner` without
  qualification, 18 times before this unit and 23 after. Every access is now
  `propertiesView.`-qualified, the dialog lints clean, and
  `scripts/qmllint-baseline.tsv` falls from 257 to 239 for Siderita.

## Limits

- `gather` runs on the Qt thread as before; `statvfs` on a stalled network
  mount would block like the `symlink_metadata` beside it. Moving the whole
  gather to the usage worker is a separate unit if a mount ever shows it.
- No Qt Quick test instantiates the dialog; `tst_folder_usage.qml` covers
  the occupation section below it.

## Follow-up

None.

## Landing

- **Base revision:** `8754e6cd028f00f089dead97c29f35ebec1c1b47`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3210b2bc2880ff7e7aeb09ee525c174f0a6a0d9ae1bef6dfa2a979f974efb2fc, verification_fingerprint sha256:a279175d63a1952eab36164a2f0a7f73ab0d33d1830c34bb56ca992d8cdfce1f
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
