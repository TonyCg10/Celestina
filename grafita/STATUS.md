# Grafita status

- **Updated:** 2026-08-05
- **Implementation:** checkpoints G0-G6 are present; G7 (reading comfort) is
  the active checkpoint and its code is written but not yet delivered
- **Author validation:** the version-1 interaction pass is closed; `VAL-G7` and
  `VAL-GRA-SAVEAS` are requested and intentionally excluded coverage is
  recorded in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- `grafita-core` owns content-based text acceptance, byte/newline-preserving
  editing, undo/redo, search/replace, indentation, highlighting, conflict
  detection, loss-free save and generation/revision-aware session outcomes.
- The standalone application supports content/path activation, untitled and
  recent documents, multiple tabs in one running instance, reordering, find,
  replace, go-to-line, syntax highlighting and guarded close.
- Siderita's embedded surface consumes the same core for edit, undo/redo, save
  and guarded close while keeping its own bounded Qt/QML adapter.
- The archived evidence records real keyboard/mouse use of both surfaces,
  modal focus containment/restoration, find/tabs and the app icon.
- The standalone editing surface numbers its logical lines, keeps the text
  clear of the frame, reports the caret's line and character column in its
  footer, and shows a scroll bar on whichever axis can move. Text size and
  wrapping come from `$XDG_CONFIG_HOME/grafita/preferences`, moved with
  `Ctrl +` / `Ctrl −` or the `Ctrl` wheel and with `F10` / `Alt + Z`. The
  encoding label is gone; the encoding itself is still a document property the
  core preserves.
- Opening and saving go through the session's file-chooser portal. The process
  selects the `xdgdesktopportal` Qt platform theme when the environment names
  none, because without a theme Qt answers `FileDialog` with a drawing of its
  own instead of asking the desktop. An explicit `QT_QPA_PLATFORMTHEME` still
  wins.
- `grafita-core` owns the mapping from a widget's UTF-16 caret offset to a line
  and a character column, so no host counts columns for itself.
- "Guardar como" obeys the same revision rule as an ordinary save: keystrokes
  that land while the worker writes and syncs keep the document dirty and stop
  a pending close, rather than being marked saved. Its destination is decoded
  by the same `url::local_path` an open uses, an existing symlink is written
  through rather than replaced, and the durability reported is the one the
  directory sync actually produced. A clean document and a state already with
  the worker queue no second write, a dismissed chooser disarms whatever was
  waiting on it, a classify answer survives an open asked for after it, the
  live search is reset when a new document is adopted, and the undo bound
  drops an action whole or not at all. Written under unit `G7-C`, covered by
  `grafita-core` tests, not yet built or deployed.
- Legacy encodings are an explicit product exclusion until a real document
  demonstrates the need; they are not an incomplete version-1 item.

## Conditional work, not active debt

- Add a legacy encoding only after a representative file and explicit encoding
  choice establish a reversible contract.
- Extract a shared visual editor surface only after the author accepts that
  design-system API and all consumers can be validated together.
- Revisit highlighter or large-document architecture only with a measured
  startup, memory or latency failure.

IME, AT-SPI, reduced-motion and cross-user/xattr reproduction are intentionally
outside the current plan. They do not keep implementation open and receive a
validation item only when the author asks to pursue them.

## Evidence boundary

The detailed G0-G6 record and earlier commands are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact canonical release passed app/core format, Clippy, tests, QML lint and
an eight-second isolated smoke. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). No installed
binary or desktop-handler state was changed.

Also on 2026-08-05, unit `G7-C` corrected the save-as, duplicate-save,
classify-staleness, search-lifetime and undo-bound defects the suite audit
found. Its automated record is the
[loss-free save-as evidence](docs/evidence/2026-08-05-loss-free-save-as.md).
No release was built and no installed binary was replaced: the author asked for
the corrections and their tests, not for the production flow. Two audit items
remain open by decision — the per-keystroke O(n) work (`GRA-M6`), which needs
its own measured unit, and the U+2028 round-trip (`GRA-M7`), which the audit
itself marks speculative and which no static change can settle.

On 2026-08-05 the platform-theme selection was built in release and installed
into `~/.local` with `scripts/run.sh` at the author's request. No desktop
handler or portal route was changed: the process only stops overriding what the
session already routes. That the dialog a real open now shows is Siderita's is
part of `VAL-SID-02`.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Content activation contract](../docs/contracts/content-activation.md)
- [Registry entry](../docs/projects.toml)
