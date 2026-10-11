# ADR 0009: Fluorita edits the media it indexes, and adds no encoder

> Amended on 2026-10-10 for one operation, a video's duration trim by an
> `ffmpeg` child process: see
> [the amendment](#amendment-2026-10-10-a-videos-duration-trim-prv-1).

- **Date:** 2026-08-19
- **Status:** accepted

## Context

Fluorita's user contract lists what the product is not, and one of those
entries — `tag editor` — was written when the application only had to show
media. It has since become the reason a person looking at their own photograph
in this library cannot turn it the right way up, cut it down, write a word on
it, or cover a number plate before sending it. The library can name, open,
describe and trash an item; it cannot change one. The author asked for that to
change, and for the result to stop short of an editing suite.

Two facts decide the shape of the answer.

The first is that the decode backend cannot help. libmpv was chosen because it
arrives as a complete playback engine, and playback is all it is: it does not
encode. Anything that rewrites samples or pixels therefore needs machinery the
dependency closure does not have. The obvious candidate is FFmpeg, and it is a
large one — a second media stack, a new hostile-input surface, long-running
jobs that need their own progress and cancellation model, and a much heavier
production verification.

The second is that the closure already contains an image writer nobody had
looked at: the Qt toolkit that reads and displays these pictures also writes
them. `fluorita/src/image.rs` already bounds what may be read, and
`cpp/imageprobe.cpp` already measures a file without decoding it. Adding pixel
output for images costs no dependency at all.

Those two facts do not divide editing into "easy" and "hard". They divide it
into operations that reorder existing bytes and operations that produce new
ones, and that line runs straight through the middle of what a person would
call the same feature. Rotating a JPEG can be a metadata change; cropping the
same JPEG cannot. Trimming a video between two keyframes is a remux; trimming
it one frame later is a re-encode. Presenting both sides of that line as one
undifferentiated "edit" would make the product quietly lossy, which is the
failure the suite's other editor — Grafita — exists to refuse.

## Decision

**Fluorita edits the media it indexes.** The `tag editor` exclusion is
withdrawn from its user contract and replaced by a narrower and truer one:
Fluorita is not an editing suite. It has no layers, masks or blend modes, no
configurable brushes or gradients, no cloning or non-rectangular selection, no
per-channel colour correction, and no rich text. It edits what its own library
already shows.

**Every operation is classified before it runs, and the classification is part
of the contract, not an implementation detail.**

- *Lossless* operations reorder or re-describe the original bytes. Orientation
  taken as metadata, tag and cover changes, and container-level cuts that copy
  streams belong here.
- *Raster* operations produce a new image. Crop, resize and every annotation
  belong here.

`fluorita-core` owns the matrix that answers, for a given item, which
operations it admits and which class each one falls in. The surface must
distinguish the two; an interface that offers them identically is a defect,
because it lets a person believe an original survived when it did not.

**No encoder enters the closure under this decision.** The image writer is the
toolkit that already reads these files. Video and audio editing is bounded to
demux and remux, which means a cut lands where the keyframes are and is
described that way rather than silently approximated. Frame-exact cutting,
format conversion, video scaling, audio normalisation and clip or GIF export
are not refused on principle — they are refused *here*, and require their own
decision carrying the encoder's cost, its input-hardening obligations and its
verification weight.

**An edit never destroys its input silently.** Saving has exactly two outcomes
and the person chooses between them:

- *Copy* writes beside the original and leaves it untouched.
- *Replace* writes the new bytes through the suite's atomic replacement and
  sends the original to the desktop Trash through `siderita-ops`. It is never
  an `unlink`, and the destination is confirmed before the source moves.

**An edit is reopenable only while its base survives.** The composed stack of
operations is persisted beside the catalogue, keyed by media identity — never
as a sidecar file in the folders the person mapped, because those folders are
theirs and Fluorita does not put things in them. A copy therefore stays
reopenable: the original it was computed from is still on disk. A replacement
flattens: the base is gone, and a stack that described it would be applied a
second time to bytes that already contain it. The product states that
difference rather than discovering it.

**The output format is a rule, not a question.** The result keeps the
original's format when that format can carry it, at high quality for lossy
ones, and falls back to PNG when it cannot. There is no format dialogue.

**Editing belongs to the standalone application.** Siderita's embedded surface
keeps content, honest state and supported transport. One editing implementation,
in one host.

## Consequences

- `fluorita/README.md` loses `tag editor` from its exclusion list and gains the
  editing-suite limit and the two save outcomes. The user contract changes; this
  is the record of why.
- `fluorita-core` gains the capability matrix and the edit stack, and stays
  free of Qt and of decode. `fluorita-engine` gains the image writer and the
  stack's persistence. The split is the existing architecture direction: what an
  edit *is* has no filesystem in it, and what it *costs* has no domain rules in
  it.
- The existing byte and pixel budgets are now read paths *and* write paths. An
  item over one of them is refused before an edit allocates, exactly as it is
  refused before a view decodes.
- Annotation coordinates are image coordinates. On a scaled display a stroke
  stored in window pixels lands where it was not drawn; that is the defect this
  clause exists to prevent, and it is the reason the author validation for this
  work is performed on the real display rather than offscreen.
- Trimming video and audio will produce cuts at keyframe boundaries. That is a
  visible limitation, is stated to the person, and is the strongest evidence
  that could later justify the encoder decision.
- Metadata editing, stream-copy trimming, frame extraction and batch
  application are authorised in intent by this decision but not opened by it.
  Each needs the stack and the writer to exist before its result is
  describable, and each opens as its own checkpoint.

## Revisit when

A cut landing at a keyframe rather than at the chosen frame becomes the
author's real obstacle in real use; an export the library cannot produce blocks
something the author actually does; or the toolkit's image writer is measured
losing quality or metadata a lossless path would have kept. The first two are
the evidence an encoder decision would need. The third is a defect in this one,
and is repaired here rather than answered with FFmpeg.

## Amendment 2026-10-10: a video's duration trim (PRV-1)

The author's capture preview (`PRV-1`, the
[design](../superpowers/specs/2026-10-10-capture-preview-design.md) §6)
names the case the "Revisit when" above foresaw: a screen recording has a
keyframe a second, so a cut at the keyframes lands up to a second away from
the frame chosen, which for a clip about to be sent is the real obstacle.

**One operation is opened, and only one: trimming a video's duration,
frame-accurately.** It is *raster*-class: the span kept is re-encoded into a
new file (H.264 and AAC in MP4) and the surface says so, a new film, before
it is saved. The encoder is `/usr/bin/ffmpeg`, run as a **child process
and never linked**: it ships in the `ffmpeg` package libmpv already needs, so
nothing new is installed and no encoder enters Fluorita's link closure. The
child gets a null standard input, writes a hidden sibling of the original,
reports its progress on a pipe, is killed and its file removed on a cancel,
and its file takes a name only after it exits 0 having written a frame. A
trim always writes MP4 holding the film's main video and one audio stream;
every other stream (further audio tracks, subtitles, cover pictures, data)
is left out, and the surface says so, with any change of container, before
the save. The video encoder is chosen the way Selenita's recorder chooses
between its own: VA-API when a render node exists and `ffmpeg` lists
`h264_vaapi`, else `libx264`, each at a constant quality (QP 20, CRF 21).
Unlike the recorder, which uses GStreamer's encoders and a fixed render
node, the trim takes the first render node it finds, and a VA-API run that
fails before anything is published is tried once more with `libx264`. A
missing `ffmpeg`, a missing encoder and a failing child are each said in
Spanish, and none of them touches the original.

Everything else in this decision stands. The two save outcomes apply to a
trim exactly as to a picture: «Guardar ambas» lands a copy beside the
original, «Guardar solo la editada» sends the original to the desktop Trash
once the result exists, never an `unlink`. Pictures keep every rule above
unchanged — the toolkit is still their only writer. Cropping a video's area,
removing its sound, changing its speed, joining clips, format conversion and
clip or GIF export remain refused here, each needing its own decision; the
child process is not a licence for them.
