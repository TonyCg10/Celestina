# Celestina

The desktop shell of the suite for the author's niri session, in pure Rust on
Wayland: an Island on the top edge of every output that grows into pages, a
launcher, a notification server with banners, and a system tray, painted with
the Celestina visual language over the compositor's glass. It is being built
in [CEL-1](docs/plans/active/2026-10-14-cel-1-island.md); today the binary is
a stub.

## User contract

- The Island, the launcher, the banners and the tray are the shell's whole
  surface; there is no dock ([ADR 0003](docs/decisions/0003-no-dock.md)).
- Surfaces take no keyboard at rest
  ([ADR 0007](docs/decisions/0007-focus-policy.md)); well-known bus names are
  claimed only when free, so the shell runs beside Noctalia
  ([ADR 0008](docs/decisions/0008-claim-free-names-only.md)).
- niri only; the compositor supplies the blur.

## Architecture

| Area | Responsibility |
|---|---|
| `crates/celestina-wayland` | Outputs, layer surfaces, popups, scale, regions, frame pacing, EGL (design §5.2) |
| `crates/celestina-paint` | The `Painter` contract over femtovg, the Haze material, text and icons (§5.3) |
| `crates/celestina-ui` | The node tree, taffy layout, keyed reconciliation, animation, hit-test, focus, the text field (§5.4) |
| `src/` | The product: Island, launcher, notifications, tray, and the providers on worker threads |
| `../celestina-rs/crates/celestina-shell` | The pure domain the product consumes |

The design is
[the shell design](../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md);
its decisions are [the decision records](docs/decisions/README.md).

## Build and use

The production entries under `scripts/` build, verify and deploy the release
binary; see
[the production artifact contract](../docs/contracts/production-artifacts.md).
Until CEL-1-B the binary accepts one argument:

```sh
celestina --check   # exit 0: the binary runs
```

From CEL-1-B the same binary is the shell and its own client: with a shell
running, `celestina <verb>` forwards the verb over `org.celestina.Shell1` and
exits. The verbs to come are `island toggle`,
`island open <activity|notifications|tray>`, `island close`,
`launcher toggle`, `notifications toggle`, `notifications dismiss-all` and
`quit`.

## Project documents

- Current status: [STATUS.md](STATUS.md)
- Implementation roadmap: [ROADMAP.md](ROADMAP.md)
- Author validation: [VALIDATION.md](VALIDATION.md)
- Local agent delta: [AGENTS.md](AGENTS.md)
