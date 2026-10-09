# Evidence: drag-and-drop from Siderita into the suite

- **Date:** 2026-10-09
- **Scope:** CONV-1-E — suite
- **Environment:** CachyOS, rustc 1.98.1, Qt 6.12.0, Python 3.14.7; debug and release builds
  in the shared session Cargo target
- **Artifact:** not applicable

## Change

- Siderita is unchanged: its entry drag already carries a `text/uri-list`
  from the Rust codec (`sessionStore.pathUri`). Each receiving window gains a
  `DropArea` with `keys: ["text/uri-list"]` that hands the dropped URLs, as
  strings and undecoded, to Rust, where `celestina_core::file_uri::to_path`
  reads each one by bytes (ADR 0008). A URI that names no local file
  (`smb://`, `https://`, `file://otra-maquina/…`, malformed) is ignored,
  noted on stderr and counted for a short notice in the window.
- The drop highlight is Siderita's (`FolderView.qml`: an accent rim at
  `borderFocus`, nothing filled). It is not lifted into celestina-style:
  Siderita's rectangle takes its radius from its own `contentFrame`, so it
  cannot move unchanged; each application keeps a local copy with a theme
  radius token (`radiusWindow`, or the card's own radius in Magnetita).
- Grafita: `GrafitaActivation.openDropped(uris)` turns the local files into
  the same `Open` actions a second launch delivers (`open_requested` →
  `openTab`), one tab per file; the bytes decide in the tab's session as for
  any open, so a non-text file shows the existing refusal. Decoding is string
  work only, so it runs where it is called; every read stays on the session
  worker.
- Fluorita: `FluoritaActivation.openDropped(uris)` queues the URIs on the
  activation worker, which decides them with `Open`'s rule: the first media
  file plays (an image opens in the viewer through the same action), the rest
  are reported on stderr; a folder that is a configured source is selected.
  A folder that is not a source is offered to the source chooser
  (`folder_offered` → `FluoritaLibrary.addFolderAt(key)`), which opens the
  portal's folder dialog at it (`current_folder`, the name's bytes,
  NUL-terminated), so adding it is one confirmation.
- Magnetita: a drop on a device card sends to that device; a drop elsewhere
  on the device page sends to the page's device (`primaryIndex`).
  `DevicesModel.sendDropped(index, uris)` decodes and sends on the model's
  owned worker (`send::dropped`, `send::send_dropped`, then the CONV-1-D
  `send::send_files` with one `SendFileUri` each); a folder or a vanished
  name is refused before the bus. The page shows `--send`'s chooser wording:
  «Enviando…», then «No se pudo enviar: …» on failure.
- qmllint, measured: on the base (`f035450b`, a detached probe worktree, a
  fresh `cargo build --release --locked` of Grafita, the grafita row set to
  0) Grafita counts 38. The first version of this unit counted 11, and that
  fall was not an improvement: bisecting `Main.qml` showed that a `for` loop
  over `drop.urls` written inside the `onDropped` handler made qmllint stop
  reporting `Main.qml` at all (its 27 `unqualified` warnings vanished; the
  base `Main.qml` in this tree counts 38, the drop block without the loop
  38, with it 11). The conversion now lives in a window function
  `droppedUris(urls)`, Siderita's shape, in all three applications, and the
  counts are the base's: Grafita 38, Fluorita 56, Magnetita 38. No row
  changes.
- Magnetita refuses a drop while a dropped send is in flight, saying a send
  is already under way, so a fast second send cannot overwrite the first one's
  outcome; the page's notice clears after 4 s like Grafita's and Fluorita's.
- Fluorita: a folder dropped while the folder chooser is already open is
  noted on stderr and in the sidebar's folder notice (the chooser is
  already open) instead of vanishing.

## Procedure

```sh
# in each of grafita fluorita magnetita:
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo build --release --locked
# from the root, for each application:
sh <app>/scripts/smoke.sh --binary .cargo-target/release/<app>
bash scripts/qmllint-cxxqt.sh <app>
cd celestina-rs && cargo test -p celestina-core
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-land-unit.py
```

## Result

- **Exit:** every command above exits 0.
- **Tests added:** Grafita `url.rs` 1 unit (`file:///tmp/a%20b` → `/tmp/a b`,
  `smb://x/y` and a remote authority ignored, a non-UTF-8 escape kept byte
  for byte and carried to the tab unchanged) and `activation.rs` 1 unit (a
  drop becomes one `Open` per local file, then one notice); Fluorita
  `activation.rs` 2 unit (the first media file plays and a remote URI is
  counted, an image opens through the same action, a source folder selects
  it, another folder is offered, a non-UTF-8 folder name survives the path
  key); Magnetita `send.rs` 2 unit (decoding as above; a dropped file reaches
  the send, a folder does not, a drop of remote URIs alone calls nothing).
- **Observed:** Grafita unit 18 passed; Fluorita unit 80 passed; Magnetita
  unit 37 passed (1 ignored); the three smoke checks pass; qmllint Grafita
  38, Fluorita 56, Magnetita 38 (each equal to its row).

## Limits

- Fluorita decides a drop in list order: a non-source folder before a song
  opens the source chooser at the folder, and the song is reported on stderr
  as ignored (no play queue), exactly as a second media file would be.
- Grafita opens whatever local path is dropped as a tab, so a dropped folder
  becomes a tab that shows the session's existing refusal.
- `file://host/path` (refused as remote), `file://localhost/…`, trailing
  slashes and escapes rest on `celestina_core::file_uri::to_path`'s own tests
  in celestina-core; this unit tests only how each application splits and
  routes its result.

- Grafita, Fluorita and Magnetita have no QML test harness; the drop handlers
  are thin (URLs to strings, one invokable) and the decisions are tested in
  Rust.
- The URLs reach Rust through QML's `url.toString()`, as Siderita's own
  external drop does.
- Nothing was dropped or sent live: the drops in a real session, the
  highlight and a real send are the author's (VAL entries).

## Follow-up

CONV-1-F.

## Landing

- **Base revision:** `f035450b415284cd20568f56ac76b8f94d7e1dcb`
- **Check:** `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2271c2ebeb7386580a5bc14d674ebdf6351500e68893e8ae08110524eca8f942, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a72c9d2605ab37378b738bfbfdcfd9aae52fd97ddaffc3fe778a5d3aba16449c, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:6d8dab386db1e93807cfa1f5c743193c76136a747609b38e6abc8679b53617fb, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh
