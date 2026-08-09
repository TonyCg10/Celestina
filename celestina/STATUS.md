# Celestina status

- **Updated:** 2026-08-08
- **Implementation:** R0-R5, R7, R8's departure slice, LVR-1 through LVR-3, the
  static hardening previously drafted as `AUD-1`, `UX-1` and `WSG-1` are
  complete
- **Design direction:** `PANEL-1` is active for the author-selected borderless
  glass bar: a soft full-width shadow and real compositor-blur capsules behind
  content. The rest of `UX-2` remains planned under the still-open `SHELL-D5`
  discussion
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
  without activating the session. `PANEL-1-B` remains active for further visual
  iteration, and `VAL-PANEL-1` is partial rather than passed at both scales.

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
