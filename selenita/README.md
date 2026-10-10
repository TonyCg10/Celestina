# Selenita

Celestina's screen capture and recording tool: one window with the capture,
the recording and the history, in the suite's glass grammar instead of a
terminal command.

## User contract

- Captures the screen, the focused window or a region, after an optional
  delay, to the clipboard and to the pictures folder.
- Records the screen to MP4, with the system sound when asked.
- Keeps a history of the last captures and recordings, with open, copy, show
  in Siderita and delete.
- Opens no files: launching it with a path only brings the window forward.

Screenshots, the history and the key-binding flags are in (SEL-1-A);
recording arrives with SEL-1-B. See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).

## Key bindings

niri runs the flags; the running window takes the capture, or a new one
starts, takes it and stays open on the history:

```kdl
binds {
    Print { spawn "selenita" "--screenshot" "screen"; }
    Alt+Print { spawn "selenita" "--screenshot" "window"; }
    Ctrl+Print { spawn "selenita" "--screenshot" "region"; }
}
```

The same request is `Capture(s)` on `org.celestina.Selenita1`, served at
`/org/celestina/Selenita` beside the shared activation interface.

## Runtime dependencies

| Dependency | Used for | From |
|---|---|---|
| niri | the focused window and the outputs over `$NIRI_SOCKET`; a tiled window's picture through `niri msg action screenshot-window` (niri also copies it to the clipboard) | SEL-1-A |
| `grim` | taking the screen or a region | SEL-1-A |
| `slurp` | choosing a region, in the theme's accent | SEL-1-A |
| `wl-clipboard` (`wl-copy`) | copying the capture | SEL-1-A |
| Fluorita, Siderita | «Abrir en Fluorita» and «Mostrar en Siderita» (`org.freedesktop.FileManager1`) | SEL-1-A |
| `gst-plugins-good` (`mp4mux`) | recording; pending, checked at start | SEL-1-B |

A missing tool is said in the window's notice. None is used under
`SELENITA_FAKE=1`; `SELENITA_TOOLS_DIR` makes the real backend run stubs
named `grim`, `slurp`, `wl-copy` and `niri` from that folder instead (the
test seam). `XDG_PICTURES_DIR` in the environment overrides the pictures
folder `user-dirs.dirs` names.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/selenita-core` | Targets, file names, history, niri client, tool argv, recording pipeline; no Qt |
| `src/` | The CXX-Qt controller, the capture worker and its backends (real and fake), the activation adapter with `org.celestina.Selenita1`, the key-binding flags, the appearance follower |
| `qml/` | The window, the capture and history cards and their rows |
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
