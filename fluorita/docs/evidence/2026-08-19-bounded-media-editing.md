# Evidence: editing the pictures the library already holds

- **Date:** 2026-08-19
- **Scope:** `F7-A` to `F7-D` of the
  [bounded-media-editing plan](../plans/archive/2026-08-19-bounded-media-editing.md):
  `docs/decisions/0009-editing-without-an-encoder.md`, `fluorita/README.md`,
  `celestina-rs/crates/fluorita-core/src/edit.rs`,
  `celestina-rs/crates/fluorita-core/src/edit_stack.rs`,
  `celestina-rs/crates/fluorita-engine/src/edit.rs`,
  `celestina-rs/crates/fluorita-engine/src/edit_store.rs`,
  `fluorita/cpp/imagecanvas.cpp`, `fluorita/src/rasteriser.rs`,
  `fluorita/src/editor.rs`, `fluorita/qml/components/Edit*.qml`
- **Environment:** Arch-based Linux 7.1.8, Qt 6 with the system libmpv, Rust
  1.97 toolchain, `cargo` offline against the committed lockfiles. No Wayland
  session was driven and no window was opened.
- **Artifact:** `fluorita/target/release/fluorita` at version `1.3.0`, built by
  `fluorita/scripts/build-production.sh`, verified and deployed to
  `~/.local/bin/fluorita`; `siderita/target/release/siderita` completed after
  it, because it consumes the same shared crates.

## Procedure

```sh
cargo fmt -p fluorita-core -p fluorita-engine --check   # celestina-rs/
cargo clippy -p fluorita-core -p fluorita-engine --all-targets --locked -- -D warnings
cargo test -p fluorita-core -p fluorita-engine --locked
cargo fmt --check                                       # fluorita/
cargo clippy --all-targets --locked -- -D warnings
cargo test
bash fluorita/scripts/build-production.sh
bash scripts/qmllint-cxxqt.sh fluorita
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** every command above exited zero. `complete-production.sh` was run
  twice: the first attempt failed inside `verify-production.sh` on the shared
  architecture guard, which was refusing
  `grafita/qml/components/EncodingDialog.qml` — a concurrent, uncommitted
  change from another session in the same checkout, in a file nothing here
  touches. Once that session repaired its own file the completion was run again
  and passed, and `siderita/scripts/complete-production.sh` passed after it.
- **Observed:**
  - `fluorita-core`: 115 tests pass, 33 of them new. They cover the capability
    matrix (a JPEG's turn is lossless, a PNG's is not, video and audio admit
    nothing here), the composable stack (a redaction placed at 1200,700 is at
    200,200 after the surrounding crop; undo restores the exact coordinates and
    the accumulated text rotation; a resize scales stroke widths and text sizes
    and undo unscales them), every refusal (crop off the canvas, canvas past the
    host's pixel budget, a canvas with no area, an object entirely off the
    picture, non-finite geometry, the object/stroke/text/history ceilings), undo
    and redo of add, update and remove, the EXIF orientation algebra including
    that mirroring and turning do not commute, and the preview mapping that a
    turn followed by a crop still names an area of the file on disk.
  - `fluorita-engine`: 96 unit tests pass, 21 of them new. The save path is
    proved against the filesystem with a fake rasteriser and a fake bin: a
    quarter turn on a JPEG changes exactly one byte and leaves the entropy-coded
    data untouched; a camera's existing orientation is added to rather than
    overwritten; a file with no EXIF falls back to the renderer and the result
    reports `Raster` rather than claiming the original survived; a truncated or
    lying EXIF segment is refused rather than read past; a copy lands as
    `foto (editado).jpg` and a second copy does not overwrite the first; a
    replacement's destination exists at the moment the bin is asked to move the
    original; a bin that refuses leaves the result written and the original in
    place; a rasteriser failure writes nothing at all; and a cancelled or
    relative-path save never writes. The recipe store round-trips every
    annotation kind including a base whose name is not valid UTF-8, refuses to
    key a recipe on a path, skips and counts corrupted records, and reports a
    recipe as unusable when its base changed size or disappeared.
  - `fluorita`: 53 tests pass. The new ones cover the ink spelling in both
    directions across the seam, the point list, and that stretching something
    with no box leaves it alone.
  - `qmllint-cxxqt.sh fluorita`: OK at the registered baseline of 47 non-fatal
    warnings. The three new QML files add none: the first pass reported 27,
    every one of which was a real defect (`layer` as an id collides with
    `Item.layer`, `GlassSurface` has `cornerRadius` and not `radius`,
    `CelestinaButton` has no `Normal` role, `CelestinaFocusRing` requires
    `target` and `cornerRadius`, and nested delegates need
    `pragma ComponentBehavior: Bound`).
  - `check-architecture-contract.sh` refused the first version of the text
    prompt: a Qt `Dialog` and `TextField` rebuilt outside the shared style. It
    is now `CelestinaModalLayer` + `GlassCard` + `CelestinaTextField`, and
    `CelestinaTextField.qml` is symlinked into `fluorita/qml/` and registered
    like every other shared control.
  - `check-language-contract.py` refused product copy spelled inside
    `editor.rs`; the words moved to `fluorita/src/editor/copy.rs` under the
    file-head marker, as `library/copy.rs` already does. It also refused a
    Spanish line added to `fluorita-engine`'s `user_message`, so the
    half-finished-replacement wording lives in the application's product copy
    and the engine says only what it already said about a failed write.
  - The version transition `1.2.4` → `1.3.0` and its `docs/version-history.tsv`
    row are in place; `python3 scripts/version_tool.py check` reports OK for six
    owners.
  - `fluorita/scripts/smoke.sh`, inside the verification, loads `Main.qml`
    offscreen. That is the first time `FluoritaEditor` and `EditSurface` have
    been *constructed*: the gate reports no QML error and no auto-binding, and
    it still finds that neither the library, nor an image, nor an unknown file
    starts the media backend — so adding an editor did not quietly make
    browsing heavier.
  - Both hosts completed. `fluorita` 1.3.0 and `siderita` are built, verified
    and deployed to `~/.local`; `status-production.sh` reports both artifacts
    current and verified. Siderita completes because it consumes the same
    `fluorita-core` and `fluorita-engine`, and its own 72 QML tests pass with
    them.
  - Handler state after deployment, as the contract requires it be reported:
    `xdg-mime query default video/mp4` is `org.celestina.Fluorita.desktop`, as
    it already was; `image/jpeg` is `gmic_qt.desktop` and this deployment did
    not change it. Editing is therefore reached from inside Fluorita, not by
    the desktop opening a photograph with it.

## Limits

- **Nothing was drawn on a real display.** The rasteriser was exercised only
  through a fake in the engine's tests: no `QPainter` call in
  `cpp/imagecanvas.cpp` has produced a pixel in this evidence, and no picture
  was opened in a window. Perceived drawing, pointer precision, the real display
  scale, the reading of a redaction and what the desktop's Trash actually holds
  are `VAL-FLU-EDIT`.
- **Constructed is not drawn.** The offscreen smoke proves the edit surface
  builds and binds; it never opens a picture, arms a tool or draws a mark.
- **The blur and the pixelation are Qt scaling, not a measured algorithm.** They
  are irreversible, which is what a redaction needs, but no measurement of how
  much of the original survives was taken.
- **Only JPEG carries a lossless turn.** TIFF, WebP, AVIF and HEIF record
  orientation too and are deliberately classified as raster until their
  containers can actually be rewritten.

## Follow-up

- `VAL-FLU-EDIT` in [VALIDATION.md](../../VALIDATION.md): the whole of what a
  person sees, on the author's own display.
- The ledger closure of `F7-A` to `F7-D`, which belongs to the author's commit
  request: none of them has an inventory yet.
