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
| UX-1-A | `celestina:` | active | `celestina-rs/crates/celestina-shell-core/src/{network,bluetooth}.rs`; `celestina/src/provider_adapter/session.rs`; focused tests | pending | Publish bounded inventories while preserving the current summary contract | pending | deferred |
| UX-1-B | `celestina:` | planned | aggregate provider command path; bounded tool execution; focused tests | pending | Execute only saved/known actions and confirm them from later observations | pending | deferred |
| UX-1-C | `celestina:` | planned | panel indicators, transient menu surfaces, QML registration and contract tests | pending | Make both indicators directly interactive and dismissible | pending | `VAL-UX-1` |
| UX-1-D | `celestina:` | planned | version, evidence, validation hand-off and exact inventory | pending | Build, verify and deploy the completed checkpoint without activation | pending | `VAL-UX-1` |

## Active unit boundary

`UX-1-A` is the only authorized product unit. It owns the pure network and
Bluetooth inventory types/parsers and the additive snapshot publication in the
existing session provider. It may update `lib.rs` only to expose owned domain
types and may add focused tests beside those paths. It does not own commands,
QML, surfaces, versioning or production delivery yet.
