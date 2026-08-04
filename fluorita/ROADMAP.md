# Fluorita implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none
- **Related author validation:** none; completed observations are in
  [VALIDATION.md](VALIDATION.md)

Fluorita 1.0 is implemented and its F0-F4 arc is closed. Do not reopen it,
repeat completed perceptual tests in this file or treat a manual check as
unfinished implementation.

## F5 — Source-first library and direct activation

**Measured need.** The author specified the standalone library as a sidebar of
the folders they map, each showing the supported content inside it. The shipped
surface is two fixed kind tabs over one flat catalogue: the mapped roots are
modelled in `fluorita-core` and carried on every record, but the projection
discards that column, so the axis the user configured is the one axis the
interface cannot navigate. Separately, a single click on an item selects it and
does nothing else — opening requires a double click — so the library reads as
if activation were broken. The contract change this needs is
[ADR 0006](../docs/decisions/0006-source-first-library-navigation.md),
delivered by suite checkpoint ACT-1.

**Bounded resource and lifecycle model.** No new decode, scan or watch
behaviour. Source configuration is a small persisted file read once per launch
and rewritten whole on change, guarded by the suite's atomic replacement, in
the same bounded percent-encoded format the catalogue store already uses.
Choosing a folder is one bounded portal request off the GUI thread; a desktop
with no portal degrades to a stated failure, not a hang. Adding or removing a
source reuses the existing scan and watch path instead of introducing a second
one, and removing one never touches a file on disk.

**Tangible result.** Launching Fluorita shows a sidebar of the mapped folders
with a button that opens the desktop folder chooser and keeps the chosen folder
as a persistent entry. Selecting a folder shows the supported content inside
it: the Gallery grid for a source contributing images or video, the Music
projection for one contributing audio. A single click opens the item.

Implementation is complete and both registered completion commands passed; the
[evidence](docs/evidence/2026-08-04-source-first-library.md) records where the
verified bytes were built and why. What remains is the ledger closure, which
belongs to the author's commit request.

- [x] Make configured sources user-owned and persistent, with stable identity
      across runs and first-run seeding from the existing XDG media
      directories.
- [x] Publish the sources to QML, resolve the selected source's projection, and
      add or remove a folder through the desktop file-chooser portal without
      blocking the GUI thread.
- [x] Replace the two kind tabs with the source sidebar and its adaptive
      content panel, and activate an item on a single click.

The build order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-08-04-source-first-library.md).

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
core/engine tests and lifecycle/resource bounds pass and every affected
deployable host completes its registered production flow. Fluorita-only work
uses `fluorita/scripts/complete-production.sh`; a shared `fluorita-core`,
`fluorita-engine` or `fluorita-qt` change also uses
`siderita/scripts/complete-production.sh`. Each command builds its release once,
verifies those exact bytes and deploys them to the author's normal test
destination. Real playback or perceptual acceptance belongs to `VALIDATION.md`
and never keeps implementation open.

## Closed evidence

The completed F0-F4 implementation, backend spike, fixes, measurements and
real-session observations are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
