# Evidence: the edit window's size and Selenita's adoption rule

- **Date:** 2026-10-10
- **Scope:** `FLU-P2-A` — `fluorita` (the author's first live use of the
  floating editor of `FLU-P1`)
- **Environment:** CachyOS, Qt 6.12.0, Rust toolchain of the workspace; a
  session worktree opened on `9fc57abf` (Fluorita 1.7.0) and rebased onto
  `69baa0f6`, where `SEL-3-A` gave `selenita_core::names` both folder names
  (`CAPTURES_FOLDER`, `RECORDINGS_FOLDER = "Grabaciones"`) and
  `selenita_core::history::FolderPaths`; offscreen only: no window was shown
  on the author's session
- **Artifact:** the session's release build at
  `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita`; the
  landing builds, verifies and deploys the registered binary

## Change

- `fluorita/qml/EditWindow.qml`: the window no longer opens at a fixed
  960 × 680 with a 480 px minimum. `barWidth` is the bottom bar's whole width
  with the surface's gutter (`CelestinaTheme.spaceLg`) on either side,
  rounded up to a whole pixel: `EditSurface.wholeBarWidth` for a picture,
  `TrimSurface.wholeBarWidth` once the editor decides the file is a video.
  The window opens at `max(1280, barWidth)` × `max(800, minimumHeight)`,
  each capped at nine tenths of `availableWidth`/`availableHeight` (read
  from the window's `Screen`; a test gives its own), and `minimumWidth` is
  `min(barWidth, nine tenths of the screen)`. The opening size is a
  declarative binding, so it is set before the window is first shown, and
  `Component.onCompleted` assigns it once, which ends the binding: a later
  change of screen or of a label's length never undoes the person's own
  resize. `minimumHeight` stays 360.
- `fluorita/qml/components/EditToolbar.qml`: the tools sit in a horizontal
  `Flickable` (`toolbarStrip`) inside the pill's own padding. While the
  toolbar has its implicit width the strip is exactly as wide as the tools,
  not interactive and not clipped, so the wheel passes to the picture as
  before. Given less (`cut`), it clips, drags, takes a plain wheel turn
  sideways (`WheelHandler`), and scrolls a tool reached by Tab into sight
  with room for its focus ring (`Window.activeFocusItem`, a `while` walk up
  the parents, no `for` loop in a handler); the scrolled tools are padded by
  `CelestinaTheme.borderFocus` at either end, so a ring is never clipped.
- `fluorita/qml/components/EditSurface.qml`: `wholeBarWidth`, and the
  toolbar's width is `min(implicitWidth, surface width − two gutters)`, so
  it scrolls instead of overflowing in the library's narrow editor too.
- `fluorita/qml/components/TrimSurface.qml`: `wholeBarWidth`, the wider of
  the three times over the bar and the row of actions (`trimActions`), with
  the two gutters.
- `fluorita/src/adopt.rs`: the private rule (`kind_of`, `same_folder`) and
  Fluorita's own `CAPTURES_FOLDER = "Capturas"` are gone; no folder name is
  spelled in `fluorita/src`. The offline append calls
  `selenita_core::history::adopted(copy, folders.folders(),
  SystemTime::now())`, the function Selenita's own `Adopt` calls, over the
  `FolderPaths` that `session_folders` builds: `FolderPaths::inside(pictures,
  videos)`, Selenita's own constructor, when the session names both folders
  (`selenita_core::names::pictures_dir` and `videos_dir`), so both folder
  names are `selenita_core::names`'s. When it names only one, that one's
  adopting folder (`captures_dir_in` or `recordings_dir_in`) stands in for
  the other, so the unnamed one adopts nothing; with neither, nothing is
  adopted. Selenita differs there: with no pictures or videos folder named
  it falls back to `$HOME` (then the temporary folder) and adopts inside
  it; Fluorita does not guess, so such a copy joins the history only
  through a running Selenita. `Answer`, the worker, the shutdown and the
  rule that the history file is written only when nobody owns Selenita's
  name or the bus is unreachable are unchanged.

## Procedure

The QML size tests were written against the new window and first failed on
real defects: the window's `contentItem` takes its size a turn after it is
shown (the tests now wait for it), and `minimumWidth` is an integer, so a
fractional bar width was a fraction short of its bar (now rounded up).
Then, from the worktree:

```sh
(cd fluorita && cargo fmt --all --check)
(cd fluorita && cargo clippy --all-targets --locked -- -D warnings)
(cd fluorita && cargo test --locked)
(cd fluorita && cargo build --release --locked)
(cd celestina-rs && cargo test -p fluorita-core)
sh fluorita/scripts/qml-tests.sh   # three times
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
```

## Result

- **Exit:** every command above exits 0. Fluorita's crate: 117 tests (114
  before; the adoption tests went from eight to eleven: the folders are
  Selenita's own names, Selenita's rule decides what the append keeps, a
  session naming neither folder keeps nothing, and a session with only a
  videos folder adopts into its recordings folder alone); `fluorita-core`:
  174; QML: 35 of 35 (29 before, six new), three runs out of three; the
  smoke passes, `--edit` included; `qmllint` stays at 56 for `fluorita`;
  the architecture, language, documentation and activation contracts pass.
- **Observed:** offscreen, the picture toolbar's implicit width is 1003 px,
  1035 px with the two gutters, which the old 960 px window could not hold.
  On a 2560 × 1440 screen the window opens 1280 × 800 with a minimum of
  1035 and the toolbar at its implicit width, not cut, its «Cerrar» ending
  inside the window; on a 1300 × 860 screen it opens 1170 × 774 (nine
  tenths). A window made narrower than the toolbar while the screen's cap
  lowered the minimum is widened back to 1035 the moment the whole bar's
  minimum applies again. On a screen whose nine tenths fall short of the
  toolbar, the minimum is those nine tenths, the toolbar is cut, clips and
  is interactive, a wheel turn moves it sideways, focusing «Cerrar» scrolls
  it into sight, and clicking it closes the window. There the scrolled tools
  are padded by `CelestinaTheme.borderFocus` at either end and a tool is
  scrolled in with that margin, so the first and the last tool, reached by
  Tab, show their whole focus ring inside the clipped strip (with no padding
  the test fails on the first tool's ring). A film's window opens at
  least 1280 × 800 with the trim's controls whole and its minimum equal to
  them (rounded up); moving the span changes that minimum but not the
  window's size, nor does a change of the screen it reads. No folder name
  is spelled in `fluorita/src`: the tests build the folders through
  `names::captures_dir_in` and `names::recordings_dir_in`, and check that
  `session_folders` names them `names::CAPTURES_FOLDER` and
  `names::RECORDINGS_FOLDER` (now «Grabaciones»).
- **Owner and reuse:** the adoption rule has one owner,
  `selenita_core::history::adopted`, the folders one constructor,
  `FolderPaths::inside`, and the folder names one owner,
  `selenita_core::names`; Fluorita only finds the session's pictures and
  videos folders.
  Taking Selenita's rule changes three edge cases of the offline append to
  what a running Selenita already did: a folder reached through a link no
  longer matches its other spelling (Selenita compares the parent as
  spelled), a copy that is itself a link is left out, and the extension, not
  the folder, decides the kind (a `.png` in the recordings folder is a
  screenshot); the entry's time is the moment of adoption, as Selenita's
  `Adopt` writes it, instead of the file's modification time.

## Limits

- The refusal of an interactive resize is the compositor's: Qt hands the
  minimum to niri through the toplevel's size hints, which the offscreen
  platform ignores (it says so: `This plugin does not support
  propagateSizeHints()`), and it takes a program's resize as given. The
  tests prove the minimum is the whole bar and that Qt widens a narrower
  window when the minimum applies; dragging an edge on niri is
  `VAL-FLU-EDIT-WINDOW`.
- The toolbar's width depends on the fonts, the icon sizes and the suite's
  text scale on the author's session; the offscreen measure is not the live
  one. The window still opens at the live toolbar's width, whatever it is.
- The screen's room is `min(Screen.width, Screen.desktopAvailableWidth)`
  (Qt Quick has no per-screen available geometry); on Wayland that is the
  output's size. A window placed on a second output is sized for the screen
  Qt reports before it is shown.
- The trim's controls get the opening size and the minimum, but they do
  not scroll: on a screen whose nine tenths are narrower than the trim's
  bar (four buttons and three times, a few hundred pixels) they would be
  cut.

## Follow-up

- `VAL-FLU-EDIT-WINDOW` in [VALIDATION.md](../../VALIDATION.md): the whole
  toolbar visible when the editor opens from the preview, and the edge that
  cannot be dragged narrower.

## Landing

- **Base revision:** `69baa0f617feec163743b2d0c17d2ba27f310a08`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:aedcb96bd1b8259765a52232da28fd8d3a0633ab62c86ecab77a8aef8b57f0d1, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
