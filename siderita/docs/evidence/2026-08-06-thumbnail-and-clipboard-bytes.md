# Evidence: 2026-08-06 thumbnails and the system clipboard carry bytes

- **Date:** 2026-08-06
- **Scope:** `SID-G7-E`; plan
  [shared-reading-surface](../plans/archive/2026-08-04-shared-reading-surface.md);
  [ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md);
  the two limits recorded by
  [`SID-G7-D`](2026-08-06-byte-exact-path-seam.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable. No real
  Wayland session, no other application on the clipboard, no deployment and no
  version transition — the author asked for the implementation, not the delivery
- **Artifact:** none built. The checks below are `cargo` matrices and the
  repository guards over the checkout; `build-production.sh` was not run

## What was wrong

`SID-G7-D` made every path crossing the Qt seam a byte-exact key, and recorded
two places where the bytes still died at a `QString`.

**The thumbnail provider.** `ThumbnailProvider::requestImageResponse` received
the entry's key — ASCII, correct — and decoded it with
`QUrl::fromPercentEncoding(id.toUtf8())`, which answers with a **`QString`**. A
`QString` cannot hold a byte that is not valid UTF-8, so the decode reintroduced
exactly the loss the key exists to prevent, one step after the key arrived
intact. Everything downstream — `QFileInfo`, the `QUrl::fromLocalFile` cache
key, `QFileInfo::suffix`, `QImageReader` — then addressed a file that does not
exist. Such an entry kept its themed glyph, and its cache key was the key of
another name.

**The system clipboard.** `siderita_set_clipboard_uris` took paths and called
`QUrl::fromLocalFile(path)`; `siderita_read_clipboard_uris` answered with
`url.toLocalFile()`. Both are `QString` paths, so copying such a file to another
application handed it U+FFFD where the byte was, and pasting one back named
nothing. The seam was carrying the wrong representation: what the desktop
actually exchanges on the clipboard is a percent-encoded URI, which is ASCII and
which the process already knows how to write.

## Procedure

Neither half re-argues ADR 0008; both apply it where the previous unit stopped.

1. **The provider's data path is `QByteArray`.**
   `cpp/thumbnailprovider.cpp` decodes the id with
   `QByteArray::fromPercentEncoding(id.toLatin1())` — the same operation at the
   byte level, losing nothing — and carries those bytes through the whole
   function. The source file is addressed by POSIX calls, because every Qt file
   API takes a `QString`: `sourceModified` runs `::stat` on the bytes and
   answers with an invalid `QDateTime` for anything that is not a regular file
   this process may read, and `ReadDescriptor` opens
   `O_RDONLY | O_CLOEXEC` on the bytes and closes the descriptor in its
   destructor unless a `QFile` has adopted it (`QFileDevice::AutoCloseHandle`),
   which is what `QImageReader` then decodes. The cache file itself is a
   hexadecimal digest, pure ASCII, and stays a `QString`.
2. **The cache key is computed over the bytes.**
   `siderita_thumbnail_cache_uri` is
   `"file://" + pathBytes.toPercentEncoding("!$&'()*+,;=:@/")`. That preserved
   set is the one `celestina_core::percent::encode_qt_path` documents (alnum,
   `-._~`, `!$&'()*+,;=`, `:@`, `/`), so the spelling still matches Qt's own
   byte for byte and the shared `~/.cache/thumbnails/` cache stays shared. A
   name that is not valid UTF-8 now has a key at all, where it previously had
   the key of a different name.
3. **The extension comes from the bytes.** `suffixOf` takes what follows the
   last `.` of the last component, so a non-UTF-8 name with an ordinary
   extension is recognised as a generatable image. `generatableImage` compares
   `QByteArray`s.
4. **A relative id is refused.** A published key is absolute; a relative one
   would `stat` against this process's working directory and key the cache on a
   different URI.
5. **The clipboard seam carries URIs, not paths.** `cpp/clipboard.cpp` takes
   `uris` and builds each `QUrl` with `QUrl::fromEncoded(uri.toLatin1())`,
   skipping an invalid one; it reads back with `url.toEncoded()`. Rust owns both
   halves of the codec: `set_clipboard` publishes `dbus::path_to_uri` — the same
   spelling the drag payload and the portal answers already use, reused rather
   than duplicated — and `clipboard_paths` decodes with `dbus::uri_to_path`,
   dropping anything that is not a local-file URI. `QUrl` stores percent-encoded
   bytes verbatim, so `%FF` survives the round trip through Qt untouched.
   `x-special/gnome-copied-files` already spelled its lines with
   `url.toEncoded()` and is unchanged.
6. **What the change does not touch.** `holds_exactly` still compares real
   `PathBuf`s, because the internal clipboard has always held paths and the
   system list is now decoded to paths before the comparison. The `file://`,
   portal and Trash encodings keep their own rules, exactly as ADR 0008 says.
7. **Ratchets.** `src/controller.rs` is unchanged in size: the bridge edit
   renames one parameter, and the new binding lives in its own `src/thumbnails.rs`
   rather than in the coordinator.

### Commands

```text
cd siderita && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --all-targets --locked
cd celestina-rs && cargo fmt --all --check && cargo clippy --all-targets --locked -- -D warnings && cargo test
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/qmllint-cxxqt.sh siderita
```

## Result

- `siderita`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` exit 0; `cargo test --all-targets --locked` —
  **93 passed, 0 failed** (86 before this unit).
- `celestina-rs`: `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --locked -- -D warnings` exit 0; `cargo test` — all workspace tests pass.
  Nothing in the workspace changed; it is verified because the seam delegates to
  its codec.
- `bash scripts/check-architecture-contract.sh` — `Architecture contract: OK`
  with no baseline row moved: `siderita/src/controller.rs` stays at 1106.
- `python3 scripts/check-language-contract.py` —
  `Language contract: OK (158 legacy file(s) ratcheted)`, unchanged.
- `bash scripts/qmllint-cxxqt.sh siderita` — `OK`, 336 non-fatal warnings, the
  same count as before this change. No QML changed.

The new tests, all in the unit they belong to:

- `src/thumbnails.rs` binds two C++ helpers so the provider is reachable from a
  test at all. The C++ cache key and
  `celestina_core::percent::encode_qt_path` produce **identical bytes** for
  ordinary names, including the sub-delimiters and `#`/`?`/space that
  distinguish this preserved set; a name containing `b"\xff"` produces a
  non-empty key, and that key is `file:///home/toni/na%FFme.png`. Against a real
  temporary directory holding a 2x2 PNG named `b"na\xffme.png"`, the provider's
  own guards and descriptor **find and decode it**, reporting 2x2; a directory,
  a non-image extension, an absent file and a relative path are each refused.
- `src/controller/fileops.rs`: the clipboard round trip — a path containing
  `b"\xff"` published as a URI and read back is the same path **byte for byte**;
  `#`, `?` and a space survive too; and an empty entry, a non-`file://` URI and
  anything else Qt may hold are skipped rather than turned into an operation on
  a bad path.

The Qt behaviours the design rests on were confirmed against Qt 6.11.1 with two
throwaway probes before the code was written, not assumed:
`QByteArray::fromPercentEncoding` decodes `%FF` to the raw byte and leaves `+`
alone; `toPercentEncoding("!$&'()*+,;=:@/")` emits uppercase escapes and equals
`QUrl::fromLocalFile(...).toEncoded()` on every name that has a `QString`
spelling; and `QUrl::fromEncoded` → `toEncoded` returns `file:///tmp/na%FFme.txt`
unchanged.

## Limits

- **No real session and no other application.** That a non-UTF-8 name shows a
  thumbnail, and that copying it to a different file manager and pasting it back
  works, is `VAL-SID-06`, still not run. Everything above is `cargo` and the
  guards.
- **No production flow.** `build-production.sh`, `verify-production.sh`,
  `deploy-production.sh` and `complete-production.sh` were not run; there is no
  version transition and no inventory. Nothing was deployed, so the author's
  installed binary does not yet contain this.
- **The provider's own entry point is not covered end to end.** The test reaches
  `siderita_thumbnail_source_size`, which shares every guard and the descriptor
  with `loadThumbnail`. What it deliberately does not exercise is the cache
  read/write around them: `loadThumbnail` answers with a `QImage`, which does
  not cross the CXX-Qt seam, and it writes into the session's shared
  `~/.cache/thumbnails/`. Redirecting that root means mutating the environment
  of a process whose tests run in parallel threads, which is a worse defect than
  the one it would cover. There is no C++ test target in this build.
- **The clipboard round trip is proved in Rust, not through `QClipboard`.** The
  Qt half needs a display connection and a `QGuiApplication`; the test covers
  the codec on both sides of it and the probe covers `QUrl`'s transparency
  between them. A clipboard owned by another process is `VAL-SID-06`.
- **`;` still keys two caches.** GLib escapes `;` where Qt keeps it raw, so a
  filename containing one lands on a different cache entry than a GTK manager
  would write. That is the pre-existing interop limit
  `percent::encode_qt_path` documents; matching Qt is the deliberate choice and
  this unit does not change it.
- **Not verified against Fluorita.** `FLU-M1` is the same defect at the same
  boundary in the other application and remains untouched.
