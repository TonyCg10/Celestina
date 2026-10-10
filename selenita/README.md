# Selenita

Celestina's screen capture and recording tool: one window with the capture,
the recording and the history, in the suite's glass grammar instead of a
terminal command.

## User contract

- Captures the screen, the focused window or a region, after an optional
  delay, to the clipboard and to the pictures folder.
- Records the screen to MP4, with the system sound when asked.
- Keeps a history of the last captures and recordings, with open, show in
  Siderita and delete, and copy for a capture.
- Opens no files: launching it with a path only brings the window forward.

- The keyboard: `1`, `2`, `3` choose the target, Enter captures (or presses
  the focused button), `R`
  starts or stops the recording, Tab walks the controls, Escape returns the
  focus to the window; on a history row, Up/Down, Home/End, Enter (open in
  Fluorita) and Delete (move to the trash).
- Every control has a Spanish accessible name for a screen reader.

Screenshots, the history and the key-binding flags are in (SEL-1-A), so is
recording (SEL-1-B), and the keyboard and accessibility pass closes the
foundation at 1.0 (SEL-1-C). See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).

## Key bindings

niri runs the flags; the running window takes the capture, or a new one
starts, takes it and stays open on the history:

```kdl
binds {
    Print { spawn "selenita" "--screenshot" "screen"; }
    Alt+Print { spawn "selenita" "--screenshot" "window"; }
    Ctrl+Print { spawn "selenita" "--screenshot" "region"; }
    Shift+Print { spawn "selenita" "--record"; }
    Ctrl+Shift+Print { spawn "selenita" "--stop"; }
}
```

`--record` starts a recording (the portal's dialog asks which output) or
stops the one under way; `--stop` only stops, and when no instance answers
on the bus it touches `$XDG_RUNTIME_DIR/selenita/stop`, which the recording
worker watches. The same requests are `Capture(s)`, `ToggleRecording()` and
`StopRecording()` on `org.celestina.Selenita1`, served at
`/org/celestina/Selenita` beside the shared activation interface.

## Recording

The recording goes through the ScreenCast portal
(`org.freedesktop.portal.ScreenCast`, one monitor, the pointer embedded), so
the person consents and chooses the output in the portal's own dialog each
time, and through `gst-launch-1.0 -e` as a child: `pipewiresrc` on the
portal's node, `vah264enc` when a render node and the element exist (else
`x264enc`), `mp4mux`, and with «Con sonido del sistema» a second
`pipewiresrc` on the default sink's monitor encoded with `avenc_aac`. The
stop sends SIGINT, which `-e` turns into an EOS so the muxer writes its
index; the file is then published as `<stem> 2026-10-09 14.32.05.mp4` (the stem is the recording card's product copy) in
the videos folder (`XDG_VIDEOS_DIR` or `user-dirs.dirs`), numbered on a
collision, and joins the history. The muxer is probed at start with
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
| `xdg-desktop-portal` with a ScreenCast backend (`wlr` under niri) | the recording's monitor and PipeWire node | SEL-1-B |
| GStreamer (`gst-launch-1.0`, `gst-inspect-1.0`) with `gst-plugin-pipewire` (`pipewiresrc`) | running the recording pipeline | SEL-1-B |
| `gst-plugins-good` (`mp4mux`) | muxing the recording; probed at start, said in the card and the notice when missing | SEL-1-B |
| `gst-plugins-ugly` (`x264enc`), `gst-plugin-va` (`vah264enc`, optional), `gst-libav` (`avenc_aac`) | encoding the picture and the sound | SEL-1-B |

A missing tool is said in the window's notice. None is used under
`SELENITA_FAKE=1`; `SELENITA_TOOLS_DIR` makes the real backend run stubs
named `grim`, `slurp`, `wl-copy`, `niri`, `gst-launch-1.0` and
`gst-inspect-1.0` from that folder instead (the test seam). `XDG_PICTURES_DIR`
and `XDG_VIDEOS_DIR` in the environment override the folders
`user-dirs.dirs` names.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/selenita-core` | Targets, file names, history, niri client, tool argv and the deadline runner, the recording pipeline, its state machine and the stop file; no Qt |
| `src/` | The CXX-Qt controller, the capture worker and its backends (real and fake), the ScreenCast portal client, the recording worker and its recorders (real and fake), the activation adapter with `org.celestina.Selenita1`, the key-binding flags, the appearance follower |
| `qml/` | The window with its key map, the capture, recording and history cards and their rows |
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
