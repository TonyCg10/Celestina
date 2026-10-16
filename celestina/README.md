# Celestina

The desktop shell of the suite for the author's niri session, in pure Rust on
Wayland: an Island on the top edge of every output that grows into pages, a
launcher, a notification server with banners, and a system tray, painted with
the Celestina visual language over the compositor's glass. It is built in
[CEL-1](docs/plans/active/2026-10-14-cel-1-island.md), whose units are landed:
today it draws one Island capsule per output that grows into the Activity,
Notifications and Tray pages, opens the launcher, serves notifications and the
tray when no other program owns those names, and answers its command verbs.
What Noctalia still does for the session is in
[the handover note](docs/handover.md).

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
binary (to `$prefix/bin/celestina`, with the style's font and icons under
`$prefix/share/celestina/`); see
[the production artifact contract](../docs/contracts/production-artifacts.md).
The one binary is the shell, its own client and its check:

```sh
celestina                          # the shell (a second one exits 0)
celestina --exit-after 3 --metrics # stop after 3 s and print the frame report
celestina --check                  # load the theme, the fonts and the appearance; exit
celestina island open activity     # forward a verb to the running shell
```

With a shell running, `celestina <verb>` calls `Command` on
`org.celestina.Shell1` (`/org/celestina/Shell`, name `org.celestina.Shell`)
and exits 0, or 1 when the shell is not running, refuses the verb or does
not answer within 3 s; an unknown verb exits 2 with the usage. The verbs are
`island toggle`, `island open <activity|notifications|tray>`,
`island close`, `launcher toggle`, `notifications toggle`,
`notifications dismiss-all` and `quit`; `launcher toggle` opens or closes the
launcher, and `notifications toggle` opens or closes the Notifications page and
`notifications dismiss-all` dismisses every notification. Bind them to keys in
niri with `spawn "celestina" "island" "toggle"`. An open page asks for the
keyboard on demand (`OnDemand`), and niri hands it the keyboard when the page
is clicked: Escape closes a page once it has been clicked, while a click
outside it closes it in any case. Bind `celestina island close` too, for a
page opened from a key.

Diagnostics go to stderr (the journal when the shell is activated) and
follow `RUST_LOG` (default `info`; `RUST_LOG=celestina=debug` for the
shell's own detail). `--no-blur` sends no blur region, to attribute frame
cost.

### The nested session

`scripts/dev-session.sh` runs the shell in a nested niri window with
`scripts/dev-session.kdl`, so it can be looked at without touching the live
session: `--own-bus` gives the nest a session bus of its own (the shell
claims its names there), `--binary PATH` names the binary (default the
release build), `--scale` and `--monitor` set the nested output's factor and
where the window goes, and `dev-session.sh --shell` opens a shell attached
to the running nest. `scripts/smoke.sh --binary PATH` runs the shell once in
such a nest with `--exit-after 3 --metrics` and checks the frame report.

## Project documents

- Current status: [STATUS.md](STATUS.md)
- Implementation roadmap: [ROADMAP.md](ROADMAP.md)
- Author validation: [VALIDATION.md](VALIDATION.md)
- Local agent delta: [AGENTS.md](AGENTS.md)
