# Capture preview — design

- **Date:** 2026-10-10
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Products:** Selenita (the corner preview) and Fluorita (the floating
  editor, now able to trim a video)
- **Identifiers:** program `PRV-1` (suite plan
  `docs/plans/active/2026-10-10-capture-preview.md`); units `PRV-1-A` and
  `PRV-1-E` (suite), `SEL-2-A` (Selenita), `FLU-P1-A` and `FLU-P1-B`
  (Fluorita)

## 1. Goal and scope

After a screenshot or at the end of a recording, the result appears as a
small floating preview in a corner of the screen, as macOS and Android do,
instead of Selenita's window. From the preview the file can be dragged into
any program that takes files (WhatsApp, Slack, the browser, Siderita): this
is how a video, which the clipboard cannot carry, reaches a chat. Clicking
the preview opens the file in a floating Fluorita editor; leaving it alone
changes nothing, because the normal flow (the file in `Capturas` or
`Recordings`, the history, the clipboard) has already happened.

An edit ends with the author's choice between the two outcomes Fluorita
already offers, named as the author named them: «Guardar ambas» (a copy
beside the original) and «Guardar solo la editada» (a replacement; the
original goes to the Trash).

In scope:

- Selenita: the corner preview after every capture and every finished
  recording, whatever started it; dragging the file out; clicking to edit;
  dismissing; Selenita's main window no longer shown after a key-binding
  capture; `Adopt(path)` so an edited copy joins the history.
- Fluorita: `org.celestina.Fluorita1.Edit(path)` and `fluorita --edit
  <path>`; a floating edit window, separate from the library, on any path;
  the existing picture editor in it; a new video mode that trims the
  duration, frame-accurate.
- Two niri window rules in the author's configuration (the preview in the
  bottom-right corner without focus; the editor floating and centred).

Out of scope: cropping a video's area, removing its sound, speed changes,
joining clips; annotations on a video; a preview for anything other than
Selenita's own results; sharing targets inside the preview (the drag is the
sharing); any change to the halted shell.

## 2. Constraints the design honours

- Zero new external dependencies (the author's rule, restated 2026-10-10).
  The preview is placed by a niri window rule, not by layer-shell-qt. The
  trim runs `/usr/bin/ffmpeg`, which ships in the `ffmpeg` package Fluorita
  already depends on through libmpv; nothing new is installed or linked.
- ADR 0012 (suite conventions): activation through
  `celestina_core::activation`, paths byte-exact, an application's own
  interface beside the shared one; the activation scanner's allowlist
  learns each new literal.
- ADR 0009 (editing without an encoder) is amended for one operation: a
  video's duration trim, a *raster*-class operation (it produces a new
  file and the interface says so), run by an `ffmpeg` child process, never
  linked. Pictures keep every rule of ADR 0009 unchanged.
- Style: tokens only, glass canvas, motion 100–500 ms honouring
  `reducedMotion`, icon-first actions, Spanish only in `qsTr()`, English
  development text, no blocking IO on the Qt thread, typed errors with
  `message_es`, no production `unwrap`.
- Ratchets never grow (qmllint per application, radius, glass canvas,
  architecture, language).

## 3. Facts the design rests on (verified 2026-10-10)

| Fact | Evidence |
|---|---|
| niri cannot let a client place its own window; window rules can (`open-floating`, `default-floating-position … relative-to="bottom-right"`, `open-focused false`) | the author's `config.kdl` already uses `open-floating` rules |
| Fluorita's picture editor exists with the two save outcomes (copy beside, replace with the original to the Trash) | `fluorita/src/editor.rs`, `EditToolbar.qml` («Guardar una copia», «Reemplazar el original») |
| Fluorita's editor today opens items the library holds | Fluorita README, user contract |
| `/usr/bin/ffmpeg` 9.0.2 is installed from the `ffmpeg` package, the one libmpv links | `pacman -Qo /usr/bin/ffmpeg` |
| Selenita's recordings are H.264 with a keyframe every 60 frames, so a lossless trim would be 1 s coarse | `selenita-core` record settings (`key-int-max=60`) |
| Fluorita's hardening plan (`fluorita/docs/plans/active/2026-09-26-hardening.md`) is still `active` with every row done | the plan file |

## 4. Selenita: the corner preview (`SEL-2-A`)

### 4.1 When

After a capture is published and after a recording is published, whether
the capture came from a key binding, `org.celestina.Selenita1.Capture` or
the window. A launch that captures from a key binding no longer reveals the
main window when it ends: the preview is the only thing shown. A window
already open stays as it is. A new result replaces the preview on screen.
Nothing about the save changes: the file, the history row and the clipboard
(when on) exist before the preview appears.

### 4.2 The window

A second top-level QML window of Selenita, frameless, titled
`qsTr("Vista previa")`, about 240 px wide at the picture's aspect ratio
(clamped between 140 and 240 px high), on the glass grammar with the
radius tokens. niri places it: the rule matches Selenita's app id and that
title, opens it floating at the bottom-right of the focused output with a
24 px margin and without focus.

### 4.3 What it shows

- a picture: the capture itself, scaled;
- a recording: its first frame, extracted to a hidden PNG in the runtime
  directory by a short `gst-launch-1.0` child (Selenita's existing runner
  and tool seam), its duration and the film glyph.

### 4.4 What it does

| Gesture | Result |
|---|---|
| drag the preview | a `text/uri-list` drag of the file, offered as a copy (`Qt.CopyAction` only); a drop target that asks to move is refused |
| click | `Fluorita1.Edit(path)`; the preview closes |
| the × button | the preview closes |
| nothing for 5 s | the preview fades out and closes |
| pointer over the preview | the 5 s timer stops; it restarts on leave |
| the file disappears (trashed from the history) | the preview closes |

The entry and exit are a 200 ms fade and slide; with `reducedMotion` they
are immediate. Keyboard: the preview takes no focus; it is a pointer
affordance, and every action stays reachable from the main window's
history, which is what screen-reader users already have.

### 4.5 Handing off to Fluorita

`Edit(path)` goes through `celestina_core::activation::open_in`-style
plumbing on the capture worker: if `org.celestina.Fluorita` is owned, the
call goes to `org.celestina.Fluorita1.Edit`; otherwise Selenita spawns
`fluorita --edit <path>` detached. A failure is a notice in the preview
(`qsTr("No se ha podido abrir Fluorita")`) and the file is untouched.

### 4.6 `Adopt(path)`

`org.celestina.Selenita1` gains `Adopt(path)`: the path joins the history
as a new row of the right kind if it is a file in Selenita's folders, else
it is ignored. Without a running Selenita, Fluorita appends the row to
Selenita's history file through `selenita-core`'s `History` (the same
atomic replace the record worker uses).

## 5. Fluorita: the floating editor (`FLU-P1-A`)

- `org.celestina.Fluorita1`, Fluorita's own interface beside the shared
  activation, with `Edit(path)`. `fluorita --edit <path>` claims the name
  and starts in edit mode without the library window; a second
  `--edit` reaches the running instance.
- The edit window is a separate top-level window titled
  `qsTr("Editar — %1")` with the file's name; niri opens it floating,
  centred, with focus. An open library window is left as it is. Several
  edit windows may coexist, one per file.
- A picture opens in the existing editor whether or not the library holds
  it: the editor's open path takes a plain path (stat and header read on
  the worker, as today) instead of a catalogue item; the recipe store keys
  by path as it already does.
- The save outcomes keep their behaviour and take the author's names:
  «Guardar ambas» (copy beside, default) and «Guardar solo la editada»
  (replace; the original to the Trash, never deleted). After «Guardar
  ambas», Fluorita calls `Selenita1.Adopt(copy)`.
- Closing an unchanged window closes it. Closing with changes asks:
  «Guardar ambas», «Guardar solo la editada» or «Descartar».
- The saved result can be dragged out of the edit window like the preview.

## 6. Fluorita: trimming a video (`FLU-P1-B`)

- A video opened through `Edit` shows the player and, beneath it, a time
  bar with two handles (start, end). Dragging a handle seeks the view to
  that frame; play plays the chosen span only.
- Saving runs, on a worker, `ffmpeg -hide_banner -nostdin -ss <start> -to
  <end> -i <original> …` with the video re-encoded by the suite's rule
  (VA H.264 at constant quality when a render node and `h264_vaapi` exist,
  else `libx264 -crf 21 -preset veryfast`) and the audio copied when the cut
  allows it, else re-encoded to AAC. The output is written hidden beside
  the original and published only after `ffmpeg` exits 0; progress comes
  from `-progress pipe:1`, cancellation kills the child and removes the
  hidden file.
- The two outcomes apply as for pictures. A trim whose handles are at the
  ends saves nothing.
- Errors (no encoder, `ffmpeg` missing or failing) are notices in Spanish;
  the original is never touched.
- ADR 0009 gets the amendment of §2 in this unit.

## 7. niri window rules (`PRV-1-E`)

Added to the author's `~/.config/niri/config.kdl` by the exit unit, after
the author's approval, and documented in both READMEs:

```kdl
window-rule {
    match app-id=r#"^org\.celestina\.Selenita$"# title="^Vista previa$"
    open-floating true
    open-focused false
    default-floating-position x=24 y=24 relative-to="bottom-right"
}
window-rule {
    match app-id=r#"^org\.celestina\.Fluorita$"# title="^Editar — "
    open-floating true
}
```

## 8. Delivery

| Unit | Kind | Outcome |
|---|---|---|
| PRV-1-A | suite | this spec and the plan; ADR 0012 follow-up (`Fluorita1.Edit`, `Selenita1.Adopt`); the scanner's allowlist; the suite ledger |
| SEL-2-A | selenita | §4 |
| FLU-P1-A | fluorita | §5 |
| FLU-P1-B | fluorita | §6 and the ADR 0009 amendment |
| PRV-1-E | suite | §7, Siderita/README cross-references, hygiene, exit |

Order: PRV-1-A, then SEL-2-A and FLU-P1-A in parallel (they meet only at
the two D-Bus methods fixed by PRV-1-A), then FLU-P1-B, then PRV-1-E.
Before FLU-P1-A, the author archives Fluorita's finished hardening plan by
hand so Fluorita has one active plan (`2026-10-10-prv-1-preview.md`).
Selenita's unit goes to a new `SEL-2` plan for the same reason: the
author also archives the SEL-1 plan (`selenita/docs/plans/active/2026-10-09-sel-1-foundation.md`,
every row done through SEL-1-F) by hand first.

## 9. Verification

- Selenita: QML tests of the preview over the fakes (appears after a fake
  capture and a fake recording; the timer; hover holds it; × closes; a new
  result replaces it; the drag's MIME data and action; a trashed file
  closes it; no main window after a key-binding capture); crate tests of
  `Adopt`; smoke reports the preview shown.
- Fluorita: `Edit` and `--edit` over fixtures; the editor opening a path
  outside the library; the ffmpeg argv builder; a 3 s fixture video trimmed
  to [1.0 s, 2.0 s] measured with `ffprobe` at 1.0 s ± one frame; cancel
  leaves no file.
- Author live checks (VAL entries): the preview in the corner without
  stealing focus; dragging a capture and a recording into WhatsApp and
  Slack; editing a picture and a video and both save outcomes; the copy in
  Selenita's history.

## 10. Decisions recorded here

- The preview belongs to Selenita (instant, it holds the file); editing
  belongs to Fluorita (the editor exists there).
- Placement by niri window rules, not layer-shell: no new dependency.
- Video trimming is frame-accurate by re-encoding through an `ffmpeg`
  child; ADR 0009 is amended for that one operation.
- The save outcomes are Fluorita's existing two, renamed after the author.
