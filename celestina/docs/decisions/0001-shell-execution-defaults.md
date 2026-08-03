# ADR 0001: Preserve settled shell execution defaults

- **Date:** 2026-08-03
- **Status:** accepted

## Context

The shell work orders settled several execution boundaries while R0-R2 were
implemented and recorded falsifiers for future phases. After those work orders
moved to history, their accepted choices needed a current canonical home rather
than leaving an archived document authoritative.

## Decision

Retain these defaults unless their named falsifier is observed:

| Area | Accepted default | Revisit trigger |
|---|---|---|
| Bus identity | stable owner `org.celestina.Shell` with versioned `Shell1` object/interface | owner collision or cross-version single-instance failure |
| Popup surface | separate anchored `PanelMenuSurface` described by `LayerSurfaceSpec` | proven accessible keyboard operation through an equivalent `xdg_popup` path |
| Provider runtime | `celestina-shell-core`, one aggregate bounded Rust helper and one narrow Qt client | measured isolation, latency or shutdown failure |
| Media and volume | bounded `playerctl` and `wpctl` composition | measured correctness, latency, shutdown or wakeup failure |
| Tray | shell-owned SNI host/watcher path | lived bar no longer needs a tray |
| Clipboard | shell-owned bounded history | the trust or resource contract cannot be met |
| Night light | bounded `wlsunset` lifecycle at 2700 K | gamma does not release cleanly or the lived value is unsuitable |
| Calendar | local month view without account sync | observed daily use demonstrates a sync requirement |

## Consequences

- Current implementation and future plans can link stable decisions instead of
  copying rationale from an archived work order.
- External tools remain narrow implementation dependencies, not authority to
  install or activate them.
- Locker, Polkit and dock choices remain open discussions and are not decided
  by this ADR.

## Revisit when

One of the table's concrete triggers is reproduced, or the author changes the
lived product requirement. Supersede this record with a dated ADR and retain the
failed evidence rather than editing the historical work order.
