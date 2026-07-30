# Replacing Noctalia — execution plan

> **Status lives in [ROADMAP.md](ROADMAP.md)**, which owns the R0–R9 checklists
> and the checkpoint history. This file is the *work order*: the session facts a
> phase needs, the files and contracts each phase touches, the evidence it must
> produce, its exit gate and its rollback. Nothing here is a checkbox — do not
> mirror status between the two documents.
> Settled execution decisions, open decisions and their falsifiers live only in
> this file; ROADMAP records implementation/gate status and dated evidence.
>
> Author decision, 2026-07-29: the shell replaces Noctalia entirely, one
> reversible responsibility bundle per phase. The parity target is the author's
> lived configuration (inventory in ROADMAP.md), never upstream Noctalia's
> surface.

## Resuming this work in a new session

1. Read, in this order: the root [`AGENTS.md`](../AGENTS.md) (mandatory suite
   contract), [`celestina/AGENTS.md`](AGENTS.md) (the shell's Rust/C++/QML
   boundary and its evidence matrix), [`ROADMAP.md`](ROADMAP.md) (state and
   phase checklists), then this file's phase section.
2. Re-verify before trusting: `git status`, the phase's checkboxes in
   ROADMAP.md, and the "Session facts" table below — it is a snapshot, and the
   session drifts. Anything marked *verified 2026-07-30* is a point-in-time
   observation, not live state.
3. Authorization: extending `celestina/` is authorized **for this plan's
   scope**, phase by phase. Two phases are explicitly gated behind a fresh
   author decision — R6 (lock/PAM) and R8's first-party Polkit agent. Nothing
   here authorizes installing packages, editing `~/.config/niri/`, enabling
   services, or committing/pushing: those are author actions, and the plan
   names them so the author can run them.
4. Never turn a Noctalia service off outside its own phase, and never as a side
   effect of another change.

## Ground rules for every phase

- **One reversible retirement bundle per phase.** Some phases replace several
  coupled verbs that share one rollback (R3, R5, R8). Each bundle is divided
  into reviewable slices and ends only when its Noctalia counterpart can be
  switched off and stay off for daily use.
- **A gate is real-session evidence**, not a build. Build proves compilation;
  smoke proves start-up; neither proves layer-shell geometry, focus, blur,
  compositor behavior, portals, hardware or accessibility. State exactly what
  was verified and what was not.
- **A fallback stays named and one command away** until the gate closes. The
  rollbacks in each phase are written for the author to run, not the agent.
- **Domain in Rust, adapters thin, presentation in QML.** New pure logic goes
  to a `celestina-rs` crate even with one consumer; Qt/D-Bus/XDG marshaling
  lives in `celestina/src/`; `qml/` never opens sockets, spawns processes or
  decides protocol. Every new QML file is registered in `CMakeLists.txt`
  `QML_FILES` (the root guard enforces exact parity with `qml/`).
- **Colors, radii, typography, motion come from `CelestinaTheme`.** The style
  guard scans `celestina/qml`. New shared components need either a
  `celestina-style/DESIGN.md` spec or two real consumers.
- **Every new dependency is justified in `Cargo.toml`** and, if it is a system
  package, approved by the author first.
- **Truthful state.** A click is a request; the UI shows confirmed results
  only. Best-effort D-Bus degrades, never blocks or crashes the panel.
- **Tests land in the same change** as the domain feature.

### Minimum evidence matrix (from `celestina/AGENTS.md`, run from the repo root)

```sh
bash scripts/check-architecture-contract.sh
cargo fmt --manifest-path celestina/Cargo.toml --all --check
cargo clippy --manifest-path celestina/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path celestina/Cargo.toml --all-targets --locked
cmake -S celestina -B celestina/build -DBUILD_TESTING=ON
cmake --build celestina/build
cmake --build celestina/build --target all_qmllint
ctest --test-dir celestina/build --output-on-failure
```

Touching `celestina-rs` adds the workspace's own fmt/clippy/test run; touching
`celestina-style` adds its guard, `all_qmllint`, the gallery and **every**
consumer (siderita, magnetita, shell).

Real-session activation is `celestina/scripts/run.sh` (builds Release and maps
the panel in the foreground; Ctrl-C stops). During development the author hides
only Noctalia's bar so its other services keep running:

```sh
noctalia msg bar-hide
```

## Session facts (verified 2026-07-30 — re-check before relying on them)

| Fact | Value |
|---|---|
| Compositor | `niri 26.04 (8ed0da4)`; adapter pinned to `niri-ipc =26.4.0` |
| Outputs | `HDMI-A-1`, `DP-1`, `DP-2` (a desktop: no battery, no backlight — brightness is DDC only) |
| Noctalia | `noctalia-git 5.0.0.r4301.g571001097-1` — a native C++/Wayland/GLES binary since v5, **not** Quickshell; `noctalia --daemon`, IPC `noctalia msg <verb>` (~100 verbs) |
| Greeter | `noctalia-greeter-git 1.0.0.r140` — separate package (greetd + its own bundled wlroots compositor); runs without the shell |
| Noctalia config | `~/.config/noctalia/settings.json` (settingsVersion 59), `plugins.json`, `colors.json`, `palettes/`, `plugins/` |
| Present on the system | `wlsunset`, `grim`, `slurp`, `gpu-screen-recorder`, `ddcutil`, `playerctl`, `brightnessctl`, `wpctl`, `wl-copy`/`wl-paste`, `polkit` (library only) |
| **Absent** — do not plan on composing these without installing them first | `cliphist`, `satty`, `swaylock`/`hyprlock`/`gtklock`, `swayidle`, `gammastep`, any Polkit **agent** package |

### Niri config touch points (`~/.config/niri/config.kdl`, line numbers drift)

| Line | Content | Phase that changes it |
|---|---|---|
| 80 | `spawn-at-startup "noctalia"` | R8 (last) |
| 82 | `sleep 1 && noctalia msg caffeine-enable && noctalia msg nightlight-force-toggle` | R3 |
| 131 | `layer-rule` placing the wallpaper namespace in the backdrop | R7 |
| 256 | `Mod+Space` → `noctalia msg panel-toggle launcher` | R2 |
| 257 | `Mod+Shift+Escape` → `noctalia msg session lock-and-suspend` | R3 |
| 258 | `Mod+Shift+A` → `noctalia msg dpms-off` | R3 (maps to `niri msg action power-off-monitors`) |
| 361 | `include "./noctalia.kdl"` | R7 (the include is frozen — last regenerated 2026-06-11, its blue accent no longer matches the monochrome palette, and templates are disabled) |

Keybinds that **already bypass Noctalia** and must keep working unchanged:
volume via `wpctl`, media via `playerctl`, brightness via `brightnessctl`,
screenshots via niri's native `screenshot-path`, `Mod+Shift+C` =
`niri msg pick-color | wl-copy`, `Mod+Shift+T` = grim+slurp+tesseract OCR.

### Two corrections to earlier assumptions

- **Clipboard history is not composed from `cliphist`.** The
  `clipboardWatch*Command` keys in `settings.json` are v4 residue; the binary is
  not installed and Noctalia v5's history is built-in (encrypted at rest), with
  the `clipper` plugin providing the richer bar panel. R2 must therefore choose
  between installing `cliphist` (new system dependency, author approval) and a
  shell-owned history over `wlr-data-control`/`ext-data-control` in a Rust
  crate. Do not write "compose the existing cliphist" — there is none.
- **There is no external locker or Polkit agent installed.** Noctalia's
  `polkit-agent` plugin is the session's only agent, which is why it is the last
  thing to leave (R8). "Compose an external tool" in R3/R8 means *install and
  verify one first* — an author decision, not a free fallback.

## Shell baseline — what already exists (line counts 2026-07-30)

| File | Lines | Responsibility |
|---|---|---|
| `src/main.cpp` | 389 | process bootstrap · `PanelManager` (per-output layer-shell lifecycle) · style import self-provisioning · `--pick-output` mode — **three responsibilities in one file; extracting `PanelManager` is R0 work before any second surface** |
| `src/niri_adapter.rs` | 284 | pinned Niri event stream → narrow workspace snapshots; 1 s socket reconnect; read-only, `Request::EventStream` only |
| `src/niriclient.{h,cpp}` | 52 / 268 | `QProcess` lifecycle, strict JSON validation, confirmed snapshots on the GUI thread, 250 ms→10 s restart backoff |
| `src/niriprotocoldecoder.{h,cpp}` | 25 / 49 | bounded line framing; discards a >1 MiB frame through its newline and recovers |
| `src/panelblurcontroller.{h,cpp}` | 39 / 119 | per-surface KWindowEffects capability/retry/geometry + the explicit `compositorBlurAvailable` fallback property |
| `src/devicesclient.{h,cpp}` | 50 / 107 | async, burst-coalesced QtDBus client of `org.celestina.Devices1` |
| `qml/Panel.qml` | 68 | hidden-until-configured three-region root window |
| `qml/WorkspaceStrip.qml` | 123 | per-output workspace pills + active window title |
| `qml/Clock.qml` | 39 | minute-aligned local time |
| `qml/PhoneStatus.qml` | 72 | phone identity, charge state, battery |
| `qml/OutputChooser.qml` | 326 | the screen-share chooser (a regular `Qt.Dialog` window, app_id `celestina`, prints `Monitor: <name>` on stdout) |

Surface configuration today (hard-coded inline in `PanelManager::ensurePanel`):
namespace `celestina-panel`, anchors Top|Left|Right, desired size `0×40`,
exclusive zone 40, `LayerTop`, `KeyboardInteractivityNone`,
`setActivateOnShow(false)`, `setCloseOnDismissed(false)`, `show()` last.

How data reaches QML today: `main()` builds one provider QObject per source
(`NiriClient`, `DevicesClient`) and injects the raw pointers into each panel via
`initialProperties` (`required property var`); `Panel.qml` binds typed scalars
down to child components, which never touch providers. A `changed()` signal per
provider re-evaluates the bindings. This is baseline, not the scalable R1
contract: R1-A fixes one aggregate Rust provider runtime and bounded Qt bridge
before another provider is added, so `PanelManager` does not accumulate one
process/client/member per widget.

Known gaps that shape the phases: there is exactly one surface kind (the
panel), the adapter socket is used for one request and its pipe is one-way, the
shell has **no command or activation channel at all**, and no popup has ever
been mapped from the panel.

Existing automated coverage: three Rust reducer tests (`niri_adapter.rs`), the
`NiriProtocolDecoder` QtTest, and the `OutputChooser` Qt Quick Test. Not
covered: `NiriClient` lifecycle, `PanelBlurController`, `DevicesClient`,
hotplug, panel rendering, real-compositor IPC restart.

---

# Phase work orders

Slice labels below are causal review units, not a second status system. Only
`ROADMAP.md` owns checkboxes. A slice may land after its local evidence passes;
the phase gate closes only after the integrated exit test on the real session.

## R0 — Foundations

**Goal.** Close the panel's pending acceptance and land the three contracts
every later phase reuses: a shared surface recipe, a popup path, a command
channel.

**Build order (review slices).** R0-A extracts `PanelManager` mechanically →
R0-B adds focus requests → R0-C adds `Shell1` and the CLI → R0-D builds the
popup candidates locally without a shared surface abstraction → R0-E proves the
second surface and settles the popup decision on real Niri → R0-F extracts only
the demonstrated surface intersection and performs integrated acceptance.
R0-A through R0-D are automation-first. R0-E and R0-F form one uninterrupted,
author-assisted live-session block: prove first, extract second, then re-run the
real acceptance before ending the block.

**Work items.**

1. *R0-A — extract `PanelManager`.* Move it out of `main.cpp` into
   `src/panelmanager.{h,cpp}` without changing behaviour. Build and CTest guard
   the mechanical move; the existing live mapping is rechecked in R0-E/F. Do
   not mix this move with a speculative surface abstraction.
2. *R0-B — workspace focus requests (pending/failed/confirmed).* Extend
   `src/niri_adapter.rs` with a dedicated bounded stdin reader, a bounded command
   queue and one serialized stdout writer so event snapshots and request frames
   cannot interleave. Each action uses a short-lived second
   `niri_ipc::Socket` (`Request::Action`). The adapter emits typed request frames;
   `NiriClient` owns a `Q_INVOKABLE` request method, current-generation
   bookkeeping and timeout → failed. An ack means accepted/pending;
   **confirmation requires a later snapshot matching the requested output and
   workspace**, never merely the next snapshot. Threads/queues close and join
   deterministically. `NiriProtocolDecoder` remains a semantic-free framer;
   message validation tests belong to Rust and `NiriClient`, not the decoder.
3. *R0-C — command channel and process roles.* The panel-mode Qt host owns the
   stable session-bus name `org.celestina.Shell`, object path
   `/org/celestina/Shell1` and versioned interface `org.celestina.Shell1`
   through QtDBus; changing the interface version never permits a second host.
   No hidden Rust daemon is added. Its minimum backward-compatible contract is
   `GetState() → a{sv}`, `Command(s verb, a{sv} options) → t request_id`,
   `Changed(a{sv})` and `CommandResult(t request_id, s state, a{sv} details)`.
   State/payloads carry a version key. Only panel mode is single-instance;
   `celestina msg <verb>` and `--pick-output` are transient clients that do not
   claim the name. The CLI exits non-zero on rejection, bus loss or timeout.
   Every later keybind routes here; nothing invents a second channel.
4. *R0-D — local popup candidates.* Build narrow, disposable candidates for an
   `xdg_popup` parented to the panel and a separate anchored layer surface. Do
   not factor a `SurfaceManager` yet: the panel is still the only proven
   consumer. Automated construction/lifecycle tests retire failures that do not
   require a compositor.
5. *R0-E — prove the second surface and settle the popup decision.* Map the
   candidates from a real `GlassContextMenu` on Niri and compare placement,
   keyboard, dismissal and focus return. The panel is
   `KeyboardInteractivityNone`, so a keyboard menu needs an `OnDemand` surface.
   Record the verdict and falsifier in this file's decision section; record the
   dated live evidence only against R0's ROADMAP gate.
6. *R0-F — extract the proven intersection and accept R0.* After R0-E fixes the
   popup contract, factor only the create/configure/teardown intersection shared
   by panel and selected popup into `src/surfacemanager.{h,cpp}`. Then verify one
   panel per output with correct height, exclusive zone, namespace and no
   keyboard focus; exercise CLI → host, focus pending → confirmed plus
   failed/timeout, adapter kill and compositor restart without stale state, and
   popup focus/dismissal after the extraction. Record the exact Qt/LayerShellQt
   tool versions, exercise invalid import/root/layer setup to a visible non-zero
   exit, and capture CP0's artifact/startup/PSS-RSS/wakeup/GPU baseline. OSD and
   launcher reuse this demonstrated contract later instead of growing
   speculative flags now.

**Evidence contract.** Each slice runs the narrow tests it changes; R0-F runs
the full matrix. Real-session capture is mandatory for panel/popup/focus
behaviour. Dated results and gate status go only in ROADMAP; the popup decision
and falsifier stay only in this work order.

**Integrated exit test.** From a clean shell start, a second panel-mode process
defers to the owner; `celestina msg` reaches that owner; one workspace request
shows pending then matching confirmed, another fails or times out visibly; the
adapter/compositor can disappear and return without stale state; the popup
dismisses and returns focus correctly; all outputs retain correct geometry;
invalid setup fails visibly and the recorded resource baseline is finite.
Noctalia remains untouched throughout.

**Gate.** Nothing retired. Noctalia untouched.

## R1 — The bar

**Goal.** The panel covers the bar configuration actually lived in, so
Noctalia's bar hides permanently.

**Build order (review slices).** R1-A freezes and proves the provider runtime →
R1-B lands flanks/workspace/clock/invisible extension points → R1-C adds SysMon
and external screenshot → R1-D media → R1-E volume/mic → R1-F
network/Bluetooth/power profile → R1-G per-output DDC → R1-H tray → R1-I
integrated parity and handover. R1-H is unconditionally separate: SNI + DBusMenu
is the phase's dominant cost.

**R1-A provider runtime contract.** Pure reduction, parsing and policy live in
one new workspace crate, `celestina-rs/crates/celestina-shell-core`. Long-lived
non-Qt IO for bar providers lives in modules of one app-local Rust binary,
`celestina-provider-adapter`, declared by `celestina/Cargo.toml`; never spawn one
helper per widget. One thin `ShellProvidersClient` QObject owns that process,
bounded line framing, generation clearing and Qt marshaling. The helper emits
coalesced provider snapshots and accepts bounded typed commands over the same
serialized-writer pattern proven by R0-B. Compare the existing Niri framer and
extract only a protocol-neutral intersection if both consumers really share it.
The Qt host remains owner of surfaces, the stable `org.celestina.Shell` name and
the `Shell1` interface; `DevicesClient` remains the phone-specific QtDBus client.
Later shell-owned non-Qt bus services, including notifications, extend the
aggregate helper rather than inventing another runtime.

A clock format does not earn fake Rust domain logic, but process polling,
subprocess lifecycle and non-Qt D-Bus IO do not migrate into QML or grow one C++
controller per provider. Widgets receive narrow confirmed scalars and are
registered in `CMakeLists.txt`. Thresholds and policy live in core/config; the
theme only maps semantic states to appearance (`fontFeaturesTabular` for
numerics).

**Work items.**

1. *R1-A — prove the runtime before providers.* Land the core crate, aggregate
   helper and bounded Qt client with fixture snapshots, command rejection,
   provider disappearance, generation reset and deterministic-shutdown tests.
   Register the crate/helper in the workspace, build and affected README/ROADMAP
   inventories in the same change. No production widget lands in this slice.
2. *R1-B — composition, workspaces, clock and extension points.* Convert
   `Panel.qml`'s flanks to composable ordered rows while the clock remains
   geometrically centred. Add click-to-focus through R0's request contract and
   scroll-to-switch with wrap, including keyboard/AT-SPI press actions. Use the
   lived format `HH:mm:ss - MMMM - dddd dd`; align the timer to seconds only
   while seconds are visible. Reserve structural insertion points for caffeine
   (R3), unread notifications (R4) and weather (R5), but paint no placeholder or
   fake state.
3. *R1-C — SysMon and screenshot.* Read CPU % + RAM from `/proc` with bounded
   polling and expose core/config thresholds as semantic states. Click-through
   to the external monitor remains an exec. The screenshot action invokes the
   exact existing external flow through a typed helper command; it does not
   reimplement capture.
4. *R1-D — media mini (MPRIS).* Reuse `magnetita-core::mpris` vocabulary
   (`PlayerState`, `MediaAction`, `playback_progress()`) — do not invent a
   second media vocabulary. Start from the installed `playerctl` subprocess
   precedent (`magnetitad/src/media.rs`) with a bounded, joined follower. Reopen
   a native `org.mpris.MediaPlayer2` reader only if that baseline fails a recorded
   correctness, latency or wakeup budget. Phone media stays on `Devices1`.
   Artwork bytes are untrusted: follow `magnetitad/src/artwork.rs` (size caps,
   signature check, disposable cache under `$XDG_RUNTIME_DIR`).
5. *R1-E — volume + mic.* PipeWire state and level; middle-click keeps launching
   the external mixer. Start from installed `wpctl`; reopen a native client only
   if bounded polling/subscription cannot meet truthfulness, latency or wakeup
   budgets.
6. *R1-F — network, Bluetooth and power profile.* NetworkManager and BlueZ
   indicators remain read-only; controls arrive in R5. Add the power-profile
   indicator/cycle over power-profiles-daemon as a confirmed request.
7. *R1-G — brightness.* Per-monitor DDC via `ddcutil` (no backlight on this
   hardware); scroll steps; value on hover. DDC calls are slow — off the GUI
   thread, coalesced, with a visible unknown state rather than a lie.
8. *R1-H — tray.* StatusNotifierItem host + DBusMenu bridge in dedicated modules
   of `celestina-shell-core` and the aggregate provider helper. This remains its
   own slice after the rest of the bar is live. Drawer presentation and passive
   items are included.
9. *R1-I — integrated parity and handover.* Run the integrated exit below,
   record its dated evidence in ROADMAP, then apply the exact persistent bar
   hide only after the author approves that config edit.

**Evidence contract.** Full matrix plus a real-session capture of the panel on
all three outputs; per-widget, record what was verified interactively and what
was not. Results and gate status live only in ROADMAP.

**Integrated exit test.** Start with each provider present, then remove or break
one at a time: no stale value survives, the rest of the bar remains usable and
slow DDC never blocks the GUI. Observe real media and volume state transitions;
exercise only the actions actually in scope — workspace focus/scroll, screenshot,
DDC, power profile and tray — and observe confirmed/failed outcomes. Restart the
shell, hotplug an output and inspect the final composition on all three outputs
before changing the persistent bar setting.

**Gate.** `noctalia msg bar-hide` becomes permanent (moved into the author's
config, not a manual command). Noctalia keeps running for everything else.
**Rollback.** Record the exact persistent config edit and its inverse; restoring
that edit plus `noctalia msg bar-show` restores the current session and the next
login.

## R2 — Launcher

**Goal.** `Mod+Space` opens a Celestina launcher: app grid, 19 pins,
categories, fuzzy search, `kitty -e` for `Terminal=true` entries.

**Work items.**

1. *Pure core module.* Desktop-entry index + fuzzy match/rank (usage boost
   optional) in `celestina-shell-core`, unit-tested. `siderita/src/apps.rs`
   already scans `.desktop` files for open-with — compare and either generalize
   it into the shared crate or record why the semantics differ. Truthfulness
   contract borrowed from
   `siderita/src/search.rs`: bounded, cancellable (`celestina_core::CancellationToken`),
   outcome says whether results were truncated.
2. *On-demand keyboard surface.* Overlay layer,
   `KeyboardInteractivityOnDemand`, opened via `celestina msg launcher-toggle`
   (R0's channel), dismissed on Escape and focus loss. Keyboard operation and
   AT-SPI are part of "done", not a follow-up — patterns:
   `siderita/qml/PickerWindow.qml` for list navigation,
   `CelestinaModalLayer` for focus containment/restore.
3. *Launch.* Spawn detached; a launch is a request — a failure surfaces.
4. *Clipboard history.* **Open decision (see corrections above):** install
   `cliphist` and compose it permanently, or own the pure history policy in
   `celestina-shell-core` with data-control IO in the aggregate provider helper.
   Either way it is a separate panel, not a launcher provider. Clipper's
   notecards/pins are a named loss unless the author asks for them.
5. *Web-search provider.* Open the URL in the default browser. Emoji, kaomoji
   and unicode providers are optional later and never gate the phase.

**Integrated exit test.** After a clean shell restart, `Mod+Space` opens exactly
one overlay; pins and categories select the expected apps, a `Terminal=true`
entry launches through `kitty -e`, web search opens the encoded URL, and a
failed desktop entry reports truthfully. Keyboard-only launch and
Escape/focus-loss dismissal work; AT-SPI exposes search, results and actions.
Clipboard history is exercised with sensitive/large input, restart and its
chosen retention policy before the old binding changes.

**Gate.** `Mod+Space` rebinds to `celestina msg`; Noctalia's launcher and
clipper go unused. **Rollback.** One line in `config.kdl` (L256).

## R3 — Session verbs: OSD, night light, caffeine, DPMS, composed lock

**Goal.** The keyboard-driven session verbs stop passing through Noctalia.

**Work items.**

1. *OSD surface.* Overlay layer, top-right, ~2 s auto-hide, no keyboard, no
   exclusive zone — built on R0's surface recipe. Honors
   `CelestinaTheme.reducedMotion`. The `Toast` spec in
   `celestina-style/DESIGN.md` §6.8 is waiting for its first consumer:
   complete that spec rather than inventing a surface; the slider inside it is
   `CelestinaSlider`, also spec-only today.
2. *Route the keys.* Volume and brightness binds move from raw
   `wpctl`/`brightnessctl` to `celestina msg` verbs that apply the change *and*
   raise the OSD (today the binds bypass Noctalia and its OSD merely observes,
   which is why this phase has to own both halves).
3. *Night light.* Compose the already-installed `wlsunset` at the lived constant
   2700 K. Verify clean gamma release on normal exit and crash; reopen a Rust
   `wlr-gamma-control` module inside the aggregate provider helper only if this
   bounded composition fails.
4. *Caffeine.* Shell-owned idle-inhibit toggle
   (`zwp_idle_inhibit_manager_v1`) in the aggregate provider helper, wired to
   the R1 bar slot. The idle chain itself (lock → screen off → suspend) lands
   **disabled by default**, mirroring the lived autostart that force-enables
   caffeine.
5. *Lock and DPMS.* `celestina msg session lock-and-suspend` composes a locker
   plus logind suspend — **no locker is installed today**, so pick and verify
   one with the author first. `dpms-off` maps to
   `niri msg action power-off-monitors` (verified present in niri 26.04).
6. *Autostart cleanup.* Drop `noctalia msg caffeine-enable` /
   `nightlight-force-toggle` from `config.kdl` L82; rebind L257/L258.

**Integrated exit test.** From a clean login, each volume/brightness key applies
one confirmed change and raises one truthful OSD; unknown/provider-loss states
do not show fabricated values. Night light releases gamma when stopped;
caffeine can acquire/release inhibition and survives shell surface churn without
silently enabling idle; DPMS exposes success and failure. Exercise one successful
lock-and-suspend cycle and verify the locker covers every output before suspend
and remains locked through resume; separately expose locker/logind failure
without suspending an unlocked session. Verify the inverse config/binds before
retiring the Noctalia paths.

**Gate.** Noctalia's OSD, night light, idle and lock paths off. **Rollback.**
Restore the two bind lines and the autostart line; both are git-tracked.

## R4 — Notifications

**Goal.** The shell is the session's freedesktop notification server.

**Work items.**

1. *Pure spec state machine* in a `celestina-shell-core` notification module:
   ids, `replaces_id`, expiry, urgency, actions, capabilities and
   `GetServerInformation`, unit-tested against the spec.
2. *Bus service* runs as a module of the aggregate Rust provider helper fixed by
   R1-A, following the `serve_devices()` zbus-blocking pattern; the Qt host keeps
   owning surfaces and `Shell1`. Icon and image bytes are hostile input:
   `artwork.rs` hygiene applies.
3. *Presentation.* Bottom, compact toasts (the lived config); action buttons;
   history capped like `RecentLog`; a DND verb on `Shell1`; the unread badge
   fills its R1 slot.
4. *Handover.* Disable Noctalia's daemon first (`enable_daemon = false`), then
   claim `org.freedesktop.Notifications` — two owners cannot coexist.
   `celestina-rs/crates/magnetitad/src/notify.rs` is an in-house consumer
   relying on `Notify`/`replaces_id`/`CloseNotification`: phone notifications
   arriving, replacing and withdrawing correctly **is** part of this phase's
   evidence.

**Integrated exit test.** Exercise new, replacement and close flows, expiry,
urgencies, an action callback, DND and malformed/oversized image data; then
restart the shell and verify it reacquires the name without two owners or stale
toasts. Phone notifications from Magnetita must arrive, replace and withdraw.

**Gate.** Noctalia notifications off. **Rollback.** Flip `enable_daemon` back.

## R5 — Control center, session menu, weather & calendar

**Goal.** The panels behind the bar's clicks, and the first multi-provider
write surface.

**Work items.**

1. *Control center panel.* Quick toggles actually used (network, bluetooth,
   night light, caffeine, DND, power profile) plus cards: audio (PipeWire
   outputs/inputs/streams — the first write surface), brightness sliders, media,
   sysmon. Built from `ListSection`, `CelestinaSwitch`, `CelestinaSectionLabel`
   — DESIGN.md reserves `ListSection` for exactly this; the closest existing
   consumer to copy is `magnetita/qml/pages/SettingsPage.qml`.
2. *Session menu.* Lock / suspend / reboot / logout / shutdown over logind,
   with the lived numbered shortcuts and countdown. Actions are requests with
   visible outcomes.
3. *Weather.* Pure policy/cache in `celestina-shell-core` and bounded Open-Meteo
   IO in the aggregate provider helper (a justified external dependency), with
   manual or IP location, feeding the R1 bar slot and a card. Calendar: month
   view. Account sync is excluded from R5; reopen CalDAV/Google only after
   observed use proves it belongs.
4. *Settings persistence.* Exactly `magnetitad/src/settings.rs`: serde with
   per-key `#[serde(default)]`, `celestina_core::atomic_file::replace`,
   persist-before-publish (a failed write never publishes a toggle),
   `celestina_core::xdg::config_home()` for the path.

**Integrated exit test.** With one provider unavailable at a time, the panel
degrades without blocking unrelated controls. Toggle every write action and
observe provider-confirmed success/failure; cancel and confirm each session
verb; restart and verify only successfully persisted settings return. Weather
failure leaves a truthful stale/unavailable state and the month view remains
local and usable.

**Gate.** Bar mouse actions open Celestina panels; Noctalia's control center,
session and calendar panels go unused. **Rollback.** Noctalia's panels still
answer while it runs.

## R6 — First-party lock & idle *(author gate: security)*

Do not start without a fresh, explicit decision from the author. Until then the
composed external locker from R3 remains the answer.

**Work items.** `ext-session-lock` + PAM locker covering every output; no
bypass if the locker crashes (the compositor must keep the session locked);
hotplug while locked; lock-on-suspend through a logind `PrepareForSleep`
inhibitor. Look: blurred/tinted capture, visible password characters, session
buttons — no media controls, countdown or fprintd (all unused today). Revisit
the idle chain with real timings once living without forced caffeine exists.

**Integrated exit test.** On the real compositor: reject an incorrect
credential, accept a correct one, kill the locker without exposing the session,
hotplug an output while locked, suspend/resume through the inhibitor and repeat
with one provider unavailable. The external locker rollback is verified before
it stops being the default.

**Gate.** The external locker becomes the fallback rather than the default.

## R7 — Wallpaper & look

**Work items.**

1. *Wallpaper.* Per-output layer surfaces at the background layer, crop fill,
   manual random from `~/Imágenes/Fondos`. Move the Niri `layer-rule` at L131
   from Noctalia's namespace to the shell's so the backdrop keeps working. The
   disc-style transition is optional and must honor `reducedMotion`.
2. *`Settings` portal.* Served by the shell (CP2's portal table in ROADMAP.md):
   colour scheme + accent. The session is permanently dark, so this is one
   value served truthfully. Replaces `syncGsettings` for GTK dark sync.
3. *Niri colors.* Replace the frozen `noctalia.kdl` include with colors stated
   from `CelestinaTheme` — a documented one-time generation, not a template
   runtime.
4. *Not replaced.* Wallpaper-derived palettes: the lived scheme is
   monochrome-on-dark, visually adjacent to the sealed CelestinaTheme dark
   scheme. An accent-from-wallpaper pipeline is a someday decision, not part of
   this phase.

**Integrated exit test.** Start/restart with all outputs, hotplug one, remove or
corrupt the selected wallpaper and verify a readable fallback. Query the
`Settings` portal from a real client and confirm dark/accent values, then verify
GTK sync and the generated Niri colours across the next login. Restoring the old
namespace/include must remain a tested rollback until the gate closes.

**Gate.** Noctalia wallpaper and theming off.

## R8 — Polkit, dock, and Noctalia leaves

**Work items.**

1. *Polkit agent* — the last invisible dependency; nothing else keeps Noctalia
   alive after R7. Staged like lock: adopt a proven external agent first (none
   is installed — this needs an author-approved package), and only then, behind
   an explicit author gate, a first-party agent with its own dialog in the
   shell's language.
2. *Dock.* Bottom, auto-hide, running apps only — per-output window list from
   the Niri adapter (which does not consume the window list today; that is new
   adapter work) plus a dock surface. Confirm its daily value before building:
   it is enabled and used today, but it is also the cheapest thing to drop.
3. *Departure.* Remove `spawn-at-startup "noctalia"` (L80); archive
   `~/.config/noctalia`; uninstall `noctalia-git` only after **seven consecutive
   daily sessions** complete without a Noctalia process or an unrecorded
   fallback. A failed day resets the soak after the blocker is corrected.

**Integrated exit test.** Authenticate, cancel and fail one real Polkit request;
if the dock decision retains it, exercise launch/focus, window removal and output
hotplug; if it drops the dock, record the loss and falsifier instead of requiring
an absent surface. Reboot into a session with no Noctalia process and run the
seven-session soak log. Every remaining external process and rollback command
is named before uninstall.

**Gate.** No Noctalia process runs in the session, and every remaining external
tool is named in ROADMAP.md. **Rollback.** Reinstall the package; the archived
config restores the old session.

## R9 — Greeter *(someday; default: keep it)*

`noctalia-greeter` is a separate package that works without the shell — after
R8 only its polkit-gated appearance sync goes unused. Default decision: keep
it. If it ever regresses: `regreet` or `tuigreet`. A first-party greeter means
owning a display-manager-grade compositor and is not planned.

---

## Settled execution defaults (2026-07-30)

These choices remove avoidable pauses while preserving a named falsifier. They
are canonical only here; ROADMAP may reference their implementation status but
does not duplicate their rationale or falsifier. Do not reopen them from
preference alone.

| Phase | Default | Reopen only if |
|---|---|---|
| R0 bus identity | stable owner `org.celestina.Shell`; object/interface versioned as `Shell1` | an existing owner collision or the cross-version single-instance test fails |
| R1 provider runtime | `celestina-shell-core` + one aggregate app-local Rust helper + one bounded Qt client | R1-A demonstrates provider isolation, latency or deterministic shutdown cannot meet its budget |
| R1 media | installed `playerctl` subprocess/follower | measured correctness, latency, shutdown or wakeup budget fails |
| R1 volume | installed `wpctl` composition | bounded observation cannot report truthful state inside its budget |
| R1 tray | own R1-H slice after the other providers | the lived bar no longer uses a tray at the R1 gate |
| R3 night light | installed `wlsunset`, fixed 2700 K | gamma is not released cleanly or the lived constant cannot be expressed |
| R5 calendar | local month view; no account sync | observed daily use proves CalDAV/Google is required |

## Open decisions log

A future session should not silently pick one of these. When it closes, move its
choice, reason and falsifier into the settled table in this file and remove its
open row. ROADMAP records only the resulting implementation/gate status and the
dated evidence that justified closing it.

| # | Decision | Phase | Notes |
|---|---|---|---|
| 1 | `xdg_popup` on the layer surface vs. a separate anchored layer surface for menus | R0 | R0-E's real-session probe decides; no preference-only choice |
| 2 | Clipboard history: install `cliphist` vs. own it over `wlr-data-control` | R2 | `cliphist` is not installed; either path changes the trust/dependency boundary |
| 3 | Which external locker to install and verify | R3 | None installed; author/package approval blocks the R3 gate |
| 4 | Which Polkit agent to adopt; whether a first-party one is ever wanted | R8 | External package and any first-party agent need author gates |
| 5 | Whether the dock is worth building at all | R8 | Enabled today; cheapest lived feature to drop |

## Named losses (detail in ROADMAP.md)

music-search (Fluorita-shaped territory), clipper notecards/pins, the catwalk
cat (optional cosmetic epilogue), screen-toolkit's annotate/pin/QR/measure/
webcam (one external tool per function, or dropped), the screen-recorder plugin
(`gpu-screen-recorder` directly — it is already unplaced in the bar), and the
`NoctaliaPerformance` toggle (obsolete once nothing heavy is left to disable).
Never: the plugin runtime, the plugin store, community palettes, app-theming
templates, desktop widgets.
