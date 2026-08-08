# Evidence: confirmed connectivity actions

- **Date:** 2026-08-07
- **Unit:** UX-1-B
- **Base revision:** `f6f54beb1a086d96e05af7b355e7d9d64db2b018`
- **Scope:** typed network and Bluetooth actions, bounded external execution,
  pending-request ownership and observation-based confirmation
- **Environment:** source checkout with no live Celestina activation
- **Artifact:** canonical release bundle built and verified, not deployed

## Result

The aggregate provider helper now accepts only the registered network and
Bluetooth verbs. Saved networks are addressed by their published UUIDs and
known devices by their published addresses; neither labels nor row positions
reach a process. Every external command has a fixed executable and argument
shape, a ten-second deadline, cancellation on helper shutdown and no shell
interpretation.

A successful tool exit publishes `accepted`, not success. The helper reserves
the request before execution but arms it only after that frame has been written,
so a provider observation cannot publish `confirmed` first or answer the same
request again after a tool failure. The twenty-second confirmation window also
starts at that point rather than charging time spent inside the tool.

Requests are keyed by provider, request ID and mutable target. A newer action
for the same adapter or device replaces the older one even when it asks for the
opposite state. Reusing an ID in another provider cannot arm or remove its
request. Refresh requests are serialized only against another refresh, and all
pending work is bounded and cancelled on helper shutdown.

## Procedure

Exercise parsing, target replacement, reservation, arming, expiry and later
observation in pure tests; exercise the aggregate helper's bounded execution
and exact argument vectors; then build and verify the canonical release bundle.

## Automated evidence

- `cargo test -p celestina-shell-core`: 247 passed.
- `cargo test --bin celestina-provider-adapter`: 45 passed.
- Clippy with `-D warnings` passed for `celestina-shell-core` and
  `celestina-provider-adapter`.
- `bash scripts/build-production.sh`: canonical release bundle built without
  installation or activation.
- `bash scripts/verify-production.sh`: passed, including architecture and
  visual guards, Rust checks and tests, QML lint, 15/15 CTest targets, helper
  integration tests and isolated production smoke.
- `git diff --check`: passed.

## Limits

No menu or panel interaction is part of this unit. UX-1-C owns the QML surfaces
that will issue these commands and present their pending, confirmed and failed
states. No NetworkManager, BlueZ, Niri, Noctalia or live shell state was changed
while collecting this evidence.
