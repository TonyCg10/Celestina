# The adb mirror picture retired — MAG-P7-D

- **Date:** 2026-09-13
- **Scope:** `MAG-P7-D` of
  [`../plans/archive/2026-09-10-storage-and-retirement.md`](../plans/archive/2026-09-10-storage-and-retirement.md):
  `celestina-rs/crates/magnetitad/src/mirror.rs`, `ROADMAP.md`, this record
- **Environment:** the workspace's tests; the author's session
- **Artifact:** `magnetitad`, deployed by `scripts/complete-production.sh`

## Design

`VAL-MAG-14` passed on the author's daily use, so the second mirror goes:
the daemon no longer spawns a scrcpy picture. `Mirror1`'s `Start` and
`Stop` keep their names for the shell's plugin and the application and
now do what `StartLink` and `StopLink` do. The adb worker stays for one
thing the link cannot do: turning the phone's screen off with the phone
unlocked, through a control-only scrcpy the author asked for; it dials
the phone where the link sees it, then the remembered or advertised
endpoint.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad && cargo clippy -p magnetitad --all-targets -- -D warnings
magnetita/scripts/complete-production.sh
```

## Result

- **Exit:** 0; 81 tests. The retired path's worker commands, its scrcpy
  child and its exit polling are gone; `MirrorCommand` keeps pairing, the
  options and the screen.

## Limits

- The screen off needs wireless debugging on the phone, paired once; the
  author turned it on for it.
