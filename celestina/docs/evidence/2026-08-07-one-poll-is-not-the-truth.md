# Evidence: 2026-08-07 the controlled-transition corrections

- **Date:** 2026-08-07
- **Scope:** `LVR-3-F`; plan
  [late-provider-insertion](../plans/active/2026-08-05-late-provider-insertion.md);
  the defects recorded by the 2026-08-07 controlled transitions in
  [VALIDATION.md](../../VALIDATION.md)
- **Environment:** the GPU safety hold ended on 2026-08-07 with `VAL-GPU-01`
  passed, so this is the first unit of this plan with executable evidence.
  Noctalia owned the session throughout; Celestina was built, tested and
  deployed, never activated
- **Artifact:** celestina 0.6.8, built once by `complete-production.sh` and
  verified as those same bytes

## What was wrong

Seven defects from one validation run. Four of them are one fault wearing four
faces: a reading treating a single unlucky observation as a fact about the
world. The other three are their own.

**Bluetooth.** `publish_bluetooth` ran `bluetoothctl devices Connected` and
withdrew the provider whenever the list came back empty. A powered adapter with
nothing paired to it was therefore indistinguishable from a machine with no
Bluetooth, from an adapter switched off, and from a query that never answered.
The author's requirement is the opposite: a powered radio is a state a person
needs to be able to see.

**The network.** `publish_network` withdrew the provider on any poll that did
not name a link. Read-only sampling during the transition measured `nmcli`
answering in 4–5 ms normally and in 2.37–3.00 s occasionally, against the shared
750 ms tool deadline in `tools.rs`. One slow reply erased the panel's Wi-Fi text
while the connection stayed in use.

**The overlays.** `OverlayController::createWindow` injected `providerSource`
into every component. `SessionMenu.qml` declares `shellSource`, so every open
logged `SessionMenu does not have a property called providerSource`. Qt does not
fail a component over an initial property it does not declare — it logs and
carries on — which is why nothing offscreen had ever noticed.

**DDC hotplug.** `brightness.rs` chose its detection interval from whether the
startup detection was empty: `REFRESH` (300 s) when it found monitors,
`REDETECT` (30 s) when it found none. Nothing woke the worker when an output
appeared. Celestina started with `DP-1` disabled; enabling it mapped that
output's panel at 00:25:06 EDT and its brightness control did not appear until
the refresh expired.

**Overlay and menu dismissal.** No transient surface had any outside-click path
at all: neither `OverlaySurface` nor any overlay QML reacted to losing focus,
and the only ways out were `Escape` and whatever the compositor did with
`setCloseOnDismissed`. Each surface was the size of its own card, so a click
outside one belonged to whatever was behind it — and when that was the panel
button which had opened it, the button asked `toggle()` about a surface that
was still mapped. That is the two-click close the author found.

**Tray items registered but never rendered.** `RegisteredStatusNotifierItems`
listed Slack and Solaar; neither appeared. Two things in the host make that
outcome possible and silent: `callAsync` dropped an error reply without a word,
and `publish()` skipped any registration with no entry in `m_read`. An item
whose `GetAll` failed was therefore registered forever and rendered never, with
nothing in the log to distinguish it from an item nobody registered. Separately,
`attach()` is re-entered on every watcher owner change — including this shell
acquiring the name itself — so two registry reads could be in flight, the older
one clearing what the newer had already read.

**Media discovery latency.** `media.rs` spawned `playerctl` every 500 ms for the
first ten seconds, then every five seconds with no player and every two with
one. A track took seconds to appear and the session paid a subprocess a second
all day for a reading that changes when somebody presses a key.

## What changed

- `celestina-shell-core/src/bluetooth.rs` — an `Adapter` of `Absent`, `Off` and
  `On`, and a `reading()` that returns `None` when a command did not answer.
  Four states, and the unreadable one publishes nothing rather than guessing.
- `celestina-shell-core/src/network.rs` — `Observation` separates a link, a
  confirmed offline state and an unreadable probe; `LinkTracker` holds the last
  confirmed link across `UNREADABLE_HOLD` (3) unreadable polls or
  `OFFLINE_HOLD` (1) confirmed-offline poll, then stops publishing it. The hold
  is bounded on purpose: a held reading is the newest thing anybody confirmed,
  never a permanent one. `TOOL_TIMEOUT` is unchanged.
- `celestina-shell-core/src/brightness.rs` — `detection_is_due` owns the
  schedule, and `REFRESH`/`REDETECT` move here with it. A request short-circuits
  the interval; neither interval was shortened.
- `src/provider_adapter/session.rs` — asks `bluetoothctl show` first and the
  device list only for a powered adapter; runs the network observation through
  the tracker. One poll thread, three readings in order, so no two of these
  commands are ever in flight together.
- `src/provider_adapter/brightness.rs` — a `REDETECT_REQUESTED` flag the worker
  `swap`s at the top of its loop. A flag rather than a queue is what coalesces a
  burst into one detection; a request arriving while `ddcutil` is mid-
  conversation is simply still set next time round. Nothing outside the worker
  thread runs `ddcutil`.
- `src/provider_adapter/main.rs` — `brightness`/`outputs-changed`, guarded
  ahead of the generic brightness arm. It is deliberately not a session verb: no
  key binding produces it and it changes no device, so it never enters the
  vocabulary `session::parse_for` owns.
- `src/panelmanager.{h,cpp}` — the manager already observes `screenAdded` and
  `screenRemoved`; both now restart a 1500 ms single-shot timer that sends one
  request. Enabling one monitor produces several `QScreen` events and they cost
  one rediscovery between them.
- `src/overlaycontroller.{h,cpp}`, `src/main.cpp` — `overlaySourceProperty`
  names the bridge each component declares, in one place. The controller adds
  `reducedMotion` and nothing else; `setExtraProperties` is gone rather than
  left as a second way in.
- `qml/SessionStatus.qml`, `qml/ControlCentre.qml` — the adapter's own state is
  what each surface reads. `bt`, `bt 2`, `bt apagado`; `sin adaptador`,
  `apagado`, `nada conectado`, or the device's name.
- `src/overlaysurface.cpp`, `src/panelmenusurface.cpp` — both surface kinds are
  anchored to all four edges with no size of their own and an exclusive zone of
  `-1`, so the compositor sizes them to the output and they cover the panel that
  opened them. The card is centred (overlays) or placed at the click
  (menus) by the content itself.
- The five overlay QML files — a dismissal `MouseArea` filling the surface,
  declared before the card so the card is above it, and a catch-all inside the
  card declared first so every control after it is still reached. The card is a
  fixed `cardWidth × cardHeight` centred in the surface.
- `qml/PanelMenu.qml`, `qml/TrayMenu.qml` — the position the surface used to
  carry in its layer-shell margins moved into `menuX`/`menuY`, clamped against
  the surface so a menu near an edge stays whole, and the popup's `closePolicy`
  now names `CloseOnPressOutside` explicitly.
- `src/panelmenucontroller.cpp` — one `placeCard` does the arithmetic both call
  sites used to repeat.
- `src/traywatcher.{h,cpp}` — `callAsync` gained an error path; a failed
  `GetAll` is retried once, then logged and published from the registration
  itself; registry reads carry a generation so a superseded reply cannot clear
  the current one; and an icon change now reaches QML, which it did not, because
  `TrayItem` does not compare icons and `items()` merges them at read time.
- `src/trayitems.{h,cpp}` — `unreadTrayItem` names an item that never described
  itself from its object path, or its bus name when the path is a bare number.
- `celestina-shell-core/src/media.rs` — the player choice, as pure policy:
  playing beats paused, then most recently heard, then the earlier bus name so a
  tie never flickers. Plus `advanced_position`, which is why the progress bar
  costs no D-Bus call.
- `src/provider_adapter/media.rs` — rewritten on `zbus`. Two match rules
  (`NameOwnerChanged` under `org.mpris.MediaPlayer2`, and every signal at
  `/org/mpris/MediaPlayer2`, which covers `PropertiesChanged` and `Seeked`), one
  event thread, and one progress thread that advances a playing track by
  arithmetic and re-reads the current player every thirty seconds as a bounded
  backstop. Transport verbs are D-Bus calls on the chosen player. `playerctl` is
  gone from this shell entirely; no second path was left behind. The backstop
  also publishes when it removes an unreadable current player, so a missed
  owner-loss signal cannot leave stale media on the panel indefinitely.
- `celestina-shell-core/src/media.rs` also owns `now_playing_line`. Magnetita
  composes the same line for the phone's KDE Connect payload and keeps its own
  copy: that one belongs to another product's prefix, and a `celestina:`
  delivery does not reach into it. Making the two agree is a suite unit of its
  own, recorded rather than done in passing.

## Procedure

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
(cd celestina-rs && cargo test -p celestina-shell-core)
(cd celestina && cargo fmt --all --check)
(cd celestina && cargo test --all-targets --locked)
(cd celestina && cargo clippy --all-targets --locked -- -D warnings)
(cd celestina && ./scripts/qmllint-production.sh)
(cd celestina && ctest --test-dir build --output-on-failure)
python3 scripts/version_tool.py check
celestina/scripts/complete-production.sh
```

## Result

- Architecture contract: OK. Language contract: OK (157 legacy files ratcheted).
- `celestina-shell-core`: 178 tests pass, including six new Bluetooth-state
  cases, six new `LinkTracker` cases, two new detection-schedule cases and ten
  new player-choice cases.
- `celestina` helpers: 34 unit tests and six tests across three integration
  binaries — held-shutdown, notification-server and the new `media_signals` —
  pass. They include two DDC request-coalescing cases that run no `ddcutil` and
  the reconciliation regression that withdraws an unreadable current player.
- `magnetita-core` (unchanged by this unit) still passes its 98 tests: the
  shell's media rewrite took nothing from it.
- Clippy with `-D warnings`: clean. `cargo fmt --check`: clean.
- QML lint: OK against the generated module.
- CTest: 14/14, with `celestina-overlay-contract` new. It creates all five
  overlay components with exactly what the controller would hand them and fails
  on any `does not have a property called` message; its companion case performs
  the old injection against `SessionMenu` and requires that message, so the
  regression can fail.
- `tst_sessionstatus.qml` covers the four adapter states plus an unread one.
- `celestina-overlay-contract` also proves each of the five overlays emits
  `dismissed()` for a click in the far corner of a surface sized like an output,
  and emits nothing for a click just inside the card's edge.
- `celestina-surface-manager` pins both surface kinds to four anchors, a zero
  desired size and an exclusive zone of `-1`.
- `celestina-tray-items` covers an item that never described itself, and the
  four registration strings this session really publishes — captured read-only
  from the bus on 2026-08-07 while Noctalia owned the watcher.
- `media_signals` is a new integration binary: a private `dbus-daemon`, a real
  exported MPRIS player, and the helper. It proves a player that was already on
  the bus when the helper started is found, that a pause and a track change
  reach the panel because the player announced them, that a name going away
  takes the reading with it, and that a session with no player publishes no
  media at all.

## Limits

Nothing here proves compositor behaviour. The DDC change was exercised against
no monitor: the automated cases prove the schedule and the coalescing, not that
`ddcutil detect` finds a newly enabled `DP-1`, and only the author's live rerun
can show that. The same applies to the Bluetooth and network readings, which
were tested against captured tool output rather than a radio or an access point.

The network hold is a policy choice with a real cost: for up to three polls
(fifteen seconds) after a genuine disconnection that also fails to read, the
panel shows a link that is gone. That is the trade the author asked for, bounded
so it expires rather than persisting.

Two corrections are by design rather than from a reproduction, and that is the
main limit of this record.

**Overlay dismissal.** No offscreen test can say what Niri does with a focused
`LayerOverlay` surface when the panel behind it is clicked, and this does not
claim to. What it claims is narrower and provable: the surface now covers the
output, so the click is inside it, and the window answers such a click with
`dismissed()`. Whether the compositor delivers it — and whether focus really
returns exactly once — is `VAL-R1-OVERLAY`.

**The tray.** The exact reason Slack and Solaar were lost is still unknown. All
four of this session's registered items answer `GetAll` correctly when asked
read-only from the bus, and nothing in the host's parsing, icon resolution or
drawer filtering distinguishes the two that failed from the two that worked. The
correction closes the chain that made *any* such loss silent and permanent, and
adds the diagnostic that was missing; it does not claim to have found the cause.
If the items appear on the next run, the log will say whether they arrived
described or unnamed, which is the fact this record could not obtain.

The media migration is exercised against a fake player on a private bus. Firefox
and Spotify publish more, and more strangely, than any fixture; only the live
rerun covers that.
