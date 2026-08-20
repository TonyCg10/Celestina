# Evidence: what shipped, and the two things nothing could reach

- **Date:** 2026-08-20
- **Scope:** `F7-B` and `F7-C` of the
  [bounded-media-editing plan](../plans/archive/2026-08-19-bounded-media-editing.md):
  `fluorita/README.md`, `fluorita/ROADMAP.md`, `fluorita/STATUS.md`,
  `fluorita/docs/plans/`, `fluorita/qml/Main.qml`,
  `fluorita/qml/components/PlayerTransport.qml`,
  `fluorita/qml/components/PlayerSurface.qml`
- **Environment:** Arch-based Linux, Qt 6 with the system libmpv, Rust
  toolchain, `cargo` offline against the committed lockfiles. No Wayland
  session was driven and no window was opened.
- **Artifact:** `fluorita/target/release/fluorita` at version `1.3.1`, built by
  `fluorita/scripts/build-production.sh`, verified and deployed to
  `~/.local/bin/fluorita`. `F7-B` carries no version: documents are not a
  product change. `F7-C` moves the exact PATCH transition `1.3.0` to `1.3.1`,
  because a capability nobody can reach is a defect and not a feature.

## Procedure

```sh
bash fluorita/scripts/build-production.sh
bash scripts/qmllint-cxxqt.sh fluorita
bash fluorita/scripts/smoke.sh
cargo test                                              # fluorita/
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash fluorita/scripts/complete-production.sh
```

## Result

- **Exit:** every command exited zero. The completion built, verified and
  deployed the artifact; `status-production.sh` reports it current and
  verified.
- **What the audit found.** Before writing a line of documentation, every
  capability the previous commit claimed was checked against the interface that
  should reach it. Two did not have one: `extract_frame` and `toggle_pacing`
  were implemented, tested and shipped in `1.3.0` with nothing in QML calling
  either, so a frame could not be kept and the pacing read-out in the player
  surface could never be turned on. This is the inverse of the defect the same
  release fixed for the item menu — there, a surface offered what it could not
  do; here, the application could do what nobody could ask for.
- **`F7-C`.** The frame now has a button in the transport, visible only for a
  moving picture and only while one is open, and the transport is given the
  open item's key so a verb that acts on the *file* rather than on the session
  can name it. The capture has `Ctrl+Shift+P`, and writing its report has
  `Ctrl+Shift+S` — shortcuts rather than controls, because a diagnostic that
  lives in the transport becomes furniture. `qmllint` holds at its registered
  baseline and the offscreen smoke constructs both surfaces.
- **`F7-B`.** The roadmap named F7 as the active checkpoint while its work was
  already delivered, and described F8, F9 and F10 as *conditional ideas* inside
  the section that lists what has not been opened — when all three were in the
  shipped binary. F11 to F15 were not described at all. The roadmap now closes
  F7, states that its single commit carried F8-F15 with it, promotes those
  sections to delivered checkpoints and adds the five that were missing. Its
  conditions list, which claimed trailer-on-hover, subtitles and
  presentation-timing were unopened, now lists what is actually still shut: the
  encoder decision, three tag containers, a captured judder and a shell-owned
  checkpoint.
- **The user contract.** `README.md` promised only that editing acts on the
  media the library holds. It now states the metadata a person can correct and
  remove, the frame, the tracks, the speed, what plays next, the zoom, the
  hover preview and the pacing report — with the refusals in the same place, so
  the document that promises is the document that limits.

## Limits

- **Neither trigger has been pressed by a person.** The button is constructed
  offscreen and the shortcuts are bound; that a frame really lands beside a
  film, and that the capture reads what a stutter looks like, is
  `VAL-FLU-PACING` and the frame half of `VAL-FLU-EDIT`.
- **The documentation is an account, not a proof.** It was written from the
  code and from what the audit above could reach; a capability that exists,
  works and was missed by both would still be missing from it.

## Follow-up

- `VAL-FLU-PACING` in [VALIDATION.md](../../VALIDATION.md).
- The encoder decision, which is the author's and gates trimming, dropping a
  track, converting a format and exporting a clip.
