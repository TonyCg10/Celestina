# Grafita

The Celestina suite's general text editor. Grafita opens textual content
regardless of filename, extension or desktop MIME classification: plain notes,
source code, JSON, KDL, configuration files and extensionless text all enter the
same document core. It is deliberately an editor, not an IDE.

- **Role:** general text editor (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust · Qt Quick/QML via CXX-Qt
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores ·
  [celestina-style](../celestina-style/) tokens and controls
- **Consumed by:** the standalone Grafita app and Siderita's embedded editing
  surface

> **Status: both surfaces are built and verified headlessly; neither has been
> seen in a real session.** The name and product
> direction were ratified by the author on 2026-07-30. `grafita-core` opens,
> edits, undoes and safely saves real files with its G1 exit checks verified.
> `Space` in Siderita routes through the content probe into a Grafita editing
> modal that has been driven end to end offscreen — open, type through Qt's own
> text widget, save with the file's CRLF intact, guarded close, undo/redo. What
> remains is the compositor and input stack: real key events, focus trapping,
> reduced motion, IME and AT-SPI. The standalone application opens a document
> named on the command line, edits and saves it, guards its own quit, and
> installs with a desktop entry and icon.
> [ROADMAP.md](ROADMAP.md) records the implementation order and what each step
> did and did not prove.

## Product contract

Grafita has two surfaces over one document core:

| User action | Surface | Contract |
|---|---|---|
| `Space` on textual content in Siderita | Embedded Grafita editor | A simple, editable modal occupying almost all of the current Siderita window |
| Double-click or `Enter` on textual content in Siderita | Standalone Grafita | The complete editor in its own application window |
| Direct launch / `xdg-open` | Standalone Grafita | Open the named document without requiring a project or workspace |

`Space` keeps its existing Siderita quick-look behaviour for images, folders,
media and binary files. Only textual content is routed to the embedded editor.
The embedded surface is not a disguised preview: it edits, tracks dirty state,
supports undo/redo and uses the same loss-free save contract as the standalone
application.

## “Any text” means content, not a list

An extension or MIME value may help desktop discovery and syntax colouring, but
it never decides whether Grafita can open a document. The shared core probes the
file bytes and returns one of three truthful outcomes:

- editable text in a supported encoding;
- textual/raw bytes that can be shown but not safely mapped back yet; or
- non-text/binary content.

The initial editable encodings are UTF-8, UTF-8 with BOM and UTF-16 LE/BE with
BOM. Unknown or malformed byte streams are never silently rewritten. Expanding
legacy encodings requires an explicit encoding choice, not statistical guessing.
This boundary is about encoding safety, not file type: `.txt`, `.rs`, `.json`,
`.kdl`, a dotfile and a file with no extension are treated identically when
their content is text.

## Shared architecture

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/grafita-core` | document bytes, encoding/newline model, edit commands, selection, undo/redo, dirty/conflict state, loss-free file IO, the bounded worker and the open/edit/save/close session both hosts drive; no Qt |
| `src/`, `qml/` | the standalone application: a CXX-Qt shell over that session, its own window and full editor chrome |
| `../siderita/src/editor.rs`, `../siderita/qml/dialogs/` | a separate thin shell over the same session, and the embedded modal |
| `org.celestina.Grafita.desktop`, `scripts/run.sh` | desktop entry and the XDG-prefix installer (`--prefix` exists so the install can be tested against a throwaway directory) |

The two hosts do not copy domain rules and do not import each other's QML. Like
Magnetita's standalone and suite surfaces, each UI composes only what it needs
over a shared, testable contract. The embedded surface intentionally omits app
chrome such as tabs or settings; that is presentation scope, not a second editor
implementation.

### The text widget does not own the text

Qt's text widgets store line breaks their own way, so handing one the document
would rewrite every CRLF file the moment it was touched. Instead the core hands
a widget a line-feed *projection* and takes its whole content back on every
change, deriving the single splice that explains the difference. Lines the user
did not touch never enter that difference, so their terminators are never
rewritten; a newline the user actually typed adopts the document's dominant
terminator like any other insertion. Both hosts share this — it is core
behaviour, not a Siderita workaround — and it is why undo, redo and save are
intercepted before the widget's own text history, which knows nothing about
savepoints, terminators or what is on disk.

Opening, probing, reading and saving run outside the Qt GUI thread through an
owned worker. Results are generation- and revision-stamped so a stale open or
save cannot replace newer state.

## The hard rule: save never destroys

Grafita writes a sibling temporary file, flushes and syncs it, preserves the
original's reproducible metadata, verifies that the opened target has not
changed underneath, and atomically renames the temporary over the resolved
target. A failure keeps the buffer dirty and the original intact. Symlinks are
followed rather than replaced, and a detected retarget or external edit becomes
a visible conflict.

## Non-goals

No project tree, build runner, debugger, terminal, language server or plugin
platform. Syntax highlighting, find/replace and indentation aids may make text
editing comfortable, but they never change Grafita into an IDE. File browsing
remains Siderita's job.
