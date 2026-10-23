# CEL-2-M — Cutover: implementation plan

- **Design:** [2026-10-23-celestina-cel-2-m-cutover-design.md](../specs/2026-10-23-celestina-cel-2-m-cutover-design.md)
- **Live order:** [2026-10-23-cel-2-m-live-validation-order.md](2026-10-23-cel-2-m-live-validation-order.md)
- **Ledger row:** CEL-2-M in `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`
- **Worktree:** `scripts/worktree.sh open celestina CEL-2-M` → `unit/celestina/CEL-2-M`
- **Kind:** `milestone` (the program closes; the landing bumps the minor)
- **Rules every task keeps:** `AGENTS.md` (root and `celestina/`); English
  code and comments, Spanish only in `celestina/src/copy.rs`; no `unsafe`, no
  production `unwrap`/`expect`; tests in the same task as the code; shared
  files (`settings.rs`, `app.rs`, `smoke.sh`, the ledger, `STATUS.md`) change
  minimally. **No task runs the shell, the lock, `pamtester` or a polkit
  prompt against the live session**: nests only, `dev-session.sh --own-bus`,
  `env -i`, the nest's display. **No task edits `~/.config`, `/etc` or the
  niri config.** Nothing is committed, pushed or landed without the author's
  direct authorization in chat.

## Preconditions (not tasks of this unit)

| Precondition | Owner | State on 2026-10-23 |
|---|---|---|
| P1 CEL-2-I landed (polkit) | its session, the author's landing | branch ready, re-review clean, live row pending |
| P2 CI green on `main` | `unit/suite/CI-GREEN` (PR 4) | `celestina` and `celestina-rust` jobs still red at the last run |
| P3 the smoke's throttling fix landed | `unit/celestina/SMOKE-FIX` | uncommitted diff |
| P4 the author's blocking rows passed (live order, "Blocks the swap") | the author | none run |
| P5 the author's answers to the design's §10 | the author | open |
| P6 (optional) CEL-2-H-b and CEL-2-L landed | their sessions | branches ready |

T1–T3 can be written before P4; T4–T6 and the landing wait for it.

## Task list

Sizes are heuristic. The model and reasoning table is presented before
execution, as the author's rule asks.

### T1 — Defaults flip, per field (M)

`celestina-rs/crates/celestina-shell/src/settings.rs`: give `Duties` a
per-field default (`#[serde(default = "on")]` through a `fn on() -> Duty`)
for `bar` (its `Option` reads `On` when `None`), `idle`, `caffeine`,
`nightlight`, `wallpaper`, `osd`, `media`, `clipboard`, `phone`, `bubbles`;
`lock` and `polkit` keep `Off` **unless the author's answer to §10.2 says
their rows passed**, in which case they flip too. `lenient_duty` degrades an
unknown value to the field's default, not to `Off`. Update the tests that
assert `Off` defaults and the written-keys test (`bar` is still never written
unless chosen). Add a test that a 1.0.0 file with no `duties` key reads every
flipped duty as `On` and `lock`/`polkit` as the chosen default. Done when
`cargo test -p celestina-shell` passes and `Duties::default()` is the cutover
set.

### T2 — Remove the capsule-only fallback (M)

`celestina/src/app.rs` and `celestina/src/bar/`: the program's §6.1 fallback
(the 1.0.0 centred ~760 px surface while `bar` is off) goes; the surface is
always the wide bar. Characterise first: snapshot the bar's views and `wants`
with `bar` on, assert unchanged after. Keep the live `bar_layout` relayout.
The `Duty::Off` branch for `bar` stays meaningful only as "no bar pieces": if
the author wants `bar: off` to still mean the 1.0.0 capsule, stop and ask;
the design says remove. Done when the characterisation tests pass and
`rg "fallback|760" celestina/src/bar celestina/src/app.rs` names nothing of
the old layout.

### T3 — The smoke's cutover run and the config probe (M)

`celestina/scripts/smoke.sh`: the nested runs that start the shell "with the
defaults" now get a throwaway `shell.json` with the duties they need **off**
(the rest-frame runs), and one **new** run starts the shell with the design's
§5.1 file (every duty on, `lock` and `polkit` included, the dead system bus,
the wallpaper fixture, the fake `wpctl`/`pactl`/`ddcutil` and
`fake-melibea.py` already there): exit 0, one `celestina-panel` and one
`celestina-wallpaper` per output, the energy workers armed or unavailable,
the lock duty's missing-service warning, the polkit registration refused, the
documented rest budget; then the file rewritten with every duty off and the
surfaces gone. New `celestina/scripts/cutover-config-probe.sh FILE`: with a
copy of a niri config passed by path (never `~/.config`), runs `niri validate
-c FILE` and the L keybinds child if present, and fails if any `noctalia`
word remains in a `spawn` or `spawn-at-startup`, if `Mod+Shift+A` does not
spawn `swaylock`, or if the startup line with `caffeine-enable` is still
there; a fixture pair (before, after) under `celestina/tests/fixtures/` with
no personal paths. Done when `smoke.sh --binary target/release/celestina`
passes in a nest and the probe passes on the after-fixture and fails on the
before-fixture.

### T4 — Documents (S)

`celestina/docs/handover.md` rewritten: "what Noctalia did, what does it
now", the recovery of the design's §6 (the backup path, the three ways back),
the dates the author reports for each row. `celestina/README.md`: the
autostart line and the binds of §5. `celestina/ROADMAP.md` and `STATUS.md`:
CEL-2 closed, the active checkpoint to `none` or the next program. The CEL-2
ledger: CEL-2-M to `done` by the landing; the plan moves to
`celestina/docs/plans/archive/` in the same unit if the author wants the
program archived with it (the roadmap rule needs the active plan and the
checkpoint to agree). A new ADR `celestina/docs/decisions/0023-cutover-defaults-on.md`
(number provisional) recording M-1 to M-8 as the author answered them. Done
when `bash scripts/check-documentation-contract.sh` and
`python3 scripts/check-language-contract.py` pass.

### T5 — Evidence (S)

`celestina/docs/evidence/2026-10-XX-cutover.md`: the smoke runs (three, as
the units do), the probe on both fixtures, the tests, the guards; **the
author's report** of the live steps as the author gives it in chat (the
backup path, the `diff` of `config.kdl` against the backup, which rows passed
and when, which stayed pending), quoted as the author's words, nothing
inferred. Done when every "Blocks the swap" row of the live order is either
`passed` in `VALIDATION.md` (the author's edit) or the author has said in
chat that M lands without it, and the evidence says which.

### T6 — Review (S)

A review by a model other than the implementer, at high reasoning, of T1–T3:
the defaults (no duty flips that the author did not name; `lock`/`polkit` as
decided), the fallback removal (no behaviour change with `bar` on), and the
smoke (nothing reaches a real bus or display). Findings fixed in the same
unit.

## What the unit never does

The design's §11, verbatim for every task: no live shell, Noctalia or
compositor started, restarted or stopped; no edit of `~/.config/niri`,
`shell.json` or `/etc`; `swaylock` untouched; no landing or push without the
author's direct authorization.

## Exit

`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`
(both workspaces), `celestina --check`, `smoke.sh` in a nest, the three
guards; then `python3 scripts/land-unit.py unit/celestina/CEL-2-M --kind
milestone` from the canonical checkout, **only on the author's request in
chat**, and only once the author has done step 12 of the design and lived
with it for the time they choose.
