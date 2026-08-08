# UX-1 — network and Bluetooth indicator menus

- **Opened:** 2026-08-07
- **Closed:** 2026-08-08
- **Plan ID:** network-bluetooth-indicator-menus
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** UX-1
- **Successor:** none; visual-design exploration is deliberately not an
  implementation plan until its product decisions settle
- **Author-validation checkpoint:** `VAL-UX-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The panel already publishes truthful network and Bluetooth summaries, but a
summary with no direct interaction forces the author into another application.
A bounded provider-owned inventory plus confirmed commands can make each
indicator useful without moving process execution or policy into QML.

## Tangible outcome

Clicking either indicator opens one dismissible menu. Network lists the current
link and bounded known Wi-Fi connections; Bluetooth lists the adapter and
bounded known devices. Every offered action stays pending until a later
provider snapshot confirms it, and failure remains visible rather than painting
the requested state.

## Scope

- Pure, bounded network and Bluetooth inventory and action vocabulary in
  `celestina-shell-core`.
- One extension of the existing aggregate provider adapter; no new helper.
- Network refresh plus activation of an already saved Wi-Fi connection.
- Bluetooth adapter power plus connect/disconnect for already known devices.
- Direct panel menus with outside-click and Escape dismissal, focus
  containment/restoration, keyboard actions and accessibility state.
- Characterization, domain, helper/protocol, QML contract and offscreen tests.

## Exclusions

- Collecting or storing new Wi-Fi credentials.
- Creating, editing or deleting NetworkManager profiles.
- Bluetooth discovery, pairing, PIN/passkey or trust policy.
- Disconnecting the current network merely to prove an action, especially in
  the author's nonstandard Ethernet/Wi-Fi layout.
- Replacing NetworkManager, BlueZ, Blueman or their policy agents.
- Notification `NameLost` and the remaining static-audit residuals.
- Any live activation without a separate explicit request.

## Build order

1. **UX-1-A — Bound the domain and snapshot contract.** Add typed inventories,
   stable identities, explicit availability and bounded parser/reducer tests.
   Extend provider snapshots additively while retaining today's summary keys.
2. **UX-1-B — Own confirmed actions in the aggregate helper.** Add typed
   commands for refresh, saved-network activation, adapter power and known
   device connect/disconnect. Serialize writes with the owning worker, bound
   tools and arguments, publish pending/failure truth, and confirm only from a
   later observation.
3. **UX-1-C — Add the two direct menus.** Make the existing panel indicators
   actionable and present provider data through registered QML components
   using CelestinaStyle tokens. Reuse the established full-output transient
   surface recipe so outside click, Escape and focus restoration behave like
   the corrected overlays.
4. **UX-1-D — Deliver and hand off.** Run the registered guards and canonical
   production exit, deploy without activation, and record only the live cases
   the author actually performs.

## Implementation exit

- Existing summary keys remain backward compatible and bounded list fields are
  rejected or truncated before crossing the helper protocol.
- No QML file launches a process or infers successful provider state.
- Unit and integration tests cover hostile text, excess rows, unavailable
  tools, command timeout/failure and stale confirmations.
- Both menus pass QML/offscreen keyboard, focus, Escape and outside-click
  contracts.
- `bash scripts/check-architecture-contract.sh`, the registered project verify
  script, `python3 scripts/version_tool.py check`, exact staged-unit checks and
  `scripts/complete-production.sh` pass before delivery.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| UX-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-07-network-bluetooth-indicator-menus/UX-1-A.numstat.tsv) | 9 files, +1773/-111 | Publish bounded inventories while preserving the current summary contract | [evidence](../../evidence/2026-08-07-connectivity-inventory-contract.md) | deferred |
| UX-1-B | `celestina:` | done | [inventory](../../inventories/2026-08-07-network-bluetooth-indicator-menus/UX-1-B.numstat.tsv) | 10 files, +2291/-14 | Execute only saved/known actions and confirm them from later observations | [evidence](../../evidence/2026-08-07-confirmed-connectivity-actions.md) | deferred |
| UX-1-C | `celestina:` | done | [inventory](../../inventories/2026-08-07-network-bluetooth-indicator-menus/UX-1-C.numstat.tsv) | 22 files, +2735/-265 | Make both indicators directly interactive and dismissible | [evidence](../../evidence/2026-08-07-two-menus-that-do-not-lie.md) | `VAL-UX-1` |
| UX-1-D | `celestina:` | done | [inventory](../../inventories/2026-08-07-network-bluetooth-indicator-menus/UX-1-D.numstat.tsv) | 13 files, +511/-159 | Build, verify and deploy the completed checkpoint without activation | [evidence](../../evidence/2026-08-08-ux-1-delivery.md) | `VAL-UX-1` passed |
| UX-1-E | `celestina:` | done | [inventory](../../inventories/2026-08-07-network-bluetooth-indicator-menus/UX-1-E.numstat.tsv) | 11 files, +1577/-18 | Preserve the invoking control's complete anchor when placing a transient card | [evidence](../../evidence/2026-08-08-menu-anchor-correction.md) | placement observed; cross-menu switching deferred |

## Active unit boundary

`UX-1-A` is complete. It owns the pure network and
Bluetooth inventory types/parsers and the additive snapshot publication in the
existing session provider. It may update `lib.rs` only to expose owned domain
types and may add focused tests beside those paths. It does not own commands,
QML, surfaces, versioning or production delivery yet.

### Boundary refined during implementation — 2026-08-07

Two paths were added to the unit after inspection showed the declared ones
could not hold the behaviour honestly. Both are recorded here before the
delivery inventory exists, and neither widens the unit's intent.

- `celestina-shell-core/src/inventory.rs`. Both inventories need the same three
  answers — the tool is absent, the tool did not answer, the tool answered —
  and the same rule that an unreadable poll republishes the last list rather
  than an empty one. Stating that twice beside two different parsers would have
  been two owners for one invariant, so it is one module with two consumers.
- `celestina/src/provider_adapter/tools.rs`. `run_bounded` collapsed "not
  installed" and "did not answer" into `None`. A summary widget cannot tell
  those apart and does not need to; a list must, because on a session without
  the tool an empty list is permanently correct and on a session with a slow one
  it erases what was already known. `probe_bounded*` is now the single
  implementation and the existing `run_bounded*` entries narrow its answer, so
  no caller has a second path and none of them changed behaviour.

`UX-1-A` is delivered and inventoried. Its boundary note above stays as the
record of why those two paths joined it.

## Active unit boundary — UX-1-B

`UX-1-B` is the only authorized product unit. It owns the typed action
vocabulary, identity validation against the last confirmed inventory, the
pending-request ledger and the confirmation rule, plus the bounded external
executions that carry an action out. It does not own QML, surfaces, versioning
or production delivery.

### Corrective refinement — ordered acceptance and target replacement

Review of the first UX-1-B implementation found that a pending entry became
observable before its `accepted` frame was written, allowing a fast poll to
publish `confirmed` first and, if the tool later failed, answer the same request
twice. The ledger therefore reserves a validated request before execution but
does not arm it for observation or expiry until the command worker has written
`accepted`. Actions aimed at the same target also replace one another
regardless of the state requested, so Bluetooth `on`/`off` and
`connect`/`disconnect` cannot remain pending together. This refinement stays
inside UX-1-B's existing protocol, ledger, helper and focused-test boundary.

UX-1-B is complete and inventoried.

## Active unit boundary — UX-1-C

`UX-1-C` owns the two indicator controls, the durable request ledger the menus
read, both menu components, their transient surfaces, QML registration and the
focused interaction tests. It does not own new connectivity capabilities,
credentials, pairing policy, versioning or production delivery.

### Corrective refinement — a ledger that outlives the menu that made it

Review of the first `UX-1-C` implementation found four defects, all of them real
and all inside this unit's own boundary. They are corrected here rather than
deferred, and `UX-1-D` returns to `planned` until they are.

1. **One request contract was applied to two.** The extracted QML ledger treated
   `accepted` as still waiting, which is right for connectivity — `UX-1-B`
   confirms those from a later observation — and wrong for every verb the
   control centre already had, because nothing ever sends those a `confirmed`.
   The control centre would have said "preguntando…" for ever. The contract is
   now declared per request by the consumer, `immediate` or `confirmed`, rather
   than inferred from a verb name.
2. **The menu destroyed the tracking it had just created.** A `MenuItem`
   activation closes its `Menu`, which dismisses the surface and destroys the
   window — and with it the per-window ledger, before any result could arrive.
   The ledger therefore moves into the host, onto an owner whose lifetime is
   already exactly the helper generation.
3. **The network indicator vanished with the link.** Visibility keyed off
   `network.kind`, so a session with no default route lost the entry point to
   the menu that exists to reconnect it.
4. **The indicators were `Text` with a `MouseArea`.** No focus, no visual focus,
   no Enter or Space.

The unit therefore adds `src/requestledger.{h,cpp}` and touches
`src/shellprovidersclient.{h,cpp}`, and `qml/ProviderRequests.qml` is deleted
rather than left as a second ledger. No new verb, tool, provider capability or
dependency joins it, and `UX-1-A` and `UX-1-B` are untouched.

`UX-1-C` is complete and inventoried, including the four corrections above.
`UX-1-D` is now the only authorized product unit: the version transition, the
`docs/version-history.tsv` row, the canonical production exit and the hand-off
to `VAL-UX-1`. It owns no product behaviour; a defect found from here opens a
corrective unit rather than widening this one.

### The panel cannot take the keyboard, and this unit does not pretend otherwise

`panelSpec` maps every panel with `KeyboardInteractivityNone` and
`acceptsFocus = false`. That is deliberate: a bar that took focus would steal it
from the window a person is working in. So on a live session there is no Tab
route to these indicators, and this unit ships no test claiming there is.

What it does ship is a control that is correct wherever a surface *does* admit
the keyboard: a real focus scope with `visualFocus`, Enter and Space activation,
and role, name, description and state exposed. The offscreen cases drive that
control in a window that accepts focus, which proves the control rather than the
panel. Opening these menus from the keyboard alone would need a session verb and
a binding; that is a product decision for a later checkpoint, not something to
imply here.

### UX-1-C boundary refined during implementation — 2026-08-07

Two extractions were required before the menus could be written, because each
recipe was about to gain its third and fourth copy. Both remove the old paths in
the same unit rather than leaving two live implementations.

- `qml/AnchoredMenu.qml`. `PanelMenu.qml` and `TrayMenu.qml` each carried their
  own copy of the contract `placeCard` in `panelmenucontroller.cpp` writes to —
  `shadowMargin`, `menuX`, `menuY` — plus the clamp that keeps a card whole near
  an output edge and the `GlassContextMenu` wiring that dismisses the surface.
  That contract had no owner while three files agreed about it by hand. It now
  has one, and `PanelMenu`, `TrayMenu` and both new menus are its consumers.
- The first implementation extracted that lifecycle to
  `qml/ProviderRequests.qml`. Corrective review then proved a QML object owned
  by the menu dies with the request it has just sent. The extraction was
  removed in the same unit and its single durable replacement is
  `src/requestledger.{h,cpp}`, owned by `ShellProvidersClient` and shared by the
  control centre and both menus.

The unit therefore also touches `qml/{PanelMenu,TrayMenu,ControlCentre}.qml`,
`qml/SessionStatus.qml`, `qml/Panel.qml`, `src/panelmenucontroller.{h,cpp}`,
`src/panelmanager.{h,cpp}`, `src/shellprovidersclient.{h,cpp}` and
`CMakeLists.txt`. No new verb,
tool, provider capability or dependency joins it.

### Vocabulary equivalence — 2026-08-07

The existing channel already carries `{id, provider, verb, options}` and answers
with a `ResultFrame`. The requested verbs enter it unchanged in spirit, under
the providers `network` and `bluetooth` that UX-1-A already registers:

| Requested | Verb on the wire | Provider | Options |
|---|---|---|---|
| `refresh` | `refresh` | `network`, `bluetooth` | none |
| `activate-saved` | `activate-saved` | `network` | `id` — a UUID from the last confirmed inventory |
| `set-powered` | `set-powered` | `bluetooth` | `powered` — a real boolean |
| `connect-known` | `connect-known` | `bluetooth` | `id` — an address from the last confirmed inventory |
| `disconnect-known` | `disconnect-known` | `bluetooth` | `id` — an address from the last confirmed inventory |

`id` rather than `uuid`/`address` because both inventories already publish their
stable identity under `id`, and a verb that names the same field the row does
cannot be wired to the wrong one.

One equivalence is worth stating: this shell already used `outputs-changed` for
"look again now" on brightness. That is a host notification rather than a
request, so `refresh` here is a real command with an id and an answer, and it is
not folded into that name.

### Limitations UX-1-C must present honestly

- **Pending is a real state and lasts up to 20 seconds.** A menu entry that was
  clicked is neither done nor failed until a later observation says so, and the
  poll that observes runs every five seconds. `refresh` wakes that poll early
  but does not shorten it, so a confirmation typically arrives in one poll and
  may take four. The menu has to show waiting rather than painting the
  requested state.
- **A disconnect is confirmed by seeing the device disconnected, not by its
  absence.** Powering the adapter down empties the device list, so a request
  aimed at a single device keeps waiting and may end as expired rather than
  confirmed. That is deliberate; the surface should not describe an expiry as a
  successful disconnection.
- **`activate-saved` is offered only for profiles already saved.** A network in
  range with no profile cannot be joined here at all, because that needs a
  password. UX-1-C should not render such an entry as disabled-but-present
  unless it also explains why.
- **Availability is mostly `unknown`.** The UX-1-A limitation stands: only the
  active profile's SSID can be learned in one bounded run, so most rows carry no
  signal and no in-range word. A menu that sorts or filters by signal would be
  sorting on absence.
- **Two refreshes cannot overlap.** The second is refused while the first is
  waiting, which the menu should treat as "already refreshing" rather than an
  error worth showing.

### Result states

`ResultFrame` gains `confirmed` beside its existing `accepted` and `failed`.
This is the same result system, not a second one: `accepted` keeps meaning "the
helper carried the request out", and a later frame with the same id reports what
the machine actually did. The Qt host forwards `state` verbatim to
`commandResult`, so no host change is needed and no existing consumer breaks.

### Recorded limitation — a saved profile's SSID

`nmcli connection show` cannot report the network a profile joins. Its terse
field list is exactly `NAME,UUID,TYPE,TIMESTAMP,TIMESTAMP-REAL,AUTOCONNECT,
AUTOCONNECT-PRIORITY,READONLY,DBUS-PATH,ACTIVE,DEVICE,STATE,ACTIVE-PATH,PORT,
FILENAME`, verified read-only against this session's own `nmcli`, and asking for
`802-11-wireless.ssid` there is rejected as an invalid field. The SSID is
reachable only per profile, which would be one process per row and is refused.

So `KnownNetwork.ssid` is `Option<String>` and is never inferred from `NAME`: a
profile's label and its network are different things that agree only by
convention. The one attribution a single bounded run supports is the active
profile's — NetworkManager attaches one profile to a device at a time and
`nmcli device wifi list` marks the access point in use, so that pairing is read
rather than guessed. Every other profile keeps `ssid: None` and therefore
`Availability::Unknown`.

`Availability::OutOfRange` consequently requires both a known SSID and a scan
that answered without it. It is reachable in the domain and tested there, and
with today's inputs only the active profile can reach it. If UX-1-B needs
availability for inactive profiles, the bounded way in is NetworkManager's D-Bus
API — one call for all profiles — not a process per row.

## UX-1-D delivery boundary

`UX-1-D` owns only the 0.7.0 milestone transition, its append-only version
history row, the canonical production exit, the implementation-lane closure in
status and roadmap, the pending `VAL-UX-1` hand-off, and this unit's evidence
and exact inventory. It adds no product behavior and does not activate a shell.

The plan moves to its stable archive path in this final inventoried unit. The
two preceding uncommitted inventories name that same final endpoint because all
three units land as one atomic batch; the already delivered UX-1-A inventory
remains immutable and the archive transition preserves its historical link.

## Active unit boundary — UX-1-E

`UX-1-E` owns only the placement correction: let the compositor position the
full-output transient surface after the panel's real exclusive zone, preserve
the invoking control's horizontal anchor inside that surface and cover the
conversion with a pure regression. Because UX-1 and this correction remain one
uncommitted delivery batch, the correction is consolidated into the 0.7.0
milestone rather than inventing a second version transition before 0.7.0 lands.
It also owns
the narrow author-only incremental restart wrapper requested to validate this
correction and the subsequent visual checkpoint without repeating the
production exit on every visual iteration. It does not own iconography, menu
composition, weather, calendar or the later visual-design checkpoint.

The author confirmed that the card no longer used the hypothetical unstacked
panel position. The later attempt to change directly from one open menu to
another still required two clicks in the live compositor. That is recorded as
an unresolved interaction requirement for the next design checkpoint; this
unit does not claim that `modal: false` has proved the compositor path.

## Closure

UX-1 closes as one 0.7.0 milestone batch. Its two provider-owned inventories,
durable request ledger, two direct menus and opener-relative surface placement
have automated evidence and a passed functional author-validation case. The
unresolved cross-menu gesture, visual language, iconography and the proposed
clock/date/weather surface are product-design inputs, not hidden amendments to
this completed functional checkpoint.
