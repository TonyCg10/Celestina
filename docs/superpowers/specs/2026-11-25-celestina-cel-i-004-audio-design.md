# CEL-I-004: output and input device selector and per-application volume

- **Date:** 2026-11-25
- **Status:** approved by the author on 2026-11-25 ("Aprobado, haz el plan")
- **Track:** improvements (`CEL-I`), parent design
  `docs/superpowers/specs/2026-11-24-celestina-standing-tracks-design.md` §6.7
- **Absorbs:** `CEL-I-005` (device selector), which is delivered in this entry

## 1. Intent

The author wants two things from the bar's volume menu:

- choosing which output and input device the session uses;
- setting the volume of each application that is playing sound.

The author marked this urgent on 2026-11-25. It moves ahead of the design entries (`CEL-D-001` onwards). The parent design had placed it after the anchored panels.

## 2. Approach (the author's choice)

1. **Chosen: extend today's level popup (`src/osd/popup.rs`, an `xdg_popup`) now.** When `CEL-D-003` lands, it moves the whole popup body, these sections included, into the anchored panel.
2. Rejected: wait for `CEL-D-002`/`003`. That puts several units between the author and a function they need now.

## 3. What the author sees

The volume popup, top to bottom:

1. The output heading and slider, and the mute switch (unchanged).
2. **Output device:** one row per sink, named as `wpctl status` names it. The default sink is marked. A click makes that sink the default.
3. The microphone heading and slider, and its mute (unchanged; only when a microphone exists).
4. **Input device:** one row per source, the same way.
5. **Applications:** one row per playback stream: its application name, its own slider and mute. Streams that only record are not listed. With no stream, one quiet line says there is nothing playing.

Copy is Spanish and lives in `src/copy.rs`.

### 3.1 Revisions after the author's look (2026-11-25)

- **Collapsible sections.** The output, input and applications sections are
  collapsible and open folded each time the popup opens. Folded, they show
  only the default output, the default input and the applications that are
  playing now. Unfolding a section by its header shows every row and grows
  the popup downward with its top edge fixed. Delivered in `CEL-I-004`.
- **Per-application mute.** Each application's mute is a round speaker button,
  struck through while muted, instead of a switch with no label. A muted
  application's name dims. Preview approved 2026-11-25 (variant A); delivered
  in `CEL-D-006`.

## 4. Decisions (Claude's calls, approved with the design)

- **One audio model.** The shell's audio worker (`src/providers/audio.rs`) stays the only audio reader and writer. It grows a graph of devices and streams beside its two default levels. There is no second watcher and no `pw-mon`.
- **Reuse the parser, not the process code.**
  - Cuprita's pure `wpctl status` parser moves out of `cuprita-core`'s `wpctl` feature into an ungated module, so the shell can call it without compiling Cuprita's audio process code. That keeps the CEL-2-F boundary: no `wpctl` feature in the shell.
  - It is a small unit of the `cuprita` project, because `cuprita-core` is Cuprita's.
- **When the graph is read.** While the duty is on and a layout holds the `volume` piece, the worker re-reads the graph on these `pactl` events:
  - `sink`, `source` and `server`;
  - new: `sink-input`.

  It settles them longer than level events (250 ms), so a wheel burst is one graph read, and it reads once at start. One read is `wpctl status` plus one `wpctl get-volume <id>` per playback stream.
- **Bounds.**
  - At most 8 devices per direction and 8 streams are shown; the rest are logged and dropped.
  - Names are cut to the row's width by the text layout, not by bytes.
  - An unreadable `wpctl status` is an empty graph, never a crash.
- **Size** (superseded in part by §3.1: folding and unfolding a section resizes the popup).
  - The popup's size is decided when it opens, from the graph at that moment, as today's popup decides on the microphone.
  - While it is open, existing rows update in place: the default mark, levels, mute.
  - A stream that ends greys out and stops taking input.
  - A new stream appears the next time the popup opens.
- **Writes.**
  - Default device: `wpctl set-default <id>`.
  - Stream level: `wpctl set-volume <id> <fraction> -l 1.0`, with the same fraction format and 100 % ceiling as the default devices.
  - Stream mute: `wpctl set-mute <id> toggle`.

  Each write is a worker command, refused when the id is not in the last published graph, and the next reading is the truth.
- **OSD.** A graph change never shows the on-screen display. A default-device switch made from the open popup moves the default readings. The popup's existing suppression keeps the display quiet, as for its sliders.

## 5. Anti-goals

- Any `wpctl` call on the event loop's thread.
- A popup that resizes or jumps while the pointer is in it.
- An agent writing to the author's real PipeWire. In nests, every audio write goes to the smoke's fake `wpctl`. A nest may read the real graph, but it never changes it.

## 6. Left to the builder

- Row anatomy (check mark or filled pill for the default).
- Exact spacing.
- Whether a stream row shows an app icon from `AppIcons` when its name resolves to one.
