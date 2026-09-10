# Input back through Mirror1 — MAG-P6-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P6-C` of
  [`../plans/active/2026-09-09-link-mirror.md`](../plans/active/2026-09-09-link-mirror.md):
  `celestina-rs/crates/magnetitad/src/mirror.rs`, this record
- **Environment:** the workspace's tests
- **Artifact:** `magnetitad`, deployed

## Design

`org.celestina.Mirror1` grows additively: `StartLink` maps the existing
options (resolution, rate, quality, audio) onto the wire's start,
`StopLink` withdraws the intent, `LinkState` is the word the daemon's
intent holds, and `LinkTouch(action, x, y, pointer)`, `LinkKey(keycode,
pressed)` and `LinkGlobal(Back|Home|Recents)` queue messages the owning
session sends on its tick. The old `Start`, `Stop` and `State` stay as
they were for the `adb` path.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad the_options_map the_link_mirror
```

## Result

- **Exit:** 0. The options map onto the wire and the queued touch reaches
  the phone's side of the loopback.

## Limits

- Keys reach the phone but the accessibility service cannot inject them;
  `AND-4` honours touches and the three global actions.
