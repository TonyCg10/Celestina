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
- The lock duty (`duties.lock` in `~/.config/celestina/shell.json`) is `off` by
  default. When it is on, `celestina lock` starts `celestina-lock`, which holds
  the `ext-session-lock-v1` lock, covers every output in black at once and asks
  for the password on the output with the keyboard focus; `celestina lock
  suspend` locks and suspends only once the compositor confirms the lock. The
  idle chain's second step (lock, then suspend) works only with the duty on, and
  every other suspend waits for the lock through a logind delay inhibitor. The
  password is checked by PAM through a service the author installs, and nothing
  but the right password unlocks: every failure leaves the session locked. There
  is no `unlock` verb, and `session suspend` is always refused and points to
  `lock suspend`. The lock paints black with the field, the status words,
  Caps Lock and a fade to black after 30 s without input; the dimmed wallpaper
  and the clock are CEL-2-H-b. Install the PAM service and read the recovery
  path first: [Installing the lock's PAM service](#installing-the-locks-pam-service).
  Decisions: [ADR 0022](docs/decisions/0022-session-lock.md); evidence:
  [CEL-2-H-a](docs/evidence/2026-10-22-cel-2-h-a.md).
- niri only; the compositor supplies the blur.

## Architecture

| Area | Responsibility |
|---|---|
| `crates/celestina-wayland` | Outputs, layer surfaces, popups, scale, regions, frame pacing, EGL (design §5.2) |
| `crates/celestina-paint` | The `Painter` contract over femtovg, the Haze material, text and icons (§5.3) |
| `crates/celestina-ui` | The node tree, taffy layout, keyed reconciliation, animation, hit-test, focus, the text field (§5.4) |
| `src/` | The product: Island, launcher, notifications, tray, the bar and its pieces (including sysmon, weather and clock), and the providers on worker threads |
| `src/lock/` | The shell's side of the lock: the supervisor of `celestina-lock`, the logind delay-inhibitor worker, the header-only backdrop bundle and the notices |
| `crates/celestina-lock` | The lock binary: an `ext-session-lock-v1` client with covers, a CPU painter into `wl_shm`, the password field and the only unlock |
| `crates/celestina-lock-verify` | The PAM verifier, one short-lived process per attempt; `pam.rs` is its one FFI module |
| `../celestina-rs/crates/celestina-lock-proto` | The lock's dependency-free vocabulary: verdicts, exit codes, status lines, the bundle format, the delay schedule |
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
`brightness up|down|set <0-100> [output]`, `lock`, `lock suspend` and `quit`; `launcher toggle` opens or closes the
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

## Installing the lock's PAM service

The session lock is `celestina-lock`, with its PAM verifier
`celestina-lock-verify` beside it (both installed next to `celestina` by the
production entries). It is **off** until you set `duties.lock` to `on`; with
it on, `celestina lock` covers every output and `celestina lock suspend` locks
and then suspends. The lock never decides the password itself: it asks PAM,
through a service of its own, `celestina-lock`. That service is a file under
`/etc/pam.d`, and only you install it: no script, no production entry and no
agent does. Its text is [`contrib/pam.d/celestina-lock`](contrib/pam.d/celestina-lock):

```text
#%PAM-1.0
# Celestina session lock: the password, through the system stack.
auth      include   system-auth
account   include   system-auth
```

The one command, from the repository root:

```sh
sudo install -m 0644 -o root -g root celestina/contrib/pam.d/celestina-lock /etc/pam.d/celestina-lock
```

Without the file the lock still covers the screen (it is never left
uncovered), but it shows that the service is missing and nothing typed can
unlock it; the shell warns once when the duty is turned on. There is no
fallback to another service. The text deliberately does not include
`pam_nologin`, which `login` and `swaylock` do: `/run/nologin`, written before
a scheduled shutdown, would otherwise refuse every unlock.

The service shares the system stack, so it shares `pam_faillock`: three wrong
passwords within 15 minutes lock the account for 10 minutes (the defaults, in
`/etc/security/faillock.conf`), and during that time the right password is
refused too, here, at `sudo`, at `swaylock` and at a VT login. The lock adds
its own delay (2, 4, 8, 16, then 30 seconds) after each wrong password.

### If the lock does not let you in

`swaylock`, with its own `/etc/pam.d/swaylock`, stays installed and bound to
`Mod+Shift+A`; nothing here removes it.

1. From a stuck lock (alive but not unlocking), press `Ctrl+Alt+F3` (niri runs
   `ChangeVt` while locked) and log in on that console.
2. `cat "$XDG_RUNTIME_DIR/celestina/lock-wayland-1.state"` prints `pid <n>`
   (`XDG_RUNTIME_DIR` is `/run/user/<uid>`; the display name is the session's,
   see `ls /run/user/<uid>/wayland-*`). Without the file, read the pid of
   `celestina-lock` from `ps -eo pid,args`.
3. `kill -KILL <n>`, by that pid only, never by name. `SIGTERM` is ignored on
   purpose.
4. The session stays locked with a dead client. If the shell is running it
   starts the lock again within a second, and `Ctrl+Alt+F<session>` shows the
   prompt. Otherwise, from the console:
   `WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/<uid> swaylock -c 000000`
   (check the display name as above): niri accepts it as the replacement of the
   dead lock and `swaylock`'s own PAM service unlocks.

**Last resort: when your own password path is closed.** The console login,
`swaylock` and `sudo` all go through `pam_faillock`, and the first two also
through `pam_nologin`; so during a `faillock` lockout, or while `/run/nologin`
exists, steps 1 and 4 are closed for the same user too. Then the way back is
**root**, on a console (`Ctrl+Alt+F3`) or over `ssh` where you allow it:

- `faillock --user <name> --reset` clears the tally at once (the default
  configuration does not lock root out: no `even_deny_root`); otherwise wait
  the unlock time (10 minutes by default).
- With `/run/nologin`, cancel the scheduled shutdown (`shutdown -c` as root) or
  wait for it.

Keep a root login (a console or an `ssh` key) available for as long as
`swaylock` is your fallback; nothing in this unit can prepare one for you. The
live validation row `VAL-CEL2-LOCK-RECOVERY` checks you know which one you
have.

### What the lock does not protect against

Recorded so nobody assumes otherwise (the author accepted these on
2026-10-22; they are not mitigated here):

- Any process running as your own user can end any lock on this compositor,
  ours or `swaylock`'s: it can `SIGKILL` the lock (the same uid is enough) and
  niri lets any client of your Wayland socket replace a *dead* lock and unlock
  it. The `faillock` tally file is yours (mode 0660), so the same process can
  also reset it, and a virtual keyboard can type guesses into the lock.
- Keys typed on the lock can reach other clients of your session before the
  lock sees them. An input method that grabs the keyboard
  (`zwp_input_method_v2`, for example fcitx5 on niri) keeps its grab across the
  lock and can take a new one while locked, and in a session instance niri
  serves `org.freedesktop.a11y.KeyboardMonitor`, which emits every key to any
  client that watched the keyboard, with no check for the lock. Neither path
  can be closed from the lock: closing them needs a change in niri. If you run
  an input method, treat the password you type at the lock as visible to it.

The lock's rehearsal, `scripts/lock-rehearsal.sh`, runs only in a nested niri
(`dev-session.sh --own-bus`) against a fake logind on a private bus and a PAM
service that does not exist, and refuses to start if the lock of a live session
is running; it never touches your session. See its header for the steps.

`celestina-lock` itself refuses to connect unless the Celestina shell started
it or the compositor at the other end is a nest started by `dev-session.sh`;
`--rehearsal` and the probes never trust the shell's marker: they need that
nest, or a `WAYLAND_SOCKET` that a test harness handed in and whose other end
is the harness itself (their parent) or such a nest. Any other socket is
refused. Run by hand against your
session (for example from a terminal with `WAYLAND_DISPLAY` set) it exits 15
with a message and has sent nothing to the compositor. This is a guard against
accidents, not a security boundary: a process that can run programs as you can
forge what it checks. To rehearse, use `lock-rehearsal.sh`, or a shell attached
to a nest (`dev-session.sh --shell`) and `env -i` with the nest's own
`WAYLAND_DISPLAY`; never export the session's display to a lock client (on
2026-10-22 a rehearsal run that way locked the real session and the author had
to restart).

## Project documents

- Current status: [STATUS.md](STATUS.md)
- Implementation roadmap: [ROADMAP.md](ROADMAP.md)
- Author validation: [VALIDATION.md](VALIDATION.md)
- Local agent delta: [AGENTS.md](AGENTS.md)
