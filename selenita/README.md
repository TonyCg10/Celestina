# Selenita

Celestina's screen capture and recording tool: one window with the capture,
the recording and the history, in the suite's glass grammar instead of a
terminal command.

## User contract

- Captures the screen, the focused window or a region, after an optional
  delay, to the clipboard and to the pictures folder.
- Records the screen to MP4, with the system sound when asked; from a key
  binding it can record the monitor in focus without asking which.
- While it records, a small bar at the top of the screen shows the time
  and stops the recording, keeping it or throwing it away.
- Keeps a history of the last captures and recordings, with open, show in
  Siderita and delete, and copy for a capture.
- Shows each new capture and recording in a small preview in the corner of
  the screen, which can be dragged into any program that takes files or
  clicked to edit the file in Fluorita.
- Opens no files: launching it with a path only brings the window forward.

- The keyboard: `1`, `2`, `3` choose the target, Enter captures (or presses
  the focused button), `R`
  starts or stops the recording, Tab walks the controls, Escape returns the
  focus to the window; on a history row, Up/Down, Home/End, Enter (open in
  Fluorita) and Delete (move to the trash).
- Every control has a Spanish accessible name for a screen reader.

Screenshots, the history and the key-binding flags are in (SEL-1-A), so is
recording (SEL-1-B), and the keyboard and accessibility pass closes the
foundation at 1.0 (SEL-1-C). The corner preview comes with SEL-2-A, the
recording bar and the focused recording with SEL-4-A. See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md)
and the
[capture preview design](../docs/superpowers/specs/2026-10-10-capture-preview-design.md).

## Key bindings

niri runs the flags; the running Selenita takes the capture, or a new one
starts and takes it without showing its window (the corner preview shows
the result; a failure shows the window with its notice) and ends once the
preview has gone and nothing records:

```kdl
binds {
    Print { spawn "selenita" "--screenshot" "screen"; }
    Alt+Print { spawn "selenita" "--screenshot" "window"; }
    Ctrl+Print { spawn "selenita" "--screenshot" "region"; }
    Shift+Print { spawn "selenita" "--record"; }
    Ctrl+Shift+Print { spawn "selenita" "--stop"; }
}
```

In the author's `tools` mode, beside `S` (`--screenshot screen`) and `R`
(`--record`), `V` records the focused monitor without the portal's dialog
(SEL-4-A; see «Recording the focused monitor» below):

```kdl
V { spawn "selenita" "--record-focused"; }
```

The screen binding (`--screenshot screen`, here Print; any key the
configuration binds to it, such as `S`) takes only the monitor niri has
focused (`FocusedOutput` over `$NIRI_SOCKET`, then `grim -o <name>`), not
every output. When niri cannot say, it takes the window's own choice
(«Todas» or one output) and says so on stderr. Inside the window,
«Pantalla» keeps its output chips, «Todas» and each output, as before.

`--record` starts a recording (the portal's dialog asks which output) or
stops the one under way; `--stop` only stops, and when no instance answers
on the bus it touches `$XDG_RUNTIME_DIR/selenita/stop`, which the recording
worker watches. The same requests are `Capture(s)`, `ToggleRecording()` and
`StopRecording()` on `org.celestina.Selenita1` (and `RecordFocused()` for
`--record-focused`, which starts or stops like `ToggleRecording()`), served at
`/org/celestina/Selenita` beside the shared activation interface. The same
interface has `Adopt(s key)` (ADR 0012, PRV-1): Fluorita hands back an
edited copy by its `pathkey` key, and a regular `.png` or `.mp4` directly in
the pictures folder's «Capturas» or the videos folder's `Grabaciones` joins
the history; anything else is ignored. The rule is
`selenita_core::history::adopted`, the one Fluorita applies too when no
Selenita answers, and both folder names are `selenita_core::names`'s.

## La vista previa

After every capture saved to a file and every finished recording, however
it was started, the result shows in a small frameless window titled «Vista
previa»: the picture, or the recording's first frame with its length and
the film glyph. It stays 5 s (the pointer resting on it holds it; it starts
again when the pointer leaves) and leaves with a 200 ms fade and slide
(none with reduced motion); a new result replaces it, the × closes it, and
so does trashing its file from the history.

- **Drag** it into any program that takes files (a chat, a mail, Siderita):
  it offers the file as `text/uri-list`, as a copy only.
- **Click** it to edit the file in Fluorita's floating editor:
  `org.celestina.Fluorita1.Edit(key)` when Fluorita runs, else
  `fluorita --edit <path>`. «No se ha podido abrir Fluorita.» says when
  that failed.

It never takes the keyboard focus, and it never keeps Selenita running:
closing the main window ends Selenita and takes the preview with it.
Selenita cannot place its own window
under niri; this window rule (spec §7, added to niri's configuration by the
suite's PRV-1-E) puts it at the bottom-right corner of the focused output
without the focus:

```kdl
window-rule {
    match app-id=r#"^org\.celestina\.Selenita$"# title="^Vista previa"
    open-floating true
    open-focused false
    default-floating-position x=24 y=24 relative-to="bottom-right"
}
```

The title has no closing `$`: Qt on Wayland appends the application's
display name, so the window's real title is «Vista previa — Selenita», and
`^Vista previa$` never matched (the preview opened tiled and focused).

The picture (or the recording's first frame) is decoded at the size it is
shown at, in device pixels (the frame's size times the screen's pixel
ratio, both dimensions, the picture's shape kept), and drawn with smoothing
and mipmaps, so a capture many times wider than the preview stays sharp.

A recording's first frame is taken by a `gst-launch-1.0` child (`filesrc !
qtdemux ! decodebin ! videoconvert ! pngenc snapshot=true`, 3 s at most)
into `$XDG_RUNTIME_DIR/selenita/poster-<n>.png` (a private folder; with no
runtime folder there is no poster); its length is read from the MP4's own
movie header.

## The recording bar

While a recording is under way, however it started (the window's
«Grabar», `R`, `--record`, `--record-focused`, the bus), a small frameless
window titled «Grabando» shows a glass pill: the red dot (a slow pulse,
still with reduced motion), the elapsed time (the recording card's clock)
and two buttons. «Detener» stops and saves, and the corner preview shows
the result as from the card; «Descartar» stops and deletes the file: the
child is killed rather than finished, the hidden file removed, and
nothing joins the history and no preview shows (when the file stays, the
window's notice says the discarded recording could not be deleted). The bar hides as soon as
the recording stops. It never takes the keyboard focus (the card's
«Detener» and `R` are the keyboard's way), a compositor's close is refused
(closing it is not a stop), and closing the main window takes it with
it. After a key-binding launch it is the only window while it records;
a discard ends that launch as the preview's end would.

Qt appends the display name to the title («Grabando — Selenita»), so
niri's rule matches its beginning only. It puts the bar at the top centre
of the focused output, 16 px from the edge, floating and without the
focus:

```kdl
window-rule {
    match app-id=r#"^org\.celestina\.Selenita$"# title="^Grabando"
    open-floating true
    open-focused false
    default-floating-position x=0 y=16 relative-to="top"
}
```

## Recording the focused monitor

`selenita --record-focused` (`V` above) records the monitor niri has
focused without the portal's output dialog. xdg-desktop-portal-wlr asks
its configured chooser which output to cast (it runs `chooser_cmd`
through `/bin/sh -c` and reads one line, `Monitor: <name>` or the bare
name), so Selenita is that chooser:

```ini
[screencast]
chooser_cmd=/home/toni/.local/bin/selenita --pick-output
```

(the line goes in the backend's configuration,
`~/.config/xdg-desktop-portal-wlr/config`, beside the `chooser_type`
already there; the backend reads it when it starts:
`systemctl --user restart xdg-desktop-portal-wlr.service`).

- A focused start asks niri for the focused output (`FocusedOutput`, as
  the screen key binding does) and writes a one-shot request,
  `$XDG_RUNTIME_DIR/selenita/pick-output` (a private `0700` folder, the
  file `0600`, written atomically: `output=<name>` and `written=<ms since
  the epoch>`), then asks the portal as before.
- `selenita --pick-output` answers before any Qt, bus or settings work: a
  request at most 30 s old is consumed (renamed away, then removed) and
  answered with `Monitor: <name>`, status 0. With no request, or a stale
  or broken one (removed), it becomes the previous chooser with the same
  standard input and output, so its dialog asks exactly as before:
  `"$HOME/.local/bin/celestina" --pick-output`, or the command line in
  `SELENITA_FALLBACK_CHOOSER` (run through `/bin/sh -c`).
- A plain start (`R`, «Grabar», `--record`) writes no request and removes
  any left over before and after the portal is asked, so it always shows
  the dialog. When niri cannot name the focused output, a focused start
  goes on without a request (the dialog asks) and says so on stderr.

Rollback: put the previous line back,
`chooser_cmd=/home/toni/.local/bin/celestina --pick-output`, and restart
the backend; `--record-focused` then shows the dialog like `--record`.

## Recording

The recording goes through the ScreenCast portal
(`org.freedesktop.portal.ScreenCast`, one monitor, the pointer embedded), so
the person consents and chooses the output in the portal's own dialog each
time, and through `gst-launch-1.0 -e` as a child: `pipewiresrc` on the
portal's node, `vah264enc` when a render node and the element exist (else
`x264enc`), both at a constant quality (CQP 20/22, CRF 21; a screen is
text, and a bitrate budget smears it) with a keyframe a second, `mp4mux`,
and with «Con sonido del sistema» a second `pipewiresrc` on the default
sink's monitor, mixed over a silent live `audiotestsrc` in an `audiomixer`
(so the branch always flows and the stop finishes even when the monitor is
silent or absent) and encoded with `avenc_aac`. The stop sends SIGINT,
which `-e` turns into an EOS so the muxer writes its index; the file is
then published as `<stem> 2026-10-09 14.32.05.mp4` (the stem is the
recording card's product copy) in the videos folder's `Grabaciones`
(`XDG_VIDEOS_DIR` or `user-dirs.dirs`, then `Grabaciones` inside it, made
when needed; `selenita_core::names::RECORDINGS_FOLDER`), numbered on a collision, and joins the history. The window
stays on screen for a capture and a recording alike: Selenita may be what
is being captured. The muxer is probed at start with
`gst-inspect-1.0 --exists mp4mux`; when it is missing the card says so, the
button is disabled and the notice names the package.

## Runtime dependencies

| Dependency | Used for | From |
|---|---|---|
| niri | the focused window and the outputs over `$NIRI_SOCKET`; a tiled window's picture through `niri msg action screenshot-window` (niri also copies it to the clipboard) | SEL-1-A |
| `grim` | taking the screen or a region | SEL-1-A |
| `slurp` | choosing a region, in the theme's accent | SEL-1-A |
| `wl-clipboard` (`wl-copy`) | copying the capture | SEL-1-A |
| Fluorita, Siderita | «Abrir en Fluorita» and «Mostrar en Siderita» (`org.freedesktop.FileManager1`) | SEL-1-A |
| `xdg-desktop-portal` with a ScreenCast backend (`wlr` under niri, the suite's build: see below) | the recording's monitor and PipeWire node | SEL-1-B, SEL-1-F |
| GStreamer (`gst-launch-1.0`, `gst-inspect-1.0`) with `gst-plugin-pipewire` (`pipewiresrc`) | running the recording pipeline | SEL-1-B |
| GStreamer's `qtdemux` and `pngenc` (`gst-plugins-good`) and an H.264 decoder (`gst-libav` or `gst-plugin-va`) | a recording's first frame for the preview; without them the preview shows the film glyph | SEL-2-A |
| Fluorita with `org.celestina.Fluorita1` (FLU-P1-A) | the preview's click: the floating editor | SEL-2-A |
| `gst-plugins-good` (`mp4mux`) | muxing the recording; probed at start, said in the card and the notice when missing | SEL-1-B |
| `gst-plugins-ugly` (`x264enc`), `gst-plugin-va` (`vah264enc`, optional), `gst-libav` (`avenc_aac`) | encoding the picture and the sound | SEL-1-B |

A missing tool is said in the window's notice. None is used under
`SELENITA_FAKE=1`; `SELENITA_TOOLS_DIR` makes the real backend run stubs
named `grim`, `slurp`, `wl-copy`, `niri`, `gst-launch-1.0` and
`gst-inspect-1.0` from that folder instead (the test seam). `XDG_PICTURES_DIR`
and `XDG_VIDEOS_DIR` in the environment override the folders
`user-dirs.dirs` names.

## The capture portal

Under niri the ScreenCast portal's backend is xdg-desktop-portal-wlr. No
release records reliably there, so the suite builds its own: upstream at a
pinned commit plus the patches in
[`packaging/xdg-desktop-portal-wlr/`](packaging/xdg-desktop-portal-wlr/README.md).

```sh
sh selenita/scripts/build-portal.sh --install-override
```

It installs `~/.local/libexec/xdg-desktop-portal-wlr` (the previous one kept
as `.prev`), writes the systemd user drop-in that points
`xdg-desktop-portal-wlr.service` at it and restarts the service. The
distribution's package stays installed and untouched. The portal routing
(`~/.config/xdg-desktop-portal/niri-portals.conf`, `ScreenCast=wlr`) and the
backend's own configuration (the output chooser, `max_fps`) are the
session's, not this script's. Rollback: delete the drop-in and restart the
service, or copy `.prev` back.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/selenita-core` | Targets, file names, history and the `Adopt` rule, niri client, tool argv and the deadline runner, the recording pipeline, its state machine and the stop file, the focused recording's pick request (`pick`), the poster's argv, a PNG's size and an MP4's length; no Qt |
| `src/` | The CXX-Qt controller, the capture worker and its backends (real and fake), the preview's poster and its hand-off to Fluorita (`preview.rs`), the ScreenCast portal client, the recording worker and its recorders (real and fake) with the discard and the focused start, the portal's output chooser (`picker.rs`), the activation adapter with `org.celestina.Selenita1`, the key-binding flags, the appearance follower |
| `qml/` | The window with its key map, the capture, recording and history cards and their rows, the recording clock they share with the recording bar's window, and the corner preview's window |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Selenita.desktop` | Desktop discovery |

## Build and use

Selenita needs Rust and a Qt 6 development environment visible to CXX-Qt. The
canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `selenita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
