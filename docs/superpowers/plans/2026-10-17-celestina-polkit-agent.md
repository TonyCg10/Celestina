# Celestina polkit agent (CEL-2-I) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver CEL-2-I, the shell's own polkit authentication agent: registration for the session, a keyboard-grabbing prompt on a dimmed overlay and the helper conversation spoken over `/run/polkit/agent-helper.socket`, behind the `polkit` duty (`off` by default). Every failure ends as "not granted"; the agent never decides a success.

**Architecture:** Pure domain in `celestina-rs/crates/celestina-shell/src/polkit.rs` (requests, queue, backoff, attempts, helper-line decoding, identity choice, `Cookie`). A bus thread in `celestina/src/providers/polkit/` reads one ordered `MessageIterator` on a dedicated system connection (no `#[interface]`), checks the sender on the header, and forwards `Begin`/`Cancel` to the loop with a drop-guard `ReplyHandle`. A helper thread speaks the socket protocol. The loop module `celestina/src/polkit/` owns the request table and the card. `Secret`/`SecretField` live in the toolkit crate `celestina-ui`.

**Tech Stack:** Rust (workspace toolchain), zbus 5 (blocking), rustix (`process`, `net` features added), zeroize, calloop, smithay-client-toolkit via `celestina-wayland`; Python 3 only for the existing guards.

**Spec:** [`docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`](../specs/2026-10-17-celestina-polkit-agent-design.md) (commit `fa5d1202`). The plan argues from the spec; executors read both. `§n`, `N-n`, `F-n`, `S-n`, `Q-n`, `U-n` below are the spec's. The spec's constants (§6.4) are used by name and defined once, in Task 3.

## Global Constraints

- **Identifiers:** project `celestina`, plan `cel-2-parity` ([ledger](../../../celestina/docs/plans/active/2026-10-16-cel-2-parity.md)), unit `CEL-2-I`, commit prefix `celestina:`; layer-shell namespace `celestina-polkit-prompt`; agent object path `/org/celestina/PolkitAgent1`, interface `org.freedesktop.PolicyKit1.AuthenticationAgent`; duty key `polkit`.
- **The halt:** `celestina-qt/` and `celestina-rs/crates/celestina-shell-core` are never edited. The C++ agent is a **read-only reference**; nothing is copied. Before the unit's first commit the author adds `celestina-qt/src/polkit{agent,conversation,promptcontroller}.cpp` and `celestina-qt/src/polkitconverse/main.cpp` as reference to the "Halted projects" salvage list (spec §8). **Task 0 stops until that edit exists.**
- **Root `AGENTS.md`:** no `unsafe` (the workspace denies it; only `egl.rs` is exempt); no production `unwrap`, `expect` or `panic!`; typed errors with context; every dependency justified in its manifest; blocking IO never on the loop thread; D-Bus, socket and file input hostile and bounded.
- **Language:** development truth in English; product copy in Spanish only in `celestina/src/copy.rs` (head `// language-contract: product-copy`). No Spanish string in any other file, tests included.
- **Privileged-unit rules (program §4.4):** every spec §6.3 invariant has its enforcing test or guard before the unit is reviewed; a failure mode of "denied", never "granted"; the duty ships `off`; no nested run or test registers against the real system bus.
- **Delivery:** prepared in a session worktree (`scripts/worktree.sh open celestina CEL-2-I`), with the ledger row moved to `active` by the implementing session, a dated evidence record under `celestina/docs/evidence/`, and the VALIDATION row `VAL-CEL2-POLKIT-LIVE`. The landing bumps versions, writes the inventory and commits; a session never bumps, never writes an inventory, never runs `build-production.sh`. Session commits are `celestina-maintenance: <English imperative>`; the author picks the kind at landing. The guards of spec §6.3 (`G`) are **tests under `celestina/tests/`**, because `scripts/` is outside the prefix's `commit_roots`.
- **Before every session commit**, from the worktree root: `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/version_tool.py check`; from `celestina/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; from `celestina-rs/` when a crate there changed: `cargo fmt --all --check`, `cargo clippy --locked -p <crate> --all-targets -- -D warnings`, `cargo test --locked -p <crate>`. Never `git stash` (subagents: say so in every prompt), never `cargo clean`.
- **Never** run the built shell against the author's real session or system bus; never send keys to the real session (`wtype`); never `pkill` by process name.

## Models and reasoning (proposal, to be approved before Task 1 starts)

The author's global rule asks for this table before implementation. It is a heuristic estimate. The implementer of a task is never its reviewer or the final adversary.

| Task | Model | Reasoning | Why |
|---|---|---|---|
| 0 Spike and preconditions | Sonnet 5.5 | medium | Reading and measuring; no design |
| 1 Shared owners (sanitiser, recogniser) | Sonnet 5.5 | medium | Mechanical extraction with tests |
| 2 Toolkit: redacting `Debug`, enter/leave, `Secret`, `SecretField` | Sonnet 5.5 | high | Secret hygiene is the unit's core |
| 3 Pure domain | Sonnet 5.5 | high | State machine of the failure table |
| 4 Bus thread | Opus 5.5 | high | Dispatch order, sender check, panic backstop |
| 5 Helper conversation | Sonnet 5.5 | high | Hostile-input parsing against a fixture |
| 6 Loop state and the card | Sonnet 5.5 | medium | UI over tested pieces |
| 7 Duty, wiring, copy | Sonnet 5.5 | medium | Wiring with a clear contract |
| 8 Guards, docs, validation row | Haiku 5.5 | low | Mechanical text and scanners |
| Review (spec §10.2.1) | Opus 5.5 | high | A model other than the implementers |
| Adversarial pass (§10.2.2) | Opus 5.5 | highest | The brief says Fable is unavailable; the reviewer and the attacker are one family, so their blind spots overlap — the author may choose another pairing |

Subagents get no fixed `effort` unless the author asks. Changing the session model needs the author's confirmation.

## Review Focus

Inputs the spec implies but no task's tests exercise by default; each is added to the owning task's tests:

1. **A `Cancel` arrives for a cookie whose `Begin` is still queued behind another request** (Task 6): the queued request disappears without ever being shown, and its `ReplyHandle` replies `Cancelled` exactly once.
2. **The compositor closes the card between `configure` and the first `wl_keyboard.enter`** (Task 6): F-6, no helper connection was ever opened.
3. **Two outputs; the focused output changes while a prompt is up** (Task 6): the card stays on its output; no second keyboard surface appears.
4. **`FAILURE` arrives before any prompt** (Task 5): F-12, `Failed`, no retry (no answer was sent).
5. **A PAM message in Spanish containing multi-byte UTF-8 arrives as octal escapes** (Task 5): decoded, validated, sanitised, in that order.
6. **The duty is switched off while an answer is in flight to the helper** (Task 7): the socket closes, the buffer is zero, the reply is `Cancelled`, and the stale helper line that may follow is ignored.

## Tasks

### Task 0: Spike and preconditions

**Files:** none changed in the repository except an evidence note `celestina/docs/evidence/2026-10-XX-cel-2-i-spike.md`.

- [ ] **Step 1: Check the precondition.** `git log -1 --format=%H -- AGENTS.md` and `rg -n 'polkitagent' AGENTS.md` must show the salvage-list edit of spec §8. If absent, stop and report; the edit is the author's.
- [ ] **Step 2: Close the checks of spec §10.3.** Record evidence for each, in the note:
  - U-1: list every `/usr/share/dbus-1/system.d/*.conf` and `/etc/dbus-1/system.d/*.conf` rule that mentions `org.freedesktop.PolicyKit1` or grants `send_destination` to uid 0; state whether the agent interface is reachable by anyone but `polkitd`.
  - U-2: in polkit's `polkiterror.c` (source, not memory) confirm the D-Bus name registered for `POLKIT_ERROR_CANCELLED` is `org.freedesktop.PolicyKit1.Error.Cancelled`.
  - U-5: `cat /sys/power/state /sys/power/resume 2>/dev/null; grep -r resume /etc/mkinitcpio.conf /proc/cmdline` — is hibernation configured.
  - U-6: `rg -n 'atspi|accesskit' celestina/Cargo.lock celestina/crates` — confirm no AT-SPI.
  - U-11: `rg` the shell and the suite for `/proc/self` uses (portals, `xdg-desktop-portal` hints) and write down what a non-dumpable window changes.
- [ ] **Step 3: Verify the zbus facts the design stands on**, against the vendored source: `blocking::MessageIterator` on a `Connection` built with `Builder::system()`; `blocking::Connection::reply` and `reply_error`; that closing the connection (dropping every clone) ends the iterator; and how `smoke.sh` and `energy/logind.rs` select a bus. Write a ten-line throwaway test (not committed) that runs a private `dbus-daemon`, sends a method call and a `NameOwnerChanged`-shaped unicast, and prints header sender/destination as the iterator sees them.
- [ ] **Step 4: Report.** If any check contradicts the spec, stop and send the contradiction to the author; do not patch the spec.

### Task 1: Shared owners — the sanitiser and the `NameOwnerChanged` recogniser

This task is a **separate preceding unit** if the author chooses it (spec Q14); the steps are the same either way.

**Files:**
- Modify/Create: the shared owner of `claimed_text` (default `celestina-rs/crates/celestina-core/src/text.rs`), with `fluorita-core::metadata` delegating; `celestina/src/tray/item.rs` and a new `celestina/src/bus_signal.rs` for the recogniser.
- Test: the owners' own tests; the existing Fluorita and tray tests stay green unchanged.

- [ ] **Step 1: Characterise first.** Run `cargo test -p fluorita-core` and `cargo test` in `celestina/`; save the names of the tests that cover `claimed_text` and `recognise`.
- [ ] **Step 2: Failing tests** for the extended set in spec §6.2 D, in the shared owner: `strips_tag_characters`, `strips_variation_selectors`, `strips_object_replacement_and_interlinear_marks`, `strips_hangul_fillers`, `newline_becomes_a_space_for_display_text`, `keeps_ordinary_accented_and_emoji_text`, `cuts_by_grapheme_at_the_limit_with_an_ellipsis`.
- [ ] **Step 3: Implement** the extension as an addition to the existing function set (a second public function for display text, so Fluorita's tag behaviour is not changed unless its tests allow); Fluorita delegates its list to the shared one.
- [ ] **Step 4: Extract the recogniser.** Move `recognise`'s `NameOwnerChanged` handling (sender is the bus driver and the message is a broadcast) to `bus_signal.rs`; the tray calls it. Tests: `accepts_a_driver_broadcast`, `ignores_a_unicast_forgery`, `ignores_a_signal_from_a_peer`.
- [ ] **Step 5:** run the global before-commit set; commit `celestina-maintenance: Share the display-text sanitiser and the owner-change recogniser` (or the suite prefix when the owner is outside the roots).

### Task 2: Toolkit — redacting `Debug`, keyboard enter/leave, `Secret`, `SecretField`

**Files:**
- Modify: `celestina/crates/celestina-wayland/src/{input,runtime,handlers}.rs`; `celestina/crates/celestina-ui/Cargo.toml` (`zeroize`), `celestina/crates/celestina-ui/src/lib.rs`
- Create: `celestina/crates/celestina-ui/src/secret.rs`
- Test: unit tests in `secret.rs`, `input.rs`; `celestina/crates/celestina-ui/tests/secret.rs`

**Interfaces (produces):**

```rust
// celestina-ui/src/secret.rs
pub const MAX_SECRET_BYTES: usize = 510;
pub struct Secret(/* Zeroizing<Vec<u8>>, capacity fixed */);   // no Clone/Debug/Display/Serialize/PartialEq/Deref
impl Secret {
    pub fn new() -> Secret;
    pub fn push(&mut self, c: char) -> Result<(), SecretRefused>;   // refuses \n \r NUL and overflow; never truncates
    pub fn pop(&mut self);                                          // zeroes the bytes of the last char
    pub fn is_empty(&self) -> bool;
    pub fn clear(&mut self);                                        // zeroes the used length
    pub fn write_line_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()>;   // two write_all
}
pub struct SecretField { /* view state only: has_content, focus */ }
```

- [ ] **Step 1: Failing tests** (names are the contract): `secret_has_fixed_capacity_across_inserts`, `secret_refuses_newline_cr_and_nul`, `secret_refuses_the_511th_byte`, `secret_pop_zeroes_the_removed_bytes`, `secret_clear_zeroes_the_used_length`, `secret_write_line_appends_one_newline_and_nothing_else`, `secret_is_not_clone_or_debug` (a local compile-time assertion, no new dev-dependency), `field_ignores_paste_copy_cut_and_select_all`, `field_ignores_preedit_and_commit_text`, `field_shows_no_length_only_empty_or_content`.
- [ ] **Step 2: Implement `Secret`** (S-1) and `SecretField` (S-2, S-4). The field consumes character keys, Backspace, Enter and Escape and nothing else.
- [ ] **Step 3: Redacting `Debug`.** Replace the derived `Debug` of `InputEvent` with an implementation that prints `utf8`, `Preedit.text`, `CommitText` and `Paste` as `<redacted>`; tests `input_event_debug_redacts_typed_text` for each variant. Add a test that formats the value the way `runtime.rs:1198` and `handlers.rs:281` log it (`{ev:?}`) and asserts no typed character appears.
- [ ] **Step 4: Keyboard enter/leave.** Deliver `FocusEnter`/`FocusLost` for a surface with the enter's serial and a monotonic timestamp; add `SurfaceCtl` accessors only if needed; tests with the existing Wayland test doubles (`enter_then_leave_is_delivered_in_order`, `leave_of_an_unfocused_surface_is_ignored`).
- [ ] **Step 5:** confirm `rg 'unsafe' celestina/crates/celestina-ui` is empty; global before-commit set; commit `celestina-maintenance: Add the secret buffer, the secret field and redacted key events to the toolkit`.

### Task 3: The pure domain

**Files:**
- Create: `celestina-rs/crates/celestina-shell/src/polkit.rs`; export from `lib.rs`
- Modify: `celestina-rs/crates/celestina-shell/src/settings.rs` (`Duties.polkit`, `lenient_duty`), `celestina-rs/crates/celestina-shell/Cargo.toml` (`zeroize` for `Cookie`)
- Test: unit tests inside `polkit.rs` and `settings.rs`

**Interfaces (produces):** the constants of spec §6.4 by name (`MAX_ATTEMPTS`, `MAX_QUEUED`, `MAX_PROMPTS_PER_MINUTE`, `BACKOFF_*`, `REGRAB_GAP`, `KEYBOARD_PROOF`, `FOCUS_GUARD`, `FOCUS_GRACE`, `IDLE_TIMEOUT`, `HELPER_SILENCE`, `MAX_HELPER_LINE`, `MAX_HELPER_LINES`, `MAX_COOKIE_BYTES`, `MAX_ACTION_ID`, `MAX_MESSAGE`, `MAX_PROMPT_TEXT`); `Cookie` (validated, zeroizing, no `Clone`/`Debug`); `RequestId(u64)`; `Request { id, cookie, action_id, message, identity_name }`; `Admission` (`Accepted`/`Refused(Refusal)` with the reply kind of §6.4); `Queue` (one active, `MAX_QUEUED` waiting, rate limit, backoff, `locked` and `duty_on` inputs, an injected clock); `Ending` (one variant per F-n that ends a request, mapping to `Reply::{Normal, Cancelled, Failed, AccessDenied}`); `HelperLine { PromptSecret(String), PromptVisible(String), Error(String), Info(String), Success, Failure }` with `decode_line(&[u8]) -> Result<HelperLine, LineError>`; `choose_identity(&[Identity], own_uid) -> Option<uid>`; `passwd_name(uid, &str) -> Option<String>`.

- [ ] **Step 1: Failing tests** for every row of §6.4, named `f01_…` to `f25_…` (e.g. `f17_second_failure_after_an_answer_is_cancelled`, `f12_failure_without_an_answer_is_failed_and_does_not_retry`), plus:
  - `only_success_yields_a_normal_reply` (an exhaustive match over `Ending` asserts the invariant of §6.4);
  - `queue_refuses_beyond_max_queued`, `rate_limit_counts_when_shown`, `backoff_after_three_dismissals_and_ends_after_the_cooldown`, `regrab_gap_is_respected`;
  - `cancel_removes_a_waiting_request_without_showing_it`, `duplicate_cookie_refuses_only_the_second`;
  - `cookie_over_the_limit_or_non_ascii_is_refused`, `action_id_accepts_capitals_and_underscore_and_refuses_controls`;
  - `decode_line_*`: `accepts_each_listed_escape`, `accepts_one_two_and_three_digit_octal`, `refuses_an_unknown_escape`, `validates_utf8_after_decoding`, `refuses_a_line_over_the_limit`, `success_and_failure_only_as_whole_lines`, `prompt_text_is_returned_undecorated`;
  - `identity_prefers_own_uid_and_otherwise_refuses`, `passwd_name_refuses_controls_and_missing_users`;
  - `duty_polkit_defaults_off_and_reads_leniently`.
- [ ] **Step 2: Implement** until green. Keep the domain free of Wayland, D-Bus and threads; time is injected.
- [ ] **Step 3:** from `celestina-rs/`: fmt, clippy, tests for `celestina-shell`; commit `celestina-maintenance: Add the pure polkit request domain and the polkit duty`.

### Task 4: The bus thread

**Files:**
- Create: `celestina/src/providers/polkit/{mod,agent,session}.rs`
- Modify: `celestina/src/providers/mod.rs`, `celestina/src/message.rs` (`Message::Polkit(PolkitEvent)`; the event carries ids and sanitised text, never a cookie or secret), `celestina/Cargo.toml` (none new; `rustix` features in Task 5)
- Test: `celestina/tests/polkit_bus.rs` against a private `dbus-daemon` with a fake polkitd and a fake logind (test-only module `providers/polkit/testing.rs`, `#[cfg(test)]` or a test-only feature)

**Interfaces (produces):**

```rust
pub struct ReplyHandle { /* connection clone + call header; Drop sends Cancelled if unanswered */ }
impl ReplyHandle { pub fn reply_normal(self); pub fn reply_error(self, e: ReplyError); }
pub enum PolkitEvent { Begin { id: RequestId, request: Request, reply: ReplyHandle }, Cancel(Cookie), OwnerChanged(Option<String>), Registration(RegistrationState) }
pub enum RegistrationState { Off, Waiting, Registered, Refused, Unavailable(String) }
pub fn start(duty: Duty, shutdown: &Arc<AtomicBool>, tx: Sender<Message>) -> io::Result<Option<Worker>>;
```

- [ ] **Step 1: Failing integration tests:** `a_fake_caller_gets_access_denied_and_nothing_else_happens` (no event, no helper, no logged arguments); `a_unicast_forged_name_owner_changed_leaves_the_cache_unchanged`; `begin_from_the_owner_reaches_the_loop`; `cancel_is_seen_while_a_begin_reply_is_pending`; `an_owner_change_is_seen_while_a_begin_reply_is_pending`; `a_restart_of_the_fake_polkitd_emits_owner_changed_and_registers_once`; `a_refused_registration_is_refused_and_never_retried`; `session_comes_from_user_display_when_get_session_by_pid_fails`; `introspection_shows_only_the_agent`; `dropping_a_reply_handle_replies_cancelled`; `a_worker_panic_drops_the_connection` (the fake sees the unique name vanish); `shutdown_joins_within_the_bound` (closing the connection wakes the iterator).
- [ ] **Step 2: Implement** per spec §4.2 and §4.5: dedicated `Builder::system()` connection (the `LogindBus::{System, Address}` pattern of `energy/logind.rs` for tests), match for `NameOwnerChanged` installed **before** `GetNameOwner`, header check before body, unique-name cache fed only by `bus_signal::recognise`, session resolution via logind `GetUser(uid).Display` with the checks of §4.5, registration with locale from `LC_MESSAGES`/`LANG` validated, unregister on clean exit, a panic-drop backstop in the `Worker` body.
- [ ] **Step 3:** no use of `#[interface]`, `std::process`, or `AuthenticationAgentResponse` in the module (the guard of Task 8 will enforce it); global before-commit set; commit `celestina-maintenance: Add the polkit agent's bus thread and its drop-guard replies`.

### Task 5: The helper conversation

**Files:**
- Create: `celestina/src/providers/polkit/helper.rs`
- Modify: `celestina/Cargo.toml` (`rustix` features `process`, `net`; justification comment for each)
- Test: `celestina/tests/polkit_helper.rs` with a fixture helper (a Unix socket in a temporary directory served by a test thread)

**Interfaces (produces):**

```rust
pub struct Conversation { /* owns the socket; reads with a bounded timeout; checks the shutdown flag */ }
pub enum HelperEvent { Prompt(String), Notice(String), Problem(String), Success, Failure, Ended(Ending) }
pub fn connect(path: &Path, user: &str, cookie: &Cookie) -> Result<Conversation, ConnectRefusal>;   // writes "<user>\n<cookie>\n"
impl Conversation { pub fn answer(&mut self, secret: Secret) -> io::Result<()>; pub fn cancel(self); }
```

- [ ] **Step 1: Failing tests** (fixture-driven): `connects_only_after_the_directory_type_and_owner_checks` (the fixture uses a temporary root-less directory through the `#[cfg(test)]` seam, never the real path); `a_world_writable_directory_is_refused`; `a_socket_mode_of_0666_is_accepted`; `a_non_root_listener_is_refused`; `writes_user_then_cookie_each_with_a_newline`; `prompt_text_is_decoded_then_sanitised`; `escaped_esc_nul_and_rlo_in_pam_text_are_removed`; `a_511_byte_secret_never_reaches_the_socket`; `success_before_any_prompt_ends_with_success`; `an_unknown_line_ends_failed`; `a_4097_byte_line_ends_failed`; `a_line_without_a_newline_ends_failed_at_the_bound`; `silence_after_the_answer_ends_failed_at_helper_silence`; `eof_before_a_verdict_ends_failed`; `more_lines_after_success_are_ignored`; `the_secret_is_zero_after_the_write_and_on_every_error`; `cancel_closes_the_socket_and_zeroes_the_buffer`.
- [ ] **Step 2: Implement** per spec §4.4: constant production path with the test seam under `cfg(test)`; `lstat`/`fstat` checks with std; `SO_PEERCRED` through rustix's safe `net::sockopt::socket_peercred`; `BufReader::take(MAX_HELPER_LINE + 1)` per line; strict decoding from Task 3; the secret received through a `sync_channel(1)` and written with `write_line_to`; reads with a timeout and the shutdown flag.
- [ ] **Step 3:** global before-commit set; commit `celestina-maintenance: Add the polkit helper conversation over the helper's socket`.

### Task 6: Loop state and the card

**Files:**
- Create: `celestina/src/polkit/{mod,state,view}.rs`
- Modify: `celestina/src/copy.rs` (Spanish strings), `celestina/src/app.rs` only through a narrow hook (the surfaces list, the launcher and Island close/refuse rule of spec §5.1)
- Test: unit tests in `state.rs` and `view.rs`; `celestina/tests/polkit_prompt.rs` with the toolkit's test doubles

**Interfaces (produces):** `PolkitState { queue: Queue, active: Option<Active>, … }` with `fn on_event(&mut self, ev: PolkitEvent, ctl: &mut RuntimeCtl<'_>)`, `fn on_input(&mut self, …)`, `fn on_frame(&mut self, …)`; `Active { surfaces, card_surface, secret: Secret, attempts, entered_at, … }`; the view tree for the card (lock glyph, title, message, action id, account, `SecretField`, PAM line, failure line, two controls).

- [ ] **Step 1: Failing tests:** `keyboard_proof_failure_cancels_and_raises_the_notice` (F-6, no helper connection); `keys_inside_the_guard_window_are_ignored`; `a_key_pressed_before_the_enter_is_ignored`; `enter_on_an_empty_field_is_ignored`; `focus_loss_wipes_the_partial_secret_and_cancels_after_the_grace`; `focus_returning_inside_the_grace_accepts_typing_again_with_an_empty_field`; `escape_cancels_and_a_click_outside_does_nothing`; `idle_timeout_is_absolute_and_not_extended_by_typing`; `other_keyboard_surfaces_are_closed_when_a_prompt_activates_and_refused_until_it_ends`; `other_outputs_get_dimming_surfaces_without_the_keyboard`; `a_new_output_gets_a_dimming_surface`; `locked_session_refuses_without_mapping` (reads `locked` synchronously, fails closed); `the_card_text_comes_only_from_the_sanitised_request` (G-style); `the_card_shows_no_per_character_glyphs`; plus the Review Focus items 1–3.
- [ ] **Step 2: Implement.** The surfaces: namespace `celestina-polkit-prompt`, `Layer::Overlay`, `Anchoring::Fill`, exclusive zone −1, `Exclusive` on the card's output only. Colours, opacity and anatomy from `Theme` tokens; `reducedMotion` honoured. The `Secret` is moved to the helper thread on submit (S-7).
- [ ] **Step 3:** global before-commit set; commit `celestina-maintenance: Add the polkit prompt's loop state and card`.

### Task 7: The duty, the wiring and the copy

**Files:**
- Modify: `celestina/src/main.rs`, `celestina/src/app.rs`, `celestina/src/message.rs`, `celestina/src/copy.rs`, `celestina/scripts/smoke.sh` (assertions only; it keeps its dead system bus)
- Test: `celestina/tests/polkit_duty.rs`

- [ ] **Step 1: Failing tests:** `duty_off_starts_no_worker_and_refuses_requests`; `switching_on_registers_and_switching_off_unregisters_and_cancels_every_request`; `switching_off_with_an_answer_in_flight_leaves_a_zero_buffer_and_a_cancelled_reply` (Review Focus 6); `a_refused_registration_reports_refused_and_does_not_retry_when_switched_off_and_on_again_it_tries_once`; `the_dumpable_window_opens_with_the_first_request_and_restores_the_previous_value`; `nested_dumpable_windows_restore_the_original_value` (rustix `set_dumpable_behavior`); `smoke_with_the_duty_on_and_the_dead_bus_reports_unavailable_and_does_not_panic`.
- [ ] **Step 2: Implement** the live switch like the other duties (follow `shell.json`), the dumpable window (S-9), the `cfg!(panic = "unwind")` assertion test, and the notices (proof failure, cool-down) through the shell's own notification path (non-stealing, display only).
- [ ] **Step 3:** `celestina --check` and `scripts/smoke.sh --binary PATH` from the **worktree's debug build** only (the landing builds production); global before-commit set; commit `celestina-maintenance: Wire the polkit duty into the shell`.

### Task 8: Guards, documents and the validation row

**Files:**
- Create: `celestina/tests/polkit_guards.rs`, `celestina/docs/evidence/2026-10-XX-cel-2-i.md`
- Modify: `celestina/{README,STATUS,ROADMAP,VALIDATION}.md`, `celestina/docs/handover.md`, the ledger row of CEL-2-I (`planned` → `active`), `celestina/docs/decisions/` (the ADR of spec §13, number assigned at landing)

- [ ] **Step 1: The guards (spec §6.3 `G`)** as tests that read the sources of `celestina/src/providers/polkit/`, `celestina/src/polkit/`, `celestina-rs/crates/celestina-shell/src/polkit.rs` and `celestina/crates/celestina-ui/src/secret.rs`: no `AuthenticationAgentResponse`, `std::process::Command`, `unwrap(`, `expect(`, `panic!`, `unsafe`, `#[interface]`; no `Secret`/`Cookie` in `message.rs`, `bus.rs`, `command.rs`; no `String`/`format!`/`push_str` in the secret paths; the card builds its description only from `Request::display`; the namespace literal appears once and a test cites it; the prompt view does not use `celestina_ui::Field`. Each test names the N-n it enforces.
- [ ] **Step 2: The evidence record.** A table N-1…N-27 and F-1…F-25, each with the test or guard that enforces it and the command that ran; the spike's findings; what did **not** run (the live session).
- [ ] **Step 3: `VALIDATION.md` — `VAL-CEL2-POLKIT-LIVE`** (author only, never blocking), run with Noctalia's agent off and the duty on: `pkexec /usr/bin/true`; a wrong password twice (cancelled, no lockout); Escape; focus theft by a second overlay client (the notice appears, the request is cancelled); `systemctl restart polkit` mid-prompt; killing the shell mid-prompt; and a Hematita service action. Rollback: switch the duty off, re-enable Noctalia's agent.
- [ ] **Step 4: Documents** (README, STATUS, ROADMAP, handover); the ADR draft; confirm the global before-commit set; commit `celestina-maintenance: Add the polkit guards, evidence and live validation row`.

### Task 9: Review and landing (not an implementation task)

- [ ] **Step 1:** the review of spec §10.2.1 by the model of the table, on the unit's whole diff, with the six questions answered in writing in the evidence record; fix what it finds in follow-up commits.
- [ ] **Step 2:** the adversarial pass of §10.2.2 on the fixed branch; the unit does not land while any "does not pass" condition holds.
- [ ] **Step 3:** the author lands the unit with `python3 scripts/land-unit.py unit/celestina/CEL-2-I --kind milestone --summary "<first own commit subject>"`; the row must be `active` and contiguous with the ledger table (landing gotchas in the project memory). The landing builds, verifies and deploys; it does **not** replace the live shell, and the duty stays `off` until the author switches it on.

## Self-review

- Every spec invariant has a task that tests it: N-1…N-7 in Tasks 2 and 5, N-8…N-11 in Tasks 3 and 6, N-12…N-15 in Task 4, N-16…N-17 in Task 5, N-18…N-20 in Tasks 1 and 6, N-21…N-23 in Tasks 4 and 8, N-24…N-27 in Tasks 3, 6 and 7. The failure table is Task 3's `f01…f25`.
- Task order puts the preconditions and the shared owners before anything that depends on them; the toolkit and the pure domain are independent and may run in parallel, in separate files.
- Not planned on purpose: the move of `InputEvent::Key` to `Option<char>` (spec S-6), a spawn fallback to the setuid helper, any input-method support, any use of `details`.
