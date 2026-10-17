# CEL-2-B — Energy and comfort: implementation plan

- **Date:** 2026-10-16
- **Status:** approved and executed by the author's go-ahead of 2026-10-16 (table of §9 as written; D2 as recommended). Kept as the unit's plan record; where it differs from what was built, [the evidence](2026-10-16-energy.md) is authoritative
- **Artifact:** none; a plan record, not a build
- **Scope:** unit CEL-2-B, as planned before implementation
- **Environment:** none; nothing in this record was run
- **Unit:** `CEL-2-B` of the plan ID `cel-2-parity` (ledger
  `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`, created by
  CEL-2-A; this unit only adds its own row)
- **Design source:** the parity program (`docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md`, on `main`)
  §5 row CEL-2-B, §4.1 to §4.5 and §7 "B"; the base design is
  the shell design (`docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`)
- **Worktree:** `scripts/worktree.sh open celestina CEL-2-B` (base
  `origin/main` at `68fd8959`, shell 1.0.0)
- **Commit prefix:** `celestina:`; kind chosen at landing (`milestone`)

## 1. Goal

The shell takes over four duties of Noctalia's, each behind its own switch in
`shell.json` and **off by default**, so nothing fights Noctalia or the niri
startup line until the author turns a duty on:

| Duty | What the shell does when `on` | What it replaces |
|---|---|---|
| `idle` | Two `ext-idle-notify` timers: the outputs off by niri's action `power-off-monitors` (20 s) and the lock-and-suspend step (240 s, see D2) | Noctalia's idle settings |
| `caffeine` | Holds a logind `idle:sleep` block inhibitor and pauses the idle chain while it is on | `noctalia msg caffeine-enable` in the startup line |
| `nightlight` | A forced 2700 K whitepoint through `wlr-gamma-control`, one controller per output, 300 ms transition | `noctalia msg nightlight-force-toggle` in the startup line |
| (none) | The session verbs `session suspend|reboot|power-off|log-out`, and the `caffeine`/`nightlight`/`idle` verbs | the verbs the keys call in Noctalia |

The session verbs are explicit commands, not a standing duty, so they need no
switch (a verb the author did not type never runs); they are the only part of
this unit that can end the session, and are tested only against a fake logind
on a private bus.

## 2. Decisions this plan fixes (the spec's §7 "B" questions)

- **D1 — Idle's lock timer.** The idle chain is a pure state machine with two
  steps, `ScreenOff` and `LockAndSuspend`. Until CEL-2-H lands a locker,
  `LockAndSuspend` **fails closed**: the step is reported (`tracing` line and
  the `idle` status) as "refused: no locker" and does nothing. A session that
  suspends unlocked wakes unlocked (the halted shell's `PowerAction::Suspend`
  rule). CEL-2-H wires the step; B ships the seam (`IdleAction::LockAndSuspend`
  and a `Locker` availability flag that is `false`).
- **D2 — Explicit `session suspend`.** Same rule as D1: refused with a stated
  reason while no locker exists. **Open question for the author (§9).**
- **D3 — Gamma ownership with three outputs.** One worker with its own Wayland
  connection (the halted adapter's shape): a controller per output, created
  only while the effect is on, rebuilt with the halted backoff
  (1 s doubling to 30 s) when an output fails, destroyed on switch-off, on
  hot-unplug and on exit so the compositor restores its own gamma. A
  controller the compositor refuses (`failed`, as on the nested `winit`
  output or when Noctalia holds it) leaves that output neutral and the duty
  reports "not applied" — never a half state.
- **D4 — Persistence of forced night light and caffeine.** Both are
  switches the person flips, persisted in `shell.json` with the durable-before-
  published rule of `Store` (do-not-disturb's pattern): `energy.night_light`
  (bool, default `false`), `energy.night_light_kelvin` (2000..=6500,
  default 2700), `energy.caffeine` (bool, default `false`). The author sets
  the two bools to `true` to reproduce the startup line; a toggle verb
  rewrites them. The duty switch decides whether the shell *acts* on them.
- **D5 — Compositor actions go through niri's IPC.** `power-off-monitors` and
  log out (`Quit`, with niri's own confirmation) reuse the niri provider's
  bounded request queue (`NiriRequest`), not a second socket path. Reboot,
  power off and suspend are logind `Manager` calls (`interactive = false`).
- **D6 — Caffeine is a logind inhibitor fd, not a child process.**
  `Inhibit("idle:sleep", "Celestina", reason, "block")` over blocking `zbus`;
  the returned fd is the lock, dropped to release. No `systemd-inhibit`
  helper. A client's `zwp_idle_inhibit` is honoured by the compositor's
  `ext-idle-notify` already (spec §3); caffeine additionally stops our own
  chain from acting.
- **D7 — `pending.rs` is not copied.** Session verbs are answered with a typed
  outcome on a bounded reply channel; there is no provider envelope to settle.
  The salvage list records what *was* copied.

## 3. Salvage by copy (needs the author's edit first)

The root `AGENTS.md` salvage list does not yet name these files. Per spec
§4.3 and AGENTS.md ("the author's edit … extends the salvage list **before**
that unit's first commit"), **the author edits that section** to add:

- `celestina-rs/crates/celestina-shell-core/src/{nightlight,session}.rs`
  into `celestina-rs/crates/celestina-shell/src/`;
- `celestina-qt/src/provider_adapter/{nightlight,sessionholds}.rs` as
  reference for `celestina/src/energy/` (`session.rs` of the adapter is the
  connectivity provider, which is CEL-2-F's, so it is **not** used here).

Until then the copies exist in the worktree uncommitted. Every copied module's
head comment records the source path and the commit it came from
(`c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413` for the adapter paths,
`7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9` for the core, as the CEL-1 copies
did; re-checked with `git log -1 -- <path>` by the task).

## 4. File map

```text
celestina-rs/crates/celestina-shell/src/
  nightlight.rs   copy: Whitepoint, transition frames, gamma_ramp (pure)
  session.rs      copy, trimmed: Switch, PowerAction (+ tests)
  idle.rs         NEW pure: the idle chain state machine (D1)
  settings.rs     additive: `duties` table (idle, caffeine, nightlight) and
                  `energy` table (D4); clamps, defaults, round-trip tests
  lib.rs          three `pub mod` lines and the docs bullets

celestina/src/energy/            ONE new module, one `pub mod energy;` in lib.rs
  mod.rs          EnergyEvent, EnergyHandle (commands in, status out), wiring
  idle.rs         worker: own Wayland connection, ext-idle-notify v1, feeds the
                  pure chain, performs its actions
  nightlight.rs   worker: own Wayland connection, wlr-gamma-control (D3)
  logind.rs       worker: Manager.Inhibit/Suspend/Reboot/PowerOff on zbus
                  blocking, CanXxx probes, fake-logind tests on a private bus
  verbs.rs        the `energy` verbs: parse + bounded reply (no bus code)

celestina/src/command.rs   additive: Verb::{Caffeine(Switch), NightLight(Switch),
                  Idle(Switch)?, Session(PowerAction)} + USAGE lines
celestina/src/message.rs   additive: `Message::Energy(EnergyEvent)`
celestina/src/app.rs       additive: construct the handle, route Verb and
                  Message::Energy; no other change
celestina/src/copy.rs      Spanish strings for the status/refusal lines only
celestina/Cargo.toml       additive: wayland-client, wayland-protocols (+staging),
                  wayland-protocols-wlr (client), rustix (fs for memfd)
```

Shared points (`app.rs`, `Cargo.toml`, `Cargo.lock`, `command.rs`, `message.rs`,
`settings.rs`, the ledger, `STATUS.md`) get small additive hunks so the rebase
`git rebase --onto origin/main <previous tip>` stays trivial. `duties` in
`settings.rs` is also touched by CEL-2-A (`bar`); whoever lands second merges
the one struct by hand.

## 5. Tasks

Each task is one commit-sized step on the unit branch (uncommitted until the
author asks; the session branch carries one unit). Every task ends with
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -D warnings`
and the tests it added. Tests never touch the session bus, the session's
Wayland display, logind, or `niri msg`.

**T1 — Pure salvage and settings** (`celestina-shell`).
Copy `nightlight.rs`, trim `session.rs` to `Switch` and `PowerAction`
(drop `SessionRequest` and its payload parsing), add the head comments, wire
`lib.rs`. Extend `settings.rs` with `Duties { idle, caffeine, nightlight }`
(enum `Duty { Off, On }`, default `Off`, unknown value → `Off`) and
`Energy { caffeine, night_light, night_light_kelvin }`; clamp kelvin to
`Whitepoint::MINIMUM_KELVIN..=MAXIMUM_KELVIN`; keep the schema compatible
(missing key = today's value, `schema_version` unchanged); round-trip, bound,
"not ours" and hostile-file tests.

**T2 — The idle chain** (`celestina-shell/src/idle.rs`, pure).
`IdleChain::new(IdleConfig { screen_off, suspend })`; inputs: `Idled(Step)`,
`Resumed(Step)`, `Caffeine(bool)`, `Duty(bool)`, `LockerAvailable(bool)`;
outputs: `Vec<IdleAction>` (`PowerOffMonitors`, `LockAndSuspend`,
`Refused(NoLocker)`, `Rearm`). Properties to test: caffeine on → no action and
the idle notifications stay armed; caffeine off while already idle → the due
step fires once, not twice; a resume before the second timeout cancels it;
duplicate `Idled` events do not repeat an action; D1's refusal is reported
once per idle period; configs are bounded (screen_off ≥ 5 s, suspend >
screen_off, ≤ 24 h).

**T3 — Night-light worker** (`celestina/src/energy/nightlight.rs`).
Port the halted adapter's `nightlight.rs` onto this shell's `Worker` and
message shape, keeping: one worker owning every proxy and fd; a controller per
output only while on; the gamma ramp over `memfd`; the transition frames; the
backoff; `failed` handling; fail-dark (neutral) on any error; teardown on
shutdown. Drop the halted `ProviderRuntime`/`Payload` envelope; publish
`EnergyEvent::NightLight(Applied | NotApplied(reason) | Off)`. Commands go
over a bounded `sync_channel` (capacity 4). Tests: ramp bytes and frame
count (pure, already in T1's module), the controller state machine with a
fake `Transport` trait (no socket), and a test that a refused controller
leaves the output neutral.

**T4 — The idle worker** (`celestina/src/energy/idle.rs`).
Own Wayland connection; bind `ext_idle_notifier_v1` and `wl_seat`; create the
two notifications from the chain's config; translate `idled`/`resumed` into
chain inputs and perform the actions: `PowerOffMonitors` through
`NiriRequest`, never a `niri msg` process. Rebuild the notifications when the
config or the duty changes. Missing global → the duty reports "unavailable"
and does nothing. Tests: the worker's event translation through a fake
dispatcher (no display).

**T5 — Logind** (`celestina/src/energy/logind.rs`).
Blocking `zbus` proxy of `org.freedesktop.login1.Manager`: `Inhibit` (fd held
in the worker), `Suspend`, `Reboot`, `PowerOff`, with `CanSuspend`,
`CanReboot` and `CanPowerOff` probes surfaced as "unavailable" instead of
failing blind; `Suspend` refused per D2; every call has a timeout and a
bounded request queue. Tests on a private `dbus-daemon` (the tray's
`PrivateBus` recipe) serving a fake `Manager` that records calls and hands
back a pipe fd for `Inhibit`: the fd closes when caffeine turns off and on
worker shutdown, the power calls arrive with `interactive = false`, and a
missing service degrades to "unavailable".

**T6 — Wiring** (`energy/mod.rs`, `verbs.rs`, `command.rs`, `message.rs`,
`app.rs`, `copy.rs`).
`EnergyHandle::start` reads the duties and the energy table (via the settings
provider's follower), starts a worker only for a duty that is `on`, and follows
changes live without a restart (a duty flipping `on` starts its worker; `off`
joins it and releases what it held: gamma restored, inhibitor dropped).
New verbs: `caffeine on|off|toggle`, `nightlight on|off|toggle`,
`idle on|off|toggle` (these flip the *state*, persisted; they do nothing when
the duty is off and say so), and `session suspend|reboot|power-off|log-out`.
Verb parsing stays bounded (≤ 3 words); `USAGE` and the `Verb::words`
round-trip tests are extended. A duty that is `off` makes its verbs answer
with the reason on stderr and a non-zero exit, so a binding pointed at the
wrong shell is noticed.

*Deviations as delivered (T6).* The verbs return no text and always exit 0
once the shell accepted them; a refusal or a failure is a journal line only
(the Wayland client has no reply channel yet). There is no `idle` verb: idle is
a duty-only switch, changed in `shell.json`. No strings were added to
`copy.rs`: nothing in the shell shows these outcomes yet. `EnergyHandle::apply`
records a command as sent only when the worker took it, so a refused one
(`Busy`, `Gone`) is retried by the next `apply`; the idle start-up sends in the
order config, locker, caffeine, enabled and stops at the first refusal, so it
does not depend on the worker's `COMMAND_QUEUE` size. Turning a duty off hands
its worker to a named reaper thread (joined within 2 s, logged) so the loop
never blocks; turning it on again first waits for that reaper, and
`EnergyHandle::stop` and `Drop` join synchronously so gamma is restored before
the process ends. A worker that does not stop within 2 s is left behind and
logged, the one bounded overlap this allows.

**T7 — Smoke and documents.**
Extend `scripts/smoke.sh` with a nested run that starts the shell with all
three duties `on` in a throwaway `XDG_CONFIG_HOME`, asserts the idle worker
bound `ext-idle-notify`, that `nightlight` either applied or reported
"not applied" (the nested output has no DRM gamma) without crashing, that the
shell exits 0, and that `caffeine on` / `session reboot` verbs hit the nest's
own bus, where no logind exists, with a stated refusal. Evidence file
`celestina/docs/evidence/2026-10-16-energy.md`; `VALIDATION.md` rows
`VAL-CEL2-IDLE`, `VAL-CEL2-CAFFEINE`, `VAL-CEL2-NIGHTLIGHT`,
`VAL-CEL2-SESSION`; `STATUS.md`, `ROADMAP.md` and `docs/handover.md` rows
updated (handover: idle, caffeine and night-light lines now "delivered, off
by default"); the ledger row filled (§8).

**T8 — Review.** Two reviewers, each a model other than the task's
implementer, high reasoning, read-only: security and concurrency of T3/T5
(fd ownership, thread affinity, the fail-closed paths, a refused controller,
a hung bus), and the pure chain plus wiring of T1/T2/T4/T6. Findings are
fixed in the worktree by the owning task's model and re-checked.

## 6. Order and parallelism

T1 first (everything imports it). Then T2, T3 and T5 are independent files
and may run in parallel; T4 follows T2; T6 follows T3, T4 and T5; T7 follows
T6; T8 last. Implementers write in disjoint files and never run `git stash`
or touch another task's files.

## 7. Safety rules for every task

- Never act on the live session: no `niri msg`, no `swaylock`, no logind call,
  no gamma request, no `systemctl`, no `pkill` by name. Nested niri
  (`scripts/dev-session.sh`) and a private bus only; the production entries
  (`build/verify/deploy/complete-production.sh`) run in the landing, not here.
- No blind input injection (`wtype`) into the real session.
- A fake logind answers every power verb; the real one is never contacted by a
  test.
- `unsafe` stays forbidden outside `celestina-wayland/src/egl.rs`; the gamma
  fd handling uses `rustix` safe wrappers.

## 8. Ledger row (for CEL-2-A's ledger file)

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CEL-2-B | `celestina:` | active | `celestina-rs/crates/celestina-shell/src/{nightlight,session,idle,settings,lib}.rs`, `celestina/src/energy/`, additive hunks in `command.rs`, `message.rs`, `app.rs`, `copy.rs`, `Cargo.toml`, `scripts/smoke.sh` | pending | Add the energy duties, each off by default behind `duties.idle`, `duties.caffeine` and `duties.nightlight`: the idle chain on `ext-idle-notify` with the screen off by niri's action, caffeine as a logind inhibitor, the forced night light on `wlr-gamma-control`, and the session verbs through logind | `celestina/docs/evidence/2026-10-16-energy.md` | VAL-CEL2-IDLE, VAL-CEL2-CAFFEINE, VAL-CEL2-NIGHTLIGHT, VAL-CEL2-SESSION |

## 9. Models and reasoning per task (proposal, awaiting approval)

A heuristic estimate, per the author's global rule; the reviewer is never the
implementer.

| Task | Model | Reasoning | Reason |
|---|---|---|---|
| T1 pure salvage + settings | Haiku 5.5 for the verbatim copies and head comments; Sonnet 5.5 for the settings schema | low / medium | mechanical copy, then a normal schema change with a clear test list |
| T2 idle chain | Opus 5.5 | high | a state machine with races (caffeine toggled while idle, resume between timers) and a fail-closed rule |
| T3 night-light worker | Opus 5.5 | high | Wayland fd/memfd ownership, per-output lifecycle, hotplug, thread affinity |
| T4 idle worker | Sonnet 5.5 | medium | plain protocol plumbing once T2 and T3 fix the shape |
| T5 logind | Opus 5.5 | high | fd ownership of the inhibitor, privileged calls, private-bus fake |
| T6 wiring + verbs | Sonnet 5.5 | medium | additive edits to shared files under a clear spec |
| T7 smoke + documents | Sonnet 5.5 (Haiku 5.5 for the VALIDATION/ROADMAP row text) | medium / low | script extension and strict document contracts |
| T8 review | Opus 5.5 for T1/T4/T6/T7 (implemented by Sonnet/Haiku); Sonnet 5.5 for T2/T3/T5 (implemented by Opus) | high | a model other than the implementer; Fable is unavailable |

**Question for the author:** D2 — should an explicit `session suspend` be
refused (fail closed, like the halted shell) until the lock of CEL-2-H
exists? Recommended: yes. Your `Mod+Shift+Escape` keeps calling Noctalia's
lock-and-suspend in the meantime.

## Procedure

The tasks of §5, executed in the order of §6 with the models of §9, then two
independent reviews and one round of fixes; what ran is in
[the energy evidence](2026-10-16-energy.md).

## Result

See [the energy evidence](2026-10-16-energy.md); it supersedes this plan
wherever they differ (the shell.json follower is the one CEL-2-E landed,
`idle` has no verb, and verbs answer only through the journal).

## Limits

A plan record: it proves nothing about the code.
