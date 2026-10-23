# Celestina implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** CEL-2
- **Related author validation:** VAL-CEL2-BAR-LIVE (it does not block); the CEL-1 rows stay queued: VAL-CEL-GLASS, VAL-CEL-ISLAND, VAL-CEL-REST,
  VAL-CEL-POPUP, VAL-CEL-KEYBOARD, VAL-CEL-IME, VAL-CEL-SCALE,
  VAL-CEL-NOCTALIA and VAL-CEL-BANNERS in [VALIDATION.md](VALIDATION.md) (they do not block);
  the CEL-2-J rows VAL-CEL2-PHONE, VAL-CEL2-BUBBLES and VAL-CEL2-BUBBLES-KEYS are pending too;
  the CEL-2-H-a rows VAL-CEL2-LOCK-PAM, -LIVE, -SUSPEND, -HOTPLUG, -RECOVERY, -NOTICES and -COEXIST are pending

## CEL-2 — Shell parity with Noctalia

**Hypothesis:** a Rust shell with the piece contract of
[the parity design](../docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md)
can take over, duty by duty and beside a running Noctalia, everything Noctalia
does in the author's niri session. **Plan:**
[CEL-2](docs/plans/active/2026-10-16-cel-2-parity.md), thirteen units
(CEL-2-A to CEL-2-M) in the plan's order; its ledger is the source of each
unit's status and exit.

**CEL-2-J (implemented in its worktree, not landed):** the phone and bubbles
duties, both off by default; evidence in
[docs/evidence/2026-10-17-phone-and-bubbles.md](docs/evidence/2026-10-17-phone-and-bubbles.md).

**CEL-2-H-a (implemented in its worktree, not landed):** the session lock's
security core, behind `duties.lock`, off by default; the ledger row is `active`.
Agent evidence is [docs/evidence/2026-10-22-cel-2-h-a.md](docs/evidence/2026-10-22-cel-2-h-a.md):
the table of every automated test id of the design with its result and command,
the nest rehearsal R-1 to R-9, the dependency closure, the `ldd` of both
binaries, the `unsafe` count, the spike answers, the residuals the author must
accept and what did not run. Reviews A and B are open. The decisions are
[ADR 0022](docs/decisions/0022-session-lock.md). CEL-2-H-b (the dimmed
wallpaper and the clock) starts after H-a has landed.

**Closed program, kept below:** CEL-1.

## Hypothesis and tangible outcome (CEL-1, closed)

A pure Rust shell on Wayland for niri can replace the halted Qt/QML shell:
one process draws an Island on the top edge of every output that grows into
pages, a launcher, notification banners and a tray, painted with the
Celestina visual language over the compositor's glass. The plan is
[CEL-1](docs/plans/archive/2026-10-14-cel-1-island.md); the design is
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
| CEL-1-H | done | CEL-1-E, F, G | The exit: the live rows queued in `VALIDATION.md` (the author runs them), the handover note on what Noctalia still does ([handover](docs/handover.md)), the documents up to date; the version is set by the landing's `--kind` | `celestina/docs/evidence/2026-10-16-exit.md` |

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
- CEL-1-H: [exit](docs/evidence/2026-10-16-exit.md)

The project was registered by CEL-1-A (suite).
