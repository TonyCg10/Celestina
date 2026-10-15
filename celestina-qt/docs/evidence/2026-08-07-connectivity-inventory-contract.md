# Evidence: bounded connectivity inventories

- **Date:** 2026-08-07
- **Unit:** UX-1-A
- **Base revision:** `6d23b25627a8e3a6c79033cf2b385edf01ba2d8b`
- **Scope:** pure network and Bluetooth inventory contracts, aggregate helper
  observation and additive provider snapshots
- **Environment:** source checkout with Noctalia retaining the live session;
  Rust unit tests and repository guards only
- **Artifact:** source and test evidence only; no production artifact was built
  or deployed
- **Product effect:** no visible menu or action; UX-1-B and UX-1-C remain
  planned

## Result

Network and Bluetooth now publish bounded inventories without changing the
meaning of their existing summary keys. `pending`, `unavailable`, `fresh` and
`held` are distinct protocol states; an unreadable first observation is never
an empty list, and a later unreadable observation retains its last confirmed
rows.

The network provider can publish saved connections without a default route and
can publish tool unavailability without inventing a link. Profile UUID remains
the stable identity. Profile labels are never treated as SSIDs: only the single
unambiguous active-profile/in-use-network case is attributed, while multiple
active radios and inactive profiles remain explicitly unknown.

Bluetooth reads `show`, paired devices and connected devices at most once per
poll. The same connected answer owns the historical count and first-device
summary and every inventory connection flag, so one snapshot cannot contradict
itself.

## Procedure

Exercise the pure inventory reducers, hostile parsers and payload composers;
exercise bounded process outcomes in the aggregate helper; then run formatting,
Clippy and the registered architecture, language and version guards without
starting a shell process.

## Automated evidence

- `cargo test -p celestina-shell-core`: 218 passed.
- `cargo test --manifest-path celestina/Cargo.toml --bin
  celestina-provider-adapter`: 38 passed.
- Clippy with `-D warnings` passed for `celestina-provider-adapter` and
  `celestina-shell-core`.
- `cargo fmt --all --check` passed for both Rust workspaces.
- `bash scripts/check-architecture-contract.sh`: passed.
- `python3 scripts/check-language-contract.py`: passed.
- `python3 scripts/version_tool.py check`: passed.
- `git diff --check`: passed.

## Limits

Inactive saved Wi-Fi profiles cannot be assigned an SSID from the bounded
`nmcli connection show` list. UX-1-B must act by UUID and must not infer an SSID
from the profile label. Richer inactive-profile availability would require one
bounded NetworkManager D-Bus inventory, not one process per profile.

No shell surface, live session, NetworkManager state, BlueZ state, Niri state,
Noctalia process or production installation was changed for this evidence.
