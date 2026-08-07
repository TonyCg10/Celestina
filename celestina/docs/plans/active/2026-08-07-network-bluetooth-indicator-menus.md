# UX-1 — network and Bluetooth indicator menus

- **Opened:** 2026-08-07
- **Plan ID:** network-bluetooth-indicator-menus
- **Status:** active
- **Scope:** celestina
- **Implementation checkpoint:** UX-1
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
| UX-1-B | `celestina:` | planned | aggregate provider command path; bounded tool execution; focused tests | pending | Execute only saved/known actions and confirm them from later observations | pending | deferred |
| UX-1-C | `celestina:` | planned | panel indicators, transient menu surfaces, QML registration and contract tests | pending | Make both indicators directly interactive and dismissible | pending | `VAL-UX-1` |
| UX-1-D | `celestina:` | planned | version, evidence, validation hand-off and exact inventory | pending | Build, verify and deploy the completed checkpoint without activation | pending | `VAL-UX-1` |

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

`UX-1-A` remains `active`. Code, tests and guards are complete; the evidence
record and the exact inventory that its `done` state requires are not, and this
unit alone delivers nothing a person can see.

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
