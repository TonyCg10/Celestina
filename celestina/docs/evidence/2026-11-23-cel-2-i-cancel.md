# Evidence: CEL-2-I-CANCEL — a cancelled polkit prompt that counted as a failed login

- **Date:** 2026-11-23
- **Scope:** `celestina/src/providers/polkit/helper.rs` (the conversation's
  thread), `celestina/src/polkit/state.rs`, `celestina/src/polkit/view.rs`,
  `celestina/src/polkit/mod.rs` (the journal line of a notice that cannot be
  raised), `celestina/src/app.rs` (that call), `celestina/src/copy.rs` (one
  label), `celestina-rs/crates/celestina-shell/src/polkit.rs` (documents of
  `Step` and one test; no behaviour), `celestina/tests/polkit_guards.rs`, the
  tests of those modules, [ADR 0021](../decisions/0021-polkit-agent.md)
  (amendment of 2026-11-23), the row `VAL-CEL2-POLKIT-LIVE` of
  `celestina/VALIDATION.md`, one sentence of `celestina/README.md`, one entry
  of `celestina/STATUS.md` and this unit's ledger row. No dependency, version
  or setting changed.
- **Environment:** CachyOS, worktree `celestina-CEL-2-I-CANCEL` cut from `main`
  at `974a7490` (celestina 1.13.2); cargo `--locked`, with
  `CARGO_TARGET_DIR=/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target-CEL-2-I-CANCEL`.
  Every test uses the in-crate fakes: a Unix socket in a temporary directory
  standing for polkit's helper, a connector that only counts, and a **private**
  `dbus-daemon` with a fake polkitd. Nothing touched the real session, the
  system bus, polkitd, PAM, `pkexec` or `faillock`, and no shell or lock was
  started for real.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The author reported on 1.13.2, in the real session, that cancelling the polkit
prompt card (Escape or the Cancel control) counts as a failed authentication
in `pam_faillock`: the third cancel in a row locks the account for ten minutes
(`deny = 3`, `unlock_time = 600`), and with it login, `sudo` and the lock
screen.

Cause, verified in polkit's `polkitagenthelper-pam.c` (`conversation_function`)
and on this host's stack (`/usr/lib/pam.d/polkit-1` includes
`/etc/pam.d/system-auth`: `auth [success=1 default=bad] pam_unix.so`, then
`auth [default=die] pam_faillock.so authfail`, and `pam_faillock.so authsucc`
last):

1. The agent opened the helper's socket as soon as the card had the keyboard
   and the guard window had passed (decision 20 of the ADR), so PAM, inside
   the helper, sat at its first `PAM_PROMPT_ECHO_OFF` in `fgets` for as long as
   the person looked at the card.
2. Any close of that socket while PAM waits for input is an end of input:
   `PAM_CONV_ERR`, which `pam_unix` turns into `default=bad`, and `pam_faillock
   authfail` records a failure. Escape, the Cancel control, the idle timeout,
   a lost keyboard, `CancelAuthentication` and the shell quitting all closed it.

The two-attempts-per-prompt rule (decision 23) was built on the belief that
only a wrong password counts.

## The change

The conversation is lazy (ADR 0021, "Amendment (2026-11-23): lazy
conversation", decision 25). In order of the unit's brief:

1. **No socket before the first submit.** The helper's thread is still started
   when the card is ready (`PolkitState::open_helper`), but it idles in
   `wait_for_answer` and calls its connector only after it took a secret from
   the one-slot channel (`helper::run`). There is no `Conversation` before
   that, so nothing can be closed on PAM.
2. **The card is ready without a PAM prompt.** `Phase::Asking` starts when the
   thread starts (the keyboard proof and the guard window), with the card's own
   generic password label `copy::POLKIT_PASSWORD_PROMPT`. `Phase::Preparing` now
   means only "the keyboard is not proven yet". PAM's words appear only for a
   second question.
3. **The secret travels at PAM's first echo-off prompt.** `helper::converse`
   keeps the submitted `Secret` as `retained`; PAM's notices before the prompt
   are shown; at the first `PAM_PROMPT_ECHO_OFF` the secret is written at once
   through `Conversation::answer` (which consumes and drops it) and the card is
   not told of that prompt. `PAM_PROMPT_ECHO_ON` is still refused. A second
   `PAM_PROMPT_ECHO_OFF` is published as `HelperEvent::Prompt` and answered
   live.
4. **A wrong answer closes and waits.** After `HelperEvent::Failure` the thread
   drops its socket (the helper has ended) and goes back to waiting for a
   submit; the card is `Asking` again with the attempts left. The pure
   `Exchange` outlives the wait, and its silence rule is not consulted while the
   thread is not reading a socket.
5. **Cancel semantics.** The thread keeps the person's cancel (`Helper::cancel`,
   which is also what dropping the handle does) apart from the shell's own flag
   (`Halt::own`, `Halt::shell`). Before any submit nothing exists to close.
   Between the connection and the first prompt the own flag is held back until
   the secret is written, for at most `helper::CONNECT_GRACE` (5 s); nothing is
   published after a cancel in any case (`emit` checks both flags first), and
   the shell's flag is never held back. After the secret is written the socket
   closes at once. The reaper that joins the thread after a cancel waits
   `CANCEL_WAIT + CONNECT_GRACE + WRITE_TIMEOUT` (a connection still being made
   may add its own bounded writes before the grace starts). The non-dumpable
   window (N-6, S-9) outlives the request for as long as the thread may hold
   the secret: `PolkitState::retire` takes a `DumpHold` from the window before
   closing its own claim and hands it to the thread's reaper
   (`Helper::cancel_holding`), which drops it after the join. The loop never
   waits (N-22). The window's counter and saved value are the process's
   (one shared by every `DumpableWindow::process()`, so a hold that outlives
   an earlier start of the duty cannot restore under a later one); a window
   over a fake flag has its own, for the tests. If the reaper's bound passes
   with the thread still alive, the hold is leaked on purpose with an error
   in the journal (fail closed). An ending is published only after the
   retained secret is dropped.
6. **Replies unchanged.** `ReplyHandle`, `Ending` and the failure table are
   untouched; only `Ending::Authenticated` replies normally.
7. **Back-off and the notice.** A cancel before any submit is still a dismissal
   (`attempted = false`), a cancel after one is an attempt. The journal line for
   a notice that could not be raised because another program holds
   `org.freedesktop.Notifications` moved from `app.rs` to
   `polkit::notice_unraised`, so it can be tested: one line per occurrence, at
   `warn` for the cool-down and the helper's absence, at `error` (unchanged) for
   the one a client that suppressed the real prompt must not be able to hide.
8. **Hygiene.** The retained secret lives in a local of the thread and is
   dropped on every way out (below); no new type holds it, and nothing about it
   reaches a log, a `Debug` or a message.

### What polkitd does with a cancel after a correct secret

Cited from polkit main as read on 2026-11-23 (read by the coordinating
session; this unit did not re-read it):
`src/polkitbackend/polkitbackendinteractiveauthority.c`,
`authentication_agent_begin_cb`. `gained_authorization` is initialised `FALSE`
at line 2145 and becomes `session->is_authenticated` only in the branch where
the agent's reply is **not** an error (line 2162); an error reply, such as our
`Cancelled` (which also sets `was_dismissed`, line 2156), always leaves it
`FALSE`, and that value is what reaches the session's callback (line 2176),
even if the helper had already told polkitd of a success. So a cancel after the
correct secret, or one held back and released after it was written, must end
denied, and does: the agent's reply is the `Cancelled` error, and a `SUCCESS`
that arrives afterwards reaches nobody.

The `pam_faillock` effect of that case follows from the host's stack and is not
changed by the reply: a **correct** password that was sent resets the counter
(`authsucc` records the success although the request is denied); a **wrong**
password that was sent is counted, which is right, because a wrong password was
really sent.

## The tests behind each point

Tags as in [2026-10-17-cel-2-i.md](2026-10-17-cel-2-i.md): `[lib]` is the
`celestina` library, `[shell]` is `celestina-shell`, `[guards]` is
`celestina/tests/polkit_guards.rs`. Names were checked against the tree with
`rg "fn <name>"`.

| Point | Enforced by |
|---|---|
| 1. no connection before a submit | `[lib] providers::polkit::helper::tests::cancel_before_any_submit_opens_no_helper_connection` (idle, cancelled, dropped and shell-flag cases, counted on the fixture's `accepted`); `providers::polkit::helper::tests::the_connector_is_called_once_per_submitted_conversation_and_never_before` (counting connector); `polkit::state::tests::cancelling_before_any_submit_opens_no_helper_connection_by_any_route` (the real thread over a counting connector, for Escape, the Cancel control, the idle timeout, a lost keyboard, polkitd's cancel, polkitd restarting, the duty off, the session locking, the card's output going away, the state dropped, and Escape with another request waiting); `polkit::state::tests::the_helper_connects_only_when_the_first_secret_is_submitted`; `[guards] the_helper_is_connected_only_after_a_secret_was_taken_from_the_slot` (one `connector(input)` call, after the first wait for a secret; spawning opens nothing; nothing outside the helper reaches the socket) |
| 2. ready without a PAM prompt | `[lib] polkit::state::tests::the_card_is_ready_to_type_without_a_pam_prompt`, `typing_before_the_card_is_ready_is_dropped`; `polkit::view::tests::asking_shows_the_cards_own_label_until_pam_asks_something_in_words_of_its_own`, `the_prompt_line_follows_the_phase` |
| 3. the secret at the first echo-off prompt | `[lib] providers::polkit::helper::tests::the_secret_travels_only_after_the_first_echo_off_prompt` (two notices, then a pause during which no byte may arrive, then the prompt, then the secret), `a_second_echo_off_prompt_is_shown_and_answered_live`, `echo_on_after_submit_is_refused`, `prompt_text_is_decoded_then_sanitised`, `a_511_byte_secret_never_reaches_the_socket`; `polkit::state::tests::a_second_prompt_from_pam_is_shown_and_takes_a_live_answer` |
| 4. a wrong answer closes and waits | `[lib] providers::polkit::helper::tests::a_wrong_answer_returns_to_asking_and_does_not_reconnect_until_the_next_submit` (connections stay 1 until the next submit, then 2), `a_wrong_answer_below_the_cap_is_followed_by_a_second_conversation_with_the_same_cookie`, `silence_while_the_person_types_does_not_end_the_conversation`; `polkit::state::tests::a_wrong_answer_shows_the_attempts_left_and_asks_again`; `[shell] polkit::tests::a_conversation_after_an_idle_retry_owes_its_prompt_and_the_count_goes_on`, `f16_failure_after_an_answer_below_the_cap_starts_a_new_conversation`, `f17_second_failure_after_an_answer_is_cancelled` |
| 5. cancel semantics | `[lib] providers::polkit::helper::tests::a_cancel_between_connect_and_the_first_prompt_is_deferred_until_the_secret_is_written` (the socket stays open while PAM is slow, the secret is written first, nothing is published), `a_deferred_cancel_is_bounded_by_the_connect_grace`, `a_shell_shutdown_is_not_deferred`, `cancel_after_the_secret_was_written_closes_the_socket`, `cancel_while_waiting_for_the_person_at_a_second_factor_closes_the_socket` (the documented residual), `the_shells_shutdown_flag_stops_the_thread`, `cancel_returns_at_once_and_a_stuck_thread_says_nothing_after_it` |
| 5b. the window outlives a held-back secret | `[lib] polkit::state::tests::the_window_stays_shut_while_a_cancelled_helper_may_still_hold_the_secret` (the flag stays non-dumpable after the request ended and across a second request, and returns to its original value only when the hold is let go); `providers::polkit::helper::tests::the_hold_of_a_cancel_is_released_only_after_the_held_back_secret_is_gone` (the hold outlives the secret: the order recorded is secret, then hold; also past the grace and with nothing retained); `polkit::dumpable::tests::a_hold_keeps_the_window_shut_after_the_request_closes_it_from_any_thread` |
| 5d. one counter for the process's window; the hold reaches the reaper; the overrun fails closed | `[lib] polkit::dumpable::tests::a_hold_that_outlives_a_window_does_not_restore_under_the_next_one` (the re-review's probe: two windows over one counter, the old hold outliving the first, the value restored once, when the last is gone), `the_process_windows_share_one_counter_and_fake_ones_do_not`; `polkit::state::tests::a_real_helper_handle_keeps_the_hold_until_its_thread_has_ended` (a real `Helper<Secret>` as `dyn HelperHandle`: the production glue of `retire`); `providers::polkit::helper::tests::a_hold_is_kept_when_the_thread_does_not_stop_within_the_reapers_bound` (the hold is kept and an error journaled) |
| 5c. a notice while a cancel is held back | `[lib] providers::polkit::helper::tests::a_notice_during_a_deferred_cancel_does_not_close_the_socket` |
| 6. replies unchanged, a late success reaches nobody | `[lib] providers::polkit::helper::tests::a_cancel_after_a_correct_secret_is_still_answered_cancelled` (on the private-bus fake polkitd, for the secret written before the cancel and for the cancel held back and released after it; the fake helper then prints `SUCCESS`, which is not published, and a stale `Authenticated` ends nothing); `providers::polkit::tests::only_the_helpers_success_answers_begin_normally`; `[shell] polkit::tests::only_success_yields_a_normal_reply` |
| 7. back-off and the notice | `[lib] polkit::state::tests::the_dismissal_backoff_still_counts_cancels_before_submit` (Escape and the Cancel control count; three cancels after a submit never start the cool-down), `three_dismissals_in_a_row_raise_the_cooldown_notice`; `polkit::tests::a_notice_that_cannot_be_raised_is_journaled_once_per_occurrence` (`WARN`, once per call, with what and why) |
| 8. secret hygiene | `[lib] providers::polkit::helper::tests::the_retained_secret_is_dropped_before_an_ending_is_published` (connector refusal, echo on, end of input, a protocol fault, silence: the secret is dropped first, then the loop hears the ending), `the_retained_secret_is_dropped_on_every_exit_path` (connector refusal, `PAM_PROMPT_ECHO_ON`, end of input, helper silence, cancel while connecting, cancel past the grace unwritten, the shell's flag), `the_secret_is_dropped_after_the_write_and_on_every_error`, `the_answer_channel_has_one_slot`, `a_full_conversation_never_logs_the_answers_or_the_cookie` (now also through a second factor); `[guards]` N-1 to N-5, N-21, N-22 unchanged and green |

Tests whose premise changed and that were ported, not deleted (the old names
appear in [2026-10-17-cel-2-i.md](2026-10-17-cel-2-i.md), which keeps its bytes):

| Old name | Now |
|---|---|
| `polkit::state::tests::typing_before_pam_asks_is_dropped` | `typing_before_the_card_is_ready_is_dropped` |
| `providers::polkit::helper::tests::a_wrong_answer_below_the_cap_opens_a_second_conversation_with_the_same_cookie` | `a_wrong_answer_below_the_cap_is_followed_by_a_second_conversation_with_the_same_cookie` (the card submits again) |
| `providers::polkit::helper::tests::cancel_closes_the_socket_and_drops_the_answer` | `cancel_after_the_secret_was_written_closes_the_socket` |
| `providers::polkit::helper::tests::cancel_while_waiting_for_the_person_closes_the_socket` | `cancel_while_waiting_for_the_person_at_a_second_factor_closes_the_socket` |
| `polkit::state::tests::ready` (the fixture) | no longer fires a PAM prompt; the card is ready at the guard window's end |

The helper tests' `drive` stands for the card: it submits the first answer at
once and answers each shown prompt and each wrong answer with the next one.
Every test that used to start a conversation by waiting for its first prompt
now starts it by submitting.

## Procedure

From `celestina/` with the target directory above, all `--locked`:

- Red first. On the unit's tests with the production code as it was
  (`Timing::grace` added as an unused field so the tests compile):
  `cargo test --lib providers::polkit::helper` failed 18 of 56 (among them
  `cancel_before_any_submit_opens_no_helper_connection` ("idle: no
  connection"), `the_connector_is_called_once_per_submitted_conversation_and_never_before`
  (1, not 0), `the_secret_travels_only_after_the_first_echo_off_prompt` (the
  prompt reached the card), `a_wrong_answer_returns_to_asking_and_does_not_reconnect_until_the_next_submit`,
  `a_cancel_between_connect_and_the_first_prompt_is_deferred_until_the_secret_is_written`,
  `a_deferred_cancel_is_bounded_by_the_connect_grace`,
  `a_cancel_after_a_correct_secret_is_still_answered_cancelled`); and with the
  loop state's old `state.rs`, `cargo test --lib polkit::state` failed 15
  (`the_card_is_ready_to_type_without_a_pam_prompt`,
  `the_helper_connects_only_when_the_first_secret_is_submitted`,
  `the_dismissal_backoff_still_counts_cancels_before_submit`, and the tests that
  ported the old prompt-first premise).
- For the review's fixes, mutations on a scratch copy of the tree (never the
  worktree): removing `&& !retained.get()` from the notice branch fails
  `a_notice_during_a_deferred_cancel_does_not_close_the_socket`; closing the
  window at once in `retire` fails `the_window_stays_shut_while_a_cancelled_helper_may_still_hold_the_secret`;
  dropping the hold before the reaper's join fails
  `the_hold_of_a_cancel_is_released_only_after_the_held_back_secret_is_gone`;
  publishing the ending before dropping the secret fails
  `the_retained_secret_is_dropped_before_an_ending_is_published`. For the
  re-review's fixes: a `process()` that makes its own counter fails
  `the_process_windows_share_one_counter_and_fake_ones_do_not`; dropping the
  hold before calling `Helper::cancel` in `HelperHandle::cancel` fails
  `a_real_helper_handle_keeps_the_hold_until_its_thread_has_ended`; dropping the
  hold instead of keeping it on an overrun fails
  `a_hold_is_kept_when_the_thread_does_not_stop_within_the_reapers_bound`.
- `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked
  -- -D warnings`; `cargo test --workspace --locked`; `cargo test --lib polkit`
  three times in a row (the timing tests are stable).
- From `celestina-rs/`: `cargo fmt --all --check`, `cargo clippy -p celestina-shell
  --all-targets --locked -- -D warnings`, `cargo test -p celestina-shell --locked`.
- From the root: `bash scripts/check-architecture-contract.sh`, `python3
  scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`,
  `python3 scripts/version_tool.py check`.

## Result

Last run, after the re-review's fixes (see the commit list of the unit's branch; earlier runs, before the fixes, were also green, with 1537 and 1542 library tests):

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked
  -- -D warnings` in `celestina/`: clean.
- `cargo test --workspace --locked` in `celestina/`: green, with 1546 tests in
  the `celestina` library (the polkit modules' 178 among them, three runs in a
  row without a failure) and 29 in `polkit_guards` (28 before the new guard).
  One unrelated test, `overview::slot::tests::a_draining_capture_keeps_the_next_opening_busy_until_it_ends`,
  failed once on a first full run while the machine was busy compiling and
  passed in every run after it, alone and in the workspace.
- In `celestina-rs/`: `cargo fmt --all --check`, `cargo clippy -p celestina-shell
  --all-targets --locked -- -D warnings` and `cargo test -p celestina-shell
  --locked` (525 tests in the library): green.
- From the root: `check-architecture-contract.sh`, `check-language-contract.py`,
  `check-documentation-contract.sh` and `version_tool.py check`: green.
- `scripts/smoke.sh --binary <debug celestina>` (a nested niri on a bus of its
  own, the system bus pointed at a socket that does not exist), once, under
  `timeout 900`: `smoke: OK`, including its last run, "polkit on over the dead
  system bus: the agent is unavailable and the shell did not panic". It does
  not exercise a prompt.

## Review

An independent read-only review by Opus 5.5 (2026-11-23, on `9e5d69af`,
report `cel-2-i-cancel-review.md` in the coordinating session's notes; it
ran `fmt`, `clippy` and `test` and three mutations on a scratch copy, and
nothing against the real session). Verdict: the change grants nothing new, can
leak no secret to a log, a `Debug` or a message, and never causes more
`pam_faillock` failures than 1.13.2 did (cancel before submit 1 to 0; a retry
then cancel 1 to 0; a held-back cancel with a wrong password 1 to 1, with the
right one 1 to 0; the residuals 1 to 1). It also re-read polkit's source and
confirmed the lines cited above, and found no other path by which the
helper's authentication grants without the agent's normal reply. No blocker.
Findings and what was done:

| Finding | Disposition |
|---|---|
| I-1 the held-back secret outlives the request and the non-dumpable window (N-6, S-9) | **Fixed** in `73159cc8`: the window's hold travels with the thread's reaper (see "The change", point 5); test `the_window_stays_shut_while_a_cancelled_helper_may_still_hold_the_secret` and the helper's `the_hold_of_a_cancel_is_released_only_after_the_held_back_secret_is_gone`. Checked by mutation: closing the window at once (state) fails the first, dropping the hold before the join (reaper) fails the second |
| I-2 a notice during a held-back cancel was untested (mutation survived) | **Fixed**: the reviewer's probe is `a_notice_during_a_deferred_cancel_does_not_close_the_socket`. Checked by mutation on a scratch copy: with `&& !retained.get()` removed from the notice branch it fails, and it passes on the code |
| M-1 an ending was published before the retained secret was dropped | **Fixed**: `drop` first in the connect-refusal and `Ended` branches; test `the_retained_secret_is_dropped_before_an_ending_is_published` (checked by mutation: emit then drop fails it) |
| M-2 the `halt.any()` check between the slot and the connector has no test | **Not testable on purpose**: the window is microseconds between two statements of the thread. The check stays and a comment says so; the subtest that claimed to cover it was a cancel during the connection and its comment now says that |
| M-3 the deferred test depended on a time limit that competed with the grace | **Fixed**: it now asserts the socket closes less than a second after the secret was written |
| M-4 the reaper could warn "did not stop in time" for a thread that was only holding the secret | **Fixed**: it waits `CANCEL_WAIT + CONNECT_GRACE + WRITE_TIMEOUT` |
| M-5 the first echo-off prompt gets the password without the person reading its text (an OTP or PIN stack would count a failure) | **Documented** in "Revisit when" of the ADR's amendment; not this host. Also noted there: a failure of the helper's socket or of `pam_faillock preauth` now shows only after typing and submitting |
| M-6 the readme said "never counts", the ledger said "logged at warn", the faillock step left a failure behind | **Fixed**: the readme says what is true; the ledger row says warn for the cool-down and helper notices and error for the unshown-prompt one; the step ends with a successful `pkexec`, which resets the counter |
| M-7 the bus test returns without testing when no `dbus-daemon` exists | **Documented** below, in the limits: the pattern is the existing harness's |

### Re-review of the fixes

A second focused read by the same reviewer (Opus 5.5, read-only, on
`d205eb08`) found no grant path, no leak, no extra `pam_faillock` failure and
no wait in the loop, and one important defect and some minors:

| Finding | Disposition |
|---|---|
| R-1 every start of the duty made a window with a counter of its own, so an old hold living in a reaper restored `Dumpable` under the next start's request and the new window then restored `NotDumpable` for good | **Fixed**: `DumpableWindow::process()` shares one counter and one saved value per process (a `OnceLock`); windows over a fake flag, which only tests use, keep their own. The reviewer's probe is `a_hold_that_outlives_a_window_does_not_restore_under_the_next_one`; `the_process_windows_share_one_counter_and_fake_ones_do_not` fails when `process()` makes a counter of its own (checked by mutation) |
| D-1 two claims said more than the code (nested windows restoring the original value between starts; the sum bounding the secret) | **Fixed**: the ADR amendment and "The change", point 5, say that the window and its holds are process-wide and that the reaper's bound is the reaper's wait, not the secret's life; the limits say what happens on an overrun |
| M-1 the reaper's bound is practical, and on an overrun the hold was released (the window failed open) | **Fixed by failing closed**: the reaper keeps the hold on an overrun (leaked on purpose, an error journaled) and the process stays non-dumpable until it exits; `a_hold_is_kept_when_the_thread_does_not_stop_within_the_reapers_bound` (mutation: dropping it instead fails the test) |
| M-2 the production glue in `HelperHandle::cancel` had no test | **Fixed**: `a_real_helper_handle_keeps_the_hold_until_its_thread_has_ended`; mutation: dropping the hold before `Helper::cancel` fails it |
| M-3 if `Reaper::spawn` cannot create a thread, `Worker::drop` joins without a bound on the loop's thread | **Documented** in the limits (pre-existing, not introduced here) |

## Limits

### Residuals

A cancel can still count in `pam_faillock` in these cases, all documented in
the ADR's amendment: (1) a cancel while PAM waits for the answer to a second
prompt (this host has none, and the person is the one holding PAM up); (2) the
shell shutting down (its own flag, never held back) between the connection and
the first prompt, or at a second prompt; (3) a grace overrun, PAM not asking
within `CONNECT_GRACE` after a cancel. A wrong password that was sent counts,
as it always did. A cancel after a correct password ends denied and
`pam_faillock` records a success.

Not a residual but worth knowing: a card that is up for a long time holds only
a thread and a cookie copy, not a connection; the helper's own silence deadline
(60 s) applies only to a conversation that exists.

- `providers::polkit::helper::tests::a_cancel_after_a_correct_secret_is_still_answered_cancelled`
  needs a `dbus-daemon`, like every test of the bus harness, and returns
  without testing where there is none. It ran here (the daemon is installed);
  a machine without it checks nothing of point 6.
- A thread that cannot stop within the reaper's wait (`CANCEL_WAIT +
  CONNECT_GRACE + WRITE_TIMEOUT`, 12 s) is left behind. The wait is a
  practical bound, not a strict one (`UnixStream::connect` has no timeout, and
  the writes before and after the grace have their own), and it bounds the
  reaper, not the secret: a stuck thread keeps the secret as long as its call
  lasts. The window fails closed then: the reaper keeps the hold (a leak on
  purpose, an `error` in the journal) and the process stays non-dumpable until
  it exits.
- Pre-existing, not introduced here: if `Reaper::spawn` cannot create its
  thread, the closure (the join and the hold) is dropped unrun, and
  `Worker::drop` then joins the helper thread without a bound on the loop's
  own thread, up to the grace when a cancel was held back. The order of the
  drops (the worker, then the hold) is what the compiler does, not something
  the language guarantees.

### What did not run

- **The real session.** No `pkexec`, no real polkitd, no real PAM, no
  `faillock` (read or write), no shell or lock started for real, no key
  injected. The effect on the counter is the author's check:
  `VAL-CEL2-POLKIT-LIVE`, step 3 (three cancels in a row, then `faillock --user`),
  pending.
- The fake helper cannot reproduce PAM: that a closed socket counts is read from
  polkit's source and this host's PAM files, and the tests prove that the agent
  no longer closes a socket PAM waits on, not what PAM does when one is.
- The loop and the thread over a real compositor: the nested smoke starts the
  duty over a dead system bus and exercises no prompt.

## Landing

- **Base revision:** `4859d6e2a1c4ce677ab790be63b5148fdd93e2db`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3e4e6ec59bf5e9376c36eefd4f13407dcb2275c5764e6063e0dd33f71d5b9265, verification_fingerprint sha256:34f73bdcaa6465b1b1c1980773de018314d049d518ea035daac34980014d6150; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:380a462f90de2632bc105c71c39552d3ff5a86998e4b56170f45500e4f663956, verification_fingerprint sha256:80379e7255146de613e1bf1c11b605914c303b47534fb669232380edee97e993
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
