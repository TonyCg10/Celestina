# Immersive content and honest catalogue

- **Opened:** 2026-08-04
- **Plan ID:** immersive-content
- **Status:** done
- **Scope:** fluorita
- **Implementation checkpoint:** F6
- **Author-validation checkpoint:** VAL-FLU-IMMERSIVE
- **Closed:** 2026-08-19
- **Successor:** F7, [bounded-media-editing](../active/2026-08-19-bounded-media-editing.md)

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
| F6-B | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-B.numstat.tsv) | 20 files, +878/-78 | Decide a render context's release from an explicit renderer claim instead of item visibility, so the mpv core is never destroyed under a live context; hold an activation that arrives during a close; stop and join the player on teardown; report a context that failed to build instead of leaving the file opening for ever; carry cancellation into the scan and tag probes; emit the MPRIS property and seek signals; project a watcher refresh under the scope in force and a scan failure over the library already on screen; own a touched file only from its real source; and give the accessible activation the same arguments as the pointer one | `cargo check`, `cargo test`, `cargo fmt` in `fluorita/` and for the `fluorita-*` crates — recorded in [render-context lifecycle evidence](../../evidence/2026-08-05-render-context-lifecycle.md) | `VAL-FLU-TEARDOWN` |
| F6-C | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-C.numstat.tsv) | 24 files, +607/-162 | Carry a file's byte-exact identity across the Qt seam as a percent-encoded path key, decode it back with a typed refusal at every entry point, and keep the lossy text a person reads in its own columns — so a file whose name is not UTF-8 can be opened, described and trashed instead of reporting that it is no longer in the library | `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` and `cargo test --all-targets --locked` in `fluorita/` and `celestina-rs/`, plus `scripts/check-architecture-contract.sh`, `scripts/check-language-contract.py` and `scripts/qmllint-cxxqt.sh fluorita` — recorded in [byte-exact path seam evidence](../../evidence/2026-08-06-byte-exact-path-seam.md) | `VAL-FLU-BYTES` |
| F6-D | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-D.numstat.tsv) | 12 files, +304/-35 | Address the image probe by path key: decode it to bytes with the byte-level call, open the file by descriptor and hand that to the reader, so a picture whose name is not valid UTF-8 is measured on itself instead of being refused as unreadable | `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked` in `fluorita/` — recorded in [image probe evidence](../../evidence/2026-08-06-image-probe-bytes.md) | `VAL-FLU-BYTES` |
| F6-E | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-E.numstat.tsv) | 8 files, +173/-6 | Give each session a generation so a render handle published by a session the player has already left is dropped rather than written over a destroyed instance, and make a close that finds no worker honour an activation parked while the previous one was closing | `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked` — recorded in [session generation evidence](../../evidence/2026-08-06-session-generation.md) | `VAL-FLU-TEARDOWN` |
| F6-Z | `fluorita:` | done | [inventory](../../inventories/2026-08-04-immersive-content/F6-Z.numstat.tsv) | 10 files, +174/-144 | Close F6 and archive this plan, retargeting every link that named it under `active/`, and name F7's plan as the active one | documentation contract and staged-unit guard — recorded in [immersive content evidence](../../evidence/2026-08-04-immersive-content.md) | `VAL-FLU-TEARDOWN` |

One unit because it is one worktree: the defect and the features were found and
fixed against each other, in the same files, and no part of it was ever going to
be committed alone. It closes as `fluorita-milestone`; the vanished-file
correction rides with it rather than pretending to be a separable delivery.

F6-B is a corrective delivery from the suite audit in
[`docs/evidence/2026-08-05-static-suite-audit.md`](../../../../docs/evidence/2026-08-05-static-suite-audit.md):
findings `FLU-C1`, `FLU-A1` to `FLU-A4`, `FLU-M2` to `FLU-M6` and `FLU-B3`. Its
spine is one invariant the code had been inferring instead of holding: the mpv
render context is freed before the core it belongs to. Everything else in the
unit either protects that ordering across close, reopen and exit, or stops a
surface from claiming a state the engine never reported. It stays `active`
because the author asked for the code and its tests without the production
flow, so it has neither an inventory nor a version transition. `FLU-M1` — the
lossy Qt seam for non-UTF-8 names — is deliberately outside it: Siderita has
the same defect at the same boundary, and one shared decision should fix both.

F6-C is that decision applied to Fluorita. The shared ruling is
[ADR 0008](../../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md):
a path crossing the seam is a percent-encoded key and display text is separate.
It is a separate unit from F6-B because it is a separate defect with a separate
cause — F6-B is about the order two teardowns run in, this is about what a path
*is* on the way to QML — and because Siderita's half of the same ADR lands under
its own prefix. It is scoped to Fluorita: `celestina-core` keeps the one codec
both halves compose, and nothing in `siderita-*` is touched here. Like F6-B it
stays `active` with no inventory and no version transition, because the author
asked for the implementation and not the delivery.

F6-D closes the one limit F6-C recorded as inevitable. It was not: a `QString`
loses the byte only if the path is decoded into one, and the probe never had to
do that. Siderita's thumbnail provider carried the identical mistake and was
repaired the same way in `SID-G7-E`, which is what makes this a seam rule rather
than two coincidences.

F6-E closes a window F6-B opened. The shortcut that stopped a track leaving the
player half-closed runs inside the interval where the worker is still publishing
its handle, and the state it left behind — a handle with no worker — made every
later activation a silent no-op. The repair is the rule the suite already
applies to a late save completion: an answer that arrives after the question
changed is not an answer.
