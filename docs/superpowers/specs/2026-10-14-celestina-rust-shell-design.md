# Celestina shell in pure Rust — design

- **Date:** 2026-10-14
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** the suite's desktop shell for the author's niri session, rebuilt
  from scratch in pure Rust on Wayland: an Island on the top edge that grows
  into pages, a launcher, a notification server with banners and a system
  tray, all painted with the Celestina visual language over the compositor's
  glass
- **Identifiers:** project `celestina`, path `celestina/`, commit prefix
  `celestina:`, registry kind `rust-shell`, unit prefix `CEL`, binary
  `celestina`, bus name `org.celestina.Shell` with interface `Shell1`,
  layer-shell namespaces `celestina-*`
- **Replaces:** the halted Qt/QML shell, which this program moves to
  `celestina-qt/` (id and prefix `celestina-qt`), still halted, kept as history
  and rollback
- **Inputs:** the study
  `/home/toni/CODIGO/estudio-shell-rust-vs-cpp-2026-10-14.md`, the spike report
  `/home/toni/CODIGO/informe-spike-isla-2026-10-14.md` and its throwaway code at
  `/home/toni/CODIGO/isla-spike` (both outside the repository; the spike's
  conclusions are restated here so this document stands alone)

## 1. Goal and scope

The author wants one shell for a niri session that behaves like iNiR's iRiS
family (an Island on an edge that grows into pages, pieces that park on the
Island or on the screen, Customize on the shell itself) and looks like
Celestina (One UI adapted to a pointer desktop, the Haze glass, one accent,
calm neutrals). The halted Qt shell proved the product direction and the
provider code; it was halted because Qt/QML could not reach the Rust domain in
process and the layer-shell surfaces fought the toolkit. The study of
2026-10-14 compared a C++ toolkit of our own (Noctalia v5, 330 000 lines) with
Rust over mature crates and recommended Rust; the spike of the same day
confirmed it on the author's machine:

| Measured | Result |
|---|---|
| Frame time, client side, DP-1 at 3840×2160@165 Hz, scale 1.5 | p50 0.12 ms, maximum 2.2 ms |
| Frames in rest after an animation | 0 (one per minute for the clock) |
| Resident memory, two outputs with one surface each | 73 MB (Noctalia v5 in the same session, three outputs with wallpapers: 210 MB) |
| `xdg_popup` over a layer surface with its own blur and grab | works with smithay-client-toolkit 0.21 |
| Island capsule to page in 350 ms on the One UI curve | exact duration, no visible jump |

This program delivers the shell in one first release, `CEL-1`, in this order:

1. **Island** on the top edge of every output: a capsule with the clock and
   the output's workspaces that grows into pages: Activity (workspaces and
   windows), Notifications (history) and Tray.
2. **Launcher**: an overlay with a text field, the session's `.desktop`
   entries ranked as the halted shell ranked them, and `/` commands that flip
   the shell's own switches.
3. **Notifications**: the `org.freedesktop.Notifications` server, banners with
   actions and expiry, the history page, do-not-disturb.
4. **Tray**: StatusNotifierItem host and watcher, icons, nested DBusMenu
   popups, click and scroll.
5. **Shared tokens**: the style's tokens move from QML into one data file that
   generates both the QML singleton's data and a Rust crate, so shell and
   applications cannot drift.

Later deliveries, named so the architecture leaves room for them and designed
in their own specs: parkable pieces (Island, screen edge, floating) and
Customize on the shell; the full-width bar layout with zones and the side
edges; OSD for audio and brightness; the media piece (MPRIS); clipboard
history; night light and caffeine tools; session lock and the polkit agent
(ported from the halted shell under a security review, ADRs 0004 and 0005 of
`celestina-qt` stay their boundary); the Ink and Light schemes with Lume
(tint that rises with the wallpaper's luminance); wallpaper; accessibility
through `accesskit`.

Out of scope for good, by the author's decision: a dock of any kind (the
halted shell's ADR 0003 is carried over as this project's own decision),
plugins or scripting, several panel families, an anime colour layer, AI chat,
music streaming, and any compositor other than niri.

## 2. Decisions the author made in brainstorming

| Decision | Choice | Alternatives considered |
|---|---|---|
| Name | The shell **is** Celestina: same id, path and prefix as the project it replaces | a new mineral (Baritina, Anglesita) |
| The halted code | **Salvage by copy**: the author edits "Halted projects" so named modules of `celestina-qt/src/provider_adapter`, `celestina-qt/src/niri_adapter.rs` and `celestina-rs/crates/celestina-shell-core` may be copied into this project's crates and adapted to one process; the halted bytes are never edited | clean rewrite; lifting the halt of `celestina-shell-core` |
| Dock | **None.** Launcher and the Island's Activity page are the complete window-reach surface | parkable dock later; dock in the first release |
| Coexistence with Noctalia | **Claim only free names** (`DoNotQueue`): while Noctalia owns Notifications or the tray watcher, the shell shows the Island and the launcher and its pages say who holds the name | replace Noctalia from day one; validate only in a nested session |
| Identity in the repository | **Move the old shell to `celestina-qt/`** so the new one takes `celestina/`; the scanner's `shell` QML role stays satisfied by the old entry | new shell at `celestina-shell/`; old shell out of the registry into `docs/history/` |
| Painter | **femtovg over OpenGL ES 3 through EGL**, behind the `Painter` contract the spike proved | wgpu over Vulkan with an own SDF painter (1 500–2 500 lines, no need while niri blurs) |
| Text | **cosmic-text for every piece of text**: shaping, font fallback, variable weights, tabular figures, the editor; glyphs rasterised by swash into an atlas the painter draws | femtovg's own layout (no `tnum`, fixed weight, no fallback), or two stacks |
| Colour schemes | **Dark ships; the model is scheme-ready**: tokens are schemes as data and the shell reads `scheme` from `appearance.toml` once it exists; Ink and Light are a later checkpoint with their own decision record and measured contrast for shell and applications together | three schemes now; dark without a scheme model |
| Edge surface of the first release | **The Island alone on the top edge** with an exclusive zone of its own height; the full bar with zones and the side edges come later | full bar with the Island as a notch; both layouts now |

## 3. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`; typed errors; no
  production `unwrap`; `unsafe` only isolated, documented and approved; every
  dependency justified; bounded, cancellable, deterministically joined
  background work; hostile input (D-Bus, compositor, files) bounded.
- The halt: `celestina-qt/` and `celestina-shell-core` are never edited; only
  the modules the author's edit of "Halted projects" names are copied, and
  the copy records its origin in the module's head comment.
- Style contract (`celestina-style/DESIGN.md`): the Haze recipe exactly (σ ≈
  12 px blur by the compositor, grain at 0.15 under the tint, the canvas
  colour at 0.70 over it, no outline, no lit edge); radii 20/12/8/10; desktop
  density (rows 40, top bar 48); the One UI curve `(0.22, 0.25, 0, 1)` with
  durations 100/200/350 ms under a 500 ms ceiling; press recoil 0.96; Inter
  Variable at 400 and 600 with tabular figures; one accent `#3e91ff`;
  reduced motion makes spatial and scale motion instant.
- Shell-specific style rules from the halted shell's ADR 0002, which this
  project keeps: no full-width plate, no hard edges, borderless capsules whose
  own fill is transparent while compositor blur is armed and one readable
  opaque fallback when it is not; blur regions are finite and reapplied
  whenever a capsule moves.
- Language: development truth in English; product copy in Spanish through a
  file whose head says `language-contract: product-copy`.
- niri only. The compositor supplies the blur through
  `ext-background-effect-v1`; the shell never blurs. The author's
  `layer-rule` for `^celestina-(panel|panel-menu|panel-child-menu|overlay|toasts|osd)$`
  already sets `xray false` for every namespace this design uses.
- Privilege: the first release runs no privileged code. Lock and polkit are
  later deliveries with their own review.

## 4. Facts the design rests on (verified 2026-10-14)

| Fact | Where |
|---|---|
| The architecture scanner requires one registered QML project of role `shell`, derived from kind `qt-shell`; a kind it does not know owns no QML root | `scripts/architecture_scanners.py:715-784` |
| The commit guard refuses the prefixes and paths of a halted project unless the commit also edits "Halted projects" | `AGENTS.md` "Halted projects"; `scripts/test-commit-scope.sh:600-672` |
| The version contract refuses a `suite-maintenance` commit that changes an existing owner's registry entry, requires every owner named in `docs/version-history.tsv` to be registered and versioned, and keeps that file append-only byte for byte; the 20 rows owned by `celestina` end at 1.3.2 | `scripts/version_contract.py:449-494, 547-560, 623-676` |
| The staged-units guard refuses any staged inventory that exists at HEAD, so the 63 inventories under `celestina/docs/inventories/` cannot move with the tree; the language guard refuses a new baseline row for any path, and `scripts/language-baseline.tsv` holds 8 rows under `celestina/` and `scripts/radius-baseline.tsv` one row `9	celestina` | `scripts/check-staged-units.py:591-598`; `scripts/check-language-contract.py:293-300` |
| `scripts/check-architecture-contract.sh:327` names `celestina/CMakeLists.txt celestina/qml celestina` literally | the rename must update it |
| The old shell's unit prefixes were `AUD BUBBLE CAST DIAG LIVE LOCK LVR PANEL REC SURF UTF UX WMAP WSG`; `CEL` is unused in the monorepo | `celestina/ROADMAP.md`, every `docs/plans/active` |
| Noctalia owns `org.freedesktop.Notifications` and `org.kde.StatusNotifierWatcher` in the author's session; `focus-follows-mouse` is on; niri 26.04 (8ed0da4) runs as the author's own build at `~/.local/lib/celestina/niri` | `busctl --user list`, `~/.config/niri/config.kdl:7`, `pgrep` |
| The patched niri emits `MinimizedWindowsChanged`, an event `niri-ipc` 26.4 does not know; `Socket::read_events` fails and reconnects in a loop on it | spike task 5 |
| `wp_viewporter` and `wp_fractional_scale_v1` are present; sctk 0.21 binds `xdg_wm_base` only through its `WindowHandler`, so the spike bound it by hand (20 lines) and had no `delegate_xdg_*` macros | spike tasks 1 and 6 |
| The EGL config needs a stencil buffer or femtovg paints square corners | spike task 2 |
| `CelestinaTheme.qml` holds roughly 400 token properties and no data file exists; `appearance.toml` holds `reduced_motion` and `text_scale` with a watcher in `celestina-settings` | `celestina-style/CelestinaTheme.qml`, `celestina-rs/crates/celestina-settings/src/{appearance,watch}.rs` |
| Salvageable Rust: `provider_adapter` 11 400 lines (audio, brightness, clipboard, launcher, media, melibea, nightlight, notifications server, portal settings, recorder, session, settings, sysmon, wallpaper, weather, worker), `niri_adapter.rs` 1 900, `celestina-shell-core` 15 000 (pure state machines and policies, tested) | `wc -l` |
| Crate versions and minimum Rust: smithay-client-toolkit 0.21.1 (1.86), femtovg 0.27.0 (1.88), cosmic-text 0.19.0 (1.89), taffy 0.15, system-tray 0.8.9, freedesktop-icons 0.4.0, niri-ipc 26.4.0; the workspace toolchain is 1.97.1 | `cargo info`, `celestina-rs/rust-toolchain.toml` |

## 5. Shape of the system

```text
niri (compositor)  ── layer-shell, xdg_popup, fractional scale, background effect, niri-ipc ──┐
                                                                                             │
celestina (one process)                                                                      │
  calloop main loop ── Wayland queue, timers, one channel of typed messages                  │
  │                                                                                          │
  ├── celestina-wayland   outputs, layer surfaces, popups, scale, regions, EGL  ◄────────────┘
  ├── celestina-paint     Painter contract → femtovg; Haze; glyph atlas (cosmic-text); icons (resvg)
  ├── celestina-ui        node tree, taffy layout, Animated, hit-test, focus, text field
  └── src/                product: Island, launcher, notifications, tray; providers on worker threads
         │
         ├── celestina-shell (celestina-rs)   pure domain: notifications state, launcher ranking,
         │                                     workspace model, coalescing, durable settings
         ├── celestina-tokens (celestina-rs)   generated from celestina-style/tokens.toml
         ├── celestina-settings                appearance.toml + watcher
         └── celestina-core                    desktop entries, XDG, atomic files
```

Dependencies point from the product down to the toolkit crates and from
everything to the pure crates, never back. The three toolkit crates know
nothing about the Island or notifications; `celestina-shell` knows nothing
about Wayland, GL or D-Bus.

### 5.1 One process, one loop, worker threads

- The main thread runs `calloop` with the Wayland source, timers and one
  `calloop::channel` of `Message` values. Everything that touches a surface,
  the painter or the UI tree runs here.
- Every provider with blocking IO (D-Bus through zbus's blocking API, DDC,
  the niri event stream, the `.desktop` scan) runs on its own named thread
  started through the `Worker` guard salvaged from the halted adapter: a
  shared shutdown flag, join on drop, a panic reported by name. A provider
  publishes typed messages into the channel and receives typed commands
  through its own bounded `sync_channel`; it never holds a reference into the
  UI. This is the halted adapter's shape without the line protocol between
  processes.
- Bursty sources (niri events, tray property changes) are coalesced with
  `celestina-shell::coalesce` before they reach the loop, so a window title
  storm costs one redraw per frame at most.
- Shutdown: SIGTERM and SIGINT through calloop's signal source; the loop stops,
  workers are joined with a bounded wait, surfaces and the EGL context are
  destroyed, claimed bus names are released.

### 5.2 `celestina-wayland`

The spike's runtime, kept in shape and rewritten with the ledger's deferred
findings closed.

- `Runtime` owns the connection, `RegistryState`, `OutputState`,
  `SeatState`, `CompositorState`, `LayerShell`, the hand-bound `xdg_wm_base`,
  `WpViewporter`, `WpFractionalScaleManagerV1`, the background-effect
  manager and one `Egl` (display, config with 8-bit stencil, shared context).
  `wp_viewporter` and fractional scale are required: the binary exits with an
  error naming the missing global instead of falling back to integer scale.
- `Surface` has a `Role` (`Layer { layer, anchor, exclusive_zone, namespace }`
  or `Popup { parent, positioner }`), an EGL window, the current scale, the
  logical size, a `FramePacing` (redraw requested → frame callback → draw;
  zero frames in rest), a cached blur region and input region.
- `Shell` and `PopupShell` traits are the contract with the product:
  `frame(&mut FrameCx)`, `input(InputEvent, &mut SurfaceCtl)`,
  `message(Message, &mut SurfaceCtl)`. `FrameCx` carries surface id, logical
  size, scale, `now` and the `SurfaceCtl`; `SurfaceCtl` offers
  `request_redraw`, `redraw_after(Duration)` (coalesced: one timer per
  surface), `set_blur_region(&[Rect])`, `set_input_region(&[Rect])`,
  `set_keyboard_interactivity`, `set_exclusive_zone`, `open_popup`,
  `close_popup`. Regions are set and committed together with the next frame,
  never on a bare commit.
- Popups: `xdg_popup` children of a layer surface, created with
  `get_popup` before the first commit, grabbed with the serial of the input
  event that opened them (pointer or keyboard, whichever was last), nested
  popups allowed (DBusMenu submenus), `popup_done` delivered as
  `CloseReason::Dismissed`. Placement is a `Placement` value the product
  computes (anchor rect, gravity, offset) and the runtime turns into an
  `xdg_positioner` with constraint adjustment slide and flip.
- Input: pointer enter/leave/motion/button/axis, keyboard key with xkb
  symbol and UTF-8, modifiers, focus enter/leave, the serial of every event;
  cursor shape through `wp_cursor_shape_v1` so hover over a row shows the
  pointer the toolkit asks for.
- Output changes (hotplug, scale, mode) arrive as messages; the product
  creates or destroys one Island per output.
- `unsafe` lives only in `egl.rs` (the EGL and GL loader calls), with the
  invariants documented at each call; every other module forbids it. This is
  the single approved exception in the project and the author's approval is
  recorded in this spec.

### 5.3 `celestina-paint`

- `Painter` is a concrete type over femtovg with a small surface the product
  and the UI tree use: `begin(logical, dpr)`, `end()`, `fill_rounded`,
  `stroke_rounded`, `haze(rect, radius)`, `image`, `glyphs(run)`, `icon`,
  `push_clip(rect, radius)`, `pop_clip`, `push_state`, `pop_state`,
  `opacity(multiply)`, `scale_about`. One `Painter` per process; the GL
  context is made current per surface before `begin`. The backend is a
  private module so a wgpu backend later is one module and no API change.
- **Haze material**: a rounded rect filled with the scheme's `glassTint`
  (canvas at 0.70), the grain texture (Haze's noise, the same PNG the style
  vendors) tiled at 0.15 *under* the tint, no outline, no edge. While the
  compositor's blur is armed the surface pixels outside the material are
  fully transparent; when `ext-background-effect-v1` is absent the material
  paints `glassFallback` (the opaque canvas) instead, the halted shell's rule.
- **Text**: a `TextSystem` holds the cosmic-text `FontSystem` (Inter Variable
  loaded from `celestina-style/fonts` plus the system's fonts for fallback),
  a `SwashCache` and a glyph atlas: one RGBA texture page per 1024×1024,
  keyed by (font id, glyph id, size in device pixels, subpixel quarter),
  rasterised at the surface's device pixel ratio so text is sharp at 1.5.
  A `TextRun` is laid out once per change and painted as textured quads.
  Measurement for layout uses the same `Buffer`, so what taffy measures is
  what is painted. Tabular figures are a `TextStyle` flag that sets the `tnum`
  feature; weight is a `TextStyle` field on the variable font.
- **Icons**: SVGs from `celestina-style/icons` (Lucide, semantic names) and
  theme icons through `freedesktop-icons` (`.desktop` entries, tray items with
  an icon name), rasterised with `resvg` at the needed device size and cached
  by (name, size). Tray pixmaps arrive as ARGB and are uploaded as images.

### 5.4 `celestina-ui`

The spike's `node.rs`, `anim.rs`, `curve.rs` and `input.rs`, split along the
seams the review named and extended with a text field.

- `Tree`: nodes with a `Kind` (`Box`, `Text`, `Icon`, `Image`, `Field`), a
  taffy `Style`, an optional `clip` with radius, `Animated` opacity, scale and
  offset, a fill role (token name, not a colour), hover and pressed flags,
  focusability and a **key**. Layout through taffy with the text system as
  the measure function; `paint_order()` is a pure function tested without GL;
  the paint walk lives in `paint.rs`, the interaction state in `input.rs`,
  highlights in `highlight.rs`.
- **Keyed reconciliation**: a view is a function from state to a
  `ViewTree` of keyed nodes; `Tree::apply(view)` matches children by key,
  updates properties in place, animates size and position changes on the
  default curve, inserts and removes with fades, and keeps hover, pressed,
  focus and running animations on nodes whose key survived. This closes the
  spike's finding that rebuilding rows lost hover and focus.
- `Animated<f32>`: retarget without a jump, `Motion::{Fast, Normal, Slow,
  Exit}` mapped to 100/200/350/150 ms, the One UI curve and the two SineInOut
  variants from DESIGN.md, `reduced_motion` turning spatial and scale
  animations into a single step while opacity fades stay.
- Interaction: hit-test on the laid-out rects respecting clip and opacity 0,
  hover with the pointer-lift fill, pressed recoil to 0.96 in 100 ms and
  release in 350 ms, keyboard focus with Tab order and the focus ring
  stroked in `focusRing`, Enter and Space activating, Escape bubbling to the
  surface.
- `Field`: a single-line editor over cosmic-text's `Editor`: cursor,
  selection by pointer and Shift+arrows, word jumps, grapheme-safe deletion,
  clipboard paste through `wl_data_device` (read only; the clipboard history
  provider is a later delivery), IME preedit and commit through
  `zwp_text_input_v3`, placeholder text in `textFaint`.

### 5.5 Product surfaces

| Surface | Layer and anchor | Where | Keyboard | Opens | Closes |
|---|---|---|---|---|---|
| Island | `Top`, anchored top, fixed logical size (the largest page plus margin), exclusive zone equal to the capsule height plus its margin; blur and input regions follow the painted silhouette every frame | one per output | `None` at rest; `OnDemand` while a page is open | click on the capsule, `celestina island open <page>` | Escape, click outside, focus loss, `celestina island close` |
| Launcher | `Overlay`, centred, its own size | the output with keyboard focus | `Exclusive` while open | `celestina launcher toggle` | Escape, launch, click outside |
| Banners | `Overlay`, anchored top, under the Island's exclusive zone, input region only on the banners | the focused output | `None` | a notification arrives | expiry, click, action, dismiss |
| Menus | `xdg_popup` of the surface that owns the button | over the parent | grab | the overflow ("more") button, a tray item | item chosen, Escape, `popup_done` |

The Island's namespaces: `celestina-panel` for the Island, `celestina-overlay`
for the launcher, `celestina-toasts` for banners, `celestina-panel-menu` and
`celestina-panel-child-menu` are not needed (popups are children, not layer
surfaces) and stay unused.

**The Island.** At rest a capsule of the capsule height (40 px) centred on
the top edge, Haze material, the clock in tabular figures at the row-title
size, the output's workspaces as dots with the active one as a wider pill.
A click or the command grows it in 350 ms on the One UI curve to the page
size (420×300 logical at first; a page may ask for more up to the surface),
fading the capsule content out over the first 40 % and the page in over the
last 60 %, as the spike did. The page has a title row (title, the overflow menu
button, close), the content, and a bottom row of page tabs (Activity,
Notifications, Tray; the Spanish labels live in the copy file). Opening a different page while open slides the
content horizontally. Closing reverses the growth. Everything the Island
needs to paint is in `IslandState` (pure, tested): open/closed, page, the
geometry at an instant, the opacities, which rects form the silhouette.

**Activity page.** The output's workspaces as rows (name or index, column
count) with the windows of the active workspace underneath (icon from the
`.desktop` entry matched by app id, title truncated); click focuses the
workspace or window through `niri-ipc` actions; no optimistic state, the
compositor's event confirms the change. Workspaces of an output that is off
keep their home through `celestina-shell::workspace_groups`, salvaged.

**Notifications page.** The history (the last 20 ended notifications and
every live one): app icon, summary, body, time, actions as buttons; dismiss
one or all; the do-not-disturb switch. While Noctalia owns the server name,
the page shows one line saying so and the switch is disabled.

**Tray page.** One row per StatusNotifierItem: icon, title, status; a click
activates, a right click opens the DBusMenu as a popup anchored to the row,
a scroll sends `Scroll`. While Noctalia owns the watcher, the same
explanation line.

**Launcher.** A Haze card with the field on top and up to eight rows under
it: icon, name, generic name; arrows move, Enter launches, Escape closes.
The index is the session's `.desktop` entries through `celestina-core`'s
reader; ranking is `celestina-shell::launcher` salvaged (prefix, word start,
substring, recent use). A query starting with `/` lists the shell's own
switches (`/notificaciones`, `/no-molestar`, `/actividad`, `/bandeja`, and
later ones as deliveries add them); choosing one runs it in place. Launching
spawns the entry's `Exec` through `setsid` with the launcher's environment,
never through the compositor, and closes.

**Banners.** Up to three stacked banners of the row-large height with icon,
summary, body (two lines), action buttons; urgency low expires in 5 s,
normal in 10 s, critical stays; hovering pauses expiry; click runs the
default action or opens the Notifications page. The state machine
(identity, replacement, expiry, bounds, capabilities) is
`celestina-shell::notifications`, salvaged with its tests.

### 5.6 Compositor integration and focus

- **Event stream.** One worker opens `$NIRI_SOCKET`, sends `EventStream`
  and reads lines. Each line is parsed on its own as `niri_ipc::Event`; a
  line that fails to parse is logged once per distinct variant name (taken
  from the JSON's outer key) and skipped, and the stream continues. Only a
  closed socket reconnects, after one second. This is what the patched niri
  requires and what the `niri-ipc` crate's `read_events` does not do.
- **Requests** (`FocusWorkspace`, `FocusWindow`, `Outputs`, `Workspaces`,
  `Windows`) use the crate's typed `Request`/`Action` on a short-lived second
  socket from a worker, with a bounded command queue of 32; a full queue
  refuses with a visible failure, never a backlog.
- **Focus policy.** Layer surfaces at rest take no keyboard
  (`KeyboardInteractivity::None`) so `focus-follows-mouse` can never steal
  focus from the author's windows by hovering the capsule. Opening a page
  sets `OnDemand`; the launcher sets `Exclusive` for as long as it is open
  because it is a modal that exists to take typing. Every open surface
  closes on Escape, on a click outside its silhouette, on keyboard focus
  loss, and on `popup_done` for popups. The shell never depends on
  `focus-follows-mouse` being on.
- **Command surface.** The binary is both the shell and its client. When an
  instance owns `org.celestina.Shell`, `celestina <verb>` forwards the verb
  to `/org/celestina/Shell` interface `org.celestina.Shell1` with a 3 s
  timeout and exits; the author binds these in niri's `binds {}`:
  `island toggle`, `island open <activity|notifications|tray>`,
  `island close`, `launcher toggle`, `notifications toggle`,
  `notifications dismiss-all`, `quit`. Verbs are typed and bounded; unknown
  verbs are refused with a message.
- **Single instance.** The first process claims `org.celestina.Shell` with
  `DoNotQueue`; a second launch with no verb prints that the shell is running
  and exits 0. `org.freedesktop.Notifications` and
  `org.kde.StatusNotifierWatcher` are requested with `DoNotQueue` after their
  objects are served; `NameTaken` disables the provider for the session and
  records the owner's name for the page to show. No retry: the author stops
  Noctalia and restarts the shell when ready.

### 5.7 Shared style: tokens as data

- `celestina-style/tokens.toml` becomes the single source of every value
  `CelestinaTheme.qml` holds as a literal today: the reference palette
  (`[ref]`), the schemes (`[scheme.dark]` now; `[scheme.ink]` and
  `[scheme.light]` later as tables of the same keys), and the scalars
  (radius, type ladder with the text-scale rule, weights, spacing, control
  anatomy, motion durations and curves, Haze: sigma 12, grain 0.15, tint
  0.70, the two shell material strengths 0.64 and 0.12, press scale 0.96).
  Colours are hex strings; curves are four numbers; every key carries a
  one-line comment that is the token's definition.
- `celestina-style/scripts/gen-tokens.py` (Python, offline, no
  dependencies beyond the standard library, like the other guards) generates:
  - `celestina-style/CelestinaTokenData.qml`, a `pragma Singleton`
    `QtObject` with the `ref` palette, one `ColorScheme`-shaped object per
    scheme and the scalars. `CelestinaTheme.qml` keeps its public API and
    its functions (`withAlpha`, `mixColors`), and binds its `schemeDark` and
    scalar `readonly` properties to `CelestinaTokenData`. Applications change
    nothing; the style guard still scans `CelestinaTheme.qml` as the theme.
  - `celestina-rs/crates/celestina-tokens/src/generated.rs`: `pub struct
    Scheme { pub canvas: Rgba, … }` with one `pub const DARK: Scheme`,
    `pub mod radius`, `pub mod motion`, `pub mod haze`, `pub mod type_scale`
    as constants, plus `Scheme::by_name`. The crate is plain Rust with no
    dependencies so any crate can use it.
- Both generated files are committed. `celestina-style/scripts/verify-production.sh`
  and `celestina-rs/scripts/verify-production.sh` run the generator into a
  temporary directory and fail on any difference, so a hand edit of either
  output or a change to `tokens.toml` without regeneration is a red verify.
  `tokens.toml` and the generator join `celestina-style`'s
  `production_inputs`.
- The shell reads `appearance.toml` through `celestina-settings` and follows
  its watcher: `reduced_motion` drives `Animated`, `text_scale` multiplies
  the type ladder exactly as `CelestinaTheme.textScale` does, and a future
  `scheme` key selects `Scheme::by_name` with `dark` as the fallback. The
  grain the spike painted at 0.05 becomes DESIGN.md's 0.15.

## 6. Crates, files and dependencies

### 6.1 `celestina/` (this project)

```text
celestina/
  Cargo.toml              workspace: members = ["crates/*", "."]; package `celestina`
  Cargo.lock
  rust-toolchain.toml     the suite's channel (no build.rs: there is no QML to register)
  src/
    main.rs               argument parsing: shell mode or client verb; single instance
    app.rs                the Shell impl: owns surfaces per output, routes messages
    island/               IslandState (pure), view builders per page, silhouette
    launcher/             state, ranking glue, `/` commands, spawn
    notifications/        banners view, history page view, server glue
    tray/                 host glue, items view, DBusMenu popups
    providers/            worker threads: niri stream + requests, notifications server,
                          tray (system-tray), desktop index, settings watcher
    copy.rs               language-contract: product-copy (every Spanish string)
  crates/
    celestina-wayland/    §5.2; the only crate with an `unsafe` module (egl.rs)
    celestina-paint/      §5.3
    celestina-ui/         §5.4
  scripts/
    build-production.sh verify-production.sh deploy-production.sh
    complete-production.sh activate-production.sh status-production.sh
    smoke.sh dev-session.sh (nested niri with the shell, for the author's checks)
  celestina.desktop       NoDisplay, StartupWMClass=celestina, as the old one
  docs/ decisions/ plans/active/ evidence/ discussions/
  AGENTS.md README.md STATUS.md ROADMAP.md VALIDATION.md
```

Registry entry (`docs/projects.toml`): `kind = "rust-shell"`,
`production_role = "activatable-shell"`, `toolchains = ["rust"]`,
`version_source = { kind = "cargo-package", path = "celestina/Cargo.toml",
package = "celestina" }` with the lock mirror, `source_roots =
["celestina/src", "celestina/crates", "celestina-rs/crates/celestina-shell"]`,
`commit_roots = ["celestina/", "celestina-rs/crates/celestina-shell/"]`,
`component_commit_scopes` with prefix `celestina-shell`, `artifact_paths =
["celestina/target/release/celestina"]`, the six scripts, and
`production_inputs` covering the workspace, `celestina-style/tokens.toml`,
`celestina-style/fonts`, `celestina-style/icons` and the pure crates it
links. `deployable = true`; `activate_script` starts or replaces the live
shell only on the author's explicit request, as the production contract
says.

### 6.2 `celestina-rs/crates/celestina-shell`

Pure domain, salvaged by copy from `celestina-shell-core` and adapted: no
line protocol, no `Payload` envelope, no host commands; plain Rust types the
product consumes in process. Modules in the first release: `notifications`
(state machine, capabilities, history cap), `launcher` (ranking), `coalesce`,
`workspace_groups`, `workspace_map`, `settings` (the shell's own durable
choices: do-not-disturb, later the pieces' placement), `calendar` (used by
the clock's date). Each module's head names the file it was copied from and
the commit of `celestina-qt` it came from. Later deliveries copy
`clipboard`, `nightlight`, `media`, `session`, `connectivity`, `pending`,
`sysmon`, `wallpaper`, `brightness` as they are needed, never before.

Dependencies: `celestina-core` (desktop entries). `serde` only if the
settings file keeps it; otherwise none.

### 6.3 `celestina-rs/crates/celestina-tokens`

Generated (§5.7). No dependencies. Consumers: the shell now; any Rust side
of an application that needs a token later (none today).

### 6.4 Dependencies of `celestina/`, each justified in `Cargo.toml`

| Crate | Why |
|---|---|
| `smithay-client-toolkit` 0.21, `wayland-client` 0.31, `wayland-protocols` 0.32 (`staging`), `calloop` 0.14, `calloop-wayland-source` 0.4 | the Wayland client: layer shell, seat, output, fractional scale, viewporter, background effect; the loop |
| `wayland-egl` 0.32, `khronos-egl` 6 (`dynamic`), `glow` 0.18 | an EGL window per surface and the GL loader femtovg needs |
| `femtovg` 0.27 | the painter |
| `cosmic-text` 0.19 | shaping, fallback, variable weights, `tnum`, the editor |
| `taffy` 0.15 | layout |
| `resvg` 0.48, `freedesktop-icons` 0.4 | SVG rasterisation and theme icon lookup |
| `zbus` 5 (`blocking-api`, `async-io`) | the bus: Shell1, Notifications, StatusNotifier, DBusMenu |
| `system-tray` 0.8 | StatusNotifierWatcher/Host and DBusMenu client; replaces the halted C++ tray |
| `niri-ipc` = 26.4.0, `serde_json` 1 | compositor types and the tolerant line parser |
| `xkbcommon` (through sctk's `xkbcommon` feature) | key symbols |
| `tracing`, `tracing-subscriber` (`env-filter`) | diagnostics to journald through stderr |
| `calloop` with its `signals` feature | SIGTERM/SIGINT inside the loop, as the spike did; no `signal-hook` |
| `celestina-core`, `celestina-settings`, `celestina-shell`, `celestina-tokens` | the suite's own |

Minimum Rust for the project: 1.89 (cosmic-text); the suite's toolchain
1.97.1 satisfies it. No async executor: zbus runs blocking on worker
threads, as the halted adapter did.

## 7. Delivery

Each unit is a ledger row in
`celestina/docs/plans/active/2026-10-14-cel-1-island.md` with inventory and
evidence, landed one at a time through `scripts/worktree.sh` and the landing;
`ROADMAP.md` carries the same table.

| Unit | Scope | Outcome |
|---|---|---|
| CEL-1-A0 | `suite` | Before the rename can land: the `history_owner` alias in `scripts/version_contract.py` (a renamed project keeps the history rows that carry its old name; the old id may be re-registered with its own baseline after that chain; no row is rewritten and no new row kind exists); inventory moves under a renamed halted owner in `scripts/check-staged-units.py`; halted trees counted as history by `scripts/check-language-contract.py` (the 8 `celestina/` rows leave the baseline under the scanner-migration rule); each rule with its tests, dormant until CEL-1-A uses it; a rehearsal of the whole rename in a scratch clone with every guard, recorded as evidence |
| CEL-1-A | `suite` (the author's edit) | `git mv celestina/ celestina-qt/`; registry entry renamed to `celestina-qt` (paths, `active_plans`, `context_documents`, scripts, artifacts, inputs), still `qt-shell` and `halted`; "Halted projects" rewritten for the new path and the salvage-by-copy permission naming the modules of §6.2; `scripts/check-architecture-contract.sh:327`, the commit-scope tests' fixtures, the root `README.md` project table and the three pointers (`HOST-HYGIENE.md`, `magnetita/ROADMAP.md`, `siderita/STATUS.md`) updated; the new `celestina` entry registered with its document set, AGENTS, scripts and a stub binary at 0.1.0 (the guards require every registered path to exist); the radius ratchet row renamed and the 11 historical links retargeted; the `celestina-shell` and `celestina-tokens` component prefixes registered under `celestina-rs`; this spec committed |
| CEL-1-B | `celestina` | Skeleton: the workspace, `celestina-wayland`, `celestina-paint` with the text atlas, `celestina-ui` with keyed reconciliation and its tests without GL; one Island capsule per output with clock and workspace dots over the tolerant niri stream; the Shell1 command surface and single instance; production scripts, `.desktop`, documents; 0.1.0 |
| CEL-1-C | `celestina-style` + `celestina-rs` | `tokens.toml`, `gen-tokens.py`, `CelestinaTokenData.qml` consumed by `CelestinaTheme.qml`, `celestina-tokens`, the drift check in both verifies; the shell switches from local constants to the crate |
| CEL-1-D | `celestina` | Island pages: Activity, Notifications (empty state while Noctalia owns the name), Tray (same), page tabs, the overflow popup with the shell's actions (reopen page, do not disturb, quit) |
| CEL-1-E | `celestina` | Launcher: overlay surface, `Field` with IME, index, ranking, `/` commands, spawn |
| CEL-1-F | `celestina` | Notifications: server (claim if free), banners, history, actions, do-not-disturb |
| CEL-1-G | `celestina` | Tray: watcher and host (claim if free), items, pixmaps and theme icons, nested DBusMenu popups, scroll |
| CEL-1-H | `celestina` | Exit: live validation rows closed, 1.0.0, STATUS/ROADMAP, and a handover note listing what Noctalia still does for the session (OSD, media, lock, polkit, wallpaper) so the author knows what the next deliveries must cover before stopping it |

CEL-1-A is the author's unit because only the author may edit "Halted
projects"; the agent prepares its diff and the author reviews and lands it.
Every later unit is an agent's.

## 8. Verification

- **Pure tests:** `celestina-shell` keeps the salvaged tests and gains tests
  for anything adapted; `celestina-ui` tests layout, `paint_order`, hit-test
  (clip, opacity 0, hidden subtrees), animation timing on the curve,
  retargeting, reduced motion, keyed reconciliation preserving hover, focus
  and running animations, and the field's editing operations; the niri
  parser has fixtures including a `MinimizedWindowsChanged` line and a
  truncated line; the notifications server is tested on a private
  `dbus-daemon` started by the test when `dbus-daemon` is on the machine,
  skipped with a notice otherwise; `IslandState` geometry and silhouette
  slices are tested as pure functions.
- **Generated outputs:** the token drift check in both verifies (§5.7).
- **`verify-production.sh`:** `cargo fmt --check`, `cargo clippy --all-targets
  -D warnings`, `cargo test`, and `celestina --check`, which loads the
  tokens, the appearance file and the fonts and exits without a compositor.
- **`smoke.sh`:** when `niri` is installed, starts a nested niri with
  `scripts/dev-session.kdl`, runs the shell with `--exit-after 3` and
  `--metrics`, and asserts exit 0, at least one frame per output and zero
  frames in rest; skipped with a notice where niri is absent. The metrics
  flag prints cpu frame p50/p99/max with `n=`, late frames measured on the
  compositor's `wl_callback.done` timestamps, and RSS, as the spike's final
  wave defined them.
- **Live validation** (`VALIDATION.md`, by the author, veil lifted): Haze
  and blur on the Island, a banner and a popup; capsule to page at 165 Hz
  without jank; zero frames in rest; RSS under 100 MB with two outputs; the
  grab semantics of a popup; keyboard-only operation of every surface;
  IME in the launcher's field; text sharp at scale 1.5 and 1.0; the shell
  started while Noctalia runs and again after stopping it.
- **Guards before every commit:** architecture, language, documentation
  contracts, the shared ratchets, `cargo fmt`, `clippy -D warnings`.

## 9. Risks and how the design meets them

| Risk | Mitigation |
|---|---|
| The UI toolkit grows without end | its scope is fixed by §5.4; anything a later delivery needs that is not a node kind, an animation or a field is a product concern, not a toolkit one; GPUI pinned to a commit remains the plan B the study named |
| Compositor blur over a surface that changes silhouette every frame shows artifacts at 4K@165 | the surface never resizes during animation; only the region does (spike); the author's live row judges it; the spike's `--no-blur` flag stays for attribution |
| The patched niri's IPC vocabulary diverges further | the tolerant parser skips unknown events; requests use the pinned crate; a request the compositor refuses is logged, not retried |
| cosmic-text's atlas and IME cost more than estimated | the atlas is bounded (one page, evict least-recent on a full page) and measured in CEL-1-B's evidence; IME is a `Field` concern only |
| Claiming bus names beside Noctalia | `DoNotQueue`, objects served first, no retry; the page says who owns the name |
| The rename of the halted shell breaks a guard or a landing | CEL-1-A0 changes the three guards the rename trips (§4) and rehearses the rename with every guard before CEL-1-A exists; CEL-1-A lists every reference found on 2026-10-14 and its evidence is the hook chain the author's commit runs plus `scripts/test-land-unit.py` (the landing tool has no dry run); the halted bytes move unchanged so `git log --follow` keeps their history |
| Two token sources during CEL-1-B and -C | CEL-1-B uses local constants copied from `tokens.rs` of the spike and CEL-1-C deletes them; the drift check lands with -C |

## 10. Decisions recorded here (to become `celestina/docs/decisions/`)

1. **Pure Rust shell on Wayland for niri only**, with a small toolkit of its
   own over smithay-client-toolkit, femtovg, cosmic-text and taffy; the
   compositor supplies the glass.
2. **The halted Qt shell is history at `celestina-qt/`**; salvage is by copy
   of named modules, never by editing.
3. **No dock** (carries ADR 0003 of the old shell).
4. **femtovg behind a Painter contract**; wgpu is a backend change, not an
   architecture change.
5. **cosmic-text is the only text stack.**
6. **Tokens are data** in `celestina-style/tokens.toml`; QML and Rust
   consume generated, committed outputs under a drift guard.
7. **Focus policy:** no keyboard at rest, on demand with a page open,
   exclusive for the launcher; the compositor's key bindings open surfaces
   through the shell's own bus interface.
8. **Coexistence:** well-known names are claimed only when free.
9. **One process, one loop, worker threads**, no async executor.
