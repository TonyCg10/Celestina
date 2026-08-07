# Celestina status

- **Updated:** 2026-08-07
- **Implementation:** R0-R5, R7, R8's departure slice, LVR-1 and LVR-2 are
  complete; LVR-3 is active and delivered `LVR-3-F` in celestina 0.6.8. Its
  live rerun opened planned corrective unit `LVR-3-G` for network retention and
  registered tray items that still do not reach the visible drawer
- **Author validation:** the 2026-08-05 follow-up passed clipboard remediation,
  tray/notification protocol flows, Spanish copy, startup diagnostics, portal
  integration and output hotplug, but failed first-generation media,
  notification-centre Escape and held-child cleanup. Version 0.6.2 corrects
  notification Escape and held-child cleanup await their focused live rerun.
  The 0.6.2 media rerun failed again and is now owned by LVR-3. See
  [VALIDATION.md](VALIDATION.md) and the
  [follow-up evidence](docs/evidence/2026-08-05-live-validation-follow-up.md)
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

- Uncommitted in the checkout: `LVR-3-E`. The helper target gathers its sources
  at configure time rather than naming ten of nineteen by hand. Source and text
  only, under the hold, and not configured.

- Uncommitted in the checkout: `LVR-3-D`. A snapshot the host would discard is
  now skipped rather than treated as the end of the Niri session, so an
  oversized frame no longer costs a reconnect loop against the compositor.
  Source and text only, under the hold.

- Uncommitted in the checkout: `LVR-3-C`, repairing two defects `LVR-3-B`
  introduced into the helper-restart path. The escalation timer now names the
  instance it was armed against, so it cannot kill the replacement that started
  inside the grace window; and the restart delay is decided by the handler that
  knows how the helper exited, so the spacing an unclean exit earns is applied
  rather than lost to a race. Source and text only: under the GPU hold nothing
  here has been compiled, tested or run.

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
  MPRIS discovery window, so a player registering just after the helper no
  longer needs a helper restart. The focused policy regression passes; the live
  full-shell rerun remains author validation.
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

- Active plan: [LVR-3 late provider insertion](docs/plans/active/2026-08-05-late-provider-insertion.md)
- Celestina 0.6.8 is built, verified and deployed by the canonical production
  exit: 181 shell-core tests, 46 helper unit tests and six tests across three
  integration binaries, Clippy and `cargo fmt` clean, QML lint, CTest 15/15 and the
  eight-second offscreen release smoke. The shell has not been activated and Noctalia still
  owns the session.
- Planned next checkpoint: `AUD-1` static audit hardening in
  [ROADMAP.md](ROADMAP.md), from the
  [2026-08-05 static shell audit](docs/evidence/2026-08-05-static-shell-audit.md);
  it opens no plan and runs nothing while LVR-3 and the GPU hold remain active
- Last completed plan: [LVR-2 live validation follow-up](docs/plans/archive/2026-08-05-live-validation-follow-up.md)
- The milestone before it: [R8 Noctalia departure](docs/plans/archive/2026-08-04-r8-noctalia-departure.md)
- Open product questions: [discussion queue](docs/discussions/README.md)
- Accepted product decisions: [decision index](docs/decisions/README.md)
- Completed detailed roadmap: [history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
- Original phase work orders: [Noctalia replacement history](docs/history/noctalia-replacement-through-2026-08-03.md)
