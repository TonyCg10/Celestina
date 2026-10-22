# Celestina

The desktop shell of the suite for the author's niri session, in pure Rust on
Wayland: an Island on the top edge of every output that grows into pages, a
launcher, a notification server with banners, and a system tray, painted with
the Celestina visual language over the compositor's glass. It is built in
[CEL-1](docs/plans/archive/2026-10-14-cel-1-island.md), whose units are landed:
today it draws one Island capsule per output that grows into the Activity,
Notifications and Tray pages, opens the launcher, serves notifications and the
tray when no other program owns those names, and answers its command verbs.
What Noctalia still does for the session is in
[the handover note](docs/handover.md).

## User contract

- The Island, the launcher, the banners and the tray are the shell's surfaces,
  with the wallpaper when its duty is on; there is no dock
  ([ADR 0003](docs/decisions/0003-no-dock.md)).
- The wallpaper duty (`duties.wallpaper` in `~/.config/celestina/shell.json`)
  is `off` by default. When it is on, the shell paints one still image per
  output from the folder `wallpaper.folder` names, on a `celestina-wallpaper`
  surface in the Background layer, scaled by `wallpaper.mode` (`fill`, `fit`,
  `stretch` or `center`). An output that another client holds in the
  Background layer, such as Noctalia's wallpaper, gets no surface and the
  shell logs the holder. The shell checks the holder only when the duty turns
  on, when an output appears and on `celestina wallpaper retry`; a holder that
  arrives later is not detected. Accepted formats are PNG, JPEG and WebP; lossless
  WebP is refused. A source is at most 8192 px a side and 40 MP, and the decode
  needs at most about 322 MiB.
- The glance pieces `sysmon`, `weather` and `clock` are in no default zone.
  Place them by id in the `bar` zones of `~/.config/celestina/shell.json`,
  for example `{"bar": {"end": ["sysmon", "weather", "clock", "tray"]}}`
  (zones are `start`, `centre` and `end`). `sysmon` shows `CPU n% · RAM m%`,
  coloured at 80 % and 90 %, and opens the Sistema page; `clock` shows the
  short date and opens the Reloj page (a Monday-first month calendar with
  Previous, Today and Next, and the weather reading); `weather` shows the
  temperature and opens the Reloj page, and is absent without a fresh reading.
  The Spanish condition words are fixed by Open-Meteo's code:

  | Code | Words shown |
  |---|---|
  | 0 | Despejado |
  | 1 | Mayormente despejado |
  | 2 | Parcialmente nublado |
  | 3 | Nublado |
  | 45, 48 | Niebla |
  | 51 to 57 | Llovizna |
  | 61 to 67 | Lluvia |
  | 71 to 77 | Nieve |
  | 80 to 82 | Chubascos |
  | 85, 86 | Chubascos de nieve |
  | 95 to 99 | Tormenta |
  | any other | Sin determinar |

- The weather is the only thing in the shell that uses the network, and only
  with a manual location: `"weather": {"latitude": 40.42, "longitude": -3.70,
  "label": "Madrid"}` in `shell.json`. Without a valid location there is no
  request, no `curl` and no piece content; the shell never works out where the
  person is, and a reading older than 15 minutes is hidden
  ([ADR 0016](docs/decisions/0016-weather-is-the-only-network-and-manual.md)).
- The clipboard duty (`duties.clipboard`) is `off` by default. When it is on,
  the shell keeps a history of the regular selection (never the primary one):
  up to 200 texts of 256 KiB and 20 PNG images of 8 MiB, for seven days. A
  selection that offers `x-kde-passwordManagerHint` is skipped unread; no
  other heuristic exists, so a secret copied without the hint is kept. Text is
  persisted to `$XDG_STATE_HOME/celestina/clipboard.json` (mode 0600) and the
  file is deleted when the duty goes off; images stay in memory. A shell
  whose clipboard duty is off (the default, also in a nest or a script that
  does not name its own state home) therefore deletes
  `$XDG_STATE_HOME/celestina/clipboard.json` at start: scripts that start the
  binary export a throwaway `XDG_STATE_HOME`. The Island page `clipboard` and
  the bar piece show previews; the verbs `clipboard select N [S]`, `remove N
  [S]`, `clear`, `pause` and `resume` are answered `ok` by the bus as soon as
  the worker has queued them, and how they ended (a stale snapshot `S`, a
  missing entry) is told to the journal only; no verb returns an entry.
- The OSD duty (`duties.osd`, `off` by default) shows a short on-screen
  display for volume, microphone and brightness changes on a `celestina-osd`
  overlay surface that exists only while it shows, and gives the `volume` and
  `brightness` bar pieces their readings. Audio is observed through
  `pactl subscribe` and read with `wpctl`; brightness through sysfs and,
  lazily, `ddcutil` (never with `CELESTINA_DDC=0`). The lock-keys OSD is not
  delivered. With the duty off, the `volume`, `microphone` and `brightness`
  verbs are rejected. See
  [the evidence](docs/evidence/2026-10-17-osd.md).
- The media duty (`duties.media` in `~/.config/celestina/shell.json`) is `off`
  by default. When it is on, the shell follows the MPRIS players on the session
  bus and shows the media piece and the Medios page: the shown player's cover,
  title and artist, with previous, play/pause and next. A pin chosen on the page
  lasts for the session only. Keys stay with `playerctl`.
- Surfaces take no keyboard at rest
  ([ADR 0007](docs/decisions/0007-focus-policy.md)); well-known bus names are
  claimed only when free, so the shell runs beside Noctalia
  ([ADR 0008](docs/decisions/0008-claim-free-names-only.md)).
- The phone and bubbles duties (`duties.phone` and `duties.bubbles` in
  `~/.config/celestina/shell.json`) are `off` by default. When on, the shell
  reads the phone from Magnetita and the minimised windows from Melibea's
  socket; it never starts either service. Evidence:
  [CEL-2-J](docs/evidence/2026-10-17-phone-and-bubbles.md).
- niri only; the compositor supplies the blur.

## Architecture

| Area | Responsibility |
|---|---|
| `crates/celestina-wayland` | Outputs, layer surfaces, popups, scale, regions, frame pacing, EGL (design §5.2) |
| `crates/celestina-paint` | The `Painter` contract over femtovg, the Haze material, text and icons (§5.3) |
| `crates/celestina-ui` | The node tree, taffy layout, keyed reconciliation, animation, hit-test, focus, the text field (§5.4) |
| `src/` | The product: Island, launcher, notifications, tray, the bar and its pieces (including sysmon, weather and clock), and the providers on worker threads |
| `../celestina-rs/crates/celestina-shell` | The pure domain the product consumes |

The design is
[the shell design](../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md);
its decisions are [the decision records](docs/decisions/README.md).
The bar that replaces the capsule-only Island on each output, and its pieces,
are in progress under the parity program: [ADR 0012](docs/decisions/0012-bar-one-surface-per-output.md)
and [ADR 0013](docs/decisions/0013-everything-is-a-piece.md).

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
`island toggle`,
`island open <activity|notifications|tray|sysmon|clock|clipboard|network|bluetooth|power|phone|media>`,
`island close`, `launcher toggle`, `notifications toggle`,
`notifications dismiss-all`, `wallpaper set <output|all> <file>`,
`wallpaper next [output]`, `wallpaper prev [output]`, `wallpaper mode <mode>`,
`wallpaper retry`, `clipboard select|remove N [S]`, `clipboard clear|pause|resume`,
`volume up|down|mute|set <0-100>`, `microphone mute|set <0-100>`,
`brightness up|down|set <0-100> [output]` and `quit`; `launcher toggle` opens or closes the
launcher, and `notifications toggle` opens or closes the Notifications page and
`notifications dismiss-all` dismisses every notification. `wallpaper set` and
`wallpaper next`/`prev` choose the picture of one output (or of all, for `set`)
from the folder's sorted image files; `wallpaper retry` checks the outputs
again, for example after another client has released one. A choice is written
to `shell.json` before it is applied. Bind them to keys in
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
