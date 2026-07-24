# Fluorita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the media app
> only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": decode correctness, frame pacing and cost must be proven on real
> files in a real Wayland session, tracked as its own goal. Nothing here is built
> yet — this is a design-stage roadmap.

## Overview

**Purpose.** Open and play whatever media the session hands it — a song, a clip,
an image — and produce the thumbnails the rest of the suite consumes. A
player/viewer, not a library: no collection scanning, no tag database, no
"recently played" store. Siderita browses; Fluorita plays what it is given.

**What it replaces, and what it doesn't.** It replaces whatever plays media on
this session today (mpv/imv or a desktop's stock viewer) — *only* once that tool
proves a recurring daily gap, per suite discipline. It does not replace a codec:
Fluorita is a shell around a decode backend, not a reimplementation of one, and
it will never grow a media library, a tag editor or a streaming client.

**Why it exists at all.** Two suite contracts already point at it. Siderita
consumes video first-frames and audio covers from the shared freedesktop
thumbnail cache and generates none; and Siderita's quick-look hands video, audio
and PDF to an info card that names Fluorita. Both are deliberate: the media
weight is *located*, not avoided.

**Shape.** A windowed app, and later an **embeddable widget** — the same player
component hosted by its own window, by the `celestina-desktop` panel (a playing
clip or now-playing music, live) and by Siderita's quick-look. Playback is
published over MPRIS2 so the shell, the media keys and Magnetita all read one
source of truth.

**Key decisions.**
- **The decode backend is the one deliberately expensive dependency, and it is
  an open decision to be settled with numbers at CP0.** The suite's non-goals
  name Qt Multimedia specifically as a framework not to add without a measured
  need, which rules it out as a default rather than a choice. The leading
  candidate is **libmpv** — one library, hardware decode, Wayland-native
  rendering, an embeddable render API and a small integration surface — with
  GStreamer (heavier, more moving parts) and raw FFmpeg (more code we own, more
  correctness we must prove) as the alternatives. Whichever wins does so on a
  measured closure size and a measured decode cost, and it lives behind one
  narrow trait in `fluorita-decode` so the decision stays reversible.
- **Images do not go through the video path.** Still images are decoded with
  what Qt already has (plus the EXIF-aware, scaled-read approach Siderita's
  thumbnailer already uses), so viewing a photo costs nothing from the media
  stack. The heavy backend is loaded lazily, only for what needs it.
- **Thumbnail production is a contract, not a feature.** The cache path, the
  `md5(file-uri)` key, the 256 px "large" size and the mtime validity rule are
  fixed by what Siderita already reads. Siderita must not change by one line for
  its video frames to start appearing.
- **Truthful state.** Position, duration and playing/paused are what the backend
  reports, never what was requested — a click is a request. A file that cannot
  be decoded says so plainly instead of showing a stalled transport.
- **Never the browser.** Fluorita opens what it is handed. If a "play this
  folder" need appears, it is a queue built from that hand-off, not a library.

## Checkpoint 0 — Play one file, and prove the backend (the hard part first)
**Goal:** a window opens a file passed on argv or by `xdg-open`, decodes and
renders it in a real Wayland session, and the backend decision is settled by
measurement rather than preference.

- [ ] `fluorita-core` — MIME → capability mapping, transport state model,
      thumbnail cache keys and validity, all pure and unit-tested without I/O
- [ ] `fluorita-decode` — one narrow trait (open, seek, frame/sample out) with
      the chosen backend behind it; a second backend stubbed far enough to prove
      the trait is not shaped around one library
- [ ] Qt/QML host — a single surface over `celestina-style`: video/image output,
      transport, position, volume, and a truthful error state
- [ ] Still images decoded without loading the media backend at all
- [ ] **Measured** — installed closure delta, memory, decode CPU and dropped
      frames for a 1080p clip, each inside a declared budget; the backend
      decision recorded with the numbers that settled it
- [ ] **Verified** — plays real audio, video and images on the author's Wayland
      session, launched both from argv and from Siderita via `xdg-open`

**Done when:** pressing Enter on a media file in Siderita opens it here and it
plays correctly — and the cost of the decode stack is a number, not a hope.

## Checkpoint 1 — The producer half (make Siderita's thumbnails appear)
**Goal:** Fluorita starts paying its way to the rest of the suite, by generating
exactly what Siderita already knows how to consume.

- [ ] Video first-frame and audio cover extraction into the shared cache —
      256 px "large" PNGs, `md5(file-uri)` key, atomic write, mtime validity
- [ ] Generation is bounded and off the UI thread, and never blocks playback
- [ ] A `.thumbnailer` entry so *other* desktop apps get the same frames, since
      the cache is shared and the standard exists
- [ ] MPRIS2 — `org.mpris.MediaPlayer2` transport and now-playing metadata
- [ ] **Verified** — browsing a video folder in Siderita fills in with real
      frames, with **no change to Siderita**; a second pass reuses the cache

**Done when:** Siderita's video and audio rows show real thumbnails purely
because Fluorita exists, and playback is controllable from outside the app.

## Checkpoint 2 — One suite (the embeddable widget)
**Goal:** the player stops being a separate window and becomes a component the
rest of the session hosts.

- [ ] The player surface extracted as an **embeddable widget** with a bounded,
      documented contract (size, lifecycle, what it costs while idle)
- [ ] Shell widget — a playing clip or now-playing music in the
      `celestina-desktop` panel
- [ ] Siderita live quick-look — the widget replaces the info card that names
      Fluorita today, so spacebar previews a clip instead of describing it
- [ ] One settings source shared with the suite (volume, defaults, per-type
      handlers), not a private store
- [ ] **Measured** — the idle cost of an embedded widget, since it lives in the
      panel and the panel lives forever

**Done when:** the same player runs in its own window, in the panel and inside
the file manager's preview, over one contract and one visual language.

## Later / someday
- [ ] Subtitles, audio-track selection, playback-speed control — each when a
      daily need appears, never for parity with a full media player
- [ ] A queue built from a multi-selection hand-off (still not a library)
- [ ] Hardware-decode tuning per format, once there is a measured reason

## Non-goals
- **No library.** No collection scan, no tag database, no watch history.
  Siderita browses; Fluorita plays what it is handed.
- **No codec reimplementation.** The decode backend is a dependency, chosen and
  measured, not something this project writes.
- **No streaming or network sources** before a local player is complete and a
  daily gap is proven.
- **No feature parity** with mpv, VLC or a stock viewer. A settings page full of
  switches is not progress.
- **No leaking media weight.** Siderita must never gain a decode dependency
  because of anything decided here; the hand-off stays freedesktop-shaped.
- **Not a general product.** Like the rest of Celestina, this is for its author's
  session.
