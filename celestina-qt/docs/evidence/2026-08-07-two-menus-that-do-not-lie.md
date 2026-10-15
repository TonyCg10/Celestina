# Evidence: 2026-08-07 two menus, and a ledger that outlives them

- **Date:** 2026-08-07
- **Scope:** `UX-1-C`; plan
  [network-bluetooth-indicator-menus](../plans/archive/2026-08-07-network-bluetooth-indicator-menus.md)
- **Environment:** Noctalia owned the session throughout. Celestina was built
  and verified, never activated, never deployed. No connectivity was changed:
  nothing here ran `nmcli`, `bluetoothctl` or any other tool, and no test can —
  the menus' only outward call is the request ledger, and every test replaces
  the thing it sends through with a recorder
- **Artifact:** celestina 0.6.8, unchanged. Versioning and delivery are
  `UX-1-D`'s

## What was built

The two connectivity indicators became controls that open their own menus in a
transient surface covering the output, drawn with the recipe the corrected
overlays already use.

- `src/requestledger.{h,cpp}` — what became of the requests surfaces made, kept
  where a surface cannot take it with it.
- `qml/NetworkMenu.qml` — the confirmed link, the bounded inventory of saved
  Wi-Fi profiles with the active one marked, an activate action per profile and
  a refresh.
- `qml/BluetoothMenu.qml` — the adapter and its switch, the bounded inventory of
  known devices with their confirmed connection state, connect or disconnect per
  device and a refresh.
- `qml/AnchoredMenu.qml` — the placement contract `placeCard` writes to, with
  one owner instead of a copy per menu. `PanelMenu`, `TrayMenu` and both new
  menus consume it.

No verb, tool, provider capability or dependency joins the shell. Nothing here
can ask for a password, create or edit a profile, start discovery, pair, forget,
trust or ask for a PIN: no such request exists in the vocabulary `UX-1-B` closed.

## The first implementation of this unit was wrong in four ways

A review rejected it. All four were real, all four were inside this unit, and
each is now reproduced by a test that fails without its fix — verified by
reintroducing each defect and watching the suite go red.

### 1. One request contract was applied to two

The first version kept every request waiting until a `confirmed` arrived. That
is right for connectivity, where `UX-1-B` confirms from a later observation. It
is wrong for every verb the control centre already had: nothing ever sends those
a `confirmed`, so audio, brightness, night light, caffeine and power would have
said "preguntando…" for the rest of the session.

The contract is now declared by the consumer at the moment of asking, and named:

- **`immediate`** — the helper answering `accepted` is the whole answer. Every
  control-centre verb.
- **`confirmed`** — `accepted` means a tool ran; the request keeps waiting until
  a later observation says `confirmed` or `failed`. Every connectivity verb.

Nothing infers this from a verb name, and no provider was given an artificial
`confirmed` it does not produce.

### 2. The menu destroyed the tracking it had just created

A menu row is a `MenuItem`. Activating one closes its `Menu`, which dismisses the
surface, and the host answers `dismissed` by destroying the window. The ledger
lived in that window, so it died before the helper had written `accepted`:
nothing was ever pending for long enough to be seen, results arrived to a
destroyed object, and reopening the menu produced an empty ledger.

The ledger therefore moved to the host, onto `ShellProvidersClient` — the owner
whose lifetime is already exactly a request's lifetime, one helper generation. It
is exposed as `providerSource.requests`, so one ledger serves the control centre
and both menus and there is no second one. `qml/ProviderRequests.qml` is deleted
rather than left beside it.

`indicatormenu_test.cpp::activatingARowClosesTheMenuAndOutlivesItsWindow` is the
case that reproduces the defect: it clicks a real row at its real position,
asserts the menu closed and `dismissed()` fired, **destroys the window**, and
only then delivers `accepted` and `confirmed`.

### 3. The network indicator vanished with the link

Visibility keyed off `network.kind`, so a session with no default route lost the
entry point to the menu whose whole purpose is reconnecting it.

The declared policy is now: the indicator is present whenever the `network`
provider is publishing anything — a link, or an inventory in any of `fresh`,
`held`, `pending` or `unavailable` — and absent only when the provider has
withdrawn entirely, which is an unreadable session with nothing truthful to
offer. With no link its reading and its accessible name both say plainly that
there is none, and it never claims Wi-Fi merely because an inventory exists.

### 4. The indicators were `Text` with a `MouseArea`

They are now `AbstractButton`s: focusable, activated by Space and by Enter, with
a `CelestinaFocusRing` bound to `visualFocus` — true for the keyboard, false for
a click — and role, name, description, state and action exposed. Click and hover
are unchanged.

## The panel cannot take the keyboard, and this unit does not pretend it can

`panelSpec` maps every panel `KeyboardInteractivityNone` with
`acceptsFocus = false`, deliberately: a bar that took focus would steal it from
the window a person is working in. **On a live session there is no Tab route to
these indicators**, and this unit ships no test claiming there is.

What is tested is the control, in a window that does accept focus: Enter, Space,
click, and that the focus ring follows `visualFocus` rather than `activeFocus`.
Reaching these menus from the keyboard alone would need a session verb and a
binding — a product decision for a later checkpoint, not something to imply here.

## How a request identifies itself

Ids are `quint64` on the wire and stay `quint64` in the ledger. They cross to QML
only as decimal strings, because a JavaScript number cannot hold one: the test
`anIdTooLargeForADoubleSurvivesAsItself` uses `9007199254740993` and
`9007199254740994`, which a double merges into one value, and shows each request
answered under its own identity.

A request is `provider + target`, holding the id it is waiting on. That is what
keeps `refresh` on `network` and `refresh` on `bluetooth` apart, and what lets a
newer request for the same target drop the older one's id so its late answer
settles nothing. A settled entry keeps no id, so a duplicate frame cannot reopen
it. Generation loss fails everything still pending and leaves everything already
reported as history. The ledger is bounded to 64 targets, oldest dropped.

## What the tests prove

| Suite | Cases | What it covers |
|---|---|---|
| `requestledger_test.cpp` | 12 | both contracts and their difference; a request that could not be sent; identity by provider and target; a replaced request's late answer; ids past 2^53; generation loss sparing settled entries; a settled request not reopened; the bound; a failure surviving until acted on again; an unknown contract sending nothing |
| `indicatormenu_test.cpp` | 16 | component per kind; declared properties; click where the indicator is; click on the card; Escape; clamped card; keyboard highlight **and Return really activating**; every row announces itself; **a real row click closing the menu and the request outliving the destroyed window**; a reopened menu showing what happened while it was closed; a failed target remaining visible after its inventory row disappears and being dismissible; providers independent; the control centre stopping asking on `accepted` |
| `tst_sessionindicators.qml` | 9 | link named; **no link keeps the entry point and still opens the menu**; every inventory state stays reachable; a withdrawn provider shows nothing; connected → disconnected keeps the entry; Bluetooth policy unchanged; click, Enter and Space; the ring follows `visualFocus` |
| `tst_networkmenu.qml` / `tst_bluetoothmenu.qml` | 5 + 4 | what each menu decodes and says: link line, list states, rows, adapter states |
| `tst_controlcentre.qml` | 3 | reading rules through the shared bridge |

### What these do not prove

They are offscreen. Whether the compositor delivers a click at the indicator to
the menu's surface, and where focus lands when the surface unmaps, are Wayland's
and belong to `VAL-UX-1`. The click case proves the window answers such a click
with `dismissed()`; it does not prove the click arrives.

The request-lifecycle cases were deliberately moved **out** of the QML menu
suites. Keeping a menu object alive and hand-delivering a result would test
something the product never does, because activating a row destroys that object.

## Procedure

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
git diff --check

cd celestina
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
./scripts/qmllint-production.sh
cmake --build build -j8
ctest --test-dir build --output-on-failure
./scripts/build-production.sh
./scripts/verify-production.sh
```

## Result

Every command above passed: **17/17 CTest targets**, including the two new ones,
`celestina-request-ledger` and `celestina-indicator-menu`. The QML suite reports
115 cases passed; `celestina-indicator-menu` 16 and `celestina-request-ledger`
12. `qmllint-production` is OK with no warning on any file this unit touched, and
the language contract stays at 157 ratcheted legacy files.

Each of the four defects was reintroduced on purpose and the suite went red for
it — the immediate contract, the durable ledger and the indicator visibility each
took their own cases down. That is the only reason to believe the tests test
anything.

## Limits

No menu has been opened on a real session. `VAL-UX-1` still needs the author to
confirm that one click opens each menu and one closes it, that Escape and an
outside click do the same, that focus returns where it came from, that the card
lands under the indicator at this output's scale, and that a real activation or
device connect reaches `confirmed` rather than expiring. Nothing offscreen
stands in for any of those.
