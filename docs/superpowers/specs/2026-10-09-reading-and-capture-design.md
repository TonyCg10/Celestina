# Calcita and Selenita — design

- **Date:** 2026-10-09
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Products:** Calcita, the suite's PDF viewer; Selenita, the suite's screen
  capture and recording tool
- **Identifiers:** program `EXT-1` (plan
  `docs/plans/active/2026-10-09-reading-and-capture.md`); projects `calcita`
  (crate `calcita-core`, prefix `calcita`, desktop id `org.celestina.Calcita`,
  units `CAL-1-*`) and `selenita` (crate `selenita-core`, prefix `selenita`,
  desktop id `org.celestina.Selenita`, units `SEL-1-*`)

## 1. Goal and scope

Two daily needs still leave the suite: a PDF opens in the web browser, and a
screenshot or a screen recording means remembering `grim`, `slurp` and a
PipeWire pipeline. Each gets a small application in the suite's grammar.

**Calcita** shows a PDF and lets the reader move through it: continuous
pages, zoom (fit width, fit page, free), go to a page, search with the hits
marked, the document's outline, text selection and copy, internal links, a
dark reading mode, recent documents. It joins the suite through the
conventions of CONV-1: single instance with `Open(paths)`, the «Abrir en»
entry in Siderita, a drop from Siderita, the shared appearance. It becomes
the `application/pdf` handler by the author's hand.

**Selenita** takes a screenshot of the screen, the focused window or a region
chosen with the suite's own selector, after an optional delay, to the
clipboard and to `the Pictures dir's `Capturas` folder`; keeps a history card with previews;
opens a capture in Fluorita; and records the screen (with the system's audio
when asked) to MP4 in `the Videos dir's `Grabaciones` folder`, started and stopped from its
window or from a niri key binding.

Out of scope for both: PDF annotation, signing, forms and printing (the
print portal stays with the browser); editing a capture beyond what
Fluorita already does (crop, mark); recording one window alone (the wlr
portal exposes outputs, not windows); OCR; anything in the halted shell.

## 2. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`, Qt adaptation in each
  application's `src`, presentation in its `qml`; typed errors; no production
  `unwrap`; no blocking IO on the Qt thread; each dependency justified.
- Style contract and the desktop-glass program: transparent canvas with
  `CelestinaBackdrop`, grouped cards with section labels (CUP-1-H grammar),
  tokens only, `CelestinaWheelScroll` on lists, dialogs through
  `CelestinaModalLayer`, icon-first actions, no tooltips, Spanish only in
  `qsTr()`, motion within 100–500 ms honouring `reducedMotion`, the shared
  appearance file (text scale, reduced motion) through `celestina-settings`.
- Suite conventions (ADR 0012): the shared activation module (claim-first,
  `Open(as paths)` byte-exact), a drop area for `text/uri-list`, an «Abrir
  en» target in Siderita, no `DBusActivatable`.
- Privilege: none. Screenshots read the compositor through `grim`
  (wlr-screencopy) and niri's IPC; recording goes through the ScreenCast
  portal, which asks the person which output to share; nothing runs as root.
- Structural model: Cuprita (registration + skeleton + document set + strip
  or single page; CUP-1-H cards), never its code verbatim.

## 3. Facts the design rests on (verified 2026-10-09)

| Fact | Where |
|---|---|
| Qt 6.12 ships QtPdf with the `QtQuick.Pdf` QML module (`PdfDocument`, `PdfMultiPageView`, `PdfScrollablePageView`, `PdfSearchModel`, `PdfBookmarkModel`, `PdfLinkModel`, `PdfSelection`) | `/usr/lib/qt6/qml/QtQuick/Pdf/`, `/usr/include/qt6/QtPdf` (package `qt6-webengine`) |
| `poppler-qt6` is also installed; not needed | `pacman -Q poppler-qt6` |
| niri actions `screenshot`, `screenshot-screen`, `screenshot-window`; `niri msg windows`/`focused-window` give the focused window's geometry and app id | `niri msg action --help`, `niri msg windows` |
| `grim` 1.5 (`-g "x,y wxh"` region, `-o` output), `slurp` 1.5 (region picker), `wl-copy` | `pacman -Q grim slurp wl-clipboard` |
| ScreenCast and Screenshot portals route to `xdg-desktop-portal-wlr`; it captures whole outputs | `~/.config/xdg-desktop-portal/niri-portals.conf` |
| GStreamer 1.28 with `pipewiresrc`, `x264enc`, `vah264enc`; `mp4mux`/`webmmux`/`vp9enc` absent (`gst-plugins-good`/`-bad` not installed); ffmpeg 9 has no PipeWire input | `gst-inspect-1.0`, `ffmpeg -devices` |
| `application/pdf` opens in LibreWolf today | `xdg-mime query default application/pdf` |
| XDG user dirs for pictures and videos exist (Spanish names) | `xdg-user-dir PICTURES`/`VIDEOS` |

## 4. Calcita

### 4.1 `calcita-core`

Pure Rust, no Qt: `Recent { path, page, zoom, opened_at }` with a bounded
store (`data_home/calcita/recent`, 50 entries, atomic through
`celestina-core`); `ZoomMode { FitWidth, FitPage, Free(f32) }` with the step
table (zoom in/out: 0.5 … 4.0 in the usual ladder); `Reading { page, zoom }`
restored per document (keyed by the document's path, byte-exact); page-label
parsing for the page indicator (current of total) and the go-to field (accepts
`12`, `+3`, `-2`, `fin`/`inicio`); the search request model (query, case
rule, direction, wrap) kept outside Qt so the keyboard rules are testable.

### 4.2 `calcita/src`

- `main.rs`: identity, claim-first activation through
  `celestina_core::activation` (`Open` = open the first document, or raise
  and open it in the same window if one is already shown — one document per
  window; a second path opens a second Calcita window inside the same
  process), the shared appearance follower.
- `controller.rs`: the current document's path (as pathkey), the restored
  reading position, `goTo`, `zoomTo`, recents, the drop handler
  (`openDropped(uris)` through `file_uri::to_path`, non-PDF refused with a
  notice), `copySelection(text)` to the clipboard.
- QtPdf is used from QML; the controller never touches page rendering.

### 4.3 `calcita/qml`

- `Main.qml`: transparent canvas, a top bar (glass pill, as Hematita's strip
  is placed) with the document's name, the page field «n / N», zoom
  controls, search toggle, outline toggle, reading-mode toggle, a menu with
  the recents and a copy-path entry; the page area is a
  `PdfMultiPageView` over a `PdfDocument`, wrapped by the suite's
  scroller/scrollbar; empty state «Sin documento» with the recents card
  (CUP-1-H grammar) and «Abrir…» through the FileChooser portal (Siderita's).
- Search: a `CelestinaTextField` in a glass card under the bar; `PdfSearchModel`
  feeds the hits; Enter/Shift+Enter walk them; the current hit is scrolled
  into view and painted with the selection token; hit count «3 de 17».
- Outline: a side card with the `PdfBookmarkModel` as a tree (rows with
  indentation), click/Enter navigates.
- Text selection with the pointer (QtPdf's selection); Ctrl+C copies; the
  selection colour is the theme's selection token.
- Links: internal links navigate; external links open through `xdg-open`
  after a confirmation pill («Abrir enlace externo»).
- Reading mode: a `ShaderEffect`-free approach — `PdfPageView` renders onto
  a Rectangle whose `layer.effect` inverts and rotates hue (a small
  fragment shader shipped as a `.frag` in the style's shader folder if one
  exists; otherwise a local shader), toggled by a bar button and `Ctrl+I`;
  images stay inverted (accepted: this is a reading mode, not a render
  mode).
- Keyboard: PageDown/PageUp/Space/Shift+Space, Home/End, `Ctrl+G` go to,
  `Ctrl+F` search, `F3`/`Shift+F3`, `Ctrl+Plus/Minus/0` zoom, `Ctrl+1`
  fit width, `Ctrl+2` fit page, `Ctrl+O` open, `Esc` closes search/outline;
  every control has an `Accessible.name`.
- Drop area over the window (`text/uri-list`).

### 4.4 Handler and hygiene

CAL-1-C registers `MimeType=application/pdf;` in the desktop entry; the
author pins `application/pdf=org.celestina.Calcita.desktop` by hand in
`mimeapps.list` after VAL-CAL; `HOST-HYGIENE.md` records that the browser
no longer holds the PDF role.

## 5. Selenita

### 5.1 `selenita-core`

Pure Rust, no Qt:

- `Target { Screen(output), Window(geometry, app_id, title), Region(geometry) }`
  and `Capture { target, delay: Duration, destinations: {clipboard, file} }`;
  `Geometry` parsing/formatting in `grim`'s `x,y wxh` form; the output
  file name rule (`Captura 2026-10-09 14.32.05.png` in `the Pictures dir's `Capturas` folder`,
  the recording stem + the same stamp + `.mp4` in the videos folder,
  collision-safe).
- `history.rs`: the last 100 captures and recordings (path, kind, taken_at,
  size) in `data_home/selenita/history`, atomic; entries whose file vanished
  are dropped on load.
- `niri.rs`: the IPC client (`$NIRI_SOCKET`, JSON request/reply as `niri msg
  --json` prints) for `FocusedWindow` (geometry, app id, title) and
  `Outputs` (names, logical geometry); byte-exact, typed errors.
- `tools.rs`: the argv builders for `grim`, `slurp` and `wl-copy` (tested),
  and the process runner with a deadline (as Cuprita's `wpctl` runner).
- `record.rs`: the GStreamer pipeline description builder
  (`pipewiresrc path=<node> ! videoconvert ! <encoder> ! <muxer> ! filesink`,
  encoder `vah264enc` when a VA-API device exists else `x264enc`, with the
  audio branch `pipewiresrc target-object=<monitor> ! audioconvert !
  avenc_aac|opusenc` when asked), the recording state machine
  (Idle → Preparing (portal) → Recording → Stopping → Idle | Failed) and
  the stop-file protocol (`runtime_dir/selenita/stop` touched by `selenita
  --stop`).

### 5.2 `selenita/src`

- `main.rs`: identity, claim-first activation (`Activate` raises; `Open`
  ignored), `--screenshot <screen|window|region>` and `--record` /
  `--stop` flags for niri key bindings (they talk to the running instance
  over `org.celestina.Selenita` with two extra methods `Capture(s)` and
  `ToggleRecording()`; when no instance runs they start one that performs
  the action and stays open on the history).
- `capture.rs` (worker): runs the plan — hide Selenita's own window if the
  target would include it (Selenita closes its own window during the delay
  and reopens on the history afterwards; no niri minimise is involved), wait the delay with a countdown pill, run `slurp`
  for a region (the suite's own selector look is slurp's with the theme's
  accent passed as `-c`/`-b` colours), run `grim`, copy with `wl-copy
  --type image/png` when asked, write the file, append to the history,
  notify through the window's notice pill (no desktop notification: the
  shell is halted).
- `portal.rs`: the ScreenCast portal client (`org.freedesktop.portal.ScreenCast`:
  `CreateSession`, `SelectSources` (monitor), `Start` → PipeWire node id and
  fd), with the person's choice of output in the portal's own dialog;
  `record.rs` (worker) drives GStreamer through the `gstreamer` Rust crate if it links cleanly beside cxx-qt
  (spike in SEL-1-B, decision recorded), else `gst-launch-1.0` as a child
  with the pipeline string, stopped with SIGINT so the muxer finalises.
  Runtime dependency `gst-plugins-good` (`mp4mux`) recorded in the README
  and checked at start with a clear notice when missing.

### 5.3 `selenita/qml`

One window, two cards (CUP-1-H grammar), no strip:

- The capture card: three big buttons (screen, window, region); a delay
  choice (0 · 3 s · 5 s · 10 s); two switches «Copiar al portapapeles»
  (on) and save-to-folder (on); the output selector when
  more than one output (popup beside the button).
- The recording card: record / stop with the elapsed time and a
  red dot while recording; a switch «Con sonido del sistema»; the last
  recording's path.
- The history card: rows with a thumbnail (image provider over the file),
  name, kind, size, time; actions «Abrir en Fluorita» (`open_in`), «Copiar»
  (image to clipboard), show in Siderita (`FileManager1.ShowItems`), delete to the
  freedesktop trash through the helper `celestina-core` or `siderita-ops`
  already has (else a confirmed delete).
- Keyboard: `1/2/3` choose the target, `Enter` captures, `R` toggles
  recording, `Delete` on a history row, arrows walk the history; accessible
  names everywhere.

## 6. Delivery phases

| Unit | Prefix | Outcome |
|---|---|---|
| EXT-1-A | `suite` | Calcita registered (unversioned) with its skeleton, icon, document set; the author's baseline hand commit follows |
| CAL-1-A | `calcita` | Open, continuous pages, zoom, go to page, keyboard; activation, «Abrir en» target in Siderita, drop |
| CAL-1-B | `calcita` | Search, outline, selection and copy, links |
| CAL-1-C | `calcita` | Reading mode, recents, appearance follower, 1.0 (`release`), `MimeType`, hygiene record; the author pins the handler |
| EXT-1-B | `suite` | Selenita registered with its skeleton, icon, document set; baseline by hand |
| SEL-1-A | `selenita` | Screenshots (screen/window/region), delay, destinations, history, open in Fluorita, show in Siderita, the niri key-binding flags |
| SEL-1-B | `selenita` | Recording through the ScreenCast portal and GStreamer (spike decides crate vs child), audio option, `--stop` |
| SEL-1-C | `selenita` | Keyboard/accessibility pass, 1.0 (`release`), documents, hygiene record |
| EXT-1-C | `suite` | Siderita's «Abrir en» gains Calcita for PDFs; the program's exit (ADR note in 0012's follow-ups or a short ADR 0013 only if a decision is new), plan archived by hand |

Calcita first (A → CAL-A → CAL-B → CAL-C), then Selenita, then EXT-1-C.

## 7. Verification

- Crate tests: recents and reading positions, page-label parsing, zoom
  ladder, geometry parsing/formatting, file-name rule, history pruning, the
  niri reply parser over captured JSON, argv builders, pipeline builder,
  recording state machine.
- QML tests (both applications get `tests/qml` harnesses from the start,
  as Cuprita has): Calcita — the bar's page field, zoom states, search
  walking over a fixture PDF (a small PDF checked in under `tests/fixtures`),
  outline rows, drop refusal of a non-PDF; Selenita — target choice, delay
  countdown, history rows and actions over a fake capture backend
  (`SELENITA_FAKE=1`).
- Smoke: each binary offscreen; Calcita opens the fixture PDF and reports
  its page count; Selenita constructs its cards over the fake.
- Live, by the author (VAL entries): open a real PDF from Siderita and
  from a drop; search and copy; the dark mode; a region capture with delay
  to clipboard and file; a window capture; a 10 s recording with audio
  played back in Fluorita; the niri bindings.
- Guards as always; the activation scanner covers the two new names (they
  join the crate's name list).

## 8. Decisions recorded here

- QtPdf over poppler: already installed with Qt, QML-native, no new
  dependency; poppler stays unused.
- One document per Calcita window, several windows per process: a reader
  compares documents side by side; tabs would hide that.
- Selenita orchestrates `grim`/`slurp`/niri rather than reimplementing
  wlr-screencopy: the tools are maintained and already installed; the suit's
  value is the workflow (delay, destinations, history, recording).
- Recording goes through the portal so the person consents per recording
  and chooses the output in the portal's own dialog; `gst-plugins-good` is
  a declared runtime dependency.
- No desktop notifications: the shell is halted; the window's pill is the
  notice surface, and the window reopens on the history after a capture.
- Region selection uses `slurp` with the theme's colours: the same look in
  every capture, driven by Selenita's delay and destinations.
