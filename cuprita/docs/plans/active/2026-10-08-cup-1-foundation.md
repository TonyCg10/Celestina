# CUP-1 — Foundation and the three sections

- **Opened:** 2026-10-08
- **Plan ID:** cup-1-foundation
- **Status:** active
- **Authorization:** the author approved the design and asked for the
  foundation plan on 2026-10-08
- **Scope:** cuprita
- **Implementation checkpoint:** CUP-1
- **Author-validation checkpoint:** `VAL-C`, `VAL-D` and `VAL-E` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

One window can replace nm-applet, Blueman and pavucontrol when each backend
sits behind a trait with a fake, so the whole surface is testable in the
session and the author's live check is only the last mile.

## Tangible outcome

The registered project, a release binary installed under the author's prefix,
whose three sections manage network, Bluetooth and audio.

## Scope

- `CUP-1-A` — the skeleton: crate stub, application, strip, scripts, documents.
- `CUP-1-B` — `cuprita-core` and the controllers over the fakes.
- `CUP-1-C` — Red. `CUP-1-D` — Bluetooth. `CUP-1-E` — Audio.
- `CUP-1-F` — implementation exit and 1.0.0.

## Exclusions

- Everything the
  [design](../../../../docs/superpowers/specs/2026-10-08-cuprita-design.md)
  lists as out of scope.
- Any reference to the Celestina shell.

## Build order

1. `CUP-1-A`, then `-B`, then `-C`, `-D` and `-E`, then `-F`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary manages
network, Bluetooth and audio.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CUP-1-A | `cuprita:` | active | `cuprita/`, `celestina-rs/crates/cuprita-core/` | — | Application skeleton: window with the three-section strip and empty pages, scripts, smoke, the document set. Lands inside suite unit AUD-1-P. | [skeleton](../../evidence/2026-10-08-skeleton.md) | None |
| CUP-1-B | `cuprita:` | done | [inventory](../../inventories/2026-10-08-cup-1-foundation/CUP-1-B.numstat.tsv) | 61 files, +4722/-238 | Models, the three traits, fakes, pure logic; controllers and list models over the fakes; the three pages and the notice pill | [evidence](../../evidence/2026-10-08-domain.md) | None |
| CUP-1-C | `cuprita:` | done | [inventory](../../inventories/2026-10-08-cup-1-foundation/CUP-1-C.numstat.tsv) | 25 files, +2140/-43 | NetworkManager client (`nm.rs`) with its change watcher and `snapshot` example; the network worker driven by the watcher; the Wi-Fi password dialog; the Red page live | [evidence](../../evidence/2026-10-08-network.md) | `VAL-C` |
| CUP-1-D | `cuprita:` | done | [inventory](../../inventories/2026-10-08-cup-1-foundation/CUP-1-D.numstat.tsv) | 35 files, +2658/-242 | BlueZ client (`bluez/`) with its change watcher and the exported `Agent1` pairing agent; the bus mechanics shared with `nm` moved to `bus.rs`; the Bluetooth worker driven by the watcher; the agent bridge and `PairingDialog`; airplane mode powers the adapter off; the Bluetooth page live | [evidence](../../evidence/2026-10-08-bluetooth.md) | `VAL-D` |
| CUP-1-E | `cuprita:` | planned | `cuprita/src/`, `cuprita/qml/` | — | Binding spike, PipeWire client and the Audio page | `scripts/verify-production.sh` | `VAL-E` |
| CUP-1-F | `cuprita:` | planned | `cuprita/`, `docs/version-history.tsv` | — | Implementation exit, 1.0.0, status and roadmap closed, plan archived | `scripts/complete-production.sh` | `VAL-C`, `VAL-D`, `VAL-E` |
