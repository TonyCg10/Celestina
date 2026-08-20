# ADR 0009: Fluorita edits the media it indexes, and adds no encoder

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
