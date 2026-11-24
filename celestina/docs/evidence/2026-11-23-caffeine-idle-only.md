# Evidence: CEL-2-B-CAFFEINE — caffeine inhibits idling only

- **Date:** 2026-11-23
- **Scope:** the `what` of the caffeine inhibitor in
  `celestina/src/energy/logind.rs` and its tests; the idle chain's answer to
  caffeine switched on after its second step
  (`celestina-rs/crates/celestina-shell/src/idle.rs`) with the test of the
  chain, the router and the lock policy together
  (`celestina/src/energy/mod.rs`); the wording that follows from it
  (`celestina-rs/crates/celestina-shell/src/lock.rs`, `settings.rs`, the
  journal line in `celestina/src/lock/mod.rs`); ADR 0025, the
  `VAL-CEL2-CAFFEINE` row and this unit's ledger row. No dependency changed.
- **Environment:** CachyOS, systemd 262, worktree `celestina-CEL-2-B-CAFFEINE`
  cut from `main` at `974a7490` (celestina 1.13.2); toolchains 1.97.1
  (pinned) and 1.99 (the runner's stable). No shell was started and no
  inhibitor was taken on the real system bus: the logind tests talk to a fake
  logind on a private `dbus-daemon` of their own.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

Caffeine held `Inhibit("idle:sleep", ..., "block")`. In the author's live
use of 1.13.2 that refused `systemctl suspend` while caffeine was on. The
author decided (relayed by the coordinating session on 2026-11-23, to be
confirmed by the author at the landing) that caffeine must stop only what
inactivity does by itself and let a suspend asked for by hand through.

The refusal came from logind itself: since systemd 257 a `block` lock also
binds the user who holds it (the earlier behaviour is the mode `block-weak`),
so the `sleep` half refused `systemctl suspend`, the shell's own `Suspend`
from `celestina lock suspend` (the session was left locked but awake) and the
power key (`HandlePowerKey=suspend` in
`/etc/systemd/logind.conf.d/80-power-button.conf`). It never refused the lid,
for which logind ignores inhibitors by default.

## The change

- `INHIBIT_WHAT` is `"idle"`. The shell's own idle chain still follows
  caffeine through `IdleChain::set_caffeine`, not through logind.
- The old `sleep` half also stopped, by accident, one suspend that inactivity
  starts: the chain hands out `LockAndSuspend`, the lock policy records the
  idle intent and starts the lock, and caffeine is switched on before the
  lock confirms by something that is not seat input (`celestina caffeine on`
  over ssh or from a script, an edit of `shell.json`). Nothing withdrew that
  intent, so the policy suspended at `locked`, and only logind's refusal
  stopped it. Now `IdleChain::set_caffeine(true)` that finds the step handed
  out and not resumed clears that mark and answers `[IdleAction::Resumed]`,
  once. The step stays handled, so that idle period hands it out again
  neither while caffeine is on nor after it is switched off; the next input
  starts a new period.
- What `Resumed` does on its way: the idle worker hands every action to
  `perform_idle` (`celestina/src/energy/mod.rs`), which posts
  `LockEvent::IdleResumed` to the loop and does nothing else; it sends the
  compositor nothing, so the outputs stay as they are. The policy
  (`celestina_shell::lock`) answers `IdleResumed` by dropping an intent
  whose origin is `Idle` and logging it; an intent of origin `Verb` (a
  person's `lock suspend`), the lock child and the delay inhibitor are
  untouched, so the session still locks and is not suspended. No consumer
  changed: only the journal line, which said "input came back", now names
  both causes.

## Tests

- `caffeine_switched_on_withdraws_a_lock_and_suspend_already_handed_out`
  (`celestina-shell/src/idle.rs`, new): the step is handed out, caffeine on
  answers `Resumed` once, nothing is handed out again while it is on or when
  it goes off in the same period, the later input has nothing left to cancel
  and the next period starts clean; caffeine on before the step, after a
  refusal, and caffeine off answer nothing, as before.
- `caffeine_switched_on_withdraws_the_idle_suspend_and_never_a_persons_own`
  (`celestina/src/energy/mod.rs`, new): the real chain, `perform_idle` and
  the real `LockPolicy` joined. The chain's step sets the idle intent,
  caffeine goes on, the lock confirms: no suspend, and the session is
  locked. The same run without caffeine suspends once, so the rig can tell.
  With a person's `lock suspend` in the same window the intent that remains
  is the person's and the confirmation suspends once.
- `the_inhibitor_is_taken_as_documented_and_closed_on_release`
  (`energy/logind.rs`, changed): the call the fake logind receives is
  compared with the literal
  `Inhibit idle|Celestina|The session was asked to stay awake|block`, not
  with a string built from the constants.
- `caffeine_inhibits_idling_and_never_sleep` (`energy/logind.rs`): pins the
  two constants; the loop over `split(':')`, which could not fail after the
  first assertion, is gone.

Mutations, each restored byte for byte (SHA-256 compared) before the next:

| Mutation | Result |
|---|---|
| `set_caffeine` without the withdrawal | both new tests fail (`left: []`, `right: [Resumed]`) |
| `INHIBIT_WHAT = "idle:sleep"` | `caffeine_inhibits_idling_and_never_sleep` and `the_inhibitor_is_taken_as_documented_and_closed_on_release` fail, the second on the literal call |

## Procedure

```sh
# in celestina-rs/
cargo test -p celestina-shell
cargo clippy -p celestina-shell --all-targets -- -D warnings
cargo fmt --all -- --check
# in celestina/
cargo test --lib energy::
cargo test
cargo clippy --all-targets -- -D warnings     # 1.97.1
cargo +1.99 clippy --all-targets -- -D warnings
cargo fmt --all -- --check
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

| Command | Exit | Output |
|---|---|---|
| `cargo test -p celestina-shell` | 0 | 529 passed, 0 failed (525 in the library) |
| `cargo clippy -p celestina-shell --all-targets -- -D warnings` | 0 | clean |
| `cargo fmt --all -- --check` (`celestina-rs/`) | 0 | clean |
| `cargo test --lib energy::` | 0 | 108 passed (18 in `energy::logind`) |
| `cargo test` (`celestina/`) | 0 | 1673 passed, 0 failed, 1 ignored (1521 in the library) |
| `cargo clippy --all-targets -- -D warnings` (1.97.1 and 1.99) | 0 | clean |
| `cargo fmt --all -- --check` (`celestina/`) | 0 | clean |
| the three guards | 0 | OK |

## Review

An independent review (Opus, a model other than the implementer's, read
only) found nothing critical and judged the change of the `what` correct and
safe. Fixed in this unit after it: the idle suspend in flight that the old
`sleep` half stopped by accident is withdrawn by the chain (I-1); the
validation row no longer asks for no `sleep` lock at all, which the lock
duty's own `delay` lock contradicts, says what holds the chain and what
holds logind, and names the power key (I-2); the fake-logind test compares
the literal call and the loop that could not fail is gone (M-1); ADR 0025 is
`proposed` with a provisional number (M-2) and says why logind refused, that
the lid never was, the `block-weak` alternative, that it supersedes the
`what` of D6, the case no longer covered and the unlocked manual suspend
with the lock duty off (M-3); this record states logind's `IdleAction` (M-4);
and `settings.rs` says "keeps the session from idling" (M-6). Left as it is:
the ledger row's place in the table and its file list, which the landing
replaces with the inventory (M-5, cosmetic).

## Limits

- Nothing ran against the real logind: that `systemctl suspend`,
  `celestina lock suspend` and the power key now go through with caffeine
  on, locked first, is the new step of `VAL-CEL2-CAFFEINE`. The withdrawal
  of an idle suspend in flight is proved on the pure chain and policy only;
  its window (from the chain's step to the lock's confirmation, a few
  seconds) is not a step of the validation row.
- On this machine logind's `IdleAction` is the default `ignore`:
  `/etc/systemd/logind.conf` sets nothing and the one drop-in sets only the
  power key (read on 2026-11-23, no bus touched). The `idle` lock therefore
  changes nothing in logind itself here; it informs the programs that read
  logind's inhibitors.
- Noctalia's idle, still running until the cutover, appears to follow a
  logind `idle` lock; the review inferred it from the binary's strings and
  nothing verified it.
- Switching the idle duty off in the same window withdraws nothing:
  `Duty::release` in `celestina/src/energy/idle.rs` resets the chain and
  passes no `Resumed` on, so the idle intent is still served when the lock
  confirms. That path is older than this unit, has nothing to do with
  caffeine and is not changed by it.
- The session lock's design under `docs/superpowers/` still describes
  `IdleResumed` as input only, and the cutover's validation order there
  still quotes `idle:sleep`; they are suite documents outside this unit's
  scope.

## Landing

- **Base revision:** `3c4e63f4cc5edfb96edbb2e0f4d6c4c38fd7c433`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9e6b43fb346ee744bb8f29b3cf39696726d412c7e27135252805b3380e4d41ed, verification_fingerprint sha256:ecee1404efbeee86461a6e0bb598e2a25f0631a893343f1b9f44df268b0131d4; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:0749399b71de2d261334bddcc0e7538701c3a5b70435b85a496aff953370b064, verification_fingerprint sha256:80379e7255146de613e1bf1c11b605914c303b47534fb669232380edee97e993
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
