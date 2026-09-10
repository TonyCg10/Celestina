# The daemon's mirror and its window — MAG-P6-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P6-B` of
  [`../plans/active/2026-09-09-link-mirror.md`](../plans/active/2026-09-09-link-mirror.md):
  `celestina-rs/crates/magnetitad/src/link_wire/mirror.rs`, the session's
  mirror branches and reader tasks in `link_wire/mod.rs`, the plan and
  roadmap records that archive `MAG-P5` and open `MAG-P6`, `VAL-MAG-14`,
  this record
- **Environment:** the workspace's loopback tests
- **Artifact:** `magnetitad`, deployed

## Design

- `OwnMirror` is the daemon's one intent: what the desktop wants, the
  state (`idle`, `starting`, `streaming`, `failed`), the window's sink and
  the input queued for the phone. The session that ticks first after a
  request owns it; other sessions leave it alone, and only the owner's
  `MirrorStarted`, video stream and end touch it.
- The session's tick sends `MirrorStart` when wanted and idle, `MirrorStop`
  when unwanted and up, and drains the input queue. The bulk stream of
  id `0xFFFF_0001` is pumped into the window as it arrives.
- `DesktopPlayer` opens the window: `ffmpeg` remuxes the raw stream into
  NUT on a pipe and `mpv` plays it low-latency with the session's display
  variables; without them there is no window and the state says `failed`.
- The session loop reads the control stream, the datagrams and the bulk
  streams on their own tasks feeding channels, so a `select!` can no
  longer drop a half-read frame or a half-accepted stream.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
```

## Result

- **Exit:** 0, 106 tests (one ignored). The loopback test requests the
  mirror, sees the start on the phone's side, answers `MirrorStarted`,
  streams 100 000 bytes on the fixed id, finds them whole in the recording
  window, gets a queued touch on the phone's side, and sees the stop close
  the window.

## Limits

- The window is not opened by any test: `mpv` needs the author's display.
- One mirror at a time, the daemon's own choice.
