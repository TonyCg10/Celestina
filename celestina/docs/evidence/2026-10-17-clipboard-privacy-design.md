# CEL-2-G — Clipboard history: privacy design

- **Artifact:** none; a design record, not a build
- **Scope:** the privacy design of unit CEL-2-G
- **Environment:** none; nothing in this record was run

- **Date:** 2026-10-17
- **Unit:** CEL-2-G of the parity program
  ([design](../../../docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md), §5 row G, §7)
- **Status:** approved by the author in chat on 2026-10-17, question by question
- **Duty:** `duties.clipboard`, `off` by default

The clipboard is where passwords, tokens and private text pass through. This
document decides what the shell may keep of it before any code exists. Every
rule below is a requirement of the implementation plan and has a test.

## 1. What is watched

- The regular selection only, over `ext-data-control-v1`. Never the primary
  selection (mouse drag, middle click). Never `wlr-data-control`.
- While the duty is `off` no protocol object exists and nothing is read.
- A `paused` state (verb `pause` / `resume`, in memory, reset to running at
  start) stops reading without stopping the connection.

## 2. What is never stored

A selection is skipped **without reading its contents** when its offered MIME
types include any of:

- `x-kde-passwordManagerHint` (the convention KeePassXC, Bitwarden and others
  use),
- a fixed list of other exclusion hints: `application/x-keepassxc-secret`
  and the like are added only when a primary source is cited in the plan; the
  list is a `const` table with one test per entry.

Decision (author, 2026-10-17): **explicit hints only.** No content heuristics
(no entropy test, no token-prefix test) and no per-application exclusion list,
because `ext-data-control` does not identify the sender and a heuristic gives
false safety while losing legitimate copies.

Also never recorded: empty text, text with NUL, text over the size bound (§3),
a selection that offers neither a text MIME nor `image/png`, and the shell's
own echo (the selection it set itself when an entry is chosen).

## 3. Bounds

| Item | Bound |
|---|---|
| Text entries | 200, most recent first, duplicate moves to the front |
| One text entry | 256 KiB (UTF-8, no NUL) |
| Image entries | 20, memory only |
| One image | 8 MiB of `image/png` bytes read, with a read deadline of 2 s and chunked reads |
| Entry age | 7 days |
| Preview in the UI | 160 characters, whitespace collapsed; at most 50 rows published |
| Persisted file read | the maximum a bounded history can occupy, never more |

Reads from the selection owner are bounded in bytes and time: the owner may
never answer, may answer too much, or may answer something that is not what it
offered. Exceeding a bound drops the selection, not the connection.

## 4. Persistence (author decision)

- **Text** is persisted to `$XDG_STATE_HOME/celestina/clipboard.json`,
  created with mode `0600`, written atomically (temp file, then rename), and
  never logged.
- **Images are never written to disk.**
- Each entry carries its capture time. Entries older than 7 days are dropped
  when the file is loaded and on every save, so a secret that slipped past §2
  lives at most seven days. A capture time more than five minutes ahead of
  the clock counts as expired too; the five-minute grace absorbs clock steps
  (early boot, a resume that corrects the clock) without letting a file that
  claims the future buy an entry a longer life.
- A loaded entry passes the same recording test as a live one (§2, §3): the
  file is ordinary bytes any process of the person could write.
- A file with a wrong mode, another owner, a symlink or an unknown schema is
  not loaded. It, and a file whose load left entries out (expired or not
  recordable), is rewritten from what was kept, or deleted when nothing was,
  as soon as the history starts.
- Turning `duties.clipboard` to `off`, live or at start: the watcher stops,
  memory is emptied, and the state file is **deleted**. Turning it `on` again
  starts empty.
- The verb `clear` empties memory and rewrites (or deletes) the file; `remove
  N` removes one entry and rewrites the file. A write never removes the old
  file before the new one is complete.

## 5. What the shell never offers

The `clipboard-text` style of access does not exist:

- No verb of the command socket, no D-Bus method and no CLI subcommand returns
  the text or the bytes of an entry or a list of previews. The verbs are
  `select N`, `remove N`, `clear`, `pause` and `resume`, and they answer
  success or failure only.
- The contents are visible only in the Island page and the bar piece, as
  bounded previews and image thumbnails.
- Contents never appear in logs, metrics, panics, error messages or the
  evidence. Logs carry counts, byte sizes, MIME types and reasons.
- The accessible name of an entry is its bounded preview and never more.
- The shell never offers history over the network, never to another
  application, and never re-publishes the primary selection.
- Tests use a fake data-control client and fixed strings. They never read the
  author's real clipboard.

## 6. Images

- Only `image/png` is read. Bytes are held in memory in the entry.
- A thumbnail is made once, with the bounded decoder the wallpaper unit
  already owns (dimension and allocation limits), under a budget tighter than
  a wallpaper's: at most 16 MP of source and 64 MiB decoded, both checked
  from the header before a pixel is decoded. A decode failure keeps the entry
  out of the history, not out of the process. The thumbnail is composited
  opaque (alpha over black, as the decoder does for every consumer); on a
  light surface a transparent screenshot therefore looks darker than it is,
  which is queued as a validation note rather than solved here.
- Choosing an image entry offers the original PNG bytes again as the new
  selection.

## 7. Selecting an entry

Choosing an entry makes the shell the selection owner with one source offering
the entry (text MIMEs, or `image/png`). The echoed `selection` event of its own
source is recognised by source identity and skipped, so no read is attempted
against itself. The source is cancelled and its data dropped when another
client takes the selection.

## 8. Surface

- Piece `clipboard` in the bar's end zone (after brightness in Noctalia's
  order): a clipboard icon, with a paused mark when paused. Click opens the
  page.
- Page `clipboard` in the Island: previews and thumbnails newest first, select
  by click or Enter, remove with Delete, a clear action with a confirmation
  step, a pause toggle. Keyboard and accessible names as every control.
  The page names the snapshot of the rows it shows when it selects or
  removes (`select N S` / `remove N S`), so a verb for rows that changed in
  between is refused. A held Delete removes one entry (the key must be
  released before it acts again). Rows that do not fit are counted in a last
  line (`clipboard_more` in `copy.rs`); the confirmation step puts the focus on
  its Cancel button.
- Spanish copy lives in `copy.rs`; nothing of an entry is ever translated.

## 9. Salvage

From the halted shell, by copy and with the head comment of source path and
commit: the pure rules of `celestina-shell-core/src/clipboard.rs` (bounds,
`is_recordable`, `is_sensitive`, the history list) adapted into
`celestina-shell`, and the protocol conversation of
`celestina-qt/src/provider_adapter/clipboard.rs` as reference for the new
worker. Two things are **not** carried over: its unbounded persisted
plaintext without expiry, and its skip-the-next-selection echo flag.

## 10. Verification

- Pure tests for every rule of §2 to §4 (including loading a hostile file).
- A fake data-control compositor in tests for the worker: selection events,
  offers with and without the hint, a silent owner, an oversized owner, an
  echo of the shell's own source.
- Nested smoke with the duty on: a fake client sets a selection; the history
  grows by one; a hinted selection adds nothing; `off` deletes the file.
- Live rows queued for the author (`VAL-CEL2-CLIPBOARD-*`): a real
  password manager's copy is skipped; images round-trip; the file's mode is
  `0600`.


## Procedure

None: this is a design and plan record, not a build. Its rules are tested by the
unit's tests and smoke, recorded in [the evidence](2026-10-17-clipboard.md).

## Result

Approved by the author on 2026-10-17 and executed; where the build differs,
[the evidence](2026-10-17-clipboard.md) is authoritative.

## Limits

Nothing here was executed. It records intent, not outcome.
