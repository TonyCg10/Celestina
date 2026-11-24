# Evidence: NEST-BACKLIGHT — a nested shell never reads or sets the host's backlight

- **Date:** 2026-11-24
- **Scope:** the backlight switch of `celestina/src/providers/brightness.rs`
  (`CELESTINA_BACKLIGHT`, the reading rule it shares with `CELESTINA_DDC`,
  the worker that touches nothing when it is forbidden), the pin of
  `celestina/scripts/dev-session.sh`, the script test of
  `celestina/tests/lock_scripts.rs`, the sentences of `README.md`,
  `STATUS.md` and `docs/handover.md` that describe a nest, and this unit's
  ledger row.
- **Environment:** CachyOS, worktree `celestina-NEST-BACKLIGHT` cut from
  `main` at `44b8cc2a` (celestina 1.14.0) and rebased onto `c83807be`
  (1.15.0, after CEL-2-M-LOCK-POLKIT), where everything below ran; toolchains
  1.97.1 (pinned) and 1.100.0. This machine has no device under `/sys/class/backlight`, so on
  it the backlight worker ends at once with or without the switch.
  **Nothing ran in the author's session:** the nested smoke opens its own
  nest windows (the author was told before the first), and no shell, lock or
  prompt ran against the live display or buses.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

`dev-session.sh` starts every nest with `CELESTINA_DDC=0` so that a nested
shell never runs `ddcutil` on the monitors of the session it is watched
from (CEL-2-M). The other brightness worker had no such switch: with the
`osd` duty on, which is the default since the cutover, a nested shell opened
and polled every `actual_brightness` under the host's
`/sys/class/backlight`, published the level, and a wheel step over the
brightness piece ran `brightnessctl` on the host's panel. The smoke's fake
`brightnessctl` covers the write in the two runs that put it first on
`PATH`; nothing covered the read, nor an interactive nest.

## What changed

- `CELESTINA_BACKLIGHT` is read once, by the rule `CELESTINA_DDC` already
  had and now shares with it (`switch_allows`: absent, empty or unreadable
  allows; only `0` or `false` forbids). Forbidden, `run_backlight` returns
  before it lists a device: the real `SysfsBacklight` opens no file and
  starts no `brightnessctl`, nothing is published, one warning names the
  variable, and a backlight change sent to the handle is refused as it is on
  a machine with no backlight (the thread has ended).
- `dev-session.sh` starts every nest, interactive or `--once`, with
  `CELESTINA_DDC=0` and `CELESTINA_BACKLIGHT=0` unless the caller set a
  non-empty value, exports both before either launch of the nest's niri,
  and the interactive nest says which each is.
- Every script of `celestina/scripts/` that starts a nested niri does so
  through `dev-session.sh` (`smoke.sh`, `lock-rehearsal.sh`,
  `wallpaper-backdrop-probe.sh`), so all of them inherit the pin; none
  starts niri by itself.

## Tests

- `providers::brightness::tests::a_forbidden_backlight_touches_nothing_publishes_nothing_and_refuses_a_change`:
  a fake environment that records every call sees none, no event arrives and
  a change is refused. It failed before the gate existed (the worker listed
  the devices and published) and passes with it.
- `providers::brightness::tests::an_allowed_backlight_is_opened_and_read`
  records the behaviour that stays, and
  `only_a_readable_refusal_turns_a_switch_off` covers the shared rule and
  pins both variable names.
- `lock_scripts::every_nest_starts_with_ddc_and_backlight_forbidden_unless_the_caller_chose`
  runs `dev-session.sh` against a fake niri that records both variables (no
  compositor, no shell, no hardware): `0` for each with nothing or an empty
  value chosen, the caller's own value otherwise, in both modes, with the
  notices; the constants of `brightness.rs` are the names the script exports
  and the export precedes both launches. It is still the one test of that
  file that starts a process. Two mutations of the script were tried and
  each failed it at the intended assertion: the backlight's default removed
  (`["0", ""]` against `["0", "0"]`) and the backlight left out of the
  export (`["0", "unset"]` against `["0", "0"]`); the script was restored
  byte for byte after each.

## Procedure

```sh
# in celestina/, on the rebased tree of this unit (35148402)
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings          # 1.97.1, pinned
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
for f in scripts/*.sh; do sh -n "$f"; done
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
# the release binaries, built as build-production.sh builds them, then three times:
timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>
# the smoke's cutover run alone, once, with its journal kept
```

The rebase met the unit that landed meanwhile in three places, each a
sentence both sides had rewritten: the paragraph of `README.md` about a nest,
the comment over the interactive nest's notices in `dev-session.sh`, and the
ledger. Both sides' words are kept in each; no code conflicted.

## Result

| Command | Exit | Output |
|---|---|---|
| `cargo test --workspace --locked` | 0 | 2274 passed, 0 failed, 2 ignored, over 45 test binaries |
| Clippy, pinned 1.97.1, `--workspace --all-targets` | 0 | clean |
| Clippy 1.100.0, `--workspace --all-targets` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `sh -n` on every script of `celestina/scripts/` | 0 | clean |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| `timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>`, three times | 0, 0, 0 | `smoke: OK` each, one attempt each, every run of the script; no other session's smoke, nest or landing ran beside any of them |

The smoke's two runs with the `osd` duty on (the OSD run and the cutover
run) start the brightness workers inside a nest. The cutover run alone, with
its journal kept, shows the pin arriving through `dev-session.sh` (the run's
own wrapper sets `CELESTINA_DDC` only) and the shell obeying it:

```text
WARN celestina::providers::brightness: brightness: CELESTINA_DDC forbids every ddcutil call
WARN celestina::providers::brightness: brightness: CELESTINA_BACKLIGHT forbids every read, wait and write of a backlight
INFO celestina::osd: osd duty on: workers started audio=true brightness=true
WARN celestina::providers::brightness: brightness: DDC is switched off by CELESTINA_DDC; nothing runs
```

## Review

A model other than the implementer, high reasoning (2026-11-24): spec and
quality both passed, no Critical or Important finding, three Minor ones of
wording (the provenance comment, the `spawn_with` documentation, a singular
"the variable"), fixed. The review traced every reader of
`/sys/class/backlight` and every caller of `brightnessctl` in `celestina/`
to the gated worker.

## Limits

- No device exists under `/sys/class/backlight` on this machine: that the
  real `SysfsBacklight` opens nothing when forbidden is shown through the
  recording fake and by `run_backlight` returning before `devices()`, not by
  watching a real panel.
- The interactive nest's notices were read from the script test, not from a
  nest started by hand.
- The release binaries of the smokes were built in a target directory of this
  session's own: `smoke.sh` builds its fake system bus beside the binary it
  is given, and the sessions' shared target directory was being rebuilt by
  another session.
- No agent ran any live behaviour.

## Landing

- **Base revision:** `e79a52e88afaba4ea3dbcc09e7872fa996db0272`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:56e6174d31c2dee1c12d47579b208494f846bb7f8fdb4027171c15860e4fe571, verification_fingerprint sha256:5b666fdcca3341a730cca4b3fad1b44c9a9c6d0955d2454bc9037b7881a1fb7a
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
