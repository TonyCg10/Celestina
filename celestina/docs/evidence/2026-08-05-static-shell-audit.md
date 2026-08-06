# Evidence: 2026-08-05 static shell audit

- **Date:** 2026-08-05
- **Scope:** read-only defect audit of the celestina host (`src/`), the Rust
  helpers (`src/niri_adapter.rs`, `src/provider_adapter/`), the QML surfaces
  (`qml/`) and `celestina-rs/crates/celestina-shell-core`, at commit `47e6924`
  after the LVR-3-A corrections landed in `9002970`
- **Environment:** static source review only, performed while the Noctalia-only
  GPU observation holds; no compiler, formatter, unit test, build, smoke,
  deployment or Celestina process ran. Repository tooling only
  (`rg`, file reads, `scripts/agent-context.py`)
- **Artifact:** none — no binary was produced or may be produced during the
  hold. The outputs are this record and the planned `AUD-1` checkpoint in
  [`ROADMAP.md`](../../ROADMAP.md)

## Procedure

Four concurrent review passes covered disjoint areas, each instructed to report
only findings traced to exact source and to confirm or refute the corrections
already recorded for the known live failures:

1. DDC and process lifecycle: `src/provider_adapter/{main,tools,brightness,held,sessionholds,session,sysmon}.rs`,
   `src/shellprovidersclient.{h,cpp}`, `src/main.cpp`, `src/shellservice.{h,cpp}`
   single-instance ordering.
2. Provider state pipeline: `src/provider_adapter/media.rs`, frame publication
   in `main.rs`, `src/providerstates.{h,cpp}`, `src/protocoldecoder.{h,cpp}`,
   `src/shellprovidersclient.{h,cpp}`, and every QML consumer of the provider
   map.
3. Session and D-Bus surface: `src/shellservice.{h,cpp}`,
   `src/session{requests,actions}.{h,cpp}`, `src/workspacefocusrequests.{h,cpp}`,
   the tray host and watcher, `src/provider_adapter/notifications.rs`, and the
   notification QML surfaces.
4. Niri channel, surfaces and remaining helpers: `src/niri_adapter.rs`,
   `src/niriclient.{h,cpp}`, surface/panel/overlay/menu/blur managers,
   wallpaper, clipboard, launcher, settings, portal, weather, sysmon, audio,
   the pure core crate, and a QML/CMake registration sweep.

Every critical and high finding below, and the process-lifecycle residuals,
were then independently re-verified against the exact source before being
recorded. Findings are anchored to symbols because line numbers drift.

## Result

### Corrections confirmed present

- All six LVR-3-A corrections: the shell name is claimed synchronously before
  any provider, Niri or tray construction (`main.cpp`, `ShellService::attach`);
  the brightness worker is an RAII owner that is cancelled and joined on every
  path; no `process::exit` remains in the aggregate helper; the bounded runner
  kills and reaps its direct child on cancellation, timeout and `try_wait`
  error; the host grants three seconds of orderly shutdown; and
  `CELESTINA_DISABLE_DDC` inertly removes every `ddcutil` path.
- The LVR-3-A late-insertion correction is mechanically sound:
  `ProviderStates::apply` treats a same-generation map change as a revision
  change (unit-tested), and `Panel.provider()` reads `revision` inside the
  binding so every panel widget re-evaluates. The 0.6.2 fast MPRIS window in
  `media.rs` is bounded and terminating.
- The LVR-2-A corrections hold: notification-centre Escape is a window-level
  shortcut, and the 0.6.0 frame-wipe is gone — an invalid frame is ignored
  rather than clearing state.
- All three bus-name claims (shell, tray watcher, notifications) are
  only-when-free with the object exported first; no synchronous D-Bus call to a
  foreign application runs on the panel's Qt thread; accepted-versus-confirmed
  semantics are honored with generation guards; hotplug, blur, wallpaper
  fallback, durable settings writes, sensitive-clipboard exclusion,
  absent-not-stale weather, core-crate purity and QML/CMake registration are
  clean.

### Critical

- **In-process session command refusals crash the panel.**
  `SessionActions::send` calls `ShellService::Command` as a plain C++ call, so
  `QDBusContext` has no call context. Every refusal arm of `Command` —
  `suspend` unconditionally, `lock`/`lock-and-suspend`, unknown verbs,
  `toggleOverlay` without a surface, `displays-off`/`log-out` with the adapter
  down, `reboot`/`power-off` without a system bus — calls `sendErrorReply`,
  which dereferences the null context (`Q_ASSERT`-guarded only in debug
  builds). The session menu ships `suspend`; arming and confirming it is a
  guaranteed crash of the whole shell. The fail-closed refusal contract holds
  on the bus path and kills the panel on the UI path.
- **One long notification freezes every provider reading.** The core bounds
  bodies to `MAX_BODY_CHARS` (800) but bodies travel inside array rows
  (`entry_json`), and `Snapshot::publish` bounds only top-level strings —
  array rows pass unchecked. The host's `readRow` rejects any row string over
  512 QChars and that rejects the whole frame; every later frame repeats the
  same entry while it stays in the published history, so all providers freeze
  stale until roughly twenty newer notifications displace it or the helper
  restarts. A plain `notify-send` with a 600-character body triggers it. This
  is the recorded 0.6.0 failure class recurring one bound further in: the
  actions-array shape was fixed, the row length was not.

### High

- **Unit mismatch in every text bound.** The Rust side counts Unicode scalars
  (`chars().count()`), the host counts UTF-16 code units (`QString::size()`).
  Text at or under 512 scalars but over 512 UTF-16 units — astral-plane
  characters such as emoji — passes the helper and invalidates the whole frame
  at the host, with the same freeze as above. Reachable through any provider
  text near the limit; media titles are published unbounded (below).
- **Unclean helper death can still overlap DDC.**
  `ShellProvidersClient::helperError` calls `kill()` (SIGKILL) on a helper
  that still reports Running, bypassing the entire cancellation chain, so an
  in-flight `ddcutil` child (up to ten seconds cold) is orphaned on the i2c
  bus; `scheduleRestart` then starts a fresh helper after 250 ms whose first
  act is `ddcutil detect`. Any unclean helper death (OOM kill, crash) followed
  by the 250 ms restart produces the same overlap — the concurrent-detection
  pattern that precedes both retained GPU-loss boots.
- **A hostile selection source hangs the clipboard thread forever.**
  `receive_text` bounds the pipe read in size but not in time; a client that
  offers a selection and never writes EOF blocks the single clipboard thread
  inside a `Dispatch` handler. The event queue is never pumped again, verbs
  fail "busy", and other applications pasting the shell's own re-selected
  entry stall. No watchdog or reconnect exists; the loss is silent and
  permanent for the session.
- **Producer markup renders live in notifications.** The toast and
  notification-centre `Text` elements do not set `textFormat`, so Qt's
  `AutoText` heuristically renders producer text as styled markup — including
  `<img src>` fetches performed by the shell process and `<a href>` — while
  the server deliberately does not advertise `body-markup`.

### Medium

- **Session-hold residuals.** The `sessionholds` thread never observes the
  shutdown flag and is never joined; its `restore()` can spawn a remembered
  `systemd-inhibit`/`wlsunset` child after `release_all()` already ran during
  a fast shutdown, orphaning it permanently. `release_all()` runs only on the
  success path of `run()`; any `?` failure after `sessionholds::spawn` leaks
  an active held child. `Held::is_held` drops its child unkilled and unreaped
  on a `try_wait` error — the same shape `tools.rs` fixed — enabling a double
  holder.
- **Late-insertion pattern incomplete.** The revision-coupled read exists only
  in `Panel.qml`. `ControlCentre.qml`, `NotificationCenter.qml`,
  `LauncherOverlay.qml` and `ClipboardOverlay.qml` still bind through direct
  map lookups; they are rebuilt on open, so exposure is a key inserted while
  the overlay is open — `weather`, which legitimately arrives minutes late, is
  the realistic case.
- **Media and launcher rows are unbounded at the source.** `media.rs` inserts
  `playerctl` text verbatim (a long title makes `publish` refuse and the panel
  shows the previous track for the whole song); `.desktop` `Name`/`Comment`
  fields travel unchecked into launcher rows, where one oversized installed
  entry invalidates every frame that lists it.
- **Niri channel lacks host-side expiry and adapter-side bounds.** Only
  workspace-focus requests expire; screenshot and action pendings live until a
  result or adapter death, and the action worker's socket has no deadline — a
  wedged compositor socket parks it silently until the queue fills. The
  adapter copies window titles, labels and output names unbounded; a
  multi-megabyte client-set window title makes every snapshot exceed the 1 MiB
  line cap, and the strip stays unavailable while the title exists.
- **Loaded clipboard history bypasses recording bounds.** The state file is
  read unbounded and restored entries never re-pass `is_recordable`; a corrupt
  or tampered file loads up to 200 arbitrarily large entries and re-persists
  them.
- **Tray growth and lifetime defects.** The watcher registry has no count or
  length bound and each item adds four signal matches that are never
  disconnected — enough churn exhausts the dbus-daemon match quota for the
  panel's whole connection. An unregister racing an in-flight `GetAll`
  resurrects the entry in the internal maps; property refresh keys by the
  sender's unique name, so well-known-name registrations never update; the
  vanished-owner cleanup tests `contains` after `take`, so a dead owner's
  watched name is never released.

### Low

- An answered-then-dismissed tray menu can reopen from a late duplicate reply
  (`PanelMenuController` never clears its pending target).
- A host→helper command line over 4 KiB is silently dropped by the helper
  while `sendCommand` returns a live request id; the host should refuse to
  send it.
- The provider client destructor can block the GUI thread up to ~6.3 s at
  shutdown.
- A `set_selection` echo that never arrives leaves the self-echo flag armed
  and silently swallows the next real copy.
- The notifications server does not watch for `NameLost` after a successful
  claim; after a bus dispute it would keep publishing a serving state it no
  longer holds.
- `ddcutil` display numbers captured at detect time are reused for up to
  300 s; an unplug/replug between detects can renumber displays and direct a
  brightness write at the wrong monitor.
- Without a session bus the single-instance guarantee lapses (documented
  tradeoff); `GetLayout` demarshal allocates the full foreign tree before
  bounding; icon-theme reads decode untrusted files on the GUI thread;
  history-entry action rows truncate silently at the published cap; the
  notification id wraps at `u32::MAX`.

## Limits

- Static review only: nothing was compiled, executed or measured, so no
  finding is validated by a failing run, and the recorded fixes are confirmed
  as present in source, not as observed behavior. Live validation cases in
  [`VALIDATION.md`](../../VALIDATION.md) remain owed and unchanged.
- The in-process crash is a null-context dereference whose observable form
  depends on build flags: release dereferences null, debug aborts on the
  assert. Either way the panel dies; the exact signal was not observed.
- The DDC overlap finding shows the restart path can recreate the concurrent
  `ddcutil` pattern; it does not claim that path caused the PCIe loss, and it
  does not alter the causation boundary recorded in the
  [GPU loss system audit](2026-08-05-gpu-loss-system-audit.md).
- Panel/overlay behavior under the late-insertion pattern was reasoned from
  binding structure; the underlying Qt mechanism of the original defect was
  diagnosed empirically and remains unpinned, so the four remaining overlays
  are recorded as exposed, not as reproduced failures.
- Corrective work is deliberately not implemented here: an audit does not
  authorize a fix. The bounded plan is the `AUD-1` checkpoint in
  [`ROADMAP.md`](../../ROADMAP.md), which starts only after LVR-3 closes and
  the author activates it.
