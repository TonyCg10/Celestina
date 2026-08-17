# Seven repairs the first live migration asked for

- **Date:** 2026-08-17
- **Scope:** the repairs the roadmap lists as `LIVE-1-A` through `LIVE-1-G`
- **Artifact:** the shell, the provider adapter, `celestina-shell-core` and
  `scripts/build-patched-niri.sh`
- **Environment:** static reading, the repository's own suites and one
  deliberate revert-and-rerun to prove a new guard; nothing was launched on the
  author's session and the nest was closed throughout
- **Plan:** none yet — `LIVE-1` is planned in [ROADMAP.md](../../ROADMAP.md)
  and cannot open its plan while `LOCK-1` holds the one active-plan slot, so
  these repairs land as maintenance ahead of the checkpoint that will own them
- **Validation:** `VAL-R8`

## Procedure

Each finding from
[the investigation](2026-08-17-live-session-investigation.md) was read against
the code before it was repaired, because two of them turned out to be wrong as
first stated. Every repair was then built, and the one whose defect was
invisible from outside its class was proved by reverting the fix and requiring
its new test to fail.

## Result

### One reported finding was wrong, and one nobody reported was real

The investigation's first critical claim — that `AppIconProvider` resolves
icons on Qt Quick's image-loader thread — does not hold. `Image.asynchronous`
is unset at the only call site (`qml/WorkspaceMapTile.qml`), its default is
false, and Qt therefore calls a non-asynchronous `QQuickImageProvider` on the
GUI thread. The header's long-standing comment was right and the finding was
not; no threading change was made.

Reading the class to establish that exposed a different defect, which nobody
had reported and which is real. The provider cached its answers but chose
whether to resolve by asking whether the *image* was null — a test that cannot
distinguish a cached miss from a name never looked up. Every miss therefore
resolved again on every frame that drew it, and a miss is the most expensive
lookup there is: `QIcon::fromTheme` walks every installed theme's directories
before concluding nothing is there. The author's workspace map, holding two
applications no theme knows, paid that walk twice per frame for as long as it
was open — on the GUI thread, while the map animated, seconds before the
session died.

### The repairs

- **`LIVE-1-A`** decides on whether the answer was *found*, not on whether it
  was null. `celestina-appicon-provider` proves it by counting searches rather
  than timing them, and the count is exposed for exactly that reason: a first
  attempt at this test measured elapsed time and **passed against the defect**,
  because on a machine with few themes installed the wasted walk hides inside
  timing noise. Reverting the fix now fails the test.
- **`LIVE-1-B`** clamps the quiet-surface size follower to a one-pixel floor
  and stops it entirely once a surface is retiring. These placements anchor one
  edge per axis, where layer-shell makes a zero extent a protocol error that
  kills the whole client — not merely the surface.
- **`LIVE-1-C`** withdraws the compositor effect before the surface dies on
  both hard-close paths, `OverlaySurface::close` and `PanelMenuSurface::close`,
  which previously hid and deleted a window with its blur object still live.
  KWindowSystem tears that object down from `surfaceDestroyed` via
  `deleteLater`, so its destroy reached the compositor a whole event-loop pass
  after the `wl_surface` was gone. Upstream niri #3660 is that exact sequence
  against this same Qt and KWindowSystem stack, reported as a fatal protocol
  error. `softCloseWindow` already ordered this correctly; the hard paths did
  not.
- **`LIVE-1-D`** arms a dense companion's region *before* showing it, in both
  the creation path — where `mapLayerSurface` ends in `show()` — and the
  refresh path, which showed first and armed second. A mapped surface with no
  region is read by the compositor's per-namespace rule as an effect over its
  whole geometry, which is the whole output; `R8-P-I` had already measured that
  at about x1.95 saturation for three resting companions and fixed only the
  resting case. It also drops companions belonging to an output that has gone
  away: they were keyed by raw `QScreen *`, so an unplugged monitor left a
  dangling key whose windows Qt reassigned to a surviving screen, where they
  kept applying the rule.
- **`LIVE-1-E`** journals which precondition failed whenever a surface's blur
  does not arm, with its output name. The positive record said nothing about
  the surfaces that never armed, which is precisely the case on the monitor
  that behaves differently from its neighbours.
- **`LIVE-1-F`** makes the nest refuse rather than lie. It now parses the
  session's commit from `niri --version`, fetches the release tag explicitly —
  `v26.04`, which is why asking for `26.04` had always failed — and **verifies
  the checked-out commit is the session's**, refusing to build anything else.
  The previous script fell back to cloning the default branch when that fetch
  failed, silently, every time.
- **`LIVE-1-G`** replaces the hardcoded 2700 K with a bounded setting.
  `Whitepoint::for_temperature` generalizes wlsunset's `calc_whitepoint`
  without changing its mathematics, `warm_2700k` is now defined in terms of it,
  and four tests hold the contract: the default reproduces the former constant
  exactly, warmer really means less blue, out-of-range values clamp rather than
  refuse the settings file, and the coolest offered temperature is the
  identity. A `night-light-temperature` verb carries `kelvin`, and the worker
  reads the preference at each transition so a change while the light is on
  lands without a toggle.

### Verification

CTest 25/25 including the two new suites, 333 `celestina-shell-core` tests,
QML lint, and the architecture, contrast and visual contracts all pass.

## Limits

None of this was run on the author's session, and none of it is proof that the
crash is fixed: the exact protocol error was never captured, so `LIVE-1-C`
repairs the best-fit mechanism rather than a measured one. The per-monitor
divergence is *instrumented* by `LIVE-1-E` and not yet corrected — the
per-output scale and anchor derivation it was written to expose has not been
changed, because the records that would say what to change do not exist yet.
`LIVE-1-F` was not exercised against a real rebuild; it is read-correct and
syntax-checked only, and the nest on disk is still the `main` build it
produced. Whether the session adopts the patched newer compositor or the shell
learns to degrade on the release remains the author's open decision.
