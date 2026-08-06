# Evidence: 2026-08-06 three ways a path key was losing its bytes

- **Date:** 2026-08-06
- **Scope:** `SID-G7-G`; plan
  [shared-reading-surface](../plans/active/2026-08-04-shared-reading-surface.md);
  the three Siderita items of stage 3 in the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md),
  applying [ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md)
- **Environment:** source correction with formatting, lint and unit tests in
  `siderita/`, plus the workspace tests for the Magnetita half in
  `celestina-rs/`. No production build, no deployment, no window opened on a
  live session, and nothing belonging to Celestina was built or run
- **Artifact:** none; no production build ran

## What was wrong

Three places produced a correct key and then handed it to something that could
not keep it whole.

**The breadcrumb separator.** `controller/navigation.rs::path_segments`
published `name\tkey` and `TopBar.qml` cut at the first tab. A tab is a legal
character in a Linux filename, and this seam exists precisely for names nobody
expects, so a folder called `mis\tfotos` moved the cut: the crumb was left
holding a fragment where a key belonged, and clicking it raised an error banner
instead of navigating. Nothing pointed at the wrong folder — the remainder does
not start with `/`, so the decode answered `NotAbsolute` — but the crumb was
dead.

**The persisted record.** `pathkey.rs::normalize` decided whether a stored
string was a key or a pre-ADR raw path by re-encoding it and relying on the
codec being idempotent over both spellings. It is not. A legacy raw path
containing a literal `%20` is already a valid key, for a *different* path, and
normalize answered with that different path silently. The recorded debt called
this a forgotten mark, which understates it: a bookmark is a navigation target
and a paste destination, so the answer was a wrong folder, not a lost star.

**The send.** `controller/mounts.rs::send_to_phone` decoded the key correctly
and then called Magnetita with `path.to_string_lossy()`. It was the last verb
that put a lossy path out of the process: sending a file to the phone, when its
name is not valid UTF-8, sent a path with U+FFFD where the byte was, naming a
file the daemon cannot find.

## What changed

- `src/controller/navigation.rs::segment_line` — the crumb line is `key\tname`,
  extracted into a named function so it can be tested without a Qt object. The
  key comes first because it is unreserved ASCII and `%XX` escapes by
  construction and therefore cannot contain the separator, while the name can
  contain anything and is now the remainder. Sanitising the name was the other
  option and was rejected: it keeps the ambiguity and pays for it by showing a
  name that is not the name.
- `qml/components/chrome/TopBar.qml::pathSegments` — reads the key on the left
  of the first tab and the display name on the right.
- `src/pathkey.rs::persist` / `::normalize` — a record written from now on
  carries a `key:` mark, a prefix neither a key nor an absolute raw path can
  start with, and a marked record is taken verbatim with no codec over it. An
  unmarked record predates the mark and keeps the old migration, because the
  bytes on disk carry no evidence either way and the author's existing files
  must keep loading; that residual ambiguity is bounded to records written
  before this change and one save of a store retires it. It is pinned by a test
  rather than left as prose.
- `src/bookmarks.rs`, `src/favorites.rs`, `src/icons.rs`, `src/folder_views.rs`,
  `src/controller/session.rs::save_tabs` — every store that keeps a path writes
  through `persist`.
- `src/devices.rs::send_file` — takes a `&Path` and calls Magnetita's new
  `SendFileUri` with the URI `dbus::path_to_uri` already writes, which is the
  same spelling the portal, the clipboard and a drag payload carry. The comment
  records that `SendFile` stays for compatibility and that this is the
  byte-exact path.
- `src/controller/mounts.rs::send_to_phone` — hands over the path, not its
  display text.

The daemon half — `SendFileUri` itself, its typed refusal, and
`Command::SendFile` becoming a `PathBuf` — is `MAG-S1-B`, recorded in
[Magnetita's evidence](../../../magnetita/docs/evidence/2026-08-06-byte-exact-send-to-phone.md).
`SendFile` is unchanged: it is a published D-Bus interface, and altering what
its argument means would break any other caller.

## Procedure

```sh
cargo fmt --all --check                                  # in siderita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
bash scripts/check-architecture-contract.sh              # at the repository root
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --all-targets --locked` | 104 passed, 0 failed |
| `check-architecture-contract.sh` | Architecture contract: OK |
| `check-documentation-contract.sh` | Documentation contract: OK |
| `check-language-contract.py` | OK, 157 legacy files ratcheted |

`src/controller.rs` stays at 1106 lines; nothing was added to it.

Ten tests were added. `navigation.rs` pins a folder name containing a tab —
`/home/u/mis\tfotos` — through the exact cut QML makes, asserting the key
decodes back to the directory and the name keeps its tab, and a second pins the
same for a name that is not valid UTF-8. `pathkey.rs` pins that a marked record
is returned verbatim including a name whose own characters spell `%20`, and
that an unmarked record still migrates, with the surviving ambiguity asserted
rather than described. `bookmarks.rs`, `favorites.rs`, `folder_views.rs` and
`icons.rs` each pin the save-then-load round trip for the key of a folder whose
name ends in the four characters `%20` — the case that used to come back as a
different path — and `favorites.rs` and `folder_views.rs` also pin that an
unmarked legacy line still loads.

## Limits

Nothing was exercised through the interface. That a crumb with a tab in its
name navigates when clicked, that a bookmark saved by this build reopens after
a restart, and that the send-to-phone menu item delivers a file whose name is
not valid UTF-8 to a real phone all belong to `VAL-SID-06` in
[`../../VALIDATION.md`](../../VALIDATION.md) and to Magnetita's
`VAL-MAG-HARDENING`; no phone was paired and no D-Bus call was made.

The mark cannot repair a record written before it. A configuration file already
holding a legacy raw path whose name literally contains `%20` still loads as the
path with a space until that store is saved once. There is no way to recover the
distinction from bytes that never carried it, and inventing a heuristic would
reintroduce exactly the guess this change removes.

Nothing outside `siderita/`, `magnetita/` and the `siderita-*`/`magnetita-*`
crates was touched, and nothing belonging to Celestina was built, checked or
run: the author's hardware-safety hold was respected in full.
