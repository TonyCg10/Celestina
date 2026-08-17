# The fall stays behind the bar, and the popup menus join the block

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-L`
- **Artifact:** Celestina 0.29.9
- **Environment:** the author's 60 fps recording of the 4K nest (scale 1.5),
  decomposed to frames; the repository's offscreen suite
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author reported two residues after the one-block unit: every falling
menu still painted in front of the bar, and the tray, phone and performance
menus still opened out of step with their material — while the calendar, the
launcher and the control centre now formed correctly. Their recording was
read frame by frame around each open.

## Result

### The body painted over the bar in the silhouette's absent frames

The frames show the calendar's body from the screen's top edge, covering the
bar's own clock, during the first third of the fall — then the seam clip
takes hold and the rest of the entry is correct. The entry window's clip
started at the seam only while `edgeShapeActive` was already true; the first
frames of a fall can run before the silhouette is built, and those are
exactly the frames the recording caught. A top attachment now clips at the
seam for the whole entry, silhouette or not, and for the whole of
`attachmentProgress < 1`.

### The popup menus' material still led their rows

The tray, phone and performance menus ride a Qt `Menu` popup and set
`animateReveal: false`, so the glass guard from the one-block unit — keyed
on `animateReveal` — did not cover them: their compositor material still
armed on expose, frames before the popup's enter transition. The guard is
now unconditional on `revealed` alone, which every route sets — the popup
route on `aboutToShow` — so their material arms with their own transition,
the same contract every other surface now keeps.

## Limits

Both fixes are in the exact code paths the frames indict, and the suite
holds at 23/23 — but the falling routes cannot be driven from here: the
proof that the body now emerges from behind the bar on a real click is the
author's, on this build, which the nest is running.
