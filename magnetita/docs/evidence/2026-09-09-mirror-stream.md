# The mirror's stream in the core and the peer — MAG-P6-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P6-A` of
  [`../plans/archive/2026-09-09-link-mirror.md`](../plans/archive/2026-09-09-link-mirror.md):
  `celestina-rs/crates/magnetita-mobile/src/{phone,mobile}.rs`,
  `celestina-rs/crates/magnetita-peer/src/{lib,main}.rs`,
  `docs/protocol.md`, this record
- **Environment:** the workspace's tests; `ffmpeg` on this host
- **Artifact:** `magnetita-peer`; the `magnetita-mobile` bindings the
  application builds

## Design

- `open_stream(id)` opens a bulk stream prefixed by a fixed transfer id
  and `close_stream(id)` finishes it without a share message; the video
  travels on `0xFFFF_0001`, the reserved audio on `0xFFFF_0002`, which
  the wire document now names.
- `send_mirror_started` and `send_mirror_stop` are the phone's answers;
  the desktop's start, stop, touch, key and global arrive in `Event` as
  small records the application maps onto its signals.
- The peer's `--mirror-file PATH` answers a start with a fixed shape and
  paces the file's bytes at 32 KiB every 8 ms down the video stream, the
  way the phone's encoder would its frames.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetita-mobile -p magnetita-peer
ffmpeg -f lavfi -i testsrc2=size=664x1440:rate=60 -t 2 -c:v libx265 -x265-params keyint=60:bframes=0 -f hevc synthetic.hevc
ffmpeg -f hevc -c:v hevc -i pipe:0 -c copy -f nut pipe:1 < synthetic.hevc | ffmpeg -f nut -i pipe:0 -f null -
```

## Result

- **Exit:** 0. The synthetic HEVC stream (846 KiB, 120 frames) passes the
  daemon's remux and decodes; the peer builds with the new flag.

## Limits

- A stream the phone's encoder produced is decoded on the author's desk
  (`VAL-MAG-14`); the synthetic one proves the pipeline, not the codec
  the S25U chooses.
