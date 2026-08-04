# Immersive content and honest catalogue

- **Opened:** 2026-08-04
- **Plan ID:** immersive-content
- **Status:** active
- **Scope:** fluorita
- **Implementation checkpoint:** F6
- **Author-validation checkpoint:** VAL-FLU-IMMERSIVE

## Hypothesis

Opening an item can be a movement rather than a jump, and the folder can be
navigated from inside it, without any new decode: the thumbnail the card is
already showing is enough to grow, to light the room and to hold the frame
while the real picture arrives.

## Tangible outcome

The library lists what exists. An item grows out of the card that was clicked
and shrinks back into it. Right-click offers Trash and Properties. A picture
gets a filmstrip of the rest of the folder on approach; a video or a track gets
previous and next, and the arrow keys seek wherever the focus happens to be.
Whatever is open lights the space it does not fill.

## Scope

- Forgetting records a completed scan of a reachable root did not find, and the
  per-root reachability the scan must report for that to be decidable.
- The open and close transition, and the poster that travels with it.
- Trash and Properties on an item, through `GlassContextMenu` and
  `CelestinaModalLayer`.
- The filmstrip, the side arrows, and the single navigator both read.
- Ambient light from the item's own artwork, and the track cover the projection
  must publish for music to have any.
- Window-level seeking, matching the volume keys that already work that way.
- The surface's return to Spanish under ADR 0007.

## Exclusions

- Live ambient light sampled from the video surface. The poster is the film's
  own frame and costs nothing; sampling a playing surface is a measured
  performance question nobody has asked yet.
- Wrapping at either end of a folder. Arriving back at the first item after the
  last reads as the application having lost your place.
- Filtering navigation by kind. Arrows walk the whole folder in projection
  order; if a video's neighbour is a picture, the surface changes accordingly.
- Any embedded Siderita surface change, and any new decode, probe or crawl.

## Build order

1. **Reachability and forgetting.** `ScanOutcome` reports the roots that
   answered; `Catalogue::forget_vanished` drops a missing record only when its
   root did. The watcher forgets a file it saw removed in a root it is watching,
   which is the same evidence.
2. **The transition.** The activation signal carries the card's scene rectangle,
   its poster and its kind. The window grows a frame between the two, hands over
   only once a picture is really on screen, and closes the session when the
   frame lands.
3. **Item actions.** Trash on a worker through `siderita-ops`, with the record
   removed only on confirmation; Properties from what the catalogue already
   knows.
4. **Navigation.** One `ContentNavigator` owns the folder and the position; the
   filmstrip and the arrows render it.
5. **Ambient light.** The artwork, cropped, blurred and dimmed, under
   everything; the video sized to the film so it stops painting black over it.
6. **Spanish.** `library/copy.rs` owns the adapter's product copy; the surface's
   `qsTr()` strings return to Spanish.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
cargo fmt --check
cargo clippy --all-targets
cargo test                   # fluorita-core, fluorita-engine, fluorita
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

`fluorita-core` and `fluorita-engine` change, so Siderita completes too. The
offscreen smoke runs in both the normal and the reduced-motion configuration,
because a transition that ends the playback session must also end it when there
is no animation to wait for.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| F6-A | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-A.numstat.tsv) | 36 files, +2306/-357 | Stop showing deleted files, open and close items as a movement, act on them, navigate the folder from inside it, and light it with its own artwork — with the surface back in Spanish | [evidence](../../evidence/2026-08-04-immersive-content.md) | `VAL-FLU-IMMERSIVE` |

One unit because it is one worktree: the defect and the features were found and
fixed against each other, in the same files, and no part of it was ever going to
be committed alone. It closes as `fluorita-milestone`; the vanished-file
correction rides with it rather than pretending to be a separable delivery.
