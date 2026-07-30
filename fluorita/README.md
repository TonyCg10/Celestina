# Fluorita

The suite's media app: it opens and plays whatever media it is handed — a song, a
clip, an image — as a **player/viewer**, never a library. Siderita is the browser;
Fluorita is what happens when you press Enter on a file. It owns the media decode
stack the rest of the suite deliberately does not carry, so that weight lives in
exactly one place and never leaks into the file manager.

- **Role:** media player / viewer — audio · video · image (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust · Qt Quick/QML via CXX-Qt · a decode backend selected by the measured F4 spike in [ROADMAP.md](ROADMAP.md)
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass
- **Speaks:** the freedesktop [thumbnail managing standard](https://specifications.freedesktop.org/thumbnail-spec/), MPRIS2, XDG MIME / `.desktop` handlers

> **Status: authorized and ready to start at F1; no code exists yet.** The name
> and startup decisions are settled, while backend selection remains an F4
> measurement result. Nothing below is verified — see [ROADMAP.md](ROADMAP.md)
> for the F1–F4 work order, checkpoint ladder and evidence gates.

## Why a first-party player

Two things in the suite are already waiting on it, and both are load-bearing
rather than nice-to-have:

1. **Siderita consumes thumbnails it will not generate.** The file manager reads
   the shared freedesktop cache for video first-frames and audio covers, and
   shows a generic `video-x-generic` / `audio-x-generic` glyph until something
   else puts one there. That "something else" is Fluorita. Siderita stays the
   lean consumer precisely because a producer is planned.
2. **Siderita's quick-look stops at video and audio.** Its spacebar preview
   renders images and text, while those two media kinds get an info card naming
   Fluorita. That hand-off is the placeholder for a live preview backed by
   Fluorita's own widget. PDF is not part of Fluorita's current contract.

So the media weight is not avoided by pretending nobody needs it — it is
*located*, behind a standards-based hand-off, in the one project whose job it is.

## Shape

A **windowed app** like Siderita, plus — later — an **embeddable widget**. The
widget is the interesting half: a playing movie or now-playing music live in the
`celestina` panel, and that same component backing a live "trailer" quick-look
inside Siderita. One player component, three hosts (its own window,
the shell, the file manager), which is only affordable because they share one
visual language and one bridge convention.

Playback state is published over **MPRIS2** (`org.mpris.MediaPlayer2`), so the
shell, the media keys and Magnetita's media-control plugin all read one source of
truth instead of each growing private glue.

## Layout (planned)

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/fluorita-core` | media domain: MIME → capability mapping, playlist-of-one vs queue, position/duration model, thumbnail cache keys and validity — pure, no I/O, no Qt |
| `../celestina-rs/crates/fluorita-decode` | conditional F4 production crate: its narrow interface is extracted only after both bounded backend probes expose their measured common control, events and render-sink contract and the author approves the winning heavy dependency |
| `src/`, `qml/` | the initial CXX-Qt bridge and Qt/QML host surfaces, consuming `celestina-style` |
| `../celestina-rs/crates/fluorita-qt` | conditional CP2 extraction only if the embeddable widget proves a second host needs a shared view contract |
| `scripts/` | run and measurement scripts (closure size, decode CPU, dropped frames) |

## Standards & interop

- **Thumbnails:** writes 256 px "large" PNGs to `~/.cache/thumbnails/large/<md5(file-uri)>.png`
  — the same cache, the same key derivation and the same mtime-based validity
  Siderita already reads. This is a **contract**: Siderita must not need a single
  line of change to see a video's first frame appear.
- **MPRIS2** for transport and now-playing, so the shell and the phone link
  control playback without knowing what Fluorita is.
- **XDG MIME / `.desktop`** for being the handler `xdg-open` reaches — Siderita
  launches it the same way it launches anything else, with no private path.

See [ROADMAP.md](ROADMAP.md) for the checkpoint ladder, the decode-backend
decision and the dependency budget.
