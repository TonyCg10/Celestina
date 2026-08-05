# R5 — control centre, session menu, weather and calendar

- **Opened:** 2026-08-04
- **Plan ID:** r5-control-centre
- **Closed:** 2026-08-04
- **Successor:** none; the roadmap is idle until a later checkpoint opens its own plan
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** R5
- **Author-validation checkpoint:** `VAL-R5` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

One surface can write to every provider the panel already reads from without
inventing a second source of truth: each control shows what its provider last
reported, sends a typed request, and reports what came back — never what it
asked for.

## Tangible outcome

A keyboard-driven control centre that toggles or steps network, Bluetooth,
night light, caffeine, do-not-disturb, power profile, volume and brightness,
shows each request's outcome, offers typed session actions, and carries a
bounded weather reading and a local month calendar. Settings survive a restart
because they were written durably before anything published them.

## Scope

In scope: the settings schema, its bounds and the durable-write rule; the
control-centre surface over the existing providers and the existing session
verbs; typed session actions with visible outcomes; a bounded Open-Meteo policy
and cache; a local calendar month view.

## Exclusions

Out of scope: new provider capabilities beyond what R1-R4 already publish;
per-application notification policy; a network or Bluetooth *manager* — this
shell asks the tools the session already has and shows what they report;
sending anything to a weather service beyond one coordinate pair; and any live
Niri configuration change.

## Build order

1. Add the settings schema, bounds and the write-then-publish rule to
   `celestina-shell-core`, with tests.
2. Persist and republish settings from the aggregate provider runtime, writing
   durably before the new value is visible anywhere.
3. Add the control-centre surface: read from providers, write through existing
   verbs, and show each request's outcome.
4. Add typed session actions with visible outcomes.
5. Add the bounded Open-Meteo policy and cache, then the local month calendar.

## Implementation exit

- Settings schema, bounds, round-trip and durable-write tests pass, including a
  write that fails and leaves the previous value intact.
- Every control shows provider-reported state, and no control paints the value
  it requested.
- Weather policy is bounded and cached, and a failed or absent reading is
  visible as absent rather than as stale.
- Calendar month arithmetic is tested without a clock.
- CMake registration, QML lint and CTest pass.
- Rust format, Clippy and package tests pass; the lockfile changes only by a
  dependency this plan declares.
- The architecture and documentation contracts pass.
- `scripts/complete-production.sh` builds once, verifies those exact bytes and
  updates the on-disk bundle; the live session is never replaced.

R5 implementation closes on this evidence. Real network and Bluetooth
switching, a real weather location, appearance and assistive-technology
behaviour remain an independent `VAL-R5` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R5-A | `celestina:` | done | [inventory](../../inventories/2026-08-04-r5-control-centre/R5-A.numstat.tsv) | 41 files, +2804/-42 | The settings schema and its write-then-publish rule; durable writes and restored choices; the control centre over existing providers and verbs; typed session actions asked twice; and a bounded weather reading beside a computed month | [R5 control centre](../../evidence/2026-08-04-r5-control-centre.md) | `VAL-R5` |

The five build-order steps closed as one unit, as R3's and R4's did: each `done`
unit needs one exclusive inventory *and* one exclusive evidence record, and one
verification run does not honestly produce five.

## Decisions and rollback

The control centre writes through the verbs R3 already delivered and reads the
providers R1-R4 already publish. It adds no second path to a device: a control
that sent its own command would be a second source of truth about the same
hardware, which is exactly what the confirmed-state contract exists to prevent.

A control never paints what it asked for. It shows what its provider last
reported and, separately, whether a request is pending, confirmed or failed —
the same distinction the session channel already makes, for the same reason: a
switch that flipped on click would be lying whenever the write failed.

The durable write reuses `celestina_core::atomic_file::replace`, which the
suite already had: temporary sibling, fsync, rename, fsync of the directory. A
second copy of that recipe here would have been a second thing to get right.

There is one writer of the settings file. Night light, caffeine and
do-not-disturb are live states owned by the providers that hold them, and each
records its choice through `settings::remember` *after* the session really
changed — a preference that persisted while the change itself failed would be a
promise nothing kept. A failed write does not roll the live state back either:
what the person asked for is happening, and only its survival past this session
failed.

A settings file this shell cannot read is left exactly as it is and the session
runs on defaults. Overwriting it would destroy a hand-edit or a newer schema in
the name of tidiness, and the person would never learn what happened.

R5-D types the four requests a person cannot take back. `power-off` and
`reboot` must never be what a mistyped verb falls through to, so each is its own
variant and near misses — `power`, `shutdown`, `power-off-now` — are refused.
The surface asks twice: the first press arms and says what will happen, the
second sends, and Escape disarms before it dismisses.

Log out is the compositor's own quit; restart and power off are asked of logind
with interactive authentication disabled, because a shell cannot answer a polkit
prompt and a request that would hang on one must fail visibly instead. Whatever
logind answers is the outcome — this shell never assumes the machine is going
down. Suspend is refused by the shell itself while no locker exists, the same
fail-closed seam as `lock`: a session that suspends unlocked wakes up unlocked.

Writing those verbs revealed that `parse_for` checked only whether a verb was
*known*, not whether the named provider serves it — so the audio provider would
have accepted `reboot`. Who serves what is now one fact in the core, and the
providers that each repeated that check no longer carry their own copy.

Weather is the one thing here that leaves the machine. It sends one coordinate
pair to Open-Meteo and nothing else, caches what comes back, and shows an
absent reading as absent rather than as a stale one. The author's location is a
setting, never a lookup. The request goes through `curl`, the way every other
provider here uses a tool the session already has: linking a TLS stack into the
shell for one small GET would be a large dependency for a small thing.

Coordinates are rounded to two decimals — about a kilometre — before they
leave. A panel's weather does not improve with more precision, and more
precision is a more exact answer to "where is this person".

A reading stops being shown at the same moment it stops being current, so the
widget is absent rather than carrying a temperature from four hours ago. That
is stricter than the refresh rule on purpose: a pending retry must never leave
a stale number on screen.

The calendar needs no provider, no service and no permission — a month's shape
follows from a rule that has not changed since 1582 — so it is computed. The
month arithmetic is tested against known dates rather than against itself.
