# Celestina shell parity program (CEL-2) — design

- **Date:** 2026-10-16
- **Status:** drafted from the author's answers in brainstorming; awaiting the
  author's review
- **Product:** the work that lets the author stop Noctalia v5 in the niri
  session and run Celestina instead: every duty Noctalia performs there today
  (bar, wallpaper, OSD, media, clipboard history, idle, lock, polkit, night
  light, caffeine, phone state, minimised windows) plus the one useful thing
  iNiR adds, in Celestina's own shell language
- **Identifiers:** project `celestina`, plan `cel-2-parity`, unit prefix
  `CEL-2-<letter>`, commit prefix `celestina:`
- **Builds on:** [the shell design of 2026-10-14](2026-10-14-celestina-rust-shell-design.md)
  (CEL-1, landed as 1.0.0), whose §1 "Later deliveries" this program turns
  into units, and the handover note `celestina/docs/handover.md`
- **Group 2 is not here:** style, animation, UX and visual polish (Ink and
  Light schemes, Lume, motion refinements, Customize, the look of the bar's
  capsules) are the author's separate programme. §8 lists the extension
  points this program leaves so group 2 is not blocked.

## 1. Goal and scope

The author wants to **stop Noctalia and run Celestina instead**, and chose the
widest bar for "done": parity with Noctalia's bar as well as its background
duties (answer of 2026-10-16). Noctalia keeps running, unchanged, until the
last unit; every unit lands beside it and is switched on by the author, duty
by duty (§4.1).

In scope: every capability of §3 marked **S** (blocks stopping), **D** (noticed
daily) or **I** (iNiR's "see the desktop" group). Out of scope for good, by the
author's decisions (restated, not reopened): a dock of any kind, plugins or
scripting, AI, anime, mascot, music streaming, any compositor but niri, shell
blur (the compositor blurs), and Noctalia's plugin functions: video
wallpapers, wallhaven, notes, translator, drive-health, game-launcher,
gamer-mode, the cat, colour picker, screen toolkit and the Claude plugins die
with Noctalia (answer of 2026-10-16). Screenshots, recording, the ruler and
the colour picker already live in Selenita, Melibea and niri.

## 2. Decisions the author made in brainstorming

| Decision | Choice | Alternatives considered |
|---|---|---|
| Bar | **Full bar with zones, the Island in its centre zone**: one layer surface per output, one exclusive zone, the Island grows from the centre | a separate bar surface; a per-setting choice between Island alone and full bar |
| Definition of done | **Parity with Noctalia's bar**, not only its background duties | the minimum session set; minimum plus clipboard and phone |
| iNiR extras | **Only "see the desktop"**: workspace overview with window previews and the keybind cheat sheet. Productivity pieces (calendar, timers, todo, world clock) and system pieces (privacy indicator, GameMode, per-app mixer) are not scheduled | all groups; none |
| Noctalia plugins | **None carried over**, apart from the two the author wrote | video wallpaper and wallhaven; notes and translator; disk and game pieces |
| Bubbles | **Native piece**: minimised windows as a bubble group and the `Mod+K` selector over Melibea's socket (not a dock: only native minimised state) | later; never |
| Magnetita | Phone state (battery, connection, media) is a native piece fed by `org.celestina.Magnetita` | — |
| First delivery | **The bar and the piece contract** (CEL-2-A) | energy and comfort first; OSD and media first |
| Clock | **Minutes at rest**; seconds run only while the Island is open. Rest stays at one frame a minute | seconds always; a setting |

## 3. Inventory and parity matrix

Method: measured in the author's session on 2026-10-16 (nothing changed):
`~/.local/state/noctalia/settings.toml`, `noctalia msg status|plugins list`,
`niri msg layers`, `busctl --user list`, `~/.config/niri/config.kdl`, the
compositor binary's protocol symbols, `~/.local/src/inir` 2.33.0 (README,
`docs/MODULES.md`, `docs/IRIS.md`, `services/`) and the halted shell's
provider adapter. The settings file also holds a third-party API key; no
document copies it.

**Facts that shape the program**

- Noctalia owns `org.freedesktop.Notifications` and
  `org.kde.StatusNotifierWatcher`. Its layers on each of the three outputs
  (DP-1, DP-2, HDMI-A-1) are `noctalia-wallpaper` (Background),
  `noctalia-bar-default` (Top) and four `noctalia-screen-corner`.
- The author's patched niri advertises `ext-session-lock`, `ext-idle-notify`,
  `zwp_idle_inhibit`, `ext-data-control` (and `wlr-data-control`),
  `wlr-gamma-control`, `ext-workspace`, `wlr-foreign-toplevel` and
  `ext-background-effect`. It does **not** advertise output power
  management: screen-off goes through `niri msg action power-off-monitors`.
- Already running outside Noctalia: `nm-applet`, `blueman-applet`,
  `blueman-tray` and `solaar` (StatusNotifierItems), `wireplumber`,
  `pipewire-pulse`, `polkitd` (with **no** graphical agent but Noctalia's),
  `magnetitad` and `melibea.service`.
- The niri config calls Noctalia from: `spawn-at-startup "noctalia"`, one
  startup line (`caffeine-enable`, `nightlight-force-toggle`), `Mod+Space`
  (launcher), `Mod+Shift+Escape` (lock and suspend) and `Mod+K` (bubbles).
  Volume, brightness and media keys run `wpctl`, `brightnessctl` and
  `playerctl` directly, so they survive Noctalia but have no on-screen
  feedback once it stops. `Mod+Shift+A` uses `swaylock` and is unaffected.
- Idle policy in force: screen off after 20 s, lock-and-suspend after 240 s
  (the screen-off matters on the OLED). Its owner is Noctalia.
- Noctalia's bar shows, per output: workspaces, magnetita, notifications at
  the start; weather and the clock at the centre; bubbles, sysmon, tray,
  network, bluetooth, volume, brightness and clipboard at the end.

**Matrix.** `S` blocks stopping Noctalia, `D` is noticed daily, `I` is the
iNiR group, `—` already done. Risk is the security or data risk of the
capability, not its effort.

| Capability | In 1.0.0 | Class | Needs | Risk |
|---|---|---|---|---|
| Notifications, tray, launcher, Activity | yes | — | — | — |
| Bar with zones; workspaces, notifications and tray as bar pieces | no | S | one wide layer surface; the piece contract (§6) | low |
| Idle: screen off, lock-and-suspend timers | no | S | `ext-idle-notify`, niri action, logind | low (OLED if wrong) |
| Caffeine | no | S | idle state; honours other clients' `zwp_idle_inhibit` through the compositor | low |
| Night light, forced 2700 K | no | S | `wlr-gamma-control`; ADR 0006 of the halted shell | medium |
| Wallpaper, a still image per output | no | S | layer-shell `Background`, namespace `wallpaper` (rule exists); an image decoder | low |
| Session lock and the suspend chain | no | S | `ext-session-lock`, PAM in a separate child, logind inhibitor; ADR 0004 | **high** |
| Polkit agent | no | S | `AuthenticationAgent` on zbus, `polkit-agent-helper-1`; ADR 0005 | **high** |
| OSD for volume, microphone, brightness | no | D | PipeWire/WirePlumber, backlight sysfs and DDC; `celestina-osd` layer | low; DDC blocks |
| Media (MPRIS) | no | D | zbus MPRIS client | low |
| Clipboard history | no | D | `ext-data-control`; own privacy design | medium-high |
| Network, Bluetooth, volume, brightness, battery and power-profile pieces | no | D | NetworkManager, BlueZ, UPower, power-profiles over zbus | low-medium |
| Phone state (Magnetita) and the bubbles (Melibea) as pieces | no | D | `org.celestina.Magnetita`, Melibea's socket | low |
| System monitor, weather, calendar (clock page) | no | D | `/proc`, a weather source and location policy | low; location is privacy |
| Workspace overview with previews; keybind cheat sheet | no | I | `ext-foreign-toplevel`/`ext-image-copy-capture`, niri config reader | medium (screen capture) |
| Screen corners | no | group 2 | — | — |
| Greeter, dock, desktop and lock-screen widgets | out | — | — | — |

## 4. Principles the units share

### 4.1 Coexistence: duties are switched on one by one

Noctalia keeps doing everything until the author turns it off. A unit that
takes over something Noctalia does ships **off** and is switched on by a key
of the shell's own `shell.json` (the file CEL-1-F already owns for
do-not-disturb), under one table:

```text
duties: bar | idle | caffeine | nightlight | wallpaper | osd | media |
        clipboard | polkit | lock | phone | ...
```

The value is `off` (default while Noctalia runs) or `on`. A duty that has an
observable owner (a bus name, a layer namespace) also reports who holds it and
the shell refuses to take it, as it does for the two bus names today
(`DoNotQueue`, decision 8). A duty without an owner signal (idle, gamma,
caffeine) is only the author's switch. The shell reads the file at start and
follows its changes (the `celestina-settings` watcher pattern); a switch is a
live change, never a restart. The author turns the matching Noctalia duty off
with Noctalia's own verbs (`bar-hide`, `osd-disable`, `caffeine-disable`,
`nightlight-disable`, its idle settings), so a unit can be validated live
without stopping Noctalia. The last unit (CEL-2-M) retires the table's
`off` values for the shipped duties: after the cutover a duty is simply on.

### 4.2 Providers

Every unit that talks to the system adds a provider in the CEL-1 shape: a named
worker thread behind the `Worker` guard, typed messages into the loop's
channel, a bounded `sync_channel` for commands, hostile input bounded,
bursts coalesced, shutdown joined. Blocking IO (D-Bus, PipeWire, sysfs, DDC)
never runs on the main thread. Pure state machines go in `celestina-shell`
(salvaged by copy where the halted adapter has them) and are tested without a
bus.

### 4.3 Salvage by copy

Each unit names the halted modules it copies; the author's edit of "Halted
projects" in the root `AGENTS.md` extends the salvage list **before** that
unit's first commit, as CEL-1 required. The additions per unit are in §5. No
unit edits `celestina-qt/`.

### 4.4 Privileged units

Lock (CEL-2-H) and polkit (CEL-2-I) carry the halted shell's ADR 0004 and 0005
over as this project's own decisions, adapted and re-reviewed rather than
ported unread, and each lands with:

1. its own design (a spec of its own, written when the unit is next);
2. a threat model and an explicit list of what the unit never does;
3. a review by a model other than the implementer, at high reasoning, as the
   author's standing rule says;
4. a recovery path that stays installed until the author's live validation
   passes (`swaylock` for the lock; for polkit, no recovery is needed because
   without an agent actions fail closed);
5. a failure mode of "locked out" or "denied", never "let in".

### 4.5 Evidence and validation

Each unit is a ledger row of `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`
with an inventory and evidence, lands through `scripts/land-unit.py`, and
queues `VAL-CEL2-*` rows for the author's live checks (they never block the
unit, as in CEL-1). Automated evidence per unit: pure tests, a private bus
where a bus is involved, and the nested-niri smoke. A privileged unit adds a
rehearsal in the nest, never in the author's session.

## 5. Decomposition

Sizes are heuristic estimates in working sessions of one agent, and the
reasoning level is the proposal the author's rule asks for before any
implementation; both are re-proposed in the table each unit presents before
it starts. **S** about one session, **M** two or three, **L** four or more or
a security review.

| Unit | Delivers | Class | Size | Salvage (author's AGENTS.md edit) | Switch |
|---|---|---|---|---|---|
| **CEL-2-A** | The bar and the piece contract: one wide surface per output, zones, the Island at the centre; Workspaces, Notifications and Tray as pieces; the `duties` table (only `bar` in it); the clock page shows seconds. Designed in §6 | S | **M** | none | `bar` |
| CEL-2-B | Energy and comfort: idle timers (screen off by niri action), caffeine, night light, session verbs (suspend, reboot, power off, log out) through logind; replaces the startup line | S | **M** | `nightlight.rs`, `session.rs` and `sessionholds.rs` of the provider adapter; `nightlight`, `session`, `pending` of `celestina-shell-core` | `idle`, `caffeine`, `nightlight` |
| CEL-2-C | OSD for volume, microphone and brightness over PipeWire, sysfs and DDC; the `celestina-osd` layer; volume and brightness pieces with a slider popup | D | **M** | `audio.rs`, `brightness.rs`; `audio`, `brightness` of the core | `osd` |
| CEL-2-D | Media: MPRIS client, the media piece and an Island page, keys keep running `playerctl` | D | **S** | `media.rs`; `media` of the core | `media` |
| CEL-2-E | Wallpaper: a still image per output, scaling, per-output choice from a folder, startup restore; palette extraction stays out (group 2) | S | **M** | `wallpaper.rs` (image selection only); `wallpaper` of the core | `wallpaper` |
| CEL-2-F | Connectivity and power pieces: network, Bluetooth, battery, power profile, as pieces with popups; `nm-applet` and `blueman` stay as the editing UIs | D | **M** | `connectivity`, `network`, `bluetooth`, `power` of the core as far as read-only state needs | none (pieces appear when placed) |
| CEL-2-G | Clipboard history: `ext-data-control` provider, bounded text and image history, secret-aware skipping, the clipboard piece and page | D | **M** | `clipboard.rs`; `clipboard` of the core | `clipboard` |
| CEL-2-H | **Session lock** and the lock-and-suspend chain: `ext-session-lock`, a PAM child, a minimal lock surface per output, logind delay inhibitor; wires idle's second timer | S | **L** | `session.rs` as reference only; ADR 0004 carried over | `lock` |
| CEL-2-I | **Polkit agent**: registration for the session, a prompt surface with a keyboard grab, the helper pipe | S | **L** | ADR 0005 carried over | `polkit` |
| CEL-2-J | Phone and minimised windows: the Magnetita piece (battery, connection, media) and the bubble group with the `Mod+K` selector over Melibea's socket | D | **M** | `melibea.rs`; `melibea` of the core | `phone`, `bubbles` |
| CEL-2-K | Glance: system monitor and weather pieces, the calendar on the clock's page; location policy decided in its own design | D | **M** | `sysmon.rs`, `weather.rs`; `sysmon`, `weather`, `calendar` of the core | none |
| CEL-2-L | See the desktop: the workspace overview with window previews and the keybind cheat sheet | I | **L** | `workspace_map` (already listed) | none |
| CEL-2-M | **Cutover**: start `celestina` in niri's autostart, rebind the keys that call Noctalia, delete the startup line, retire the `off` defaults, update the handover note and the roadmap; Noctalia stops being started. Never replaces the live session on its own | S | **S** | none | — |

**Order and why.** A first because every later piece plugs into its contract
and group 2's visual work needs the bar to exist. B and C next because they
remove the startup line and the missing feedback the author notices at once.
D and E are small, daily and independent. F to G fill the bar's end zone. H and
I are late on purpose: they need the toolkit to be mature (a new surface role
and a keyboard-grabbing prompt) and their own reviews, but they are **required
before M**. J, K and L finish parity and the iNiR group. M closes the program.

**Stopping point.** Noctalia can be stopped after M. Because the author chose
bar parity, there is no earlier supported stop, but the duty switches (§4.1)
let the author retire Noctalia's duties one at a time as units land, and
`swaylock` plus the direct media, volume and brightness keys cover what a
half-migrated session needs.

## 6. CEL-2-A in detail: the bar and the piece contract

### 6.1 The surface

Today the Island is a layer surface anchored to the top and centred, about
760 logical px wide, with an exclusive zone of 48 (the 40 px capsule plus its
8 px gap). CEL-2-A changes the same surface, per output, to:

- **anchored top, left and right** (full output width), namespace
  `celestina-panel` (the author's layer rule already covers it);
- height as today: the page's largest size plus margin, so the Island still
  grows inside it and the surface never resizes during animation;
- **exclusive zone 48**, unchanged: the bar's thickness is the token the style
  already names (DESIGN.md: top bar 48), so no window moves when the bar
  replaces the capsule-only layout;
- keyboard `None` at rest, `OnDemand` with a page open (decision 7 unchanged);
- blur and input regions are the **union of the painted silhouettes**: each
  zone's capsule and the Island (capsule or page), computed from the same
  `silhouette_slices` function, set and committed with the frame. Everything
  else on the surface is transparent and passes input through. The surface
  being wide costs only a transparent clear; the evidence records frame time
  and RSS against the 1.0.0 numbers (§9).

When the `bar` duty is `off` the surface keeps its 1.0.0 shape (centred,
about 760 wide, the capsule only), so the author's current session is
unchanged until the switch. That fallback is a stage of the migration, not a
second layout to maintain: CEL-2-M removes it.

### 6.2 Zones

Three zones, `start`, `centre` and `end`, laid out by taffy in one row across
the output with the centre truly centred on the output. A zone with no piece
draws no capsule (borderless capsules, ADR 0002 of the halted shell, carried
over by CEL-1): one capsule per non-empty zone, Haze material, the existing
tokens. The **Island is pinned in the centre zone**, exactly one, and cannot
be removed; other centre pieces flank it. How the capsules look is group 2's:
every dimension and colour is a token read through `Theme`.

### 6.3 The piece contract

A **piece** is one thing the bar can hold. The contract is a static table of
trait objects, the shape `PageView` already uses:

```text
trait Piece {
    id()              -> PieceId            // stable name, also the layout key
    name()            -> &str               // accessible name, copy.rs
    bar(&BarCx)       -> Option<View>       // the widget in a zone, or None (absent)
    page()            -> Option<Page>       // the Island page it opens, if any
    activate(&Input)  -> PieceAction        // click, key, scroll: what the piece asks for
}
```

- `BarCx` is `PageCx`'s sibling: the theme, the output, and typed read-only
  slices of the shared state (`model`, `history`, `tray`, and later one
  slice per unit: audio, media, clipboard...). A unit adds its slice and its
  pieces; it never reaches another piece's state.
- `PieceAction` is the typed request a piece makes (`OpenPage`, `Tray(..)`,
  `Notification(..)`, later `Volume(..)`): the Island turns it into an
  `IslandAction` as it does today. A piece never touches a surface, a bus or
  a process.
- A piece with a `page()` appears as a tab of the Island, so the tab strip
  and `island open <page>` come from the table, not from the `Page` enum
  literals. The three CEL-1 pages become the pieces' pages; no behaviour of
  the pages changes.
- Widgets are `View` slices of the existing toolkit: keyed, focusable,
  keyboard-operable, with name, role and state exposed as every control must.
  A piece that needs a popup uses the existing `xdg_popup` path.

CEL-2-A migrates exactly three pieces: **Workspaces** (the output's
workspaces as dots with the active one a wider pill, the same data and the
same click-to-focus the Activity page uses), **Notifications** (a bell with
an unread mark; click opens the Notifications page, scroll does nothing) and
**Tray** (the items as icons inline in a zone; click, right click and scroll
exactly as the Tray page's rows send them; the DBusMenu popup anchors to the
icon; items in `tray.hidden` are not shown, the way Noctalia hides
`nm-applet` and `blueman-tray`). The Activity page remains the window list.

**No duplicated dots.** While the Workspaces piece is placed in a zone, the
Island's resting capsule drops its workspace dots and shows only the clock;
when it is not placed, the capsule shows both as in 1.0.0. One rule, no
setting.

### 6.4 Layout data

The layout is part of `shell.json`, the file the shell already owns and
writes atomically, evolved compatibly (a missing key means today's value):

```text
bar: { start: ["workspaces","notifications"],
       centre: [],                // the Island is implicit, centre pieces flank it
       end: ["tray"],
       tray_hidden: ["nm-applet","blueman-tray"] }
duties: { bar: "off" }
```

Unknown piece ids are ignored with one log line each; a piece listed twice
shows once; the file is read once at start and followed on change. The
default (used while the key is missing) is the order above, which mirrors
Noctalia's start and end zones for the pieces this unit has. Each later unit
adds its pieces to the default end zone in Noctalia's order (bubbles, sysmon,
tray, network, bluetooth, volume, brightness, clipboard). There is no UI to
edit the layout in this program: the author edits the file, and Customize on
the shell is group 2's.

### 6.5 The clock

The Island's capsule keeps showing minutes and keeps its one-frame-a-minute
timer. When an Island page is open, the clock in the page title area shows
`HH:MM:SS` with a one-second timer that exists only while the page is open
and stops when it closes, so rest is unchanged.

### 6.6 Files

```text
celestina/src/pieces/        mod.rs (trait, table, PieceId, BarCx, PieceAction)
                             workspaces.rs notifications.rs tray.rs
celestina/src/bar/           layout.rs (zones, pure), view.rs, silhouette.rs
celestina/src/island/        pages derive from the table; capsule drops dots by rule
celestina-rs/crates/celestina-shell/src/settings.rs   bar layout and duties, serde, bounded
```

`bar/layout.rs` is pure: from the layout data and the output width it returns
the zone rects; tested without GL. `shell.json`'s schema change is covered by
the existing round-trip, bound and "not ours" tests plus new cases.

### 6.7 Verification

- **Pure tests:** zone geometry at 1920, 2560 and 3840 logical widths and at
  scale 1.0 and 1.5; the union of silhouettes for a closed and an open
  Island; defaults, unknown ids, duplicates and a hostile file for the
  layout; the capsule's dot rule; the table yielding the tab order.
- **Characterisation first:** the three pages' views and `wants` are
  snapshotted before the table replaces the enum, as the architecture
  standard requires for a refactor, and asserted unchanged after.
- **Nested smoke:** the nest started with `bar` on, `--metrics`: at least one
  frame per output, zero frames in rest, frame p50/p99/max and RSS reported.
- **Live rows (`VALIDATION.md`, author):** `VAL-CEL2-BAR-LIVE` the bar beside
  Noctalia with Noctalia's bar hidden by `noctalia msg bar-hide`; blur
  regions follow every capsule; popups from tray icons; click-outside and
  focus policy with the wider surface; two outputs of different scale.

### 6.8 Risks

| Risk | Mitigation |
|---|---|
| A wide surface raises frame cost or memory on the 4K output | the evidence compares against the 1.0.0 numbers (p50 0.12 ms, 73 MB for two outputs); if it fails, the same layout is made of two surfaces by the same piece contract, a change in `bar/` and `app.rs` only |
| Input regions that miss a silhouette steal clicks from windows | the union is computed from the same function as the blur region and tested against the painted rects; the live row checks the edge pixels |
| Replacing the `Page` enum with a table breaks the verbs | `island open <page>` words come from `PieceId`; the characterisation tests and the command parser tests stay green unchanged |
| `focus-follows-mouse` over a full-width strip | the surface takes no keyboard at rest (decision 7); the live row checks hovering the bar's idle stretches |
| Two bars stacked while Noctalia's is visible | `bar` ships off; the live row prescribes `noctalia msg bar-hide` |

## 7. Later units: what each design must still decide

These are listed so each unit starts from a known question, not designed
here.

- **B:** whether idle's lock timer waits for H or is a no-op until then; the
  gamma transition and `wlr-gamma-control` ownership with three outputs; how
  the forced night light is persisted and toggled.
- **C:** OSD layout and the volume and brightness pieces' slider (the look is
  group 2, the behaviour is here); DDC latency and the bounded command queue;
  which of PipeWire's client crates or `wpctl` in a worker is the observer;
  the microphone's OSD and the lock-keys OSD Noctalia has switched off.
- **D:** which player is "active"; the media page's size and tabs.
- **E:** scaling modes and per-output images; the decode library, bounded
  against hostile files; how the `wallpaper` namespace and niri's backdrop
  rule interact with `place-within-backdrop`.
- **F:** which NetworkManager and BlueZ properties are read-only state and
  which actions the popups may send; secrets never pass through the shell.
- **G:** a privacy design of its own: what is never stored (password
  managers' hints, secret MIME types), size and count bounds, image handling,
  clearing, and what `clipboard-text` style access the shell never offers.
- **H and I:** their own specs (§4.4).
- **J:** Melibea's socket protocol version, a guard when `melibea.service` is
  absent, the selector's keyboard model.
- **K:** weather source and location (Noctalia auto-locates today; the shell
  must not call the network without a decision recorded).
- **L:** capture permission and rate for window previews; whether previews
  are on demand only.
- **M:** the exact niri config diff, ordering of autostart, and the rollback.

## 8. Extension points left for group 2

- **Tokens:** every size, radius, spacing, opacity and colour of the bar and
  its capsules comes from `Theme`, which is the data of `tokens.toml`; group 2
  changes those values and the schemes without touching the contract.
- **Piece look:** a piece's `bar()` returns a `View`; group 2 can restyle it or
  replace the zone capsule without changing `Piece`, `BarCx` or the layout
  data.
- **Layout:** the zones and the piece table are the model Customize and
  parkable pieces (Island, edge, floating) would edit; `shell.json`'s `bar`
  table is that model's persisted form. This program adds no UI for it.
- **Motion:** pieces animate through `Animated` and `Motion`; the contract
  carries no timing.
- **Schemes:** `Theme::by_name` already selects the scheme; the pieces read
  roles, never colours.
- **Edges:** the surface's anchors are one place (`bar/layout.rs`); a side
  edge or a bottom edge is that module's change plus vertical zones.

## 9. Verification and guards

Per unit: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
`cargo test`, `celestina --check`, `smoke.sh`, then the common guard
`bash scripts/check-architecture-contract.sh`, and the language and
documentation contracts for the unit's documents. Production artifacts are
built, verified and deployed by the landing, never in a session's worktree.
CEL-2-A adds the frame and memory comparison of §6.1 to its evidence.

## 10. Decisions recorded here (to become `celestina/docs/decisions/`)

10. **The parity program is delivered in thirteen units, A to M**, in the
    order of §5; Noctalia stops only at M.
11. **Duties are switched on one by one**, off by default while Noctalia holds
    them (§4.1).
12. **The bar is one surface per output with the Island in its centre zone**
    (§6.1).
13. **Everything the bar holds is a piece** behind one contract (§6.3).
14. **Lock and polkit carry ADR 0004 and 0005 over** and land with their own
    design, review and recovery path (§4.4).
15. **The clock rests at minutes**; seconds run only while the Island is open
    (§6.5).

(The numbers continue the list of the CEL-1 design, which ends at 9.)

## 11. Delivery of this document

This program's own landing is a documents unit: this spec, then the ledger
`celestina/docs/plans/active/2026-10-16-cel-2-parity.md` with CEL-2-A's row
(the other rows are `planned`), registered through the roadmap's
"Active implementation checkpoint" rule when the author opens the program.
The spec is committed in `main` as `suite-maintenance:`, as the CEL-1 design
was, only on the author's explicit authorization in chat. Before CEL-2-A
starts, the model and reasoning table the author's global rule requires is
presented and approved.
