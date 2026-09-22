# The Sensors page's keyboard, its failure state and its gate — H4-D

- **Date:** 2026-09-22
- **Scope:** `H4-D` of
  [`../plans/archive/2026-09-22-h4-sensors-fixes.md`](../plans/archive/2026-09-22-h4-sensors-fixes.md):
  the eight findings of the `H4` whole-branch review the controller did not
  defer, and 0.5.1
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; every run headless (`QT_QPA_PLATFORM=offscreen`)
- **Artifact:** `hematita/target/production-artifact.toml`, verified and
  deployed

## What changed

1. **The page is one Tab stop whose arrows scroll**
   (`qml/components/SensorsPage.qml`, `SensorChipCard.qml`, `SensorRow.qml`).
   The `Flickable` + `Column` + card `Repeater` is now a `ListView` whose
   delegates are the cards: integer model `page.cards.length`, each delegate
   reading `page.cards[index]`, `activeFocusOnTab: true`,
   `keyNavigationEnabled: true`, `highlightFollowsCurrentItem: true`,
   `Accessible.role: Accessible.List`, `Accessible.name: qsTr("Sensores")`,
   and the `CelestinaScrollBar` reporting on the list. The card and the row
   take no focus (`activeFocusOnTab: false`), and the row's focus ring is
   gone. Before this, forty-six rows each answered Tab inside a surface with
   no current item: crossing the page cost forty-six stops and a focused row
   below the fold could not be scrolled to, because nothing knew a row was
   current. Rows keep `Accessible.role: ListItem` and their name, which is
   what a screen reader walks. This is the same shape the process list already
   has.
2. **A failed tick shows only its reason** (`src/sensors.rs`). The
   `Unavailable` arm of `apply` set `available` and the reason and returned,
   leaving the last good chips on screen beneath the error line — values from
   a second that did not happen. It now calls a new `clear_lists()` (every
   published list back to empty) and bumps `revision` before returning, so the
   page shows the reason and nothing else. The extremes map is deliberately
   left alone: a tick that reads again resumes the session's minima and
   maxima instead of starting over.
3. **The smoke asserts the page has chips** (`qml/Main.qml`,
   `scripts/smoke.sh`). Once the section walk reaches Sensores and
   `sensorHub.revision` is at least 1, the window prints
   `hematita-sensors <chips> <channels>` once, from both the hub's
   `revisionChanged` and the walk's own step (a reading has usually landed
   long before the walk arrives, so waiting for the next revision would have
   been waiting for nothing). `smoke.sh` requires
   `hematita-sensors [1-9][0-9]* [1-9][0-9]*$`: a page that constructs without
   error but publishes no chip at all is a working page showing nothing, which
   no error message would have said. Every existing gate — the auto-binding
   scan, the ten-second liveness, the error scan, the CPU shape line, the
   section walk — is unchanged.
4. **`seen` is a `HashSet<String>`** (`src/sensors.rs`). The extremes
   `retain` did a linear scan of a `Vec` per key, which is the channel count
   squared each tick.
5. **The row names its extremes** (`SensorRow.qml`). `Accessible.name` now
   reads label, value, extremes and limit, so a screen reader hears the
   session's minimum and maximum a pair of eyes reads in the subtitle.
6. **`ordinals` and `totals` are null-prototype objects**
   (`SensorsPage.qml`). A plain object literal inherits `Object.prototype`, so
   a driver literally named `constructor` or `toString` would have read as an
   existing count.
7. **A repeated driver numbers every chip** (`SensorsPage.qml`). `chipTitle`
   takes `total` as well as `ordinal` and appends the ordinal only when the
   machine has more than one chip of that driver — but then to all of them, so
   this machine's two `nvme` chips are *1* and *2* rather than one unnumbered
   card beside a *2*. `weave` counts the totals in a first pass.
8. **The search applies on `accepted`** (`ProcessTable.qml`). The two
   `Keys.on*Pressed` handlers became one `onAccepted`, which is the signal
   `TextField` already emits for Return and Enter.

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
sha256sum hematita/target/release/hematita ~/.local/bin/hematita
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 10 hematita/target/release/hematita
```

## Result

- **Exit:** 0 for every command; the offscreen run ends at its timeout
  (`rc=124`, alive throughout).
- `complete-production.sh` — build, verify and deploy in one pass:
  `hematita-core` 59 unit tests and 11 capture tests passed, doc-tests 0;
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline
  warning(s))` with no suppressions added; `smoke: OK — binary alive for 10 s,
  every section was shown, the first row published the CPU contract, the
  Sensors page published chips, no QML errors, no auto-bindings`; manifest
  `verified`; `>> verified Hematita deployed to /home/toni/.local without
  rebuilding`; `installed: OK /home/toni/.local/bin/hematita`. Compiled on the
  first attempt; this was the unit's only build.
- `status-production.sh` — `artifact: hematita current and verified`,
  `installed: OK /home/toni/.local/bin/hematita`.
- The installed bytes are the checkout's bytes:
  `f995906481ffccafa6914dbd622b218cd4eb09eea6f0220d41c53e8c3773b815` for both
  `hematita/target/release/hematita` and `~/.local/bin/hematita`.
- The manual ten-second walk printed exactly two lines and matched none of
  `TypeError`, `ReferenceError`, `Unable to assign`, `Cannot read property`:

```
qml: hematita-shape cpu 3 60
qml: hematita-sensors 9 44
```

  Nine chips and forty-four channels — this machine's nine `hwmon`
  directories, and forty-four channels from its forty-six `_input`/`_average`
  files, since a power channel offering both `_input` and `_average` is one
  channel.
- Version: `0.5.0` → `0.5.1` (`bug`, unit `H4-D`) through
  `scripts/version_tool.py bump`; `version_tool.py check` — `version-contract:
  OK (8 owners)`.

## Limits

- **Still no visual check, and still no key pressed.** Every run was
  headless. That the page is now one Tab stop whose arrows move by card
  follows from the `ListView` and from the two components refusing focus —
  the same construction the process list already uses — but nothing here
  presses Tab or an arrow. `VAL-H4` gained the step that does: Tab into
  Sensores once and arrow through the cards, confirming the page and not its
  rows is the single stop.
- **The failure state is not exercised.** `/sys/class/hwmon` was readable in
  every run, so the empty-list path was reasoned and compiled, not observed.
  Making it fail would mean interfering with the running session's sysfs,
  which is not something to do to the author's machine.
- **The smoke's new gate proves a count, not a value.** Nine chips and
  forty-four channels is not that any one of them is plausible; that is
  `VAL-H4`.
- Deferred by the controller and untouched: a chip's static files are never
  re-enumerated while its name is unchanged, power channels are never graded,
  and one unreadable tick still erases a channel's extremes. All three are
  `H5`.

## Follow-up

`H4-D` closes H4's fix wave and the plan is archived. `H5` (services and
privileged actions) is the next checkpoint, not yet opened. `VAL-H4` is
pending in the author's lane and does not block this closure.
