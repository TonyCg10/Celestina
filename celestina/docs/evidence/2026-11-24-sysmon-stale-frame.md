# Evidence: SYSMON-STALE-FRAME — the frame the panel drew for a reading that was gone

- **Date:** 2026-11-24
- **Scope:** the redraw timer of a surface
  (`celestina/crates/celestina-wayland/src/surface.rs`, `runtime.rs`), the
  bar's cadence and the Island's frame (`celestina/src/island/cadence.rs`,
  `src/island/mod.rs`), `App::sysmon_read` (`src/app.rs`), the budget of the
  smoke's cutover run after its switch (`celestina/scripts/smoke.sh`) and
  this unit's ledger row.
- **Environment:** CachyOS, worktree `celestina-SYSMON-STALE-FRAME` cut from
  `main` at `3ecb7b5b` (celestina 1.15.1); toolchains 1.97.1 (pinned) and
  1.100.0; nested niri through `scripts/smoke.sh` and through a copy of its
  cutover run that keeps the journal (own bus, a system bus address that is
  not there, fakes first on `PATH`, throwaway configuration and state
  homes). The author was told before the first nest. **Nothing ran in the
  author's session** but the nests' own windows.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

A surface at rest draws no frame but the clock's minute (design §5.2, ADR
0015). [SMOKE-REST](2026-11-24-smoke-rest.md) made the runtime write each
rest frame to the journal, and every run of the smoke's cutover run showed
the same one: the panel, 12.02 s after start, three or seven seconds after
every duty had been switched off.

The system monitor publishes a reading every two seconds and the bar copies
it at most once in ten (`island::cadence`): a changed reading inside the
period is held back and a redraw is scheduled for the period's end. The
runtime had one timer per surface, kept at the earliest deadline asked for,
and nothing could take a deadline back. So the second reading, four seconds
in, scheduled the copy for twelve; the switch stopped the monitor and
withdrew the reading, which the bar shows at once; and the timer still fired
at twelve seconds for a copy nobody was waiting for. The same happened
without any switch when the reading came back to the value the bar already
showed.

A wake that draws nothing was not a way out: the timer's callback asks for a
frame and `Runtime::draw` always draws it and counts it.

## What changed

- `RedrawTimer` keeps the plain deadlines as before and, beside them, **one**
  deadline per surface that can be withdrawn
  (`SurfaceCtl::redraw_after_withdrawable`, `withdraw_redraw`). The timer
  fires at the earlier of the two; withdrawing never moves a plain deadline,
  and the event loop's timer is removed when nothing is armed. It is still
  one timer per surface, and the toolkit crate knows nothing of the Island.
- The Island holds the ten-second copy with that deadline. Its frame is the
  authority: `Island::hold_sysmon` arms it while a copy is pending and
  withdraws it when none is, and `App::sysmon_read` does the same when a
  reading arrives or goes. The clock's minute and seconds, polkit, the OSD
  and the wallpaper's retry keep the plain deadline.
- `cutover_run` allows the surfaces other than the wallpaper one frame after
  the switch, where it allowed two (the first of which was this one).

## Procedure

```sh
# in celestina/, on the tree of this unit (ec8b40ca)
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings          # 1.97.1, pinned
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
for f in scripts/*.sh; do sh -n "$f"; done
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
# the release binaries, built as build-production.sh builds them, in a target
# directory of this session's own; the cutover run alone, three times; then
timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>   # three times
```

No other session's smoke, nest or landing ran beside any run recorded here:
the runner waits for them and repeats a run one overlapped.

## Tests

Each new test was seen failing before the code, or against the product line
reverted:

- `surface::tests::a_withdrawable_deadline_goes_without_touching_the_others`
  (the timer's state: plain and withdrawable deadlines in either order, a
  withdrawal with a plain deadline earlier, later or absent).
- `a_reading_withdrawn_while_a_copy_is_held_back_leaves_no_deadline`,
  `a_reading_that_comes_back_to_what_the_bar_shows_withdraws_the_copy` and
  `the_clocks_minute_survives_a_withdrawn_copy`, which drive the Island's
  own `hold_sysmon` over the real `RedrawTimer`.
- Guards that passed before and after:
  `a_held_back_copy_still_arrives_once_when_its_period_ends` and
  `a_copy_held_back_stays_due_while_the_system_page_opens_and_closes`.

## Result

| Command | Exit | Output |
|---|---|---|
| `cargo test --workspace --locked` | 0 | 2280 passed, 0 failed, 2 ignored, over 45 test binaries |
| Clippy, pinned 1.97.1, `--workspace --all-targets` | 0 | clean |
| Clippy 1.100.0, `--workspace --all-targets` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `sh -n` on every script of `celestina/scripts/` | 0 | clean |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| the cutover run alone, journal kept, three times | 0, 0, 0 | no `metrics: rest frame` line in any journal |
| `timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>`, three times | 0, 0, 0 | `smoke: OK` each, one attempt each, every run of the script |

The cutover run before and after this unit, the smoke's own line:

```text
before (main, 3ecb7b5b, every run on record):
  (rest_frames=1: the wallpaper 0, at most 2 with 0 output changes; the rest 0 before the switch, at most 2, and 1 after it, at most 2; over 2 surfaces)
after (this tree, the three runs alone and the three full smokes):
  (rest_frames=0: the wallpaper 0, at most 2 with 0 output changes; the rest 0 before the switch, at most 2, and 0 after it, at most 1; over 2 surfaces)
```

Rest frames of the other runs in the three full smokes, unchanged from
`main` (budget in parentheses): first run with the notification 1, 1, 1
(2); bar 0 (1); wallpaper 0 (2); glance 1, 1, 1 (2); glance with weather 1,
1, 1 (2); clipboard 0; connectivity 0 (1); phone and bubbles 0 (0); media
with a paused player 1, 1, 1 (2). The glance runs keep their one frame: the
first reading reaching the bar, which somebody needs. Resident memory of the
cutover run: 101.4, 103.0 and 103.1 MiB.

A fourth full smoke also ended `smoke: OK` and is not counted: another
session's nest ran beside it for a few seconds, so it was repeated.

## Review

A model other than the implementer, high reasoning (2026-11-24): spec and
quality both passed with no Critical or Important finding. It walked the
timer through every order of plain and withdrawable deadlines, a withdrawal
twice, one deadline firing while the other is pending, a surface destroyed
with a timer pending, several Islands and the System page, and found no
lost, late, double or stale fire. Four Minor findings, fixed: no test ran
the product's own withdrawing line (the frame and the tests now share
`hold_sysmon`), half of one test decided nothing, two sentences of the
smoke's comment, and the doc comments now say a surface has exactly one
withdrawable deadline.

## Limits

- That removing an event-loop timer which is already due drops its fire is
  tested on the timer's state and seen in the nested runs below, not in a
  test of the live event loop.
- A surface has one withdrawable deadline. A second user of it would replace
  or withdraw the system monitor's; the doc comments say so and nothing
  enforces it.
- Other waits of the same shape were looked at and left alone, since they
  are not the system monitor's: the polkit prompt's waits can outlive a
  prompt that settles, and the OSD popup's and the wallpaper's retry were
  not examined for it.
- This host has `/etc/pam.d/celestina-lock`, so the cutover run's
  missing-PAM branch and its notice allowance were not exercised.
- No agent ran any live behaviour.

## Landing

- **Base revision:** `3ecb7b5b465671f983c7b9a3f647cde0d803a297`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:17ebd1b56b668a552fa08a48981832bfb00ae283a52026b1a87e38359cd0a5a5, verification_fingerprint sha256:3ae50f1a14d4b01855146e5fa4b3b4050056f8a4276053881ae0b82e9046a2e7
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
