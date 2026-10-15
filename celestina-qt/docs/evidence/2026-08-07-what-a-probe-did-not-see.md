# Evidence: 2026-08-07 what a probe did not see, and where the tray items were

- **Date:** 2026-08-07
- **Scope:** `LVR-3-G`; plan
  [late-provider-insertion](../plans/archive/2026-08-05-late-provider-insertion.md);
  the two cases the author's rerun of `LVR-3-F` still failed — `VAL-R1-NET` and
  `VAL-R1-TRAY` in [VALIDATION.md](../../VALIDATION.md)
- **Environment:** Noctalia owned the session throughout. Celestina was built,
  tested and deployed, never activated. No `ddcutil` was run, no monitor was
  touched, no live transition was attempted
- **Artifact:** celestina 0.6.8, built once by `complete-production.sh` and
  verified as those same bytes. `LVR-3-F` is uncommitted, so this unit lands in
  the same batch and takes no second version transition

## `VAL-R1-NET` — what was wrong

`LVR-3-F` gave the network reading a `LinkTracker` with two bounds:
`UNREADABLE_HOLD = 3` and `OFFLINE_HOLD = 1`. The first is the defect. A run of
four probes that saw nothing retired the last confirmed link — so a long enough
stretch of `nmcli` latency turned "I could not look" into "you are
disconnected", by repetition alone. On this machine `nmcli` normally answers in
4–5 ms and has bursts of several seconds against a shared 750 ms deadline, and
the session poll runs every five seconds: twenty seconds of that burst was
enough to blank the panel while `wlan0` stayed connected to `Tonys 1` and the
default route stayed on it.

There was a second, quieter path to the same place. `observe_network` treated
"the route names a device, and the device list does not describe it as
connected" as `Offline` — a *confirmed* disconnection, which `OFFLINE_HOLD = 1`
retires after two polls. That is exactly what a Wi-Fi card re-associating for
two seconds looks like: still routed, still carrying, and reported by `nmcli` as
`connecting` rather than `connected`. The author's own capture shows why this
matters here: Ethernet was also connected, and only the default route said which
of the two carried the session.

## `VAL-R1-NET` — what changed

A third path was found in review and is corrected with the other two:
`observe_network` required **both** commands to answer before classifying
anything. So a routing table that answered "there is no default route" — the one
piece of positive evidence that nothing carries the session — was thrown away
whenever `nmcli` was the slow one, and a real disconnection could be held
indefinitely. The order is now the fix.

- `celestina-shell-core/src/network.rs` — `read_route` classifies the routing
  table's answer alone into `Unreadable`, `NoDefault` or `Through(device)`;
  `needs_device_list` says whether anything is left to ask about; and
  `observe_with` produces the observation from both answers, delegating the
  routed case to the existing `observe`. Two of the three outcomes are settled
  by the route alone and need no help from `nmcli`.
- `src/provider_adapter/session.rs` — the routing table runs first and is
  classified first. `nmcli` runs only when the route named a device, so a
  session with no default route is not waiting on a command whose answer is not
  consulted. The shared `TOOL_TIMEOUT` is unchanged, the five-second interval is
  unchanged, no dependency was added, and both commands still run on the session
  provider's own thread.
- `celestina-shell-core/src/network.rs` — `UNREADABLE_HOLD` is gone, not raised.
  `Observation::Unreadable` holds the last confirmed link for as long as it
  lasts and can never retire it.

### The offline streak, stated once

Review found the comments and the test disagreeing about what "two consecutive"
meant: the test allowed `Offline → many Unreadable → Offline` to retire a link,
and those are not consecutive. The semantics are now one thing, and the code,
the comments and the tests all say it:

- `Carrying` confirms the link and resets the offline streak;
- `Unreadable` keeps the link and **resets** the offline streak, because an
  offline confirmation must not stay armed across an arbitrary gap and fire
  against a session that reconnected in between;
- two **consecutive** `Offline` observations retire the link, which at the
  five-second poll is about ten seconds;
- nothing retires a link by repetition of `Unreadable`.

## `VAL-R1-TRAY` — what was demonstrated

The first pass at this unit checked four hypotheses with unit and QML tests,
found the tray code correct in all four, and concluded that the author had been
looking at a folded drawer. Review rejected that conclusion, because none of
those tests touched the path the live failure actually ran through:
registration, asynchronous `GetAll`, QtDBus demarshalling of `a{sv}` and
`a(iiay)`, the generation guard on registry re-reads, `m_read`, `publish()` and
`items()`. The conclusion was an inference from the absence of a defect in the
parts that were tested.

The review was right. A new integration test walks that path against a private
`dbus-daemon` this process starts itself, with the real watcher name claimed on
that bus and four fake StatusNotifierItems exported from four separate bus
connections in the shapes this session really publishes. On the first run it
reproduced the live symptom exactly:

```text
FAIL!  : TrayWatcherTest::everyRegisteredItemReachesTheHostsPublishedList()
         published 2: nm-applet, blueman
```

Four registered, two published, and the two lost were Slack and Solaar.

### The defect

`refreshRegistrations()` rebuilt the registration list wholesale from the
registry snapshot its reply carried. `attach()` is re-entered whenever the
watcher name changes owner — including this shell taking that name itself — so
a second read is in flight during startup. An application that registered while
one of those reads was on the wire had its `StatusNotifierItemRegistered`
handled first and was then removed by the reply, which had been composed before
it existed.

Nothing recovers from that. The item stays registered with the watcher, and no
second registration signal is ever coming for it, so it is absent from the panel
for the rest of the session while `RegisteredStatusNotifierItems` keeps listing
it. That is precisely the shape of the live report, and it is why no `GetAll`
error appeared in the journal: the items were never asked.

The generation guard added in `LVR-3-F` does not help here, and could not: it
drops *superseded* replies, and this is the newest reply carrying a stale
snapshot.

### What changed

- `src/traywatcher.cpp` — a registry read is now a reconciliation rather than a
  reset. It records the registrations known when the read is **sent**, and on
  reply removes only what was in that baseline and is absent from the snapshot,
  then adds what the snapshot brought and it did not already have. A
  registration learned while the watcher was answering is newer than the answer,
  and the answer is not entitled to remove it.
- `src/traywatcher.{h,cpp}` — `removeRegistration` is the one place that forgets
  a registration and everything keyed by it; `itemUnregistered` now calls it
  instead of repeating its body. The wholesale `forgetItems()` on every refresh
  is gone, so the four match rules per item are no longer dropped and re-added
  on each read.

With that, the same test publishes all four in under a tenth of a second.

### What the tray test really proves

- Four applications register from four separate bus connections, by object path
  alone, exactly as Chromium and Ayatana do.
- All four are read asynchronously through `Properties.GetAll` and demarshalled
  by QtDBus, including Chromium's `a(iiay)` pixmap — Slack's 22 × 22 pixels
  resolve to an icon source the drawer can draw — and Ayatana's absent
  `IconPixmap` key.
- All four reach `m_read`, `publish()` and `TrayWatcher::items()`, with Slack
  named by its `Id` because it published no title, and Solaar keeping its icon
  name, title and menu.
- A second host reading the same registry from scratch also finds four, which is
  the re-entrant `attach()` path that produced the loss.
- An item that registers and never exports its object is retried once and then
  published under the name its registration gives, rather than dropped silently.

### What it does not prove

It does not prove that this defect is the *only* reason the author saw two items
rather than four, and it does not retire the folded-drawer explanation — it
demotes it. A folded drawer showing only attention items is still the state a
person sees by default, and it still said nothing about what was behind it. Both
remain possible descriptions of what happened on 2026-08-07, and only the
author's rerun can say which, or whether both did.

The visible count added to the folded drawer is kept. No test showed it wrong,
and it is a real improvement whatever the cause of the live report was: four
registered applications behind a bare chevron used to look exactly like none,
because the count lived only in `helpText`, whose visible tooltip this control
deliberately switches off.

### What was also checked, and holds

- The model keeps all four of this session's real `GetAll` shapes
  (`TrayItemsTest::thisSessionsFourItemsAllSurviveIntoTheModel`).
- The open drawer instantiates four delegates with real geometry.
- The right flank does not clip them on a 1920-pixel output with the real
  `SessionStatus`, `AudioLevel`, `BrightnessLevel`, `NotificationIndicator` and
  `CaptureButton` beside them. This was worth checking: `PanelFlank` clips, the
  tray is the innermost widget of the trailing flank, and that is how the media
  widget once vanished from the left flank (`tst_panelflank.qml`).
- `qml/TrayDrawer.qml` gained two `objectName`s, on the delegate and the toggle,
  so those regressions can count what the drawer really built. Nothing reads
  them at runtime and they add no API.

No status was rewritten, no application is special-cased, and nothing is
permanently unfolded.

## Procedure

```sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
git diff --check

cd celestina-rs
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test -p celestina-shell-core --locked

cd ../celestina
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
./scripts/qmllint-production.sh
ctest --test-dir build --output-on-failure

./scripts/complete-production.sh
./scripts/status-production.sh
```

## Result

Every command above passed.

| Suite | Count |
|---|---|
| `celestina-shell-core` | 181 |
| `celestina` helper unit tests (two binaries) | 46 |
| `celestina` integration tests (`held_shutdown`, `media_signals`, `notification_server`) | 6 |
| CTest targets | 15/15, one of them new: `celestina-tray-watcher` |
| QML test functions | 93, of which `TrayDrawer` contributes 6 |

Two intermediate results are kept because each nearly became a false conclusion.

The first run of the folded-drawer case failed on `toggle.visible`. The toggle
was not the problem — a `TestCase` reports every descendant invisible until the
case itself is `visible: true` and `when: windowShown`. Both are now declared,
and the assertion measures the control rather than the harness.

The first run of the tray integration case failed with `published 2: nm-applet,
blueman`. That one *was* the problem, and it is the defect this unit exists to
correct. Without that test the previous pass would have shipped with the
conclusion that the tray code was correct.

## Limits

Neither case is closed by this. `VAL-R1-NET` needs a long live session to show
that the Wi-Fi text stops blinking, and a deliberate disconnection to show that
it still leaves within about ten seconds. `VAL-R1-TRAY` needs the author to look
at the folded bar, see a `4` beside the chevron, open it and find Slack and
Solaar there.

What is now demonstrated automatically is that the host publishes all four
through the real D-Bus path, and that it did not before this unit. What remains
an inference is that this defect is what the author hit: the reconciliation bug
and the unreadable folded drawer are both sufficient to produce the report, and
only the rerun can separate them. If items are still missing with the drawer
open, the `LVR-3-F` diagnostics will say whether each one arrived described or
unnamed, and the integration test above is the place to reproduce whatever it
says.

The 1920-pixel layout case measures one output width with one set of readings.
A longer Wi-Fi name, a second monitor's brightness entry or a wider clock would
each move the margin, and the case pins the shipped configuration rather than
proving the flank can never overflow.
