# Evidence: the floating editor that opens any path

- **Date:** 2026-10-10
- **Scope:** `FLU-P1-A` — `fluorita` (program `PRV-1`, design §5)
- **Environment:** CachyOS, Qt 6.12.0, Rust toolchain of the workspace; a
  session worktree, offscreen only: no window was shown on the author's
  session
- **Artifact:** the session's release build at
  `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita`; the
  landing builds, verifies and deploys the registered binary

## Change

- `fluorita/src/activation.rs` serves `org.celestina.Fluorita1` at
  `activation::object_path(&FLUORITA)` on `Owner::connection()`, beside the
  shared interface, as Selenita serves `Selenita1`. `Edit(s key)` decodes a
  `pathkey` key and refuses one that does not decode, is not absolute or is
  not a regular file once links are followed with
  `org.freedesktop.DBus.Error.InvalidArgs`; that answer is one `stat` on the
  bus's thread. An accepted key reaches QML as
  `FluoritaActivation.editRequested(key)`, waiting until `start` when the
  window is not up yet; past `activation::INBOX_LIMIT` the oldest waiting
  edit is dropped, counted and said, as the shared inbox does.
  `fluorita --edit PATH` asks a running Fluorita through the same method and
  exits 0; a running Fluorita that refuses or does not answer ends the
  launch with status 1; otherwise the launch claims the name with no paths
  and queues the edit for its own window. If another launch wins the name
  in between, the edit is offered to it up to five times, 200 ms apart, and
  this launch exits. The path is made absolute and nothing more (no link is
  resolved), so it is the spelling a bus `Edit` for the same file carries,
  and it is refused before any window by the same rule. `requested_media`
  became `requested_launch`, so `--edit`'s path is no longer read as media
  to play.
- `fluorita/src/editor.rs` already opened by path key: the catalogue was
  never consulted, and a file outside every library root is measured on the
  opener's thread like any row. The save worker's body is now `land()` over
  three seams (the Trash, the recipe store, the adoption hook), so a test can
  run the real toolkit save against a scratch Trash and store; `recipes`
  lost its default-store wrappers (`remember`, `reopen`), the editor naming
  the store instead. New: `savedKey`/`savedUrl` (the written file, published
  after a landed save until the next open; the URL from
  `celestina_core::file_uri`) and `nameOf(key)` for titles. Every existing
  invariant stands: the GUI thread never encodes, nothing is published
  before the engine confirms, coordinates are image pixels, paths are keys.
- `fluorita/src/adopt.rs` (new): after a copy lands, one worker of its own
  calls `Selenita1.Adopt(key)` when `org.celestina.Selenita` is owned. Only
  when nobody owns that name, or the bus or the question of who owns it is
  unavailable, does it apply ADR 0012's rule (a `.png` directly in the
  pictures folder's «Capturas», an `.mp4` directly in the videos folder's
  `Recordings`) and append through `selenita_core::History` (load, push,
  save). An `Adopt` call that fails on a running Selenita is reported and
  its file left alone, so a running Selenita's own copy of the history is
  never raced. Failures are said on stderr and never touch the save.
  `main` calls `adopt::shutdown(SHUTDOWN_WAIT)` after the event loop (the
  hand-off timeout plus a second): the queue is closed and what is in it is
  delivered, because «Guardar ambas» and then closing the last window is
  the usual way a copy arrives and the process would otherwise end under
  the worker. `selenita-core` is a path dependency,
  justified in `fluorita/Cargo.toml`; its own crates.io dependencies
  (`chrono` and its platform crates) enter `fluorita/Cargo.lock`, already in
  the suite through Selenita.
- `fluorita/qml/EditWindow.qml` (new): a top-level `Window` per file
  (`transientParent: null`), its own `FluoritaEditor`, title
  `qsTr("Editar — %1").arg(fileName)`, `property bool video` (false; the trim
  fills it), the existing `EditSurface`, and after a save the result with a
  `Drag.Automatic` `text/uri-list` drag offering `Qt.CopyAction` only.
  `Main.qml` instantiates one per open key over a `ListModel` (a file already
  open comes forward instead), takes `editOnly` from `main.rs` to stay hidden
  and unscanned after `--edit`, and scans the library only if something later
  shows it.
- `EditToolbar.qml` says «Guardar ambas» and «Guardar solo la editada»; its ×
  is «Cerrar». `EditCloseQuestion.qml` (new, Grafita's unsaved dialog as the
  model) asks «Guardar ambas» (primary, focused), «Guardar solo la editada»
  or «Descartar» when `EditSurface.leave()` finds unsaved changes; the
  window's close, the ×, and Escape all go through it, in the library's
  editor too. A save chosen there closes the editor when it lands; when it
  fails, the editor stays open with its edit and the way out is forgotten,
  so a later close asks again and ends the edit once.
- `fluorita/scripts/qml-tests.sh` and `fluorita/tests/qml/` (new): Fluorita's
  QML harness, Selenita's model, with a `FluoritaEditor` stand-in;
  `fluorita/tests/fixtures/picture.png` is a 32 × 24 RGB PNG written by a
  short Python `zlib` script. `smoke.sh` gains the `--edit` launch.
- `docs/projects.toml` lists `celestina-rs/crates/selenita-core` among
  Fluorita's production inputs (the architecture contract requires every
  linked path package); that file is outside the `fluorita:` scope, so it is
  a separate `suite-maintenance:` commit on the branch.

## Procedure

Tests were written first and seen failing: the activation, adoption and
save-path tests did not compile (`unresolved imports super::edit_target`,
`super::adopt`, `super::land`), and `tst_edit_window.qml` failed with
`EditWindow is not a type`. Then, from the worktree:

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

The `qmllint` row was measured on the untouched tree first, after a release
build of it: 56.

End to end on a private bus (`dbus-run-session`, offscreen, a scratch `HOME`),
with a temporary `console.warn` of the window title in `EditWindow` that was
removed afterwards: `fluorita --edit "a b.png"` started; `busctl --user call
… org.celestina.Fluorita1 Edit s <key of "b b.png">` returned; `Edit` with a
folder's key and with a missing file's key failed; a second
`fluorita --edit "c b.png"` exited 0; the first process logged three windows,
`Editar — a b.png`, `Editar — b b.png` and `Editar — c b.png`. `gdbus` named
the folder's refusal `org.freedesktop.DBus.Error.InvalidArgs`, and an `Edit`
for a file already open raised its window instead of opening a second.
After the review's fix round the same run on the final binary, without the
trace, gave the same answers: `Edit` returned `()`, the folder was
`InvalidArgs`, and a second `--edit` exited 0 with the first process alive.

## Result

- **Exit:** every command above exits 0. Fluorita's crate: 97 tests (80
  before, 17 new: seven for `Edit`/`--edit` (the linked-folder spelling, the
  inbox overflow and the retry among them), eight for adoption (two for the
  bounded shutdown), two for the save path and the title); `fluorita-core`:
  174; QML: 13 of 13, three runs out of three; the smoke passes with its new `--edit` step; `qmllint` stays
  at 56 for `fluorita` (`EditWindow.qml` and `EditCloseQuestion.qml` add none,
  and no `for` loop sits in a handler); the architecture, language,
  documentation and activation contracts pass.
- **Observed:** a picture under a scratch folder that no library root holds
  opens through `prepare` and saves both ways with the real toolkit: a copy
  beside it with the original's bytes unchanged and the copy handed to the
  adoption hook, then a replacement under the original's name with the
  original in a scratch Trash, byte for byte, and nothing adopted. Adoption
  writes Selenita's history first-in for a copy in «Capturas» or
  `Recordings` with no Selenita or no bus, asks a running Selenita and
  writes nothing, ignores a copy elsewhere, and writes nothing when a
  running Selenita's `Adopt` fails; a copy queued just before the shutdown
  is delivered, and a stuck hand-off holds the end for the bound only. The edit window's title is `Editar — picture.png`, its surface
  shows the fixture (`Image.Ready`), an unchanged close closes at once, a
  changed close shows the three answers and waits, each answer saves (copy
  or replacement) or discards and closes, a save that fails on the way out
  keeps the window and the edit and a later «Descartar» ends the edit once
  (the test fails without the fix), dismissing keeps the edit, and a
  saved result offers `text/uri-list` with the CR LF line as
  `Drag.Automatic`, `Qt.CopyAction` only.
- **Found on the way:** `qmlcachegen` of Qt 6.12 copied the `"\r\n"` escape
  of the drag's binding into its generated C++ as a raw line break and the
  release build failed; the line end is spelled
  `String.fromCharCode(13, 10)` with the reason beside it.
- **Owner and reuse:** the save outcomes, the measurement and the landing
  stay the editor's and the engine's; the history format stays
  `selenita_core::History`'s. ADR 0012's adoption rule is applied here only
  for the case Selenita cannot answer; Selenita's `Adopt` (`SEL-2-A`) applies
  it when it runs, so the rule has two spellings until one owner in
  `selenita-core` can hold it.

## Limits

- No window was looked at: floating and centring are niri's rule, which the
  suite exit adds; focus, the drag into another program and the quit when
  the last edit window closes are `VAL-FLU-EDIT-WINDOW`.
- The QML tests run over a stand-in editor; what a real save writes, trashes
  and adopts is proved by the crate tests above, not through QML.
- Fluorita's `verify-production.sh` does not run the new QML harness yet:
  doing so makes it a verification input in `docs/projects.toml`.
- Two races are recorded in [STATUS.md](../../STATUS.md#known-issues): a
  Selenita starting while Fluorita appends to its history, and an `--edit`
  that loses the name to a concurrent launch, whose library the shared
  claim's `Activate` then shows.

## Follow-up

- `VAL-FLU-EDIT-WINDOW` in [VALIDATION.md](../../VALIDATION.md).
- `FLU-P1-B`, the video trim, fills `EditWindow.video`.
- One owner for the adoption rule in `selenita-core` once `SEL-2-A` lands.

## Landing

- **Base revision:** `16dd76e580834bbfd140a7e6ddd0bfb9daeab495`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:6634c4469d7ff7d22ab36b3dba4577f836b2820dce1b289214e97001fa58e11d, verification_fingerprint sha256:6f8ccadcc75fc2531a7a9af8e801b2f1e027cb39e5d50f2c48d3454b07a304ff
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh
