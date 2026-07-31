# Fluorita F2 spike — measuring the decode backend

This directory is a **measurement harness, not product code**. It adds no
dependency to `celestina-rs`, is imported by no application, and ships nothing:
it drives the media stacks already installed on the machine so that F2's backend
decision rests on numbers instead of preference. The winning dependency only
enters `fluorita-engine` after the author approves it.

## What is compared

| Candidate | Why it is in the running |
|---|---|
| `libmpv` | one library, hardware decode, Wayland-native output, an embeddable render API (`mpv/render_gl.h`) |
| GStreamer | explicit pipelines, a VA-API plugin per codec, a Qt6 sink that would carry frames into Qt Quick |
| FFmpeg (libav*) | the decoder every other candidate already links; maximum control, maximum code we own |
| Qt Multimedia | already in the toolchain and trivially embeddable; the suite's non-goals rule it out as a *default*, not as a measured option |

## What is measured

- **Installed closure** — recursive package delta over the Qt Quick baseline,
  and over baseline + FFmpeg, since every candidate pulls FFmpeg anyway.
- **Decode cost** — time to first frame, throughput without pacing, CPU seconds
  per second of decoded content, peak PSS. Hardware modes are named apart
  (`hw-copy` returns frames to RAM, `hw-gpu` keeps them on the GPU) because
  comparing one candidate's `hw-gpu` against another's `hw-copy` would invent a
  difference that does not exist.
- **Derived resources** — metadata extraction, video poster, embedded cover and
  a bounded trailer, validated against the budget `fluorita-core` already froze
  (5 s · 1280×720 · 24 MiB), plus how a cancelled job dies.
- **Qt Quick integration** — whether each candidate's render path actually
  exists on this machine.

## Running it

```bash
python3 fluorita/spikes/run_all.py --source ~/Vídeos/clip.mp4 --cover ~/Imágenes/foto.jpg --out /tmp/fluorita-spike
```

Fixtures are derived from real media (a short loop of the author's own files),
written only under `--out`; no file of the user's is touched. The report lands
at `<out>/report.md` with the raw JSON beside it.

The presentation pass is separate because it **opens windows on the live
session**:

```bash
python3 fluorita/spikes/measure_presentation.py --out /tmp/fluorita-spike --abrir-ventanas
```

## A note on `qt_probe.qml`

It is a throwaway measurement probe, deliberately outside any `qml/` tree, and
it is not registered with a QML module. It is **not** a precedent for landing
product QML in Fluorita: that still has to arrive together with its guard-list
additions and negative fixtures, as the roadmap's host milestone records.

## What it cannot tell you

A headless pass measures decoding, not playing. Frame pacing, tearing, VSync
behaviour and the true cost of Qt Quick composition need a real Wayland surface —
and `libmpv` in particular cannot even create its VA-API device without one, so
every headless number for it is its *worst* configuration. Read the tables as
one input to the decision, never as the decision.
