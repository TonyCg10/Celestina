# Celestina implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** CEL-1
- **Related author validation:** VAL-CEL-GLASS, VAL-CEL-ISLAND, VAL-CEL-REST,
  VAL-CEL-POPUP, VAL-CEL-KEYBOARD, VAL-CEL-IME, VAL-CEL-SCALE,
  VAL-CEL-NOCTALIA and VAL-CEL-BANNERS in [VALIDATION.md](VALIDATION.md) (they do not block)

## Hypothesis and tangible outcome

A pure Rust shell on Wayland for niri can replace the halted Qt/QML shell:
one process draws an Island on the top edge of every output that grows into
pages, a launcher, notification banners and a tray, painted with the
Celestina visual language over the compositor's glass. The plan is
[CEL-1](docs/plans/active/2026-10-14-cel-1-island.md); the design is
[the shell design](../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md).

## Scope

- The first release, CEL-1: the Island, the launcher, notifications, the tray
  and shared tokens.

## Exclusions

- Parkable pieces, Customize, the full-width bar, OSD, media, clipboard
  history, night light, lock, polkit, other schemes and wallpaper: later
  deliveries with their own designs.
- A dock, plugins, scripting and any compositor other than niri, for good.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CEL-1-B | done | CEL-1-A | The workspace, `celestina-wayland`, `celestina-paint` with the text atlas, `celestina-ui` with keyed reconciliation and its tests without GL; one Island capsule per output with clock and workspace dots over the tolerant niri stream; the Shell1 command surface and single instance | `celestina/docs/evidence/2026-10-14-skeleton.md` |
| CEL-1-C | done | CEL-1-B | `celestina-style/tokens.toml`, the generator, `CelestinaDesignTokens.qml`, `celestina-tokens` and the drift check in both verifies; the shell switches from local constants to the crate (a `celestina-style` and `celestina-rs` unit) | its owners' evidence |
| CEL-1-D | done | CEL-1-C | The Island pages: Activity, Notifications, Tray, page tabs, the overflow popup | `celestina/docs/evidence/2026-10-16-island-pages.md` |
| CEL-1-E | done | CEL-1-D | The launcher: overlay, `Field` with IME and paste, index, ranking, `/` commands, spawn | `celestina/docs/evidence/2026-10-16-launcher.md` |
| CEL-1-F | done | CEL-1-D | Notifications: server (claimed if free), banners, history, actions, do-not-disturb | `celestina/docs/evidence/2026-10-16-notifications.md` |
| CEL-1-G | done | CEL-1-D | The tray over `zbus` blocking: watcher and host (claimed if free), items, icons, nested DBusMenu popups, scroll | `celestina/docs/evidence/2026-10-14-tray.md` |
| CEL-1-H | active | CEL-1-E, F, G | The exit: the live rows queued in `VALIDATION.md` (the author runs them), the handover note on what Noctalia still does ([handover](docs/handover.md)), the documents up to date; the version is set by the landing's `--kind` | `celestina/docs/evidence/2026-10-16-exit.md` |

## Implementation exit

`celestina/scripts/verify-production.sh` passes on the CEL-1-H tree and the
live rows are queued in `VALIDATION.md`.

## Closed evidence

- CEL-1-B: [skeleton](docs/evidence/2026-10-14-skeleton.md)
- CEL-1-C: [tokens as data](../docs/evidence/2026-10-14-tokens-as-data.md) (suite)
- CEL-1-D: [Island pages](docs/evidence/2026-10-16-island-pages.md)
- CEL-1-E: [launcher](docs/evidence/2026-10-16-launcher.md)
- CEL-1-F: [notifications](docs/evidence/2026-10-16-notifications.md)
- CEL-1-G: [tray](docs/evidence/2026-10-14-tray.md)

The project was registered by CEL-1-A (suite).
