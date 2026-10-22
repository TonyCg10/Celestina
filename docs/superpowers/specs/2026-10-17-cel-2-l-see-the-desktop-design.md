# CEL-2-L — See the desktop: overview and shortcut sheet — design

- **Date:** 2026-10-17
- **Unit:** CEL-2-L of [the parity program](2026-10-16-celestina-parity-program-design.md)
  (§5 row L, §7 row L)
- **Status:** design agreed with the author in chat on 2026-10-17 and
  **changed by the author the same day** (see §3): the capture is one
  whole-output preview per workspace, not per-window thumbnails. The plan
  follows in `docs/superpowers/plans/2026-10-17-cel-2-l-see-the-desktop.md`
- **Scope:** `celestina/` only. No change to `celestina-qt/` or
  `celestina-shell-core`.

## 1. What the unit delivers

One centred overlay panel with two modes:

- **Desktop** — every workspace of the focused output laid out as the person
  arranged it (columns left to right, rows down a column, true relative sizes),
  each window a tile with icon and title (geometry only). The workspace that is
  on screen on the panel's output also carries a **preview**: one capture of
  that whole output, drawn as a rounded, dimmed backdrop of that workspace's
  strip, behind its tiles.
- **Shortcuts** — the keybinds of the author's niri config, grouped by binding
  mode, read from `config.kdl`.

There is no duty switch (design §5 row L): nothing runs until the person asks
for the panel with a verb, and nothing is watched or captured in between.

## 2. Decisions the author made (2026-10-17)

| Question | Decision |
|---|---|
| What a window tile shows | **geometry only**; the workspace on screen shows one **whole-output preview** from an on-demand capture (changed by the author on 2026-10-17, see §3) |
| Capture frequency | **once, when the panel opens**; discarded on close; never refreshed while open |
| Where it is drawn | a **centred overlay panel**, like the launcher; the two modes share it |
| Source of the shortcut sheet | **`config.kdl` read-only**, with the `kdl` crate, **parsed in a child process** (added by the author on 2026-10-17 after a review; see §5) |

## 3. Facts the design rests on (verified 2026-10-17, niri 26.04)

- niri exposes `wlr-screencopy` (`capture_output`) and
  `ext-foreign-toplevel-list`. It exposes **no per-window capture protocol**
  (`ext-foreign-toplevel-image-capture-source` is absent), so a window's
  picture could only be a crop of an output capture.
- **Why there are no per-window thumbnails.** niri 26.04 reports
  `tile_pos_in_workspace_view = null` for every tiled window (also recorded in
  `selenita/docs/evidence/2026-10-09-screenshots.md`), so a window's rectangle
  on the output cannot be known and cannot be cropped. The author chose on
  2026-10-17 to capture the whole output once and show it as the preview of the
  workspace that is active on it; window tiles stay geometry only.
- niri's IPC gives each window `WindowLayout`: `pos_in_scrolling_layout`,
  `tile_size`, `window_size`, `tile_pos_in_workspace_view` (null on 26.04) and
  `window_offset_in_tile`. The shell's `NiriModel` drops them today
  (`providers/niri.rs`, `Window`); this unit keeps only what the geometry
  tiles draw (the column and row of `pos_in_scrolling_layout` and
  `tile_size`), so a change that only scrolls the view publishes nothing.
- niri's `block-out-from "screen-capture"` window rule applies to
  `wlr-screencopy`: a blocked window is painted black in the capture. This is
  the author's own permission list; the shell adds none and bypasses none.
- The author's config declares binding modes as top-level
  `binding-mode "name" [exit-on-action=true] { KEY { action; } }` nodes and the
  normal binds under `binds { }`. niri has no IPC that lists binds.
- `wayland-protocols-wlr` with `client` is already a dependency (CEL-2-B), so
  capture adds no protocol crate. `kdl` is the one new dependency.

## 4. Capture: permission, rate, bounds

**Permission.** Capture happens only as the direct result of the person's
verb (`celestina overview open|toggle`, normally bound to a key). There is no
standing permission and no background capture. The privilege sits in a worker
that owns its own Wayland connection, as the energy workers do, and exists only
for the duration of one capture; a second verb while one is in flight is
refused, not queued. The one-at-a-time mark is process-wide: a capture that
was given up (closed, timed out) and is still finishing its current step
keeps the next opening's request refused until it has really ended, and that
opening shows geometry only. The mark travels with the request and is cleared
when the capture ends, when its capturer panics, or when the request is
discarded undelivered; a worker never clears a mark it does not hold. The
worker looks at the stop before connecting, during every wait (the first
syncs included), before each `copy` request, and before and after each
read-back.

**Rate.** Exactly one `wlr-screencopy` frame of one output per opening, with
`overlay_cursor = 0`. No damage tracking, no refresh. Closing the panel drops
every preview; previews are never written to disk, logged or sent over the
bus.

**What is captured.** Only the output the panel opens on (data
minimisation): the app resolves that output, closes an open launcher, and only
then asks for the capture. The whole of that output, once, scaled so its
longer side is at most `PREVIEW_MAX_SIDE = 640` px, keyed by the workspace that
is active on it (an output with no active workspace is not captured; with no
output to open on, nothing is captured and the panel does not open). The
panel draws it as the preview of that workspace. Other workspaces, and every
window tile, are geometry only. Because the source is the composited output,
the preview is what was on screen at that instant. Windows covered by
`block-out-from "screen-capture"` appear black; nothing here treats that
differently.

**Never captured by the panel itself.** The panel's surface is created *after*
the frame is received (or after `CAPTURE_TIMEOUT`, 400 ms, when the geometry
is shown without previews), so the capture never contains the panel. A
`shortcuts` verb while the capture is in flight only changes the mode the
panel will open in; it creates no surface. A timeout cancels the capture
before the surface appears, and a reply that arrives after it is dropped.
Every opening has a number that its capture, timer and bindings reader carry
back, so a reply of an opening that was closed or timed out is never used.
Closing while waiting cancels the capture (its worker is signalled and let go
off the loop's thread), so a re-open asks anew. If the output the panel was
going to open on goes away while the capture is in flight, the opening is
given up whole (capture cancelled, previews dropped); the panel never opens
on another output instead. When the surface cannot be created, nothing that
was to follow it (reading the bindings, handing over previews) runs.

**Bounds.** Frame buffers are `wl_shm`, sized by the compositor's announced
`buffer` event; the worker refuses dimensions above 8192×8192 (a per-frame cap
of 256 MiB) or a stride that does not fit, and on top of that refuses, before
allocating, a frame that would take the request's peak memory past a total
budget of 128 MiB. The peak counts the storage of every frame plus the copy
the worker reads back from the largest one (frames are read back one at a
time, each into a buffer of its own size), so one 4K or 5K output fits and an
8K frame (about 127 MiB, twice that while read back) is refused. It checks the frame against the output's
logical size for a real scale in `[announced − 1, announced + 0.5]`
(`wl_output` announces the ceiling of a fractional scale) and an aspect ratio
within 1 %, so a rotated or odd frame is refused; it scales the whole frame
and frees it before returning. At most `MAX_PREVIEWS = MAX_OUTPUTS` (16)
previews per reply, in practice one, each with its longer side at most
`PREVIEW_MAX_SIDE = 640` px (at most 1600 KiB, about 1.6 MiB). Pixel formats
outside Argb8888/Xrgb8888 are refused (the workspace shows no preview); both
are treated as opaque (alpha forced to 255, since Wayland's Argb8888 is
premultiplied). A preview already made survives a later failure of the
connection (a partial result). Any failure degrades to geometry-only with one
log line; it never blocks opening the panel. Debug output of frames, previews
and the panel prints sizes, never pixels.

## 5. Architecture

Dependencies point from the product down (celestina `AGENTS.md`).

| Piece | Where | Notes |
|---|---|---|
| Overview geometry: columns, rows, shares | `celestina-rs/crates/celestina-shell/src/workspace_map.rs` | **already copied** from the halted core (salvage list); this unit uses it, it does not copy again |
| Scale of a captured frame | new `celestina-shell/src/thumbnail.rs` | pure: rect clipping, scale choice (`PREVIEW_MAX_SIDE`), pixel-format conversion on byte slices; tested without a compositor |
| Shortcut model: parse, group | new `celestina-shell/src/keybinds.rs` | pure: takes `kdl` text, returns `Vec<BindGroup>`; each bind keeps the title, the raw action token and the spawn program apart; bounded (file size, `{` and comment nesting before the recursive parser runs, nodes, label length, include depth) |
| Capture worker | new `celestina/src/capture/` (`mod.rs`, `worker.rs`) | a named worker behind the `Worker` guard, its own Wayland connection, bounded command channel, typed reply message |
| Config reader | `celestina/src/providers/keybinds.rs` | resolves the path (`$NIRI_CONFIG`, then `$XDG_CONFIG_HOME/niri/config.kdl`), read once each time the Shortcuts mode opens (no watcher, no standing thread). The whole read (open, size bound, includes, `kdl` parse) runs in a **child process**: the shell's own binary re-executed with the internal `--keybinds-probe PATH`, empty environment, no stdin or stderr, a JSON answer on stdout read through a 2 MiB cap within a 3 s deadline (the child is killed past it). Any other exit, a signal or a garbled answer is `Invalid`, a child that cannot be started is `Unreadable`, and the parent never touches the path itself (missing, unreadable and too-large are the child's answers; the child lowers its core, address-space and CPU limits first); the parent re-applies the model bounds to what it decodes. The author chose this on 2026-10-17 after a review showed small crafted files still overflow the `kdl` parser's stack past the nesting pre-scan. The waiting thread is off the UI thread. `celestina --check` reads through the same child |
| Panel | new `celestina/src/overview/` (`mod.rs`, `view.rs`, `state.rs`) | one `celestina-overview` overlay layer surface, modelled on `launcher/`: keyboard exclusive, Esc, outside click and focus loss close it |
| Niri model | `providers/niri.rs` | `Window` gains its layout numbers; additive |
| Verbs | `command.rs` | `overview toggle\|open\|close`, `shortcuts toggle\|open\|close`; additive, with the usage text |
| Copy | `copy.rs` | all Spanish text of the panel |

`app.rs` gains two fields (`overview`, the panel with its state machine and
short-lived workers, and `outputs`, the outputs as the compositor last
described them) and match arms; no other behaviour of a shared file changes
(other CEL-2 units edit the same points). No worker is joined on the loop's
thread: a worker that is done or given up is dropped on a short-lived
detached thread (the landing rebase can swap that for `main`'s
`providers::worker::Reaper`).

## 6. Panel behaviour

- **Open.** Verb → resolve the output the panel opens on (none: nothing
  happens) → in Desktop mode, close an open launcher, request the capture of
  that output only and wait for the reply or the timeout → create the surface
  on that output. Shortcuts mode opens at once. The surface is the launcher's
  shape: all edges anchored, transparent outside the card, input region the
  whole surface.
- **Desktop mode.** Rows of workspaces of the focused output; a workspace row
  is a strip of its columns. Tiles are laid out from `workspace_map` shares;
  the focused window has the focus accent. A workspace with a preview shows it
  as a rounded, dimmed backdrop of its strip, fitted to the output's aspect
  ratio and behind the tiles, which keep their icon and title. Keyboard: arrows move between
  tiles, Enter activates, Tab or `1`/`2` switch mode, Esc closes. Click on a
  tile asks niri to focus that window (existing `NiriRequest`) and closes; a
  click on a workspace label focuses the workspace. The panel changes only
  when niri's events say the focus changed; it never edits the layout.
- **Shortcuts mode.** A scrollable list of groups (the normal binds, then one
  group per binding mode with its entry key), each row a key chord and a short
  label: `hotkey-overlay-title` when present (the author's own words);
  otherwise for `spawn` the Spanish copy `copy::shortcuts_open_program` ("open"
  and the first argument's basename) and for `spawn-sh`
  `copy::SHORTCUTS_RUN_COMMAND` ("run a command"); otherwise
  niri's raw action token verbatim (`focus-column-left`), never made into
  English prose. A search field is out of
  scope. Chords are shown as the config spells them. An unreadable or missing
  config shows one explanatory line, never an error screen.
- **Accessibility and style.** Roles and names for every tile and row;
  `visualFocus` ring; contrast from tokens only; motion honours the shell's
  reduced-motion setting. The panel uses the Haze card, spacing and type
  tokens of the launcher; no new colours.
- **Multi-output.** Only the panel's output exists in the panel, and only that
  output is captured. The request and reply types still carry a list, so a
  later change to show all outputs needs no protocol work, but it would be a
  new decision about what is captured.

## 7. Salvage

`workspace_map` is already on the salvage list and already in
`celestina-shell`. Nothing else is copied from the halted tree. No edit of
`AGENTS.md` is needed by this unit. (The sibling "Salvamento CEL-2 restante"
session owns any edit of that section.)

## 8. Risks and what the unit never does

- Capture is the sensitive part. It never captures without a verb, never
  repeats, never persists, never exposes pixels outside the process, and is
  tested only in a nested niri with test windows (never the author's real
  session).
- A compositor that lacks `wlr-screencopy` or returns `failed` gives geometry
  only; the panel still opens.
- `kdl` parses KDL v2 and v1; niri's config is v1 syntax. The plan's first
  task proves the author's real `config.kdl` (read-only copy) parses and
  records the chosen crate version and features in the manifest. If the crate
  cannot read it, the unit stops and reports rather than hand-rolling a parser.
- Binding modes are a feature of the author's patched niri; a stock niri has
  none and the sheet simply shows one group.
- The sheet lists what the config says, not what the compositor currently
  honours; it does not execute anything.
- The unit never grabs keys globally, never sends keys, and never edits
  `config.kdl`.
- The `kdl` crate recurses, and files of a few KiB built from slashdash
  comments, type annotations, property chains or failed escapes overflow its
  stack and abort the process that parses them; a lexical pre-scan
  (`check_nesting`, kept as a cheap first defence) cannot close every such
  path. So the shell never parses the config in its own process: the probe
  child does, and its crash is an `Invalid` sheet, not a dead shell.

## 9. Verification

Pure tests for `thumbnail`, `keybinds` and the panel's state and view
(including the 15-key author config as a fixture trimmed to the shapes it
uses); a worker test against a fake `wlr-screencopy` server is not required
(the real protocol is exercised in the nest); the nested-niri smoke gains a
step that opens the overview and the sheet with two test windows side by side
(two columns) and checks that exactly one preview per output is logged and is
non-black overall, that the half holding the unblocked window is painted and
the half holding the window ruled `block-out-from "screen-capture"` is black
(at most a fifth of the other half), that the capture's preview line comes
before the line that creates the panel's surface (so no panel pixel can be in
the capture; the luma figures alone cannot show it), and that the Shortcuts
mode loaded as many groups as the throwaway profile declares.
The worker logs mean luma of the preview and of its left and right halves,
never pixels.
Author rows `VAL-CEL2-OVERVIEW` and `VAL-CEL2-SHORTCUTS` are queued.
