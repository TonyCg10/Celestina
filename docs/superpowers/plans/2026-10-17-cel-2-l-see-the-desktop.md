# CEL-2-L — See the desktop: implementation plan

- **Design:** [2026-10-17-cel-2-l-see-the-desktop-design.md](../specs/2026-10-17-cel-2-l-see-the-desktop-design.md)
- **Ledger row:** CEL-2-L in
  `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`
- **Worktree:** `scripts/worktree.sh open celestina CEL-2-L` →
  `unit/celestina/CEL-2-L`
- **Rules every task keeps:** `AGENTS.md` (root and `celestina/`); English
  code and comments, Spanish only in `celestina/src/copy.rs`; no `unsafe`, no
  production `unwrap`/`expect`; a typed error keeps its source; tests in the
  same task as the code; shared files (`app.rs`, `command.rs`, `message.rs`,
  `copy.rs`, `smoke.sh`, `Cargo.toml`/`Cargo.lock`, the ledger, `STATUS.md`)
  change **minimally and additively**. **Subagents never run `git stash`,
  commit, push or land**; the orchestrator commits nothing either: the
  worktree is left with the diff for the author's landing request.
- **Test windows only.** Anything that captures runs in the nested niri of
  `scripts/dev-session.sh` with test clients the script starts; never against
  `WAYLAND_DISPLAY` of the author's session. No `pkill` by name.

## Task list

Sizes are heuristic. Model and reasoning per task are in the table the author
approves before execution.

### T1 — `kdl` spike and manifest (S)

Add `kdl` to `celestina-rs/crates/celestina-shell/Cargo.toml` with a comment
that justifies it (reads the niri config the shell never writes). Prove the
**v1 syntax** of niri's config parses: copy the author's
`~/.config/niri/config.kdl` read-only into the test fixtures **trimmed to the
node shapes it uses** (`binds`, `binding-mode "x" exit-on-action=true`,
`repeat=false`, `hotkey-overlay-title`, comments, `spawn` with several
strings) — no personal paths. If the crate cannot parse it with any feature
set, stop and report; do not write a parser. Done when a test parses the
fixture and the lockfile diff is only `kdl` and its tree.

### T2 — `keybinds` model (M)

`celestina-shell/src/keybinds.rs`, pure: `parse(&str) -> Result<Vec<BindGroup>, KeybindError>`;
`BindGroup { name: Option<String>, enter: Option<String>, binds: Vec<Bind> }`,
`Bind { chord, title, action, program }`: the `hotkey-overlay-title`, the
raw action token and, for `spawn`, the first argument's basename, kept apart;
the panel composes the label (title, else Spanish copy for `spawn` and
`spawn-sh`, else the raw token verbatim). Bounds: `{` and `/* */` nesting
checked before the recursive parser runs, file ≤ 256 KiB, ≤ 512 binds, ≤ 24 groups, chord ≤ 48 and
label ≤ 96 chars (bounded with `crate::bounded`), `include` depth ≤ 2 and only
inside the config's directory (the reader resolves, the parser only reports
`include` paths). Tests: fixture groups, titles, hostile giant/garbage input,
a bind with no action, unknown nodes ignored.

> **Design change, 2026-10-17 (author's decision).** niri 26.04 reports
> `tile_pos_in_workspace_view = null`, so there are no per-window thumbnails.
> T3, T5, T6, T8 and T9 below describe the delivered design: one
> whole-output preview of the panel's output only (`PREVIEW_MAX_SIDE` 640,
> at most 1600 KiB, about 1.6 MiB; `MAX_PREVIEWS = MAX_OUTPUTS` bounds a
> reply), keyed by the workspace active on that output and drawn behind that
> workspace's strip; window tiles are geometry only.

### T3 — `thumbnail` scale and preview (M)

`celestina-shell/src/thumbnail.rs`, pure: `Rect`, `clip(rect, output) -> Option<Rect>`,
`crop_scale(frame: &Frame, rect, max_side) -> Result<Thumb, ThumbError>` where
`Frame { width, height, stride, format, data: &[u8] }`; accepts only
Argb8888/Xrgb8888 (little-endian BGRA bytes), outputs RGBA8, box-filter
downscale so the longer side ≤ `PREVIEW_MAX_SIDE` (640; the whole output is
the rect), refuses sizes over 8192×8192, strides that do not fit and `data`
shorter than `stride*height`. Mean luma and left/right half lumas of a
preview (for the smoke) live in the capture module. Tests: clipping edge cases, odd sizes, stride padding,
hostile dimensions, scale-factor rounding, 1×1.

### T4 — niri model keeps the layout (S)

`providers/niri.rs`: `Window` gains the layout numbers the tiles draw
(`tile_size` and a derived `column`/`row`; not the view position or the
window size, so a scroll alone republishes nothing); `reduce` fills them; `WindowLayoutsChanged` already flows.
Additive: existing `Window` constructors in tests get the new fields through a
default. Tests: reduce keeps the numbers; a layout change updates only that
window; a floating window has no column.

### T5 — capture worker (L, security-sensitive)

`celestina/src/capture/` — one `wlr-screencopy` frame per output on a Wayland
connection of its own, in a worker behind the `Worker` guard (shape of
`energy/idle.rs`). API: `CaptureHandle::request(CaptureRequest { generation, outputs }) -> Result<(), Busy>` with one
`OutputRequest { name, logical_width, logical_height, scale, workspace_id }`
per output (the app names only the panel's output), a typed
`Message::Captured(CaptureReply { generation, result })` (`Done { previews:
Vec<(workspace_id, Thumb)> }` or `Unavailable`) into the loop's channel, a
`CAPTURE_TIMEOUT` of 400 ms, `wl_shm` pool via `memfd_create`, the dimension
and format refusals of T3, the frame freed before the reply, one whole-output preview per output
produced by `thumbnail::crop_scale` (`MAX_PREVIEWS = MAX_OUTPUTS`, at most
1.6 MiB each), a 128 MiB budget for frames in flight, one in flight at a time
(the in-flight mark held by a guard), deterministic shutdown. A
missing global or `failed` event is `CaptureResult::Unavailable` with one log
line. **Never** logs or stores pixels. Tests: request/queue/Busy/shutdown
behaviour through a `Spawner` seam as `energy/mod.rs` does; the protocol runs
in the nest (T9).

### T6 — panel: state, Desktop mode (M)

`celestina/src/overview/{mod,state,view}.rs`, modelled on `launcher/`:
namespace `celestina-overview`, `Layer::Overlay`, keyboard `Exclusive`,
Esc/outside-click/focus-loss close. State: mode, selection over tiles built
from `workspace_map` shares (the module's `Window` is fed from the T4 model),
workspace previews keyed by workspace id (`set_preview`, uploaded in the next
frame, released on close). View: workspace strips, tiles with icon and title
(grapheme cut), geometry only, a rounded dimmed preview backdrop behind the
strip's tiles when the workspace has one (fitted to the output's aspect),
focus accent for the focused window, roles/names, `visualFocus`, reduced-motion. Actions: `Focus(window)`,
`FocusWorkspace(id)`, `SwitchMode`, `Close`. All strings in `copy.rs`. Tests:
selection movement across columns/rows/workspaces, empty model, tile geometry
from shares, view tree for a strip with and without a preview backdrop.

### T7 — panel: Shortcuts mode and the config reader (M)

`providers/keybinds.rs`: resolve path (`$NIRI_CONFIG`, then
`$XDG_CONFIG_HOME/niri/config.kdl`, then `~/.config/niri/config.kdl`), read
bounded off the UI thread, follow `include`s through T2's report, read once
each time the Shortcuts mode opens (no watcher, no standing thread);
missing/unreadable/over-size
→ `Unavailable` with one line. `overview/view.rs` gains the Shortcuts mode:
scrollable groups, chord and label rows, one explanatory line when
unavailable. Tests: path resolution with a private environment, the view for groups, empty and unavailable.

### T8 — verbs and app wiring (S)

`command.rs`: `overview toggle|open|close`, `shortcuts toggle|open|close`
(parse, `words()`, the usage text, the tests table). `message.rs`: `Captured`
and `OverviewClosed`. `app.rs`: an `overview: Option<Overview>` field, the
open sequence (capture → panel after reply/timeout; Shortcuts opens at once),
input/frame routing, close with the output (as the launcher does), the focus
action sent through the existing niri request queue. `celestina --check`
reports only the keybinds config state (path and what reading it gives); it
opens no Wayland connection, so it does not report the capture global. Tests: the open/close
state machine through the existing app test seam.

### T9 — nested smoke (M)

`scripts/smoke.sh`: one more nested run with two test windows (plain
coloured clients the script starts, one under `block-out-from
"screen-capture"` in a throwaway niri config), `celestina overview open`,
hold two seconds, `shortcuts open`, close; asserts exit 0, both verbs logged,
a frame on `celestina-overview`, the capture logged once per output as one
preview, the preview non-black, and of its two halves (the windows are two
columns side by side) the one holding the unblocked window painted and the one
holding the blocked window black (the worker logs mean luma of the preview and
its halves, not pixels), and no frame at rest. Skipped with a notice
without niri, like the others.

### T10 — documents (S)

Evidence `celestina/docs/evidence/2026-10-17-see-the-desktop.md` (what ran,
what remains), `VALIDATION.md` rows `VAL-CEL2-OVERVIEW` and
`VAL-CEL2-SHORTCUTS` as `pending`, the ledger row (`active` → evidence link;
the landing closes it), `STATUS.md` one additive paragraph, README verbs.

### T11 — verification and review (M)

`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`,
`celestina --check`, `celestina/scripts/verify-production.sh`,
`celestina/scripts/smoke.sh --binary <built>`,
`bash scripts/check-architecture-contract.sh`, the language and documentation
guards. Then the review by a model other than each implementer, at high
reasoning: T5 (capture) gets its own review; the rest one pass over the diff.
Fix pass, re-run, evidence updated. Production build/verify runs only through
the entries the landing uses, never started against the author's session.

## Landing

Not part of this task: **no land and no push** without the author's direct
authorization in the chat. Before landing, rebase onto the new `origin/main`
tip as the program's one-by-one rule says
(`git rebase --onto origin/main <previous tip>`), keeping the other units'
duties in `app.rs`, `command.rs`, `message.rs`, `copy.rs`, `smoke.sh` and the
ledger.
