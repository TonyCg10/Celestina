# Fluorita implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none
- **Related author validation:** none; completed observations are in
  [VALIDATION.md](VALIDATION.md)

## No active implementation checkpoint

Fluorita 1.0 is implemented and its F0-F4 arc is closed. Do not keep a version
or milestone open for optional comforts, repeat completed perceptual tests in
this file or treat a manual check as unfinished implementation.

## Conditions for opening the next checkpoint

A new checkpoint must begin with a measured user need, a bounded resource and
lifecycle model, and a tangible result. Current ideas remain conditional:

- trailer-on-hover requires a defined trigger/cancel interaction and proof that
  one active trailer per host stays within budget;
- subtitles/tracks/speed or queues/playlists require an accepted product slice,
  not opportunistic controls;
- shell MPRIS presentation requires a shell-owned checkpoint and preserves
  Fluorita as the single confirmed playback source;
- presentation-timing work requires reproducible judder and must account for
  Qt owning the final frame swap.

None authorises a streaming catalogue, codec rewrite, global filesystem crawl
or duplication between standalone and Siderita.

## Implementation exit

For any newly authorised checkpoint, close implementation when focused
core/engine tests pass, both hosts are verified where the shared contract is
affected, lifecycle/resource bounds are automated, and
`scripts/verify-production.sh` passes against the artifact made by
`scripts/build-production.sh`. Real playback or perceptual acceptance belongs
to `VALIDATION.md` and never keeps implementation open.

## Closed evidence

The completed F0-F4 implementation, backend spike, fixes, measurements and
real-session observations are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
