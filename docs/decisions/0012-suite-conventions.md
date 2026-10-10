# ADR 0012: Suite conventions — one activation interface, one appearance file

- **Date:** 2026-10-09
- **Status:** accepted

## Context

Before `CONV-1` the first-party applications behaved as neighbours rather than
as one suite. Grafita, Hematita and Cuprita each kept a private single-instance
hand-off, Siderita and Fluorita had none, and Siderita reached the others only
by spawning a process. Reduced motion came from an environment variable and
there was no text size at all. Siderita offered «Abrir con…» like any file
manager but had no direct way into the suite, and no application accepted a
file dropped from Siderita. Each private copy decided names, timing and path
encoding differently, and a name that is not UTF-8 crossed some of them as
lossy text, against [ADR 0008](0008-byte-exact-paths-across-the-qt-seam.md).

The design is
[the suite conventions spec](../superpowers/specs/2026-10-09-suite-conventions-design.md);
the delivery is
[the plan](../plans/archive/2026-10-09-suite-conventions.md).

## Decision

- **Activation.** Every application that keeps to one window owns
  `org.celestina.<App>` and serves `org.celestina.Application1` at
  `/org/celestina/<App>` with `Activate()` and `Open(as paths)`, through
  `celestina_core::activation` (feature `activation`), the one owner of the
  names. A launch claims the name *first*, with the object already served,
  before any window exists; a launch that finds the name taken hands its
  paths to the owner within a bounded wait and exits. Every `Open` element is
  a `celestina_core::pathkey` key, so a path crosses byte for byte. Requests
  that arrive before Qt is ready wait in a bounded inbox. Without a bus, or
  with an owner that does not answer, the launch opens its own window. Other
  processes reach a running instance through `activation::open_in` and spawn
  only when nobody owns the name. The architecture contract's scanner
  (`scripts/activation_contract.py`) refuses a bus-name request or a suite
  bus-name literal outside the shared owner. Only this convention has a
  scanner; the appearance, open-with and drop conventions rest on review and
  tests.
- **Appearance.** `$XDG_CONFIG_HOME/celestina/appearance.toml` (by default
  `~/.config/celestina/appearance.toml`), owned by
  `celestina-settings`, is the source of truth for reduced motion and the
  text scale; every window follows it through a watcher on a worker. Cuprita's
  Apariencia section is its one editor, and a read-modify-save starts from
  `load_stored()`, the file without the override. `CELESTINA_REDUCED_MOTION`
  still forces reduced motion on, at read time, and is never written back.
  The text scale multiplies the nine font tokens (`fontMini`…`fontDisplay`)
  only; layout tokens, radii and row heights stay fixed, and the meaning of
  `reducedMotion` is unchanged.
- **Open-with inside the suite.** Siderita's «Abrir en» offers Grafita for a
  file, Fluorita for media or a folder and Hematita for a folder, when
  installed, through `open_in`. Sending to the phone is always an explicit
  action — Siderita's send-to-phone entry or Magnetita's desktop action
  `--send` — and never a plain open: Magnetita's main entry ignores file
  arguments.
- **Drag-and-drop.** A receiving window accepts `text/uri-list` only, hands
  the URLs to Rust undecoded and decodes each with
  `celestina_core::file_uri::to_path`; a URI that names no local file is
  ignored with a notice. A drop goes down the same path as `Open` or a send.
- **What the suite does not do** (spec §11):
  - the portal `Settings` interface is neither read nor written; the shell is
    halted, and a later shell may mirror the file;
  - no light colour scheme and no accent choice;
  - no settings daemon: the file and each application's watcher suffice;
  - `DBusActivatable` stays off: process start launches an application, the
    bus only reaches a running one;
  - each application keeps its own store; `appearance.toml` holds only what
    must be the same everywhere;
  - Siderita's own text scales stay until a later unit decides them.

## Consequences

- A second launch and «Abrir en» reach the running window through one
  interface; a drop is handled by the receiving window itself (an open, or a
  send in Magnetita). A non-UTF-8 name survives each of them.
- A new application adopts the convention by registering its name in
  `celestina_core::activation`; a private claim fails the architecture
  contract.
- A change of text size redraws every open window without a restart, and no
  layout rule depends on it.
- The halted shell's `org.celestina.Shell1` and its portal `Settings` backend
  are untouched.
- The author's live checks are `VAL-F` (Cuprita), `VAL-GRA-OPEN`,
  `VAL-FLU-OPEN`, `VAL-SID-SEND` and the three `VAL-*-DROP` rows.

## Follow-ups

- **EXT-1 (2026-10-09).** Two applications adopted the convention as written:
  Calcita owns `org.celestina.Calcita` and Selenita `org.celestina.Selenita`,
  both registered in `celestina_core::activation` and both serving
  `org.celestina.Application1` claim first. Siderita's «Abrir en» offers
  Calcita for PDFs (decided by name, like media) through `open_in`, and
  Calcita accepts a drop as the drag-and-drop rule says. Selenita additionally
  serves its own interface, `org.celestina.Selenita1`, on the connection that
  owns its name (`Owner::connection()`, added by `EXT-1-S`) with `Capture(s)`,
  `ToggleRecording()` and `StopRecording()`, so niri's key bindings reach the
  running window; the scanner allowlists that literal by file. `StopRecording`
  was added to the interface after `Capture` and `ToggleRecording`: an
  additive method on an application's own interface is a compatible
  evolution and needs no new name. No new decision arose, so there is no
  ADR 0013: the one open question of the program, the `gstreamer` crate or
  `gst-launch-1.0` as a child, was settled by the recording spike's measured
  rule and is recorded in
  [Selenita's recording evidence](../../selenita/docs/evidence/2026-10-09-recording.md).
- **PRV-1 (2026-10-10).** The capture preview joins Selenita and Fluorita
  through two methods, both on an application's own interface beside the
  shared one, served on the connection that owns the application's name
  (`Owner::connection()`), and both taking one string argument, a
  `celestina_core::pathkey` key (`pathkey::encode(path)`), so the path crosses
  byte for byte:
  - Fluorita serves `org.celestina.Fluorita1` at its activation object path
    (`celestina_core::activation::object_path(&FLUORITA)`) with
    `Edit(s key)` → `()`: Fluorita opens the file in a floating edit window.
    A key that does not decode, or that names anything but a regular file,
    is refused with `org.freedesktop.DBus.Error.InvalidArgs`.
  - Selenita's `org.celestina.Selenita1` gains `Adopt(s key)` → `()`: a
    regular file inside Selenita's pictures `Capturas` folder or videos
    `Recordings` folder joins the history, its kind taken from the
    extension (`.png` a screenshot, `.mp4` a recording); any other key is
    ignored without error.

  Selenita calls `Edit` when the preview is clicked and spawns
  `fluorita --edit <path>` when nobody owns `org.celestina.Fluorita`;
  Fluorita calls `Adopt` after saving a copy beside the original and, when
  nobody owns `org.celestina.Selenita`, appends the row through
  `selenita_core::history::History` itself. `Adopt` is an additive method on
  an existing interface, a compatible evolution; `org.celestina.Fluorita1` is
  an interface, not a new bus name. The scanner allowlists the new literals
  by file: `org.celestina.Fluorita1` in `fluorita/src/activation.rs` (served)
  and `selenita/src/preview.rs` (client), and `org.celestina.Selenita1` in
  `fluorita/src/adopt.rs` (client). The design is
  [the capture preview spec](../superpowers/specs/2026-10-10-capture-preview-design.md).

## Revisit when

A shell returns and wants to mirror the appearance to the portal, the suite
needs a setting that is not appearance, or an application needs more than one
window per process.
