# Evidence: OVERVIEW-SLOT-FLAKY — three capture tests that depended on a gap being short

- **Date:** 2026-11-24
- **Scope:** the tests of `celestina/src/overview/slot.rs` (their rig and two
  of them), one test of `celestina/src/capture/worker.rs`, and this unit's
  ledger row. No code of the product changed.
- **Environment:** CachyOS, worktree `celestina-OVERVIEW-SLOT-FLAKY` cut from
  `main` at `000f1081` (celestina 1.15.2); toolchains 1.97.1 (pinned) and
  1.100.0; eight cores. No nest, no shell: unit tests only.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The `celestina` workflow failed on `main` for commit `9e236bbb` (run
45493805678) and passed for the next commit, which holds the same code:

```text
test overview::slot::tests::a_draining_capture_keeps_the_next_opening_busy_until_it_ends ... FAILED
panicked at src/overview/slot.rs:569:9:
assertion `left == right` failed
  left: Err(Unavailable)
 right: Ok(())
```

Since CI-VISIBLE the landing stops while the newest run of a workflow is
red, so a test that fails now and then can hold every landing until another
push.

**The cause is in the test, not in the product.** One capture runs at a
time: `CaptureHandle::request` sets a process-wide in-flight mark, the
request carries it as a `Ticket`, and `capture::worker::run` drops the
ticket, clearing the mark, only after `Capturer::capture` has returned. The
rig's capturer says `ended` from inside `capture`. The test waited for
`ended` and then asked for the next capture every five milliseconds until it
was taken. But an attempt made while the mark is still set is refused as
busy, and `Slot::request_capture` then lets go of the worker it had just
started for it; the rig hands out a fixed number of workers (three in this
test), so one attempt inside the gap between `ended` and the ticket's drop
left none for the next, the spawner failed, and `request_capture` answered
`Unavailable`. The gap is a few instructions long unless the worker's thread
loses the processor in it, which a loaded runner arranges.

The sibling test `a_cancelled_capture_publishes_nothing_and_a_reopen_asks_anew`
had the same loop with eight workers "to spare": the same failure, seven
attempts later.

In the product the spawner never runs out, so this path cannot give
`Unavailable`; an opening that comes inside that gap is refused as busy and
its panel opens with geometry only, which is what `request_capture`
documents.

**A narrower gap of the same kind, found by the review.**
`capture::worker::tests::a_panicking_capturer_never_leaves_the_request_stuck_in_flight`
asked the handle again until it stopped answering "in flight" and expected
"gone". A panic unwinds through the request's ticket first, which clears the
mark, and only then through the queue, which closes it; a request made
between the two is taken and discarded when the queue closes, exactly as
`spawn` says it may be. The test took that `Ok` for its last answer. It was
never seen to fail, here or on the runner.

## Procedure

```sh
# in celestina/
# 1. the gap made wide by hand: a sleep after `ended.send` in the rig (removed again)
cargo test --lib overview::slot::tests::a_draining_capture       # three times at 20 ms
cargo test --lib overview::slot                                   # at 60 ms
# 2. no change at all: the test binary of main, the slot tests 300 times beside 32 busy loops
# 3. the fix: red (the rig lingers, the old loops), then green; the same 300 runs
# 4. the worker's panic test: the unwinding made long by hand (removed again), red, then green
cargo test --lib capture::worker::tests::a_panicking_capturer    # three times each way
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings          # 1.97.1, pinned
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

| Step | Outcome |
|---|---|
| a 20 ms sleep after `ended.send` in the rig, the draining test, three runs | failed three times, each `left: Err(Unavailable)`, `right: Ok(())`: the runner's assertion |
| the same at 60 ms, every slot test | the draining test and its sibling failed with that assertion; the other eight passed |
| the unchanged test binary of `main`, 300 runs of the slot tests beside 32 busy loops | 6 failed, every one the draining test with that assertion |
| the rig lingering 60 ms by itself (`Held::linger`), the old loops | both tests failed, as above: the red of the fix |
| the fix, every slot test | 10 passed |
| the fixed test binary, the same 300 runs beside 32 busy loops | 0 failed |
| the worker's panic test with the unwinding made 40 ms long between the ticket and the queue (a local with a sleeping `Drop` in the worker's closure, removed again), three runs | failed three times, `left: Ok(())`, `right: Err(Gone)` |
| the same with its fix | passed three times; and without the sleep, with the 32 tests of `capture` |
| `cargo test --workspace --locked` | exit 0: 2300 passed, 0 failed, 2 ignored, over 46 test binaries |
| Clippy, pinned 1.97.1 and 1.100.0, `--workspace --all-targets` | exit 0 on both, clean |
| `cargo fmt --all -- --check` | exit 0 |
| the architecture, language and documentation guards | exit 0, OK |

**The fix.** The rig's capturer has a `linger`: how long it stays in
`capture` after it has said it ended. The two tests run with 60 ms of it, so
the gap is always wide; they wait until the rig's own mark is clear
(`mark_cleared`), which is exactly what the next request needs, and then ask
once. Their rigs hand out exactly the workers the test needs (two and
three), so a wasted worker now fails the test instead of being absorbed.
What each test asserted before, it still asserts.

The worker's panic test now treats "taken" like "in flight" while it waits:
both mean the worker is not gone yet, and the test ends only on "gone", with
the mark clear.

## Review

A model other than the author of the change, high reasoning (2026-11-24),
read the diff, the worker and the slot, and ran the tests in loops. The
diagnosis is right and complete; the two slot tests are deterministic but
for their two-second waits, cannot find the mark clear too early (the test's
own thread sets it), and still assert what they did, the exact budgets
adding one real check (a request that started two workers would fail). The
refusal the tests met is, in the product, the documented behaviour of an
opening that comes while a given-up capture is finishing
(`docs/superpowers/specs/2026-10-17-cel-2-l-see-the-desktop-design.md`), not
a defect. No Critical or Important finding; three Minor ones: the worker's
panic test above, fixed here; a sentence of a comment, fixed; and the rig
leaking a mark of its own where the capture tests have a helper for it,
which was there before and is left.

## Limits

- The runner's failure was reproduced here by widening the gap and by load,
  with the same assertion; the runner itself is only seen in the workflow's
  next runs.
- The two-second waits of the tests (for a capture to start, to end, for the
  mark to clear) are still bounds in time: a machine that starves a thread
  for two seconds would fail them, with a message that says which wait.
- No agent ran any live behaviour.

## Landing

- **Base revision:** `000f1081618d698465be809044f08fc9e831347a`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7e4d2852d39f0a09f6d903223f3b728e82e03f65d4e0929368c4143b3be5f295, verification_fingerprint sha256:8667d89778833e7e352026551c22816f05ccd565fad0d5f62cc9af70e220ded6
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
