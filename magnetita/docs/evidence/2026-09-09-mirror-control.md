# The Mirror control prefers the link — MAG-P6-D

- **Date:** 2026-09-09
- **Scope:** `MAG-P6-D` of
  [`../plans/active/2026-09-09-link-mirror.md`](../plans/active/2026-09-09-link-mirror.md):
  `src/devices.rs`, this record
- **Environment:** the workspace's tests
- **Artifact:** `magnetita`, deployed

## Design

`mirror_start` calls `StartLink` first and falls back to `Start` when the
daemon has no such method; `mirror_stop` withdraws both. The snapshot the
card shows reads `LinkState` and lets `starting` and `streaming` stand in
for the `adb` path's `connecting` and `mirroring`, so the card and its
QML change nothing.

## Procedure

```sh
cargo test && cargo clippy --all-targets -- -D warnings
```

## Result

- **Exit:** 0, 14 tests.

## Limits

- The link mirror opens its own `mpv` window; the card's stop closes it.
