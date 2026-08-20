# Evidence: 2026-08-06 the byte-exact path seam between Fluorita and Qt

- **Date:** 2026-08-06
- **Scope:** `F6-C`; plan
  [immersive-content](../plans/archive/2026-08-04-immersive-content.md);
  [ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md);
  suite audit finding `FLU-M1` from
  [`../../../docs/evidence/2026-08-05-static-suite-audit.md`](../../../docs/evidence/2026-08-05-static-suite-audit.md).
  Fluorita only — Siderita's half of `SID-A2` is a separate unit and was not
  touched
- **Environment:** Arch-based Linux, Qt 6, source changes with formatting, lint,
  unit tests and QML lint. No production build, no deployment, no version
  transition and no window opened on the live session — the author asked for the
  implementation, not the delivery
- **Artifact:** none; no production build ran

## What was wrong

`fluorita/src/library/project.rs` published every gallery and music row's path
with `item.path.to_string_lossy()`. `fluorita-core` stores those bytes exactly —
the catalogue is percent-encoded on disk and every record carries a raw
`PathBuf` — so the loss happened only in the projection towards QML, and every
byte that is not valid UTF-8 became U+FFFD. `library.rs`'s `describe_item` and
`trash_item` then rebuilt a `PathBuf` from the `QString` that came back, so a
file named `na\xffme.png` listed in the grid and answered the item-is-gone
notice (`copy::ITEM_GONE`) to both verbs. `player.rs::open` had the same shape,
as did two values that cross the same `QString` seam inside the process: the folder
the portal returns (`run_folder_choice`) and the path a finished trash move
reports (`run_trash`), so a chosen directory whose name is not UTF-8 was mapped
as a root the scan finds nothing in.

## What changed

- **`fluorita/src/pathkey.rs`** is new: `encode` composes
  `celestina_core::percent::encode(percent::path_bytes(path))`, and `decode`
  refuses a value with the typed `PathKeyError` (`Empty`, `NotAscii`,
  `Malformed`, `NotAbsolute`) instead of salvaging it. It adds no codec — the
  suite's single implementation stays in `celestina-core` — it names the
  composition and the refusal so both are in one place.
- **`library/project.rs`** publishes `pathkey::encode` for the gallery and music
  path columns. The display name, the artist, the album and the sidebar's
  location stay lossy and stay in their own columns.
- **`library.rs`** renames the published columns to say what they carry —
  `gallery_keys`, `music_keys`, `source_locations`, `detail_location` — and
  `describe_item`, `trash_item`, `item_trashed` and `folder_chosen` all decode.
  A new unpublished `described: Option<PathBuf>` records which item the
  properties panel is about, so a trash that removes it closes the panel by
  bytes rather than by the label two different files can share.
- **`library/work.rs`** hands the key back from `run_trash` and
  `run_folder_choice`; **`library/detail.rs`** renames `ItemDetail::path` to
  `location` so the display field cannot be mistaken for an argument.
- **`player.rs::open`** decodes and, on refusal, publishes an error with the new
  `copy::UNREADABLE_KEY` rather than opening whatever the characters spell.
- **`main.rs`** publishes `requestedKey` instead of a lossy `requestedPath`.
- **QML** stops composing anything from a path. `Main.qml` derived the window
  title with `path.substring(path.lastIndexOf("/") + 1)`; the activation signal
  now carries the row's `name` beside its `key`, so the label comes from the
  column that already holds it. `GalleryGrid`, `MusicList`, `ContentDock`,
  `ContentNavigator`, `LibraryView`, `LibrarySidebar` and `ItemMenu` follow the
  same split: `key` for identity and every verb, `name`/`location` for reading.

Deliberately unchanged: `fluorita-core/src/artwork.rs` keeps
`percent::encode_qt_path`, because the freedesktop thumbnail cache key must
match Qt's own spelling byte for byte and is not a seam value; `activation.rs`
and `folders.rs` already decoded by bytes; the engine still addresses files by
descriptor.

## Procedure

```sh
cd fluorita && cargo fmt --all --check \
  && cargo clippy --all-targets --locked -- -D warnings \
  && cargo test --all-targets --locked
cd celestina-rs && cargo fmt --all --check \
  && cargo clippy --all-targets --locked -- -D warnings \
  && cargo test
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/qmllint-cxxqt.sh fluorita
```

## Result

- `fluorita`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` clean; `cargo test --all-targets --locked` — **47
  passed, 0 failed**.
- `celestina-rs`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` clean; `cargo test` — every package **0 failed**
  (largest suites 155, 98, 83, 75, 75, 64 passing).
- `bash scripts/qmllint-cxxqt.sh fluorita` — reported OK for
  `org.celestina.fluorita` with the 53 non-fatal baseline warnings and no new
  one; no unresolved property or member among the renamed columns and signals.
- `python3 scripts/check-language-contract.py` — no Fluorita finding. Two test
  fixture paths first written with accented Spanish directory names were
  replaced with English ones rather than marked as exceptions; the codec is
  exercised by a space and by the `\xff` byte, not by an accent.
- `bash scripts/check-architecture-contract.sh` — sealed colour, contrast and
  QML visual contracts OK; no Fluorita finding.

New tests, all in the unit they cover:

- `pathkey`: an ordinary path round-trips; `b"/home/toni/na\xffme.png"` encodes
  to the ASCII `"/home/toni/na%FFme.png"`, decodes back byte-for-byte and is not
  equal to its lossy spelling; each of the four refusals is returned for its own
  malformed shape and each carries a distinct sentence.
- `library::project`: `a_name_that_is_not_utf8_is_published_and_stays_resolvable`
  builds a catalogue holding a picture and a track named `na\xffme.*` under a
  configured root and asserts (a) both appear in the projection, (b) each
  published key decodes to the identical `PathBuf`, and (c)
  `Catalogue::find_by_path` resolves the record from the published key — which
  is exactly what `describe_item` and `trash_item` ask.
  `a_key_that_this_process_did_not_emit_is_refused_without_panicking` covers the
  malformed, empty and replacement-character cases.

## Limits

- Static and unit-level only. No production build, no deployment and no run on
  the author's session: nobody has yet right-clicked a file whose name is not
  UTF-8 in a real window. That is `VAL-FLU-BYTES` in
  [VALIDATION.md](../../VALIDATION.md).
- **A non-UTF-8 image was refused rather than displayed. Closed by `F6-D`.**
  This unit skipped the probe for a path it could not spell, which reported the
  picture unreadable on file size alone — the same visible outcome as before,
  but deliberate, and without the hazard of measuring a *different* file that
  happens to exist under the lossy name. `F6-D` removed the limit instead:
  `imageprobe` now takes the key, decodes it to bytes and opens the file by
  descriptor, so such an image is measured on itself.
- **The path key lived in the Fluorita adapter, not in `celestina-core`. Closed:** ADR
  0008 is a suite rule and its natural home is beside the codec it composes, but
  `celestina-core` is outside this unit's authorised scope and Siderita's half
  of the same defect is being implemented separately. `pathkey.rs` is a
  two-function composition over the existing single codec, not a second recipe;
  when Siderita's seam lands, the second consumer is what the reuse rule asks
  for before extracting it upward.
- QML lint proves registration and resolution, not behaviour: the renamed
  signals and columns are checked to resolve, and no offscreen surface smoke was
  run for this unit.
