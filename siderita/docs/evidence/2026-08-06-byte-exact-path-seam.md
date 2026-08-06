# Evidence: 2026-08-06 byte-exact path seam for non-UTF-8 names

- **Date:** 2026-08-06
- **Scope:** `SID-G7-D`; plan
  [shared-reading-surface](../plans/active/2026-08-04-shared-reading-surface.md);
  [ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md);
  suite audit finding `SID-A2` in
  [`../../../docs/evidence/2026-08-05-static-suite-audit.md`](../../../docs/evidence/2026-08-05-static-suite-audit.md)
- **Environment:** Arch-derived Linux, Qt 6, `cargo` stable, offscreen Qt
  platform for the QML tests and the startup smoke. No real Wayland session, no
  portal request from another application, no deployment, no version transition
  — the author asked for the implementation, not the delivery
- **Artifact:** `siderita/target/release/siderita`, built by
  `cargo build --release --locked` so that `scripts/qmllint-cxxqt.sh` and
  `scripts/smoke.sh` had a current generated QML module to inspect. This is not
  a canonical release: `build-production.sh` was not run

## What was wrong

`SID-A2`. `siderita-core` keys every entry on its raw `OsString` and has a test
that a name of bytes `b"name\xff"` survives a scan. The Qt seam threw that away:
`controller/scan.rs` published each row's path with `to_string_lossy`, and every
verb rebuilt a `PathBuf` from the `QString` that came back. The round trip is not
reversible, so the returned path named a file that does not exist: such an entry
listed with a U+FFFD in its name and could not be opened, renamed, copied or
trashed. QML made it worse by composing paths itself — breadcrumbs joined
components, the save picker concatenated a folder and a typed name, the quick
look built a `file://` URL with `encodeURIComponent`, and the sidebar cut a
display name out of the path string.

## Procedure

The decision was already taken in ADR 0008 and is not revisited here. What was
implemented is its application to Siderita.

1. **The codec has one owner.** The rule itself lives in
   `celestina_core::pathkey` — `encode`, `decode` and the typed
   `PathKeyError` — which the Fluorita side of ADR 0008 landed in the same
   checkout. `siderita/src/pathkey.rs` deliberately adds no second copy: it is
   only the Qt marshalling that core module leaves to each application
   (`publish` to a `QString`, `decode` from one, `decode_list` over a
   `QStringList`) plus `normalize`, which migrates a record persisted before
   the decision, idempotently over both spellings.
2. **Acceptance.** `siderita/src/controller/keys.rs` adds `accept_key`,
   `accept_keys` and `accept_mark` on the controller: they decode, report a
   typed refusal through `op_error` and answer `None`, so a caller that hands
   over a raw path sees why rather than watching a verb do nothing. An empty
   argument stays the no-op it always was.
3. **Publication.** `controller/scan.rs::publish_location` publishes the folder
   twice — `current_path` (lossy, for reading) and the new `current_path_key`
   property. The `paths` column of `rows_ready`, `entry_path`, the search hits,
   the Trash and Recientes rows, the places, the bookmarks, the favourites, the
   custom icons and the volume/phone mounts all publish keys.
4. **Entry.** Every invokable that acts on a file decodes:
   `start_at`, `open_key`, `set_custom_icon`, `set_custom_icon_accent`,
   `toggle_favorite`, `add_bookmark`, `reveal_path`, `preview_text`,
   `open_properties`, `rename_path`, `rename_paths`, `trash_path`,
   `trash_paths`, `copy_to_clipboard`, `copy_paths_to_clipboard`, `drop_uris`,
   the destination of `drop_uri_list`, `restore_trash`, `purge_trash`,
   `open_with`, `send_to_phone`, `path_uri`, `path_exists`,
   `display_location_name`, `save_tabs`/`saved_tabs`, and the embedded
   surfaces' `request_preview`, `request_launch`, `launch_standalone` and
   `is_media` in `src/editor.rs` and `src/media.rs`. `open_location` stays the
   one entry that takes prose, because prose is what the path bar produces.
5. **QML stops composing paths.** Breadcrumbs now come from
   `path_segments()` as `name\tkey` lines, including the Magnetita mount
   collapse that used to live in `TopBar.qml`; the save picker asks
   `child_key(name)`; the quick look asks `path_uri(key)`; the thumbnail
   delegates hand the key to the provider verbatim instead of re-encoding it;
   and the favourite rows, the batch-rename dialog and the overwrite prompt take
   their readable name from `display_location_name` instead of cutting the path
   up.
6. **Persisted marks.** `bookmarks.tsv`, `favorites.conf`, `icons.conf`,
   `folder-views.conf` and the saved tab list now hold keys, migrated on load by
   `pathkey::normalize`, so a star or a remembered view on a non-UTF-8 folder
   survives a restart.
7. **What was deliberately left alone.** `src/dbus.rs`'s `file://` codec,
   `src/portal.rs`'s answers to other applications and the Trash spec's own
   percent encoding keep their rules, exactly as ADR 0008 says. The portal
   decodes the picker's keys and re-encodes with `dbus::path_to_uri`; a test
   asserts the two spellings agree.
8. **Ratchets.** `controller.rs` shed the display helpers into
   `controller/display.rs` and the mark formatting into `controller/marks.rs`,
   so the bridge additions did not grow it: its architecture row falls from
   1171 to 1106, and its language-debt row is retired because the Spanish it
   carried moved to a file that declares `language-contract: product-copy`.

### Commands

```text
cd siderita && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --all-targets --locked
cd celestina-rs && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && cargo test
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/qmllint-cxxqt.sh siderita
cd siderita && cargo build --release --locked && bash scripts/qml-tests.sh && bash scripts/smoke.sh
```

## Result

- `siderita`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` exit 0; `cargo test --all-targets --locked` —
  **86 passed, 0 failed**.
- `celestina-rs`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` exit 0; `cargo test` — all workspace tests pass.
- `bash scripts/check-architecture-contract.sh` — `Architecture contract: OK`
  after lowering `siderita/src/controller.rs` from 1171 to 1106.
  `siderita/qml/views/FolderView.qml` is unchanged at 831.
- `python3 scripts/check-language-contract.py` —
  `Language contract: OK (158 legacy file(s) ratcheted)` after retiring the
  `siderita/src/controller.rs` row.
- `bash scripts/qmllint-cxxqt.sh siderita` — `OK`, 336 non-fatal warnings,
  the same count as before this change. None of the new members is flagged.
- `bash siderita/scripts/qml-tests.sh` — **47 passed, 0 failed, 0 skipped**.
- `bash siderita/scripts/smoke.sh` — `OK — binario vivo 8 s, sin errores QML,
  sin auto-bindings`.

The new tests, all in the unit they belong to:

- `src/pathkey.rs`: the application side agrees with the core codec it
  delegates to (a non-UTF-8 path keys to `/tmp/na%FFme` and decodes back byte
  for byte); `%2`, `%zz`, the empty string, a relative path and the *lossy
  spelling itself* are refused with the right `PathKeyError`; `normalize`
  migrates a legacy raw path and is idempotent over both spellings.
- `src/controller/scan.rs`: against a real temporary directory holding a file
  named `b"na\xffme"` — the entry **is listed** with `na\u{fffd}me` as its
  display name while its published key decodes to the file itself; it **can be
  renamed** through that key; it **can be trashed** through that key, leaving a
  `.trashinfo` behind; a key the seam did not produce is refused without
  panicking; and `file://` + key equals `dbus::path_to_uri`.
- `src/controller/display.rs`: a non-UTF-8 name shows with a replacement
  character.
- `src/controller/navigation.rs`: crumbs walk every level and the last one
  carries the real directory, not its display text; a phone mount collapses
  into one device crumb.
- `src/bookmarks.rs`, `src/icons.rs`: a legacy raw-path record migrates to its
  key on load.

## Limits

- **No real session.** Everything above is compilation, unit tests, an
  offscreen QML test run and an eight-second offscreen startup. That a
  non-UTF-8 name can actually be opened, renamed, dragged and trashed *by hand*
  in the author's compositor is `VAL-SID-06`, not run.
- **No production flow.** `build-production.sh`, `verify-production.sh`,
  `deploy-production.sh` and `complete-production.sh` were not run, there is no
  version transition and no inventory. The release binary built here exists only
  so `qmllint` and the smoke had a current generated QML module.
- **Thumbnails for a non-UTF-8 name still do not resolve.** The provider is
  C++ and addresses files through `QString`, which cannot hold such a name.
  The entry keeps its themed glyph — a missing thumbnail, never a wrong file —
  and `cpp/thumbnailprovider.cpp` says so at the decode site.
- **The system clipboard still speaks paths.** `cpp/clipboard.cpp` exchanges
  `QUrl` with the rest of the desktop, so copying a non-UTF-8 name *to another
  application* remains lossy. Inside Siderita the internal clipboard carries
  real `PathBuf`s and is unaffected.
- **Mark migration has one blind spot.** `pathkey::normalize` cannot tell a
  legacy raw path that literally contains `%XX` from a key. Such a record
  normalizes to a different key and its star, icon or remembered view is
  forgotten. Nothing is deleted and nothing else is affected.
- **Not verified against Fluorita.** `FLU-M1` is the same defect at the same
  boundary in the other application and is untouched here; ADR 0008 is the
  shared rule, but only Siderita implements it so far.
