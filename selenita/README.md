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

The features arrive in the SEL-1 units; the skeleton shows the three cards
(capture, recording, history), each with a muted line. See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).

## Runtime dependencies

| Dependency | Used for | From |
|---|---|---|
| niri | the focused window and the outputs, over `$NIRI_SOCKET` | SEL-1-A |
| `grim` | taking the capture | SEL-1-A |
| `slurp` | choosing a region | SEL-1-A |
| `wl-clipboard` (`wl-copy`) | copying the capture | SEL-1-A |
| `gst-plugins-good` (`mp4mux`) | recording; pending, checked at start | SEL-1-B |

None is needed by the skeleton, and none is used under `SELENITA_FAKE=1`.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/selenita-core` | Targets, file names, history, niri client, tool argv, recording pipeline; no Qt |
| `src/` | The CXX-Qt controller, the activation adapter, the appearance follower |
| `qml/` | The window and its three cards |
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
