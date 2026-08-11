# Celestina status

- **Updated:** 2026-08-11
- **Implementation:** R0-R5, R7, R8's departure slice, LVR-1 through LVR-3, the
  static hardening previously drafted as `AUD-1`, `UX-1` and `WSG-1` are
  complete
- **Design direction:** `PANEL-1` is active for the author-selected borderless
  glass bar: no full-width shadow, real compositor-blur capsules, dense dark
  content material with a fixed light/white foreground, and nearly transparent
  contextual carriers. The rest of `UX-2` remains planned under the still-open
  `SHELL-D5` discussion
- **Author validation:** the author closed the LVR-3 phase on 2026-08-07 after
  first-generation media, the four-item tray, Bluetooth state retention,
  output-triggered DDC rediscovery, outside-click dismissal and a clean
  Noctalia to Celestina to Noctalia lifecycle all passed. Wi-Fi remained visible
  throughout the exercised session; deliberate offline testing was unsafe in
  the live network layout and remains deferred rather than inferred. See
  [VALIDATION.md](VALIDATION.md)
- **Live migration:** Noctalia remains the rollback and must not be removed.
  `scripts/handover-status.sh` reports the unrecorded responsibilities, and the
  removal tool refuses while any are unbuilt, unrecorded or failed
- **GPU safety hold: ended 2026-08-07.** `VAL-GPU-01` passed: a long
  Noctalia-only observation and two controlled handovers, one without DDC and
  one with DDC, hotplug, brightness and media, produced no PCIe device loss.
  That is strong negative reproduction evidence, not proof, so every DDC
  invariant stands — one owning worker, global serialization, coalesced
  operations, a bounded timeout, deterministic cancellation, a killed and
  reaped child, and no frequent polling. Builds, tests and deployment run
  again; Noctalia still owns the session and Celestina is not activated. See
  the
  [system audit](docs/evidence/2026-08-05-gpu-loss-system-audit.md) and
  [lifecycle record](docs/evidence/2026-08-05-ddc-process-lifecycle.md).

## Current checkout truth

- **Delivered prototype snapshot — `PANEL-1-B`.** Celestina 0.11.0 records the
  cumulative uncommitted B-H sequence as one milestone delivery. It includes
  the contextual menu hierarchy, panel grouping, tray and wallpaper tools,
  non-xray compositor profile, canonical shared glass and the final fixed
  light/white foreground over dense dark content cards and panel capsules. The
  retracted contrast analysis, appearance publication, host adapter and QML
  polarity inputs are absent rather than dormant. The wallpaper gallery,
  per-output selection, same-path image reload and atomic import remain. The
  canonical production exit passed and deployed the verified bundle without
  activation; a nested-only restart confirmed the fixed-white instance while
  leaving host Niri and Noctalia intact. The earlier B-H labels below preserve
  prototype chronology, not separate published versions. `PANEL-1` remains
  active and `PANEL-1-I` is reserved for the next author-selected design pass.

- **Delivered in celestina 0.10.0 — `PANEL-1-A`.** The first borderless-glass
  panel baseline removes the hard full-width plate, keeps its soft wallpaper
  shadow, and places each content group on a finite compositor-blur capsule.
  Workspaces retain per-monitor grouping as positional state marks; status and
  action readings use the canonical 1.2.0 glyph catalogue; CPU and memory keep
  their percentages; the phone omits its device name; and the existing control
  centre, clipboard, notifications and session surfaces have permanent panel
  entry points. Tray registration is reconciled after startup and its wrapper
  follows the item model, so four published items remain visible after a host
  restart. The canonical production exit built, verified and deployed 0.10.0
  without activating the session. `PANEL-1-B` now applies the bounded Velo
  candidate to every existing interactive shell menu: network, Bluetooth and
  tray preserve their real Qt `Menu` lifecycle, while workspace map, control
  centre, clipboard, notification centre, session and launcher preserve their
  custom overlay focus and dismissal models. Every surface publishes one light
  compositor-glass body and uses denser tint-only sections rather than a stack
  of resting row pills. A bounded worker now samples each output's exact ready
  wallpaper and selects a surface-local light or dark foreground by WCAG
  contrast without changing the global theme. The host admits that result only
  when output, source, file revision, inventory generation and crop geometry
  match the exact image request that reached `Image.Ready`; all exposed text
  foregrounds stay on the two measured candidates. Loading, stale, failed and
  no-blur paths keep the established light-ink/dark-fallback pair. Panel-opened
  cards follow the real opener; launcher stays centred. Session confirmation
  text can no longer resize its full-output input surface and jump the power
  menu over the bar. Automated construction, geometry, blur-region,
  interaction and contrast checks pass. An agent-run nested scale-1 comparison
  switched the panel and an open control centre from light ink on the dark
  requested wallpaper to dark ink on the previous bright wallpaper, then back
  again without restart; the requested wallpaper was restored. The same unit
  presents capture as `Caja de herramientas`, adds a folder-backed wallpaper
  gallery beside it, removes the tray's visible count, and persists bounded
  pin/hide choices. The corrective C prototype replaces the rejected expanding
  hidden list with one fixed four-column icon grid and adjacent visible/hidden
  selectors. It reserves three rows in either mode, scrolls overflow,
  never paints producer names, and restores focus only after the exact durable
  key and requested mode are confirmed; unrelated tray activity cannot consume
  that pending transition. Pinned items render beside the compact opener. The
  opener and inventory heading use the semantic system-tray glyph. Exact icon
  names resolve through the Qt theme and its installed GTK fallback chain, so
  Solaar's published `battery-good` resolves without an application heuristic;
  applications such as Slack that publish only a pixmap retain that artwork.
  Every unresolved foreign icon stays a fixed-size glyph rather than producer
  text. A foreign tray menu uses a child surface so the inventory stays mapped.
  The D prototype replaces the two painted selector labels with accessible
  eye and eye-off glyphs, grows only the application artwork inside each fixed
  grid tile from 19 to 23 pixels, and suppresses every shell hover tooltip at
  its local button boundary without removing hover feedback or assistive names.
  Foreign D-Bus menus are now capped to the logical output space remaining
  below their real request before the carrier adopts its size, so an overflowing
  real Qt Menu stays anchored, exposes a draggable scroll route and keeps every
  action reachable by arrow key. Escape still closes only that child while the
  tray inventory remains mapped.
  Wallpaper selection persists a user-chosen folder, rejects a scan beyond the
  512-entry safety bound instead of publishing a partial catalogue, and exposes
  every accepted image through deterministic pages of at most 64 thumbnails.
  The menu reports total and page navigation rather than a terminal limited-
  gallery label, and a catalogue/id click changes only the invoking output; no
  source path is embedded in the panel. The canonical production exit built,
  verified and deployed its candidate without host-session activation. The
  E prototype identifies Niri 26.04's automatic xray policy as the reason those
  finite blur regions showed the wallpaper even above an application. The
  registered nested profile now sets `xray false` only for the panel, primary
  menu, tray-child menu and interactive overlay namespaces; live-session
  documentation exposes the identical block as a manual opt-in. The first
  live-reload comparison changed an over-application
  sample from wallpaper-like `srgb(42,47,43)` to `srgb(24,106,116)`, but a
  later reconstruction returned to the wallpaper and that sequence was not
  accepted as durable evidence. The final controlled cycle started nested Niri
  PID 1102853 from one stable exact rule, opened a uniform teal application and
  crossed its edge with the launcher. In
  `/tmp/celestina-non-xray-clean-start.png`, pixel `(186,291)` in the glass
  above the application is `srgb(31,106,115)`, close to the uncovered
  `(16,56)` reference `srgb(0,91,102)`, while `(686,291)` in the same glass
  above the wallpaper is `srgb(33,39,33)`. Closing and reopening the
  launcher preserved those values in
  `/tmp/celestina-non-xray-clean-reopen.png`. A first Celestina-only restart
  produced PID 1106789 and adapters 1107007/1107009 and preserved them in
  `/tmp/celestina-non-xray-after-celestina-restart.png`. A second registered
  Celestina-only restart, without a Niri reload, produced PID 1110628 and
  adapters 1110890/1110891. In
  `/tmp/celestina-blur-control-launcher-clean-after-restart.png`, those same
  three coordinates retained the same three values. After the canonical
  production exit, the final nested-only restart loaded the verified bytes as
  current PID 1127567 with adapters 1127828/1127829. The same values remain at
  the same coordinates in
  `/tmp/celestina-blur-control-launcher-production-final.png`. This live colour
  split was sampled on the launcher namespace `celestina-overlay`; the other
  exact namespaces share the validated matcher but were not separately sampled.
  Nested Niri stayed PID 1102853 and host Niri PID 1224 plus Noctalia PID 1276
  remained intact. The author's live Niri configuration was not edited. The
  earlier `/tmp/celestina-contextual-tools-live.png` records the recapped
  panel. The author-owned all-menu, scale-2, multi-output, native folder chooser
  and real tray gesture review is still pending. That includes visual
  acceptance of the eye controls and larger artwork, hover confirmation across
  the shell, and a
  real wheel/drag pass through an overflowing foreign menu. Non-xray blur is
  more expensive and experimental in Niri 26.04; motion/drag behavior and text
  contrast over arbitrary application content remain author-run checks because
  Wayland does not expose those pixels to Celestina's wallpaper-derived ink
  analysis. The F prototype removes the last shell-local menu
  material recipe: both the very light outer veil and every denser content
  section now use CelestinaStyle's `GlassSurface.ExternalBackdrop`.
  `CompositorGlassRegion` remains only the shell-owned KWindowEffects geometry
  and fallback adapter, so each menu still publishes exactly one blur region
  and no QML capture attempts to read another Wayland client. The final
  canonical exit passed and deployed without host-session activation. A
  nested-only restart replaced the old shell with PID 1224284 and adapters
  1224469/1224470 on `wayland-2`; nested Niri PID 1144687, host Niri PID 1224
  and Noctalia PID 1276 remained intact. Opening Control Centre through the
  session command confirmed one 30-fragment compositor shape and emitted no QML
  construction or binding error. Visual acceptance of the material remains in
  the author-owned matrix. The G prototype narrows the
  reference-backed treatment to the two requested information surfaces:
  contextual content cards and panel capsules now share CelestinaStyle's
  dense matte `ContentSurface`, while the contextual carrier uses the much
  lighter `ContextualVeil`. Both roles have zero elevation, use no QML capture
  and leave the menu at one compositor region. The content material polarity
  follows the same measured foreground decision, so the established bright and
  dark wallpaper pairing is preserved. Other suite glass keeps its compatible
  default material. The canonical production exit passed all Rust unit and
  integration suites, QML lint and QuickTests, CTest 17/17 and the release
  smoke, then deployed without host-session activation. A nested-only restart
  replaced PID 1224284 with PID 1336218 and adapters 1336400/1336401 on the
  unchanged `wayland-2` nest; nested Niri 1144687, host Niri 1224 and Noctalia
  1276 remained intact. The live bright-backdrop capture confirms dark ink over
  matching dense light cards/capsules and a nearly transparent outer field.
  Toasts, OSD, output
  sharing and new clock/weather behavior remain outside the Velo redesign. The
  rejected earlier fields remain recorded, and this is not yet an accepted
  suite-wide UX-2 language. `VAL-PANEL-1` is partial rather than passed at both
  scales.

- **Delivered in celestina 0.8.0 — `DIAG-1`.** Every Celestina process now writes a
  structured, bounded, always-on JSONL journal under
  `$XDG_STATE_HOME/celestina/diagnostics/`, correlated by one `run_id` the host
  generates and exports before it spawns either helper. It records classes of
  event, technical identities and timings — never clipboard content,
  notification bodies, media metadata, window titles, launched commands or
  secrets. `scripts/diagnostic-report.sh` collects it read-only after a reset.
  The format, privacy rules and the limits of what it can prove are in
  [docs/diagnostics.md](docs/diagnostics.md). The canonical production exit
  built, verified and deployed 0.8.0 without activating the live session.
  `VAL-DIAG-1` is the next author-owned check.


- Delivered in celestina 0.8.0: `WSG-1`. A workspace now carries the monitor it
  belongs to, not only the one it is on. Niri publishes the second and never the
  first, so the home is remembered from a frame that could see more than one
  output — or declared by the author in the shell's settings — and a frame that
  cannot tell a displaced workspace from a native one teaches nothing. A strip
  carrying more than one monitor's workspaces opens the group holding the focus
  and shows every other as one capsule naming its monitor, its count and its
  urgency; a capsule click is an ordinary focus request and the group opens
  because the focus arrived. A strip of one group renders exactly as it did
  before. A missing, corrupt, oversized or future-schema memory file degrades to
  that same flat strip rather than failing. The canonical production exit built,
  verified and deployed 0.8.0 without activating the session. `VAL-WSG-1` — the
  live capsule, its assistive route and the moment the memory is first taught —
  is not run.


- Delivered in celestina 0.7.0: `UX-1`. Network and Bluetooth retain truthful
  panel summaries while each now opens its own dismissible menu. Saved network
  profiles and known Bluetooth devices use provider-owned stable identities;
  actions remain pending after tool acceptance and settle only from a later
  observation. The durable host ledger survives menu destruction, keeps failed
  targets visible even if their row disappears and distinguishes these
  confirmed actions from the control centre's immediate requests. The canonical
  production exit built, verified and deployed 0.7.0. `VAL-UX-1` passed in the
  live Niri session on 2026-08-08. Follow-up work remains for opener-relative
  a deliberate shell-wide visual-usability pass. The menu surface now follows
  the compositor's real exclusive-zone placement and keeps the invoking
  control's horizontal anchor. Directly changing from one open menu to another
  still required two clicks in the last live observation and is not claimed as
  fixed. A clock/date calendar-and-weather menu with location management is a
  separate product extension beyond UX-1.

- Delivered together in celestina 0.6.8: `LVR-3-G`, in the same atomic batch as
  `LVR-3-F`.
  A network probe that saw nothing can no longer retire a link at any repetition
  count — `UNREADABLE_HOLD` is removed rather than raised — and only a poll that
  positively found no default route can, twice over. A route naming a device the
  device list cannot explain is unreadable, not offline, which is what a Wi-Fi
  card re-associating looks like.

  The tray had a real defect, found by walking the whole D-Bus path against a
  private bus rather than by reasoning about the parts. A registry read rebuilt
  the registration list wholesale from the snapshot its reply carried, so an
  application that registered while that read was in flight was removed by an
  answer composed before it existed — and no second registration signal was
  ever coming for it. The new `celestina-tray-watcher` integration test
  reproduced the live symptom on its first run, publishing two of four with
  Slack and Solaar missing. A registry read is now a reconciliation against the
  registrations known when it was sent, and all four are published.

  The model, the open drawer and the 1920-pixel flank layout were also checked
  and hold. The folded drawer additionally now shows how many items are behind
  its chevron, which it never did. Both live cases stay failed until the author
  reruns them.

- Delivered in celestina 0.6.8: `LVR-3-F`, the
  first unit of this plan with executable evidence. Four readings stopped
  treating one unlucky observation as the truth: Bluetooth publishes the
  adapter's own state so a powered radio with nothing on it stays visible; the
  network holds its last confirmed link across a bounded run of unreadable
  polls without raising the shared 750 ms tool deadline; each overlay receives
  only the properties it declares, so the session menu no longer logs a runtime
  property error; and output hotplug asks the single DDC worker for one
  coalesced rediscovery instead of waiting out the 300-second refresh. Beyond
  those four: every transient surface — the five focused overlays, the panel's
  context menu and a tray item's menu — now covers its own output, so a click
  outside a card is the surface's to answer and the panel button that opened an
  overlay is behind it rather than in front of it; a tray item that registers
  and then fails to describe itself is retried, logged and shown under the name
  it registered with instead of being dropped silently; and media is driven by
  MPRIS owner and property signals over `zbus`, with `playerctl` gone from this
  shell entirely and only a one-second progress tick and a thirty-second
  bounded reconciliation left. The
  canonical production exit ran and deployed the verified bytes to
  `~/.local`; the session was not replaced. Recorded in
  [one poll is not the truth](docs/evidence/2026-08-07-one-poll-is-not-the-truth.md).

- Delivered in celestina 0.6.7: `LVR-3-E`. The helper target gathers its sources
  at configure time rather than naming ten of nineteen by hand. Later canonical
  exits compiled and exercised that gathered target after the hold ended.

- Delivered in celestina 0.6.6: `LVR-3-D`. A snapshot the host would discard is
  now skipped rather than treated as the end of the Niri session, so an
  oversized frame no longer costs a reconnect loop against the compositor. It
  is included in the verified 0.6.8 bundle.

- Delivered in celestina 0.6.5: `LVR-3-C`, repairing two defects `LVR-3-B`
  introduced into the helper-restart path. The escalation timer now names the
  instance it was armed against, so it cannot kill the replacement that started
  inside the grace window; and the restart delay is decided by the handler that
  knows how the helper exited, so the spacing an unclean exit earns is applied
  rather than lost to a race. Later canonical exits and controlled shutdowns
  exercised the complete lifecycle after the hold ended.

- A C++20/Qt 6.9+ host maps one top layer-shell panel per output and owns the
  `org.celestina.Shell1` session interface.
- Rust helpers reduce Niri state and carry the aggregate providers through the
  pure `celestina-shell-core` contracts.
- Media is read from the session bus rather than asked for: the helper follows
  `org.mpris.MediaPlayer2.*` owners appearing and disappearing and whatever a
  player says at its own object path. Nothing is spawned for it, and the only
  clock left advances a playing track's progress between two things the player
  said.
- The panel contains workspace/window, system, media, audio, DDC and tray
  paths. Workspace, audio, microphone, DDC, CPU/RAM and tray paths passed the
  follow-up. Version 0.6.2 gives the first helper generation a bounded fast
  MPRIS discovery window, and 0.6.8 replaces polling with MPRIS signals. The
  final full-shell rerun passed without replacing the helper.
- The launcher and clipboard-history overlays are implemented and use the same
  surface and command contracts.
- Typed volume, mute and brightness session verbs enter through
  `org.celestina.Shell1`, reach their provider and are answered `pending` and
  then `confirmed` or `failed` from a later reading, never from acceptance.
- A corner on-screen display shows volume, microphone and per-monitor
  brightness. It is raised by what a provider published, never by a request, so
  a key that changed nothing raises nothing; it never takes focus or the
  keyboard.
- Night light and the idle inhibitor are held states. Version 0.6.2 handles the
  host's ordinary Unix termination signal, releases both held children before
  exit and gives stdin-driven shutdown enough time to drain. A process
  regression proves an active fake inhibitor is gone before the helper exits;
  the repeated live lifecycle rerun remains author validation.
- `displays-off` is composed through Niri, whose own answer is the outcome.
- `lock` and `lock-and-suspend` are refused: no locker provider exists while
  SHELL-D1 is open, and a shell that cannot lock says so instead of reporting
  success. The provider seam is the refusal site.
- The optional Niri bindings, the tool each verb needs and the rollback are in
  [README.md](README.md). Nothing applies them: the shell never edits a Niri
  configuration, and deleting the block is the whole rollback.
- No task document authorizes changing Niri configuration, installing a locker,
  activating the shell or stopping Noctalia.

- A control centre — `celestina msg control-centre-toggle` — changes volume,
  mute, night light, caffeine, do-not-disturb and the power profile through the
  verbs that already existed, and shows each request as pending, confirmed or
  failed beside the provider's own reading. Network and Bluetooth are read-only
  there: this shell is not a manager for either.
- The session menu — `celestina msg session-menu-toggle` — asks twice before
  ending anything: log out through the compositor, restart and power off
  through logind, and suspend refused while no locker exists.
- The control centre also carries a month calendar, computed rather than
  fetched, and a weather reading that is absent rather than stale. No location
  means no weather and no request: this shell does not look up where somebody
  is.
- Choices survive a restart: they are written durably before anything publishes
  them, and night light, caffeine and do-not-disturb are restored at startup.
- Toasts appear in the top-right corner and never take focus; the notification
  centre — the panel's unread indicator, or
  `celestina msg notifications-toggle` — is the keyboard path to every action a
  toast offers. The on-screen display moved low and centred so a volume key
  cannot paint over a notification.
- The aggregate helper can be the session's `org.freedesktop.Notifications`
  server, but claims the name only when it is free. The follow-up proved
  takeover, replacement, close, action, DND, history, unread state and rollback
  without unrelated providers disappearing. Version 0.6.2 moves Escape to a
  window-level shortcut, and the offscreen focus regression passes; the live
  focused-button rerun remains author validation.

- The shell draws the session's wallpaper itself: one background surface per
  output, sized by the compositor, reserving nothing. An output with no image
  of its own paints a deliberate fallback rather than another screen's picture
  or a black rectangle, and a file that fails to decode falls back the same
  way. Images live in `$XDG_DATA_HOME/celestina/wallpapers`, named for the
  output (`DP-1.png`) or `default.*`.
- Physical removal and reconnection of `DP-2` changed only that output's panel,
  wallpaper and workspaces. The appearance backend and its public portal route
  returned the sealed dark-scheme/accent values through a tested rollback. The
  live Niri preference file needed an explicit
  `Settings=celestina-shell` selection. The README now records descriptor
  installation, selection, broker restart and exact rollback while preserving
  Siderita's FileChooser backend.

## Durable boundaries

`celestina-shell-core` owns pure protocol and policy. Rust helpers own bounded
non-Qt IO. C++ owns Qt, D-Bus and layer-surface adaptation that CXX-Qt cannot
express cleanly. QML owns presentation only. See [AGENTS.md](AGENTS.md) and the
suite [architecture standard](../docs/standards/architecture.md).

## Evidence boundary

The canonical release bundle was built and verified on 2026-08-03: Rust tests,
direct QML lint, CTest 11/11, an eight-second offscreen smoke of the release host
with the compiled style module, and dynamic-library checks passed. Exact
artifacts, commands and limits are recorded in the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). This does not
replace any real Niri check in [VALIDATION.md](VALIDATION.md).

R3 ran its registered exit end to end on 2026-08-04: `complete-production.sh`
built the 0.2.0 bundle once, verified those exact bytes — Rust checks, QML lint,
CTest 13/13 and an eight-second offscreen smoke — and deployed them to the
author's normal test prefix under `~/.local`. The live session was not replaced
and no service, package manager or configuration was touched. The record is
[the R3 completion evidence](docs/evidence/2026-08-04-r3-completion.md).

The deployed 0.6.0 bundle was verified again and activated by the author on
2026-08-04. CTest 13/13 and the release smoke passed, and normal panel,
workspace, audio, DDC and session-hold paths worked. The run stopped when the
first live notification invalidated the complete provider frame; it also found
missing browser media, an undismissable clipboard empty state, English product
copy and startup accessibility/application-id diagnostics. The exact stop
point, causes and unrun checks are in the
[live validation evidence](docs/evidence/2026-08-04-live-validation-failures.md).

The author exercised the corrected 0.6.1 checkout across the full handover on
2026-08-05, including watcher/notification rollback, helper and held-child
failure, settings persistence, portal integration and physical output hotplug.
The exact pass/fail matrix and external session changes are in the
[follow-up evidence](docs/evidence/2026-08-05-live-validation-follow-up.md).
Afterward `complete-production.sh` rebuilt, verified and deployed the same
source without activating Celestina; the live session remained on Noctalia.

Celestina 0.6.2 closes the bounded LVR-2 implementation checkpoint. The
canonical production exit passed all Rust tests, QML lint, CTest 13/13 and the
eight-second release smoke, then deployed the verified bundle without
activating it. The three corrected live cases remain recorded as failed until
the author reruns them against 0.6.2.

## Records

- Completed plan:
  [UX-1 network and Bluetooth indicator menus](docs/plans/archive/2026-08-07-network-bluetooth-indicator-menus.md)
- Celestina 0.6.8 is built, verified and deployed by the canonical production
  exit: 181 shell-core tests, 46 helper unit tests and six tests across three
  integration binaries, Clippy and `cargo fmt` clean, QML lint, CTest 15/15 and the
  eight-second offscreen release smoke. The author then completed the controlled
  live rerun and restored Noctalia, which still owns the session.
- Current checkpoint: `PANEL-1`, limited to the panel shadow, borderless glass
  capsules, dynamic blur regions and the phone/flank geometry they require.
  `UX-2` beyond the panel, conditional lock, Polkit and dock work remain outside
  it
- Last completed plan: [LVR-3 late provider insertion](docs/plans/archive/2026-08-05-late-provider-insertion.md)
- The milestone before it: [R8 Noctalia departure](docs/plans/archive/2026-08-04-r8-noctalia-departure.md)
- Open product questions: [discussion queue](docs/discussions/README.md)
- Accepted product decisions: [decision index](docs/decisions/README.md)
- Completed detailed roadmap: [history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
- Original phase work orders: [Noctalia replacement history](docs/history/noctalia-replacement-through-2026-08-03.md)
