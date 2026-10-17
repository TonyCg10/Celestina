# Celestina session lock (CEL-2-H-a, CEL-2-H-b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the shell's own session lock in two units of the `cel-2-parity` ledger. **CEL-2-H-a** is the security core: an `ext-session-lock-v1` client (`celestina-lock`) that covers every output in black, asks for the password, and is the only thing that can end the lock after a PAM child (`celestina-lock-verify`) says yes; the shell's side (supervisor, sleep worker, logind delay inhibitor, `lock` verbs, idle step 2, notices, `duties.lock`); the recovery path; the CPU painter with the field, status, Caps Lock and the fade; the backdrop bundle format v1 frozen and reserved (header-only). **CEL-2-H-b** is the presentation over that frozen format: the dimmed wallpaper (thumbnails on the wallpaper worker, the bundle's pixels, the lock's backdrop reader and scrim) and the clock and date. Every failure ends locked or denied; nothing but `Authenticated` of the current attempt unlocks.

**Architecture:** Three processes (spec §4.1). The pure vocabulary lives in the dependency-free crate `celestina-rs/crates/celestina-lock-proto` (verdicts, exit codes, status-line codec, bundle codec, `Backoff`, PAM service enum and paths). The shell's pure policy is `celestina-rs/crates/celestina-shell/src/lock.rs`. The toolkit crate `celestina-wayland` gains a lock profile behind a `gl` feature split. `celestina/crates/celestina-lock` (CPU painter over `tiny-skia`/`cosmic-text`, one embedded font) and `celestina/crates/celestina-lock-verify` (one FFI module `pam.rs`, the project's second documented `unsafe` exception) are new packages. The shell's providers live in `celestina/src/lock/` (`supervisor.rs`, `sleep.rs`, `bundle.rs`).

**Tech Stack:** Rust (workspace toolchain `celestina-rs/rust-toolchain.toml`), smithay-client-toolkit 0.21.1 `session_lock`, wayland-protocols (single-pixel-buffer, viewporter), calloop, zbus 5 (blocking) for the sleep worker only, rustix (`fs`, `process`, `time`, `event`), `tiny-skia` 0.12, `cosmic-text` 0.19, `fontdb` 0.23, `zeroize` (new to the lockfile), `libc`, `nix` (`user`), system `libpam`; shell and Python 3 only for the existing scripts and guards.

**Spec:** [`docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`](../specs/2026-10-17-celestina-cel-2-h-session-lock-design.md) (commit `4c2be848`). The plan argues from the spec; executors read both completely. `§n`, `H-n`, `TM-n`, `F-n`, `Q-n`, `T-xx`, `R-n` below are the spec's. Where this plan and the spec differ, the spec wins and the executor stops and reports.

## Global Constraints

- **Identifiers:** project `celestina`, plan `cel-2-parity` ([ledger](../../../celestina/docs/plans/active/2026-10-16-cel-2-parity.md)), units `CEL-2-H-a` and `CEL-2-H-b`, commit prefix `celestina:` at landing (session commits are `celestina-maintenance: <English imperative>`; the author picks the kind at landing, as in the CEL-2-I plan), duty key `duties.lock`, validation rows `VAL-CEL2-LOCK-*`, binaries `celestina-lock` and `celestina-lock-verify`, PAM services `celestina-lock` and `celestina-lock-rehearsal`, status-line protocol 1, backdrop bundle version 1.
- **The halt:** `celestina-qt/` and `celestina-rs/crates/celestina-shell-core` are never edited. Nothing is copied by this program (spec §13); the halted lock files are read-only reference, and only after the author's "Halted projects" edit (Prerequisite P1). Executors do not read halted files before that edit exists.
- **Root `AGENTS.md`:** no `unsafe` outside `celestina/crates/celestina-wayland/src/egl.rs` and, from Task 2 onward, `celestina/crates/celestina-lock-verify/src/pam.rs`; no production `unwrap`, `expect`, `panic!`, no `#[allow]`/TODO/FIXME/HACK to hide debt; typed errors with context; every dependency justified in its manifest; blocking IO never on a loop thread; D-Bus, process, file and stdin input hostile and bounded; workers shut down deterministically and publish only current results.
- **Language:** development truth in English (code, comments, tests, identifiers, documents, commit subjects). Product copy is Spanish only in `celestina/crates/celestina-lock/src/copy.rs` and `celestina/src/copy.rs`, each with the head `language-contract: product-copy`. No Spanish literal anywhere else, tests included. The proposed literals of spec §4.6 are used by constant name; the author may reword them.
- **Privileged-unit rules (parity design §4.4):** every spec §6 "never" item has its enforcing test, guard or structural reason before review; every failure is "locked" or "denied", never "unlocked"; the duty ships `off`; the PAM service is installed by the author, never by an agent or a script; no secret is ever formatted into a log.
- **Nothing is rehearsed in the author's live session.** Every run of the lock, the verifier, the shell and the rehearsal happens in the nest (`celestina/scripts/dev-session.sh`) on a private session bus and a private fake-logind bus. Never type the author's password. Never run a wrong-password rehearsal against the production service `celestina-lock` (it counts toward the author's `faillock`). Never send keys to the real session (`wtype`) or run `swaylock` against it. Never open the lock, the shell or `pamtester` against the real session bus or real system bus. **Never kill any process by name** (`pkill`, `killall`, `pkill -x niri` killed the author's session once): the nest and every child are stopped by the pid recorded by their own launcher (`.kdl`/`--once` pid file, the pidfd of the supervisor, the pid in `lock-<display>.state` of the nest's own display). Every prompt to a subagent says so, and says **never `git stash`** (a stash reverts the shared tree while others edit) and never `cargo clean`.
- **Delivery:** each unit is prepared in its own session worktree (`scripts/worktree.sh open celestina CEL-2-H-a`; H-b opens after H-a has landed, from the new `origin/main`), with the unit's ledger row moved from `planned` to `active` by the implementing session, a dated evidence record under `celestina/docs/evidence/`, and the unit's `VAL-CEL2-LOCK-*` rows in `celestina/VALIDATION.md`. A session never bumps a version, never writes an inventory, never runs `build-production.sh`/`verify-production.sh`/`deploy-production.sh`/`complete-production.sh`, never commits or pushes without the author's explicit request. The landing (`python3 scripts/land-unit.py unit/celestina/CEL-2-H-a --kind milestone --summary "<first own commit subject>"`, run by the author from the canonical checkout) writes inventories, bumps, builds, verifies and deploys; it does not replace the live shell. The guards of spec §5/§9 that need scanning are **tests** under `celestina/crates/*/tests/` or `celestina/tests/`, because `scripts/` is outside the prefix's `commit_roots`.
- **Before every session commit**, from the worktree root: `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/version_tool.py check`; from `celestina/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; from `celestina-rs/` when a crate there changed: `cargo fmt --all --check`, `cargo clippy --locked -p <crate> --all-targets -- -D warnings`, `cargo test --locked -p <crate>`; scope with `python3 scripts/commit_scope.py --check "<subject>"` (staged paths on stdin). The scripted form is called **the standard set** below.
- **Dependency discipline:** the lock binaries' transitive closure is pinned by T-D1 against the checked-in allowlist of spec §4.14 (`celestina/crates/celestina-lock/tests/closure.allow`). Any new crate in `cargo tree -e normal --prefix none -p celestina-lock -p celestina-lock-verify` fails the test and stops the executor.

## Prerequisites and stop points (the author's, in order)

The plan does not start implementing until the first three exist. Each is a **stop**: the executor checks, reports and waits.

| Stop | Owner | What must exist | How an executor checks it |
|---|---|---|---|
| **P0** | author | Approval of the complete specification, of §11's model table and of this plan (the table below repeats it) | the author's message in chat |
| **P1** | author | The `suite-maintenance:` prerequisite commit of spec §13, one commit for both units: the "Halted projects" salvage entry for CEL-2-H (reference only); `docs/projects.toml` (the `celestina` entry's `artifact_paths`, `source_roots`, `commit_roots`, `production_inputs` gain `celestina-lock`, `celestina-lock-verify` and `celestina-rs/crates/celestina-lock-proto`); the `celestina-rs/Cargo.toml` member `crates/celestina-lock-proto`; the parity ledger's `CEL-2-H` row replaced by two `planned` rows `CEL-2-H-a` and `CEL-2-H-b` (both `celestina:`) | `git log -1 --format=%H -- AGENTS.md docs/projects.toml`; `rg -n 'CEL-2-H \(CEL-2-H-a' AGENTS.md`; `rg -n 'celestina-lock-proto' docs/projects.toml celestina-rs/Cargo.toml`; `rg -n '^\| CEL-2-H-[ab] ' celestina/docs/plans/active/2026-10-16-cel-2-parity.md` must print two `planned` rows and no bare `CEL-2-H` row |
| **P2** | author (spike gate) | After Task S and before Task 3: the author has read the spike notes and accepts, amends or rejects each answer that changes §4 | the note `celestina/docs/evidence/2026-10-XX-cel-2-h-spikes.md` and the author's reply |
| **P3** | author (non-blocking) | Manual installation of the PAM service: `sudo install -m 0644 -o root -g root celestina/contrib/pam.d/celestina-lock /etc/pam.d/celestina-lock`, only when the author runs `VAL-CEL2-LOCK-PAM`. No agent installs, edits or checks `/etc` beyond read-only `stat` | not a gate for any task; the shell warns at duty-on if absent |
| **P4** | author | Landing of CEL-2-H-a (`land-unit.py`) before CEL-2-H-b is opened | `git log origin/main` shows the H-a landing and `celestina/docs/inventories/2026-10-16-cel-2-parity/CEL-2-H-a.numstat.tsv` exists |
| **P5** | author | Review decisions: acceptance in writing of any residual finding of Review A or B (spec §10 exit criteria) | the author's message and the evidence record |

If P1 is missing, an executor opens no worktree for H-a, edits nothing, and reports. The worktree for the unit is opened by `scripts/worktree.sh open celestina CEL-2-H-a` only after P1.

## Models and reasoning (the approved table of spec §11, restated)

A heuristic estimate under the author's global rule, fixed by the author's choice OD-2 (b). The implementer of security code (Sonnet 5.5, Haiku 5.5) is never its reviewer. Subagents get no fixed `effort` except Review B, which the author asked for. Changing the session model needs the author's confirmation.

| Task | Unit | Model | Reasoning |
|---|---|---|---|
| S spikes (Q1, Q2, Q4, Q6, Q7, Q10, Q11, Q16, Q17, Q18, Q19) | H-a | Opus 5.5 | high |
| 1 proto, policy, idle, settings | H-a | Sonnet 5.5 | high |
| 2 `celestina-lock-verify` and `pam.rs` | H-a | Sonnet 5.5 | high |
| 3 toolkit lock profile | H-a | Sonnet 5.5 | high |
| 4 `celestina-lock` core | H-a | Sonnet 5.5 | high |
| 5 CPU painter, view, copy | H-a | Sonnet 5.5 | medium |
| 6 shell integration | H-a | Sonnet 5.5 | high |
| 7 production entries, PAM text, rehearsal, README | H-a | Sonnet 5.5 | medium |
| 8 ADRs, rows, evidence | H-a | Sonnet 5.5 (ADRs), Haiku 5.5 (rows) | medium / low |
| 9 Review A over H-a | H-a | Opus 5.5 | high |
| 10 Review B over H-a, clean context | H-a | Opus 5.5 | highest |
| 11 fixes and re-review | H-a | the implementer's model; the same reviewer | as the task |
| B1 thumbnails and bundle pixel blocks | H-b | Sonnet 5.5 | medium |
| B2 backdrop worker and scrim | H-b | Sonnet 5.5 | medium |
| B3 clock and date | H-b | Sonnet 5.5 | medium |
| B4 rehearsal R-10, captures, rows | H-b | Sonnet 5.5; Haiku 5.5 for the rows | medium / low |
| B5 ledger row, evidence, STATUS | H-b | Haiku 5.5 | low |
| B6 Review A over H-b incl. the §0 freeze | H-b | Opus 5.5 | high |
| B7 fixes and re-review | H-b | the implementer's model; the same reviewer | as the task |

(The spec's task ids T0–T12 map as: T0 = P0/P1, TS = S, T1 = 1, T2 = 2, T3 = 3, T4 = 4, T5a = 5, T6 = 6, T8a = 7, T9a = 8, T10a = 9, T11 = 10, T12a = 11; T7 = B1, T4b = B2, T5b = B3, T8b = B4, T9b = B5, T10b = B6, T12b = B7.)

## Execution order

**CEL-2-H-a:** P0 → P1 → open worktree → S → **P2** → 1 → (2 and 3 are independent, different files; they may run in parallel by separate subagents) → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → author lands → **P4**.
**CEL-2-H-b:** open worktree from the new `origin/main` → B1 → B2 → B3 → B4 → B5 → B6 → B7 → author lands.

Task S precedes Task 3 because Q1, Q4 and Q6 may change the toolkit profile. Task 1 may start once Task S has no open contradiction; Tasks 2 and 3 never edit the same file. Task 6 needs Tasks 1 and 4 (the policy and the real child). Task 7 needs 6.

## Ledger

Both rows are registered by the P1 commit as `planned`. The implementing session changes **its own row only**: `planned` to `active` on opening (the row must stay contiguous with the table; the landing gotchas are in the project memory `aterrizajes-land-unit.md`), `done` is written by the landing.

| Unit | Commit prefix | Status at plan time | Files / areas | Intended change (the row's text) | Author validation |
|---|---|---|---|---|---|
| CEL-2-H-a | `celestina:` | planned | `celestina-rs/crates/celestina-lock-proto`, `celestina-rs/crates/celestina-shell/src/lock.rs`, `celestina/crates/celestina-{lock,lock-verify,wayland}`, `celestina/src/lock`, `celestina/src/energy`, `celestina/contrib/pam.d`, `celestina/scripts` | Add the session lock's security core: the black covers, the PAM verifier, the shell's supervisor, delay inhibitor, verbs, notices and idle step 2, the recovery path, the CPU painter with the field, status, Caps Lock and the fade; freeze backdrop bundle format v1 (header-only from the shell); carries ADR 0004 over (H-1…H-18) | VAL-CEL2-LOCK-PAM, -LIVE, -SUSPEND, -HOTPLUG, -RECOVERY, -NOTICES, -COEXIST |
| CEL-2-H-b | `celestina:` | planned | `celestina/crates/celestina-lock/src/{backdrop,clock}.rs`, `celestina/src/lock/bundle.rs`, `celestina/src/wallpaper`, `celestina/src/providers/wallpaper.rs`, `celestina/src/island/clock.rs` | Add the lock's presentation over the frozen format: the dimmed wallpaper per output and the clock and date from a realtime timerfd | VAL-CEL2-LOCK-BACKDROP, -CLOCK |

## Review Focus

Inputs the spec implies but no default test exercises; each is added to the owning task's tests.

1. **Double Enter and a verdict after output hotplug** (Task 4): two Enters while `Verifying` start one attempt; a verdict that arrives after the output holding the prompt disappeared still unlocks or denies exactly once and the prompt moves.
2. **`PrepareForSleep(true)` while the child is in `Ending`** (Task 1, Task 6): the policy keeps the inhibitor, remembers one pending request, the exit spawns again at once; no proof is minted from `Ending`.
3. **Unlock during `PrepareForSleep`** (Task 1): `unlocked` while `Preparing` never re-takes the inhibitor (Q17 assumption) and spawns again.
4. **A status line of exactly 64 bytes, of 65 bytes, with no newline, and with an embedded NUL** (Task 1 codec, Task 6 supervisor): bounded, logged, ignored, never a state change.
5. **The bundle's file offset after `pwrite`, a bundle exactly at 16 MiB, and a header whose output count disagrees with the file size** (Task 1 codec, Task 6 `bundle.rs`).
6. **A secret of exactly 512 bytes made of multibyte characters, and a 513th byte** (Task 4): never reaches the verifier; the key is ignored.
7. **The verifier is killed at the deadline while the pidfd and the stdout of the supervisor race** (Task 4): `Unavailable` arrives once, no second attempt starts before the reap.

## Common command blocks

**The standard set** (run before every session commit; stop on the first failure):

```sh
# from the worktree root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
# from celestina/
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
# from celestina-rs/ when a crate there changed
cargo fmt --all --check
cargo clippy --locked -p celestina-lock-proto -p celestina-shell --all-targets -- -D warnings
cargo test --locked -p celestina-lock-proto -p celestina-shell
```

**The nest rule** (every task that runs a binary): start only `celestina/scripts/dev-session.sh` (never `--session`, never the author's niri config), with `--own-bus` and the private fake-logind bus of the energy tests; stop it with the pid its own launcher recorded; check first that `$XDG_RUNTIME_DIR/celestina/lock-*.state` names no live pid for a display other than the nest's (the nest shares the desktop's `XDG_RUNTIME_DIR`).

---

## Unit CEL-2-H-a: the security core

Open: `scripts/worktree.sh open celestina CEL-2-H-a` (after P1); set the H-a ledger row to `active`.

### Task S: Spikes (Opus 5.5, high)

**Files:** none changed in the repository except the evidence note `celestina/docs/evidence/2026-10-XX-cel-2-h-spikes.md`; throwaway clients and scripts live in the scratchpad directory, outside the repository, and are not kept.

Each answer is an evidence entry (command, output, conclusion). No answer is patched into the spec; a contradiction is reported (stop point P2).

- [ ] **Step 1: Check P1.** `git log -1 --format=%H -- AGENTS.md`, `rg -n 'CEL-2-H' AGENTS.md docs/projects.toml celestina/docs/plans/active/2026-10-16-cel-2-parity.md`. If absent, stop and report.
- [ ] **Step 2: Nest probes with a throwaway sctk client** (a scratch crate outside the repo that depends on `smithay-client-toolkit` 0.21.1; run only inside the nest, killed by its recorded pid):
  - **Q1** kill the client between `lock()` and `locked`: does niri end `Locked` with a dead client (nest capture and niri log)?
  - **Q2** after killing a locked client, does a second client's `lock()` get accepted ("replacing existing dead lock") and can `swaylock` take over and unlock? (R-3 shape; swaylock runs against the nest's `WAYLAND_DISPLAY` only, checked from the environment file of `dev-session.sh`.)
  - **Q4** is a `wp_single_pixel_buffer_v1` buffer with a viewport destination accepted as a lock surface's first buffer (no `dimensions_mismatch`)? Otherwise the zeroed shm cover is the only path.
  - **Q6** does `wl_keyboard.enter` reach the lock surface under the pointer and move with it (nest has one output; multi-output is live only, VAL).
  - **Q7** does a virtual keyboard (`wtype` against the **nest's** socket, its path read from the nest's environment file, never the real session) reach the nest's lock surface? If not, R-7 is dropped and T-V covers the path.
  - **Q11** does `ext-idle-notify` report `resumed` for input while locked (nest, short delays)?
  - **Q16** with fcitx5 started **inside the nest only** (private `XDG_RUNTIME_DIR` subdirectory for it if needed), can its keyboard grab receive keys typed on the lock surface? Never start an input method in the live session.
  - **Q19** note how often niri logs a failed locked frame during all runs; keep the log lines.
- [ ] **Step 3: Non-nest probes.**
  - **Q10** against B's private-bus logind fixture (`celestina/src/energy/logind.rs` tests show the pattern): how does a blocking zbus 5 signal iterator end within `JOIN_WAIT` on shutdown (drop of every connection clone, or an `eventfd` plus `poll`)?
  - **Q17** read logind's documentation and source (`systemd` `logind-dbus.c` inhibitor handling around `PrepareForSleep`): does logind wait for a delay inhibitor taken after the signal? Record the primary source, not memory. The policy assumes not; the live row `VAL-CEL2-LOCK-SUSPEND` measures it.
  - **Q18** in the worktree, two consecutive production-shaped builds (`cargo build --release --locked -p celestina` then `cargo build --release --locked -p celestina-lock -p celestina-lock-verify` once those exist; before they exist, a feature-flip build of `celestina-wayland`): does the shared `target/` keep both feature sets without rebuilding each other? Time the second. (Debug and scratch builds only; never the production entries.) If the packages do not exist yet, this probe is repeated in Task 3 step 6 and noted.
- [ ] **Step 4: Report.** The note ends with a table "Q, answer, changes spec section?". Any answer that changes §4 stops here for the author (P2). Do not edit the spec.

### Task 1: `celestina-lock-proto`, the shell policy, `IdleAction::Resumed`, `duties.lock` (Sonnet 5.5, high)

**Files:**
- Create: `celestina-rs/crates/celestina-lock-proto/Cargo.toml` (no dependencies at all), `celestina-rs/crates/celestina-lock-proto/src/lib.rs` (split into private modules `verdict.rs`, `exit.rs`, `lines.rs`, `bundle.rs`, `backoff.rs`, `service.rs` if `lib.rs` grows past a screen of re-exports), `celestina-rs/crates/celestina-shell/src/lock.rs`
- Modify: `celestina-rs/Cargo.toml` is already done by P1 (verify only); `celestina-rs/crates/celestina-shell/Cargo.toml` (path dependency on `celestina-lock-proto`, with a justification comment), `celestina-rs/crates/celestina-shell/src/lib.rs` (module list), `celestina-rs/crates/celestina-shell/src/idle.rs` (`IdleAction::Resumed`), `celestina-rs/crates/celestina-shell/src/settings.rs` (`Duties.lock`, lenient, default off)
- Test: unit tests inside each file

**Interfaces (produces):** in `celestina-lock-proto`: `enum Verdict { Authenticated, Refused, Unavailable, AccountRefused, Unsupported, ServiceMissing, BadRequest }` with `from_exit(Option<i32>, signaled: bool) -> Verdict` and `exit_code(self) -> i32` (spec §4.3 (c) table; `Authenticated` has no public constructor outside `from_exit` of status 0 and carries no payload); `enum LockExit` (0, 1, 2, 10, 11, 12, 13, 15, 16) with `from_status`; `enum StatusLine { Hello(u16), Requested, Locked, Attempt(AttemptKind), Unlocked }` and `AttemptKind { Refused, Account, Unavailable, Timeout, Unsupported, ServiceMissing }` with `parse_line(&[u8]) -> Result<StatusLine, LineError>` (≤ 64 bytes, ASCII, unknown line `LineError::Unknown`, never panics) and `render(&StatusLine) -> String`; `LineReader` (4 KiB bounded buffer, drops and reports an overlong line); `struct Bundle { prior_failures: u8, offset_known: bool, utc_offset_s: i32, outputs: Vec<OutputBlock> }` with `encode(&Bundle) -> Vec<u8>`, `decode(&[u8], meta: FileMeta { size: u64, seals_ok: bool }) -> Decoded` implementing every bound of spec §4.3 (a) including "header kept when a block fails"; `struct Backoff` (2, 4, 8, 16, 30, 30 s; `failures`; `record(Verdict)`; `wait()`); `enum Service { Production, Rehearsal }` with `parse(&str)`, `name()`, `paths()`, and `service_check(Service, existing: &[PathBuf]) -> Result<(), Verdict>`; constants `SECRET_MAX: usize = 512`, `ATTEMPT_DEADLINE`, `PROTOCOL_VERSION: u16 = 1`. In `celestina-shell::lock`: `LockPolicy` with the state, event, action and side-state of spec §4.4 (`State`, `Event`, `Action`, `LockedProof` with a private constructor and a `generation: u64`, an injected clock), `IdleAction::Resumed`.

- [ ] **Step 1: Characterise first.** From `celestina-rs/`: `cargo test --locked -p celestina-shell idle` and save the names of the idle tests (they must pass unchanged). From `celestina/`: `cargo test --locked energy` and save the names of the energy tests that mention the locker.
- [ ] **Step 2: Failing tests for the codec** (names are the contract): `t_b1_bundle_round_trip`, `t_b2_each_bound_of_the_bundle` (file size over 16 MiB, size not matching the header, a missing seal, bad magic, version 2, name length 0/65, side 0/641, invalid UTF-8 name, 9 outputs), `t_b3_header_is_kept_when_a_block_fails`, `t_b4_seeded_random_bytes_never_panic_or_over_allocate` (a seeded xorshift, 10 000 inputs, allocation bounded by the declared size), `eight_maximal_outputs_fit_16_mib`, `a_header_only_bundle_has_zero_outputs`, `status_line_round_trip_for_every_line`, `status_line_of_64_bytes_ok_65_refused_nul_refused` (Review Focus 4), `line_reader_drops_an_overlong_line_and_resynchronises`, `verdict_from_every_exit_code_and_signal`, `backoff_schedule_is_2_4_8_16_30_30`, `unavailable_does_not_count_and_waits_2_s`, `t_n1_service_check_over_given_path_lists` (production missing → `ServiceMissing`; rehearsal file present → `BadRequest`; no `stat`, no `/etc`), `service_parse_is_a_closed_enum`, `lock_exit_table_matches_the_spec`.
- [ ] **Step 3: Implement the codec** until green. The crate has `#![forbid(unsafe_code)]`, no dependencies, no allocation past the bound before validation. Run from `celestina-rs/`: `cargo test --locked -p celestina-lock-proto`.
- [ ] **Step 4: Failing tests for the policy** (`celestina-shell::lock`): `t_p1_every_row_of_the_event_table` (one test per row of spec §4.4, named after the row, including `lock_during_ending_serves_one_pending_request`), `t_p2_inhibitor_invariant_over_random_event_sequences` (seeded; held iff duty on, available, not `Locked`, `Awake`; none taken while `Preparing`), `t_p3_proof_only_in_locked`, `t_p4_relaunch_bound_three_per_sixty_seconds`, `t_p5_failures_carry_over_to_the_relaunch`, `t_p6_a_missing_pam_file_does_not_change_availability`, `t_p7_idle_intent_dropped_by_resumed_verb_intent_not`, `t_p8_no_event_or_input_derives_from_a_file_or_pid` plus `t_x1_stale_foreign_and_other_display_files_leave_the_policy_idle_without_a_proof`, `t_p9_exit_10_retries_twice_never_suspends_and_releases_the_inhibitor_when_preparing`, `watchdog_spawning_5s_requested_10s_ending_5s`, `notice_exactly_for_the_explicit_refusals_of_section_4_9`, `prepare_for_sleep_in_ending_keeps_the_inhibitor_and_spawns_again` (Review Focus 2), `unlocked_while_preparing_does_not_take_an_inhibitor` (Review Focus 3), `proof_generation_bumps_on_unlocked_exit_and_duty_off`, `exit_16_never_relaunches`, `exit_13_never_relaunches`, `duty_off_does_not_touch_the_running_lock`.
- [ ] **Step 5: Implement the policy** as a pure state machine (no IO, no threads, injected clock; actions are data). `LockedProof` has a private constructor in the module and is minted only in `Locked`. Keep `Notice` payloads as an enum of reasons, never text.
- [ ] **Step 6: `IdleAction::Resumed` and `duties.lock`.** Failing tests first: `t_i1_resumed_only_after_a_handled_lock_and_suspend_and_b_tests_unchanged` in `idle.rs`; `duty_lock_defaults_off_and_reads_leniently` and `duty_lock_round_trips_and_ignores_garbage` in `settings.rs`. Implement.
- [ ] **Step 7: Verify.** From `celestina-rs/`: `cargo fmt --all --check`; `cargo clippy --locked -p celestina-lock-proto -p celestina-shell --all-targets -- -D warnings`; `cargo test --locked -p celestina-lock-proto -p celestina-shell`; `cargo tree -e normal --prefix none -p celestina-lock-proto` prints only the crate itself. Then the standard set. Commit `celestina-maintenance: Add the lock vocabulary, the shell lock policy and the lock duty`.

**Exit criteria:** every id T-P1…T-P9, T-B1…T-B4, T-N1, T-I1 and T-X1 passes by name; B's idle tests are unchanged and green; `celestina-lock-proto` has zero dependencies; the idle step-2 refusal path is untouched until Task 6.

### Task 2: `celestina-lock-verify` and `pam.rs` (Sonnet 5.5, high)

**Files:**
- Create: `celestina/crates/celestina-lock-verify/Cargo.toml` (deps: `celestina-lock-proto`, `libc`, `nix` with `user`, `zeroize`, `rustix` with `process`, `fs`; each with a justification comment; links `pam` through the build via `println!("cargo:rustc-link-lib=pam")` in `build.rs` or `#[link(name = "pam")]` on the extern block, one only), `src/main.rs` (`#![forbid(unsafe_code)]`), `src/conversation.rs` (pure, `forbid(unsafe_code)`), `src/pam.rs` (`#![allow(unsafe_code)]`, `#![deny(unsafe_op_in_unsafe_fn)]`, about 80 lines, every `unsafe` block with its invariant), `src/hardening.rs` (dumpable, rlimit, tracer check, via rustix)
- Modify: `celestina/Cargo.toml` (workspace `members = ["crates/*", "."]` already picks it up; confirm the workspace `unsafe_code = "deny"` lint admits the module-level `allow`), `celestina/AGENTS.md` ("Local boundary": the second documented `unsafe` exception, `pam.rs`, modelled on the `egl.rs` line), `celestina/Cargo.lock` (via cargo, `zeroize` new)
- Test: unit tests in each file; `celestina/crates/celestina-lock-verify/tests/contract.rs` for the real binary

**Interfaces (produces):** the binary contract of spec §4.3 (c): argv `--protocol 1 --service <production|rehearsal-name>`, stdin = the secret (1..=512 bytes, no NUL, exactly as typed, then EOF), exit codes 0–6; `conversation::Conversation::respond(&[Prompt]) -> Result<Vec<Response>, ConvError>` pure (first `PAM_PROMPT_ECHO_OFF` gets the secret; a second echo-off prompt or any echo-on is `PAM_CONV_ERR` and marks `Unsupported`; `PAM_ERROR_MSG`/`PAM_TEXT_INFO` answered empty and counted, text never copied; `num_msg` outside `1..=32` is `PAM_CONV_ERR`).

- [ ] **Step 1: Failing tests, pure part** (`conversation.rs`, `hardening.rs`, `main.rs` argv): `t_v2_only_the_first_echo_off_prompt_gets_the_secret`, `echo_on_prompt_is_refused_and_unsupported`, `t_v3_messages_are_counted_never_copied`, `num_msg_zero_and_thirty_three_are_refused`, `allocation_failure_frees_every_response_built` (via an injected allocator seam), `t_v1_argv_stdin_exit_table` (every row of the exit table with a fake PAM seam), `t_v4_secret_bounds` (empty, 513 bytes, NUL, exact 512), `uid_zero_is_refused_before_pam`, `unknown_protocol_and_unknown_service_are_bad_request`, `t_h1_not_dumpable_and_core_limit_zero_after_hardening`, `t_h2_non_zero_tracer_pid_exits_6` (parse a fixture of `/proc/self/status`), `no_new_privs_is_never_set` (source scan plus `prctl(PR_GET_NO_NEW_PRIVS)` read in a test).
- [ ] **Step 2: Failing test for the constants:** `t_v5_pam_constants_match_the_installed_header` reads `/usr/include/security/_pam_types.h` and compares every constant of spec §4.11 (`PAM_SUCCESS`, `PAM_SYSTEM_ERR`, `PAM_BUF_ERR`, `PAM_PERM_DENIED`, `PAM_AUTH_ERR`, `PAM_CRED_INSUFFICIENT`, `PAM_AUTHINFO_UNAVAIL`, `PAM_USER_UNKNOWN`, `PAM_MAXTRIES`, `PAM_NEW_AUTHTOK_REQD`, `PAM_ACCT_EXPIRED`, `PAM_CONV_ERR`, `PAM_ABORT`, the four message styles, `PAM_DISALLOW_NULL_AUTHTOK`, `PAM_MAX_NUM_MSG`, `PAM_MAX_RESP_SIZE`); the test skips with a notice only if the header is absent.
- [ ] **Step 3: Implement the pure parts and `main`** (secret read into `Zeroizing<Vec<u8>>` with capacity 512 reserved up front; user from the real uid through `nix::unistd::User::from_uid`; the service enum from the proto crate; `service_check` before `pam_start` for production; refuse if the rehearsal file exists; hardening first thing: `set_dumpable_behavior(false)`, `RLIMIT_CORE 0`, then the tracer check; secret zeroed before `pam_end`).
- [ ] **Step 4: Implement `pam.rs`**: hand-written declarations of `pam_start`, `pam_authenticate`, `pam_acct_mgmt`, `pam_end`, `pam_conv`, `pam_message`, `pam_response`; the conversation callback is `extern "C"` and cannot unwind; responses are `libc::malloc`'d and NUL-terminated; `pam_authenticate(h, PAM_DISALLOW_NULL_AUTHTOK)` then `pam_acct_mgmt(h, 0)`; no `pam_setcred`, `pam_open_session`, `pam_chauthtok`. Each `unsafe` block carries a `// SAFETY:` invariant line. Then `celestina/AGENTS.md`: extend "Local boundary" with the exception (the author's approval of the spec is the "prior").
- [ ] **Step 5: Contract test against the real binary** (`tests/contract.rs`, no root, no `/etc` writes): `t_v8_rehearsal_service_with_no_such_file_answers_1_via_other` (outside `faillock`: `other` denies without touching the tally; confirm by reading `/run/faillock/<user>` before and after only if present, never writing it) and the case "rehearsal file present → 6" through `service_check` over a fed list; a bad protocol answers 6; the `ldd` of the test binary's sibling shows only `libpam`, `libc` and their dependencies.
- [ ] **Step 6: Verify.** From `celestina/`: `cargo fmt --all --check`; `cargo clippy -p celestina-lock-verify --all-targets --locked -- -D warnings`; `cargo test -p celestina-lock-verify --locked`; `rg -n 'unsafe' celestina/crates/celestina-lock-verify/src` lists only `pam.rs` (and the `unsafe` count is recorded for the evidence); `cargo tree -e normal --prefix none -p celestina-lock-verify` compared with the §4.14 allowlist (create `celestina/crates/celestina-lock-verify/tests/closure.allow` here; T-D1 proper lands in Task 4). Standard set. Commit `celestina-maintenance: Add the PAM verifier and its single FFI module`.

**Exit criteria:** T-V1…T-V5, T-V8, T-H1, T-H2 pass; `unsafe` exists only in `pam.rs`; `NO_NEW_PRIVS` appears nowhere; the verifier writes nothing to stdout/stderr; the secret is zeroed on every path (tests assert the buffer after each exit path).

### Task 3: The toolkit's lock profile (Sonnet 5.5, high)

**Files:**
- Create: `celestina/crates/celestina-wayland/src/lock.rs`
- Modify: `celestina/crates/celestina-wayland/Cargo.toml` (a `gl` feature, default on, gating `egl`, `blur`, `popup` and their crates; sctk `session_lock` feature; single-pixel-buffer/viewporter protocol features, each justified), `celestina/crates/celestina-wayland/src/{lib.rs,runtime.rs,handlers.rs,surface.rs,input.rs}`
- Test: unit tests in `lock.rs` and `input.rs`; `celestina/crates/celestina-wayland/tests/lock_profile.rs`

**Interfaces (produces):** the lock profile of spec §4.13: binds the session-lock manager, single-pixel buffer, shm, viewporter, fractional scale and seat, and never layer shell, `xdg_wm_base`, data device, primary selection, text input or background effect; `Role::Lock`; the cover committed on the first `configure` of every lock surface (single-pixel buffer scaled by the viewport, else a zeroed shm XRGB8888 buffer); `LockEvent::{Locked, Finished, SurfaceConfigured, OutputAdded, OutputRemoved}`; `UnlockCapability` handed once at `locked`, neither `Clone` nor `Copy`, `unlock(self)` consuming it; the lock profile does not stop on SIGINT/SIGTERM/SIGHUP (logs them); `Modifiers::caps_lock`; `Debug` of `Key` redacting keysym and text.

- [ ] **Step 1: Characterise the shell profile first.** `cargo test -p celestina-wayland --locked` and `cargo test --locked` in `celestina/`; save the test names. Those stay green unchanged; `celestina` itself must still build with the default `gl` feature and link EGL.
- [ ] **Step 2: Failing tests** (fake runtime / Wayland test doubles already used by the crate): `t_w1_first_commit_of_a_lock_surface_is_the_cover`, `t_w2_the_lock_profile_binds_exactly_its_list`, `t_w3_key_debug_shows_no_keysym_or_text` (format `{ev:?}` the way `runtime.rs` and `handlers.rs` log it and assert no typed character appears), `t_u1_unlock_capability_is_created_once_is_neither_clone_nor_copy_and_is_consumed` (a compile-time assertion without a new dev-dependency, plus a source scan that no public `unlock_session` exists), `lock_profile_does_not_stop_on_sigterm_sigint_sighup`, `an_output_added_before_and_after_locked_gets_exactly_one_surface`, `a_removed_output_destroys_its_surface`, `cover_falls_back_to_zeroed_shm_without_the_single_pixel_global`, `caps_lock_is_reported_from_the_xkb_modifier_state`, `the_gl_feature_off_does_not_link_egl_blur_or_popup` (a `cargo tree -e normal -p celestina-wayland --no-default-features` check for the GL crates, run as a test of the crate's own manifest or in Task 4's T-D1).
- [ ] **Step 3: Implement** `lock.rs`, the profile-dependent globals and signal policy in `runtime.rs`/`handlers.rs`, `Modifiers::caps_lock` and the redacting `Debug` in `input.rs`. Zero changes in behaviour for the shell profile.
- [ ] **Step 4: Feature split.** Gate `egl.rs`, `blur.rs`, `popup.rs` and their dependencies (`khronos-egl`, `wayland-egl`, `glow` as applicable) behind `gl`. Verify: `cargo check -p celestina-wayland --locked --no-default-features` and `cargo check -p celestina-wayland --locked` both pass; `cargo tree -e normal --prefix none -p celestina-wayland --no-default-features` lists no EGL or GL crate.
- [ ] **Step 5: Nest check of the cover (Q1/Q4 follow-through).** Using a tiny example in the crate's `examples/` (`required-features` not needed; not shipped) run inside the nest only: lock, cover commits, `locked` event arrives, process killed by its recorded pid; record the time to `locked`. If Task S answered Q4 negatively, use the shm cover only (the single-pixel path is then deleted in this task, not left dormant).
- [ ] **Step 6: Q18 re-check** if Task S could not run it (two feature sets in `target/`).
- [ ] **Step 7: Verify** (`cargo fmt`, `clippy`, `test` for `celestina-wayland`, then the standard set). Commit `celestina-maintenance: Add the toolkit's session-lock profile and the GL feature split`.

**Exit criteria:** T-W1…T-W3 and T-U1 pass; the toolkit has exactly one unlock path and it consumes a capability; the shell binary is byte-for-byte functionally unchanged (characterisation tests unchanged).

### Task 4: `celestina-lock` core (Sonnet 5.5, high)

**Files:**
- Create: `celestina/crates/celestina-lock/Cargo.toml` (deps of spec §4.14, each justified; `[features] fake-verifier = []`; `[[bin]] unlock_probe` with `required-features = ["fake-verifier"]`), `src/main.rs` (argv `--rehearsal`, hardening, tracer check, verifier pin, state file, run), `src/machine.rs`, `src/verify.rs`, `src/unlock.rs`, `src/state_file.rs`, `src/backdrop.rs` (header-only reader here: validates and keeps `prior_failures` and `offset_known`, no pixels yet), `src/hardening.rs`, `src/bin/unlock_probe.rs`, `tests/closure.rs`, `tests/closure.allow`, `tests/guards.rs`
- Modify: `celestina/Cargo.toml` only if the workspace needs the member explicit (it uses `crates/*`)
- Test: unit tests in each file; integration tests in `tests/`

**Interfaces (produces):** the lock-side state machine of spec §4.5 (`Connecting`, `Requesting`, `Prompt {field, status, wait_until, faded}`, `Verifying {attempt, deadline}`, `Unlocking`), the single unlock site (`unlock.rs` holds the only consumption of `UnlockCapability` and takes the `Authenticated` token), `verify.rs` (spawn from the pinned fd, write the secret, close, pidfd poll to the 15 s deadline, `pidfd_send_signal(SIGKILL)` and `kill_process_group` of its own group, the `Unavailable` verdict sent as soon as the kill is sent, reap bounded to 2 s, no new attempt while a previous pidfd is unreaped), status lines on stdout via `celestina-lock-proto`, exit codes per spec §4.3, `LockExit::Connection lost → 16`.

- [ ] **Step 1: Failing tests, machine** (no Wayland; fake runtime): `t_m1_every_transition_of_section_4_5`, `t_m2_stale_verdicts_are_discarded`, `t_m3_delay_schedule_2_4_8_16_30_30`, `t_m4_unavailable_waits_2_s_and_does_not_count`, `t_m5_rehearsal_never_reaches_unlocking`, `enter_with_an_empty_field_does_nothing`, `enter_while_waiting_does_nothing_and_keeps_the_countdown`, `double_enter_starts_one_attempt` (Review Focus 1), `keys_during_verifying_are_dropped`, `a_verdict_after_the_prompt_output_vanished_unlocks_or_denies_once_and_the_prompt_moves` (Review Focus 1), `t_k1_a_key_past_512_bytes_is_ignored_so_the_secret_is_never_refused_as_oversized` (Review Focus 6, with multibyte characters), `backspace_removes_the_last_char_and_escape_clears`, `t_m6_fade_after_the_token_delay_a_key_or_motion_restores_and_the_field_survives`, `t_o1_to_t_o3_outputs_added_removed_and_zero_outputs_at_locked` (fake runtime), `t_o4_connection_lost_after_requested_exits_16`, `prior_failures_from_the_header_seeds_the_backoff`.
- [ ] **Step 2: Failing tests, verify and unlock:** `t_v6_a_fake_verifier_that_sleeps_is_killed_at_the_shortened_deadline_group_included_and_the_verdict_is_unavailable` (Review Focus 7), `a_fake_that_exits_0_yields_authenticated_only_for_its_own_attempt`, `a_signalled_or_unknown_exit_is_unavailable`, `no_second_attempt_starts_before_the_previous_reap`, `t_u1_unlock_rs_is_the_single_call_site_and_authenticated_has_no_public_constructor` (type tests plus a source scan like the language guard's), `unlocked_is_printed_only_after_the_sync_round_trip_and_exit_is_0`.
- [ ] **Step 3: Failing tests, environment and hardening:** `t_e1_lock_and_verifier_get_only_the_allowlisted_environment` (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `MESA_SHADER_CACHE_DISABLE=true` for the lock; `PATH` only for the verifier), `no_override_variable_is_read` (source scan for `env::var` of anything but the allowlist), `t_e3_a_pinned_binary_replaced_on_disk_still_runs_the_pinned_bytes_and_a_script_is_refused` (this also answers Q9: `Command` executing `/proc/self/fd/N` of a CLOEXEC descriptor; if it does not work, stop and report: the spec's pinning mechanism changes), `t_h1_not_dumpable_after_hardening`, `t_h2_non_zero_tracer_pid_exits_15_before_requested`, `t_l1_no_typed_byte_appears_in_any_tracing_event_at_trace_level_through_a_full_attempt` (a capturing subscriber), `the_subscriber_filter_is_fixed_at_info`.
- [ ] **Step 4: Failing tests, dependency pin:** `t_d1_transitive_closure_equals_the_allowlist` runs `cargo tree -e normal --prefix none -p celestina-lock -p celestina-lock-verify` (the production build invocation of spec §4.14) and compares the deduplicated names with `tests/closure.allow`; `unlock_probe_is_absent_without_fake_verifier`; the lock's tree contains none of `celestina-shell`, `celestina-paint`, `celestina-ui`, `zbus`, `image`, `serde_json`, `niri-ipc`, `khronos-egl`, `femtovg`, `freedesktop-icons`, `resvg`.
- [ ] **Step 5: Implement** machine, verify thread (blocking `poll` on the pidfd, no busy loops), unlock site, state file (`$XDG_RUNTIME_DIR/celestina/lock-<WAYLAND_DISPLAY>.state`, mode 0600, sanitised display name, best-effort `flock`, content `pid <n>` only; nothing ever reads it for a decision), header-only bundle reading (bounded read on the `backdrop` worker, 2 s, 16 MiB; stdin not a memfd means no backdrop), rehearsal flag, `unlock_probe` (the same core with a fake verifier, rehearsal only).
- [ ] **Step 6: Guards as tests** (`tests/guards.rs`): the lock sources contain no `unwrap(`/`expect(`/`panic!`/`unsafe`; no `Command::new` outside `verify.rs`; no `std::fs::read*`/`File::open` of a path built from input; no `format!`/`Display` of a key's text or keysym; the namespace of nothing (the lock creates no layer surface); every `never` item of spec §6 that is structural names its test or scan in a comment of the guard. Each guard test cites the TM-n it enforces.
- [ ] **Step 7: Nest run, minimal.** In the nest only: run the built `celestina-lock --rehearsal` with the toolkit's cover and no painter yet; check `hello 1`, `requested`, `locked` lines in order; `SIGKILL` by recorded pid; the `ldd` of `celestina-lock` shows no `libEGL`, no `libfontconfig`. Record in the evidence.
- [ ] **Step 8: Verify** (`cargo fmt`, `cargo clippy -p celestina-lock --all-targets --locked -- -D warnings`, `cargo test -p celestina-lock --locked`, then the standard set). Commit `celestina-maintenance: Add the lock process core and its single unlock site`.

**Exit criteria:** T-M1…T-M6, T-K1, T-O1…T-O4, T-V6, T-U1, T-E1, T-E3, T-H1, T-H2, T-L1, T-D1 pass; the unlock site is unique; the closure equals the allowlist; Q9 is answered by T-E3.

### Task 5: The CPU painter, the view and the copy (Sonnet 5.5, medium)

**Files:**
- Create: `celestina/crates/celestina-lock/src/paint.rs` (over `tiny-skia`, glyphs via `cosmic-text` and `swash`, one embedded face loaded into an **empty** `fontdb::Database` handed to `FontSystem::new_with_locale_and_db`; no `load_system_fonts`, no fontconfig, no icon lookup), `src/view.rs` (scrim, the two capsules, the bullet row, status text, Caps Lock, no-keyboard message, rehearsal banner, placement on the focused output, damage rectangle, shm pool with two buffers per output), `src/copy.rs` (head `language-contract: product-copy`, the constants of spec §4.6), `celestina/crates/celestina-lock/assets/` is **not** created: the font is `include_bytes!("../../../../celestina-style/fonts/InterVariable.ttf")` (verify the relative path; it is outside the package, so check the registered `production_inputs` already include `celestina-style/fonts`)
- Modify: `celestina/crates/celestina-lock/Cargo.toml` if the dependency list needs `tiny-skia`, `cosmic-text` (no default features), `fontdb` (justified), `celestina/crates/celestina-tokens` usage for sizes and colours (add the `lock.idle_fade_after` token and scrim token only if the token generator owns them; if adding a token needs `celestina-style/tokens.toml` and the generator, **stop**: that is a suite change, report to the author instead of editing it here)
- Test: unit tests in `paint.rs`, `view.rs`

- [ ] **Step 1: Failing tests:** `t_f1_the_font_database_holds_exactly_the_embedded_faces_and_no_system_font_is_loaded`, `no_icon_lookup_exists` (source scan), `t_t1_field_and_status_text_tokens_over_the_scrim_composited_over_pure_white_reach_4_5_to_1` (the clock is not yet drawn in H-a; the contrast test is written for the field and status text now and extended in Task B3), `the_field_shows_one_bullet_per_char_and_never_the_text`, `the_prompt_is_drawn_only_on_the_focused_output`, `prompt_follows_pointer_enter_before_any_keyboard_enter_and_defaults_to_the_first_output_by_connector_name`, `a_painter_failure_keeps_the_covers_and_typing_blind_still_works` (F16), `faded_outputs_are_drawn_black_prompt_included`, `damage_is_the_prompt_rectangle_only_per_key_press`, `caps_lock_shows_the_message`, `no_keyboard_capability_shows_the_message`, `copy_constants_exist_and_are_spanish_only_in_copy_rs` (a scan that no Spanish literal appears outside `copy.rs`).
- [ ] **Step 2: Implement** the painter and view. No motion exists, so `reducedMotion` is trivially honoured (a comment states why). No D-Bus, no notifications, no user name, no media or window titles: the view takes only the machine's state.
- [ ] **Step 3: Nest check and R-10 precursor.** In the nest at scale 1.5 on the 4K window: run the lock with the painter, capture the cover then the prompt (`grim` against the **nest's** socket only), and record per-frame time of a key press (p50, p99) for H-15. If p99 exceeds 16 ms after damage tracking, report to the author before continuing (H-15's revisit condition); do not switch to EGL on your own.
- [ ] **Step 4: Verify** (`cargo fmt`, `clippy`, `test -p celestina-lock`, standard set, `ldd` check again). Commit `celestina-maintenance: Add the lock's CPU painter, prompt view and copy`.

**Exit criteria:** T-F1, T-T1 (field and status) pass; `ldd` shows no `libEGL`/`libfontconfig`; a capture of the cover and the prompt exists; the frame time is recorded.

### Task 6: Shell integration (Sonnet 5.5, high)

**Files:**
- Create: `celestina/src/lock/mod.rs` (`LockHandle`: duty on/off, availability to idle, routing of the policy's actions), `celestina/src/lock/supervisor.rs` (worker `lock`: pinning, spawn with `process_group(0)` and `env_clear()`, stdout reader, pidfd, liveness check before a proof, the sealed memfd bundle write, the PAM-file check, the notices, one `poll` over stdout, the pidfd and an `eventfd`), `celestina/src/lock/sleep.rs` (worker `lock-sleep`: its own system-bus connection, the delay inhibitor fd, the `PrepareForSleep` subscription, woken by the bus or an `eventfd`), `celestina/src/lock/bundle.rs` (header-only bundle in H-a: `outputs = 0`, `offset_known = 0`, `prior_failures` carried; positioned writes with `rustix::io::pwrite`, `memfd_create` with `MFD_ALLOW_SEALING | MFD_CLOEXEC`, the four seals, file offset asserted 0 before spawn), `celestina/tests/lock_supervisor.rs`, `celestina/tests/lock_sleep.rs`, `celestina/tests/lock_guards.rs`
- Modify: `celestina/src/energy/mod.rs` (`perform_idle(LockAndSuspend)` sends `Message::Lock(LockAndSuspend(Idle))` to the loop and never waits; `IdleSlot::sync` publishes the policy's availability on every change instead of a constant `false`; `IdleChain::resumed` returning `IdleAction::Resumed` wired to drop an idle-origin intent), `celestina/src/energy/logind.rs` (remove `locker_available` and `NO_LOCKER`; `Request::Power(Suspend)` without a proof is refused with the "suspend goes through the lock" reason; `LogindHandle::suspend(LockedProof)` is the one way to suspend; compare the proof's generation with the shared `AtomicU64` immediately before `Suspend` and drop a stale proof with a log; make the connect helper, `CALL_TIMEOUT` and `CONNECT_TIMEOUT` `pub(crate)` and reuse them from `sleep.rs`), `celestina/src/energy/verbs.rs` (`session suspend` always refused, pointing to `lock suspend`), `celestina/src/command.rs` (`Verb::Lock`, `Verb::LockAndSuspend`; no `unlock`), `celestina/src/message.rs` (`Message::Lock`), `celestina/src/app.rs` (narrow hook: owns `LockHandle`, follows `duties.lock` live), `celestina/src/copy.rs` (the `NOTICE_LOCK_*` and `NOTICE_PAM_MISSING` constants), `celestina/src/island/clock.rs` (**H-b only**; not here), `celestina/src/lib.rs`, `celestina/Cargo.toml` (path dependency on `celestina-lock-proto`, rustix features `fs`, `process`, `event` justified)
- Test: as listed

- [ ] **Step 1: Characterise first.** Save the names and results of the `energy` tests (`cargo test --locked energy`), the smoke's fifth run text (`celestina/scripts/smoke.sh`), and `Verb::parse` tests.
- [ ] **Step 2: Failing tests, supervisor** (fake lock binary that prints lines and exits, built as a test helper): `t_s2_line_parsing_bounds_exit_table_relaunch_epipe_tolerance_and_notices_rate_limited_to_one_per_10_s`, `t_p10_the_proof_is_dropped_when_the_childs_pidfd_is_readable_at_hand_off`, `spawn_uses_a_process_group_env_clear_and_the_allowlist`, `the_pinned_descriptor_is_what_executes` (reuses Task 4's T-E3 mechanism), `a_binary_failing_fstat_is_not_available` (not regular, group-writable, other-writable, not executable, wrong owner), `a_missing_pam_file_logs_and_posts_the_notice_but_availability_stays_true`, `spawning_older_than_5s_kills_the_child_through_its_pidfd`, `requested_older_than_10s_is_not_killed`, `a_status_line_of_65_bytes_or_with_a_nul_is_dropped_with_a_log`, `the_child_outlives_the_supervisor_and_ignores_epipe` (kill the supervisor test harness, child keeps running, killed by recorded pid at the end).
- [ ] **Step 3: Failing tests, bundle:** `t_z1_the_memfd_offset_is_0_at_spawn_and_the_childs_first_read_returns_the_magic`, `a_bundle_with_8_maximal_outputs_fits_16_mib` (via the proto codec, shared with Task 1's test), `a_missing_offset_sets_offset_known_0`, `the_memfd_is_sealed_against_grow_shrink_write_and_seal`.
- [ ] **Step 4: Failing tests, sleep and logind** (with B's private-bus logind fixture; never the real system bus): `t_s1_inhibit_sleep_delay_released_on_locked_reheld_after_unlock_and_suspend_false_recorded_only_after_locked`, `no_inhibitor_is_taken_while_preparing` (Review Focus 3), `shutdown_joins_within_the_bound` (answers Q10 in practice), `t_g2_a_proof_whose_generation_is_older_than_the_current_one_is_dropped_before_suspend`, `suspend_without_a_proof_is_refused`, `t_g1_no_subscription_to_logind_lock_or_unlock_signals` (a source scan of `celestina/src/lock/` and `celestina/src/energy/`), `unlocked_proof_cannot_be_forged_outside_the_policy_module` (compile-fail style via a private-constructor scan).
- [ ] **Step 5: Failing tests, commands and wiring:** `t_c1_lock_and_lock_suspend_parse_unlock_and_every_other_word_are_refused_and_session_suspend_is_refused_with_the_duty_on_or_off`, `idle_step_2_is_accepted_only_when_the_policy_publishes_availability`, `idle_resumed_drops_only_idle_origin_intents`, `refusals_of_explicit_requests_post_a_notice_and_idle_refusals_do_not`, `at_most_one_identical_notice_per_10_s`, `one_suspend_intent_at_a_time_and_the_origin_is_logged` (TM-26 / T-P1), `a_shell_restarted_under_a_running_lock_is_idle_and_asks_the_compositor` (T-P8 at integration level), and the smoke's fifth-run text update.
- [ ] **Step 6: Implement** in this order: `bundle.rs`; `supervisor.rs`; `sleep.rs`; `mod.rs`; the energy rewiring (removing `locker_available`/`NO_LOCKER` and every dead branch in the same step, no second active path); verbs; `Message::Lock`; copy constants. Notice delivery is a bounded `Notify` call (`CALL_TIMEOUT`) to the owner of `org.freedesktop.Notifications`, best effort, made on the lock worker.
- [ ] **Step 7: Guard tests** (`tests/lock_guards.rs`): no `std::process::Command` in `celestina/src/lock/` outside `supervisor.rs`; no `unwrap(`/`expect(`/`panic!`/`unsafe`; no `PATH` lookup (`Command::new` receives a descriptor path); no `Unlock` verb or method exists anywhere in `celestina/src`; no `Secret`-like type or password in `message.rs`, `bus.rs`, `command.rs`; each cites TM-n.
- [ ] **Step 8: Smoke and check.** `celestina --check` and `celestina/scripts/smoke.sh --binary PATH` from the **worktree's debug build** only, nest only, own bus.
- [ ] **Step 9: Verify** (`cargo fmt`, `clippy`, `test --workspace`, standard set). Commit `celestina-maintenance: Wire the session lock into the shell`.

**Exit criteria:** T-P10, T-S1, T-S2, T-Z1, T-G1, T-G2, T-C1, T-E3 (shell side) pass; `NO_LOCKER` and `locker_available` no longer exist; the idle chain's second step is available iff the policy says so; B's energy tests pass unchanged except the documented refusal text.

### Task 7: Production entries, PAM text, README section, smoke and rehearsal (Sonnet 5.5, medium)

**Files:**
- Create: `celestina/contrib/pam.d/celestina-lock` (the three-line text of spec §4.11, exactly; never installed by a script), `celestina/scripts/lock-rehearsal.sh`
- Modify: `celestina/scripts/build-production.sh` (builds the shell as before and the two lock binaries in an invocation of their own: `cargo build --release --locked -p celestina-lock -p celestina-lock-verify`, so feature unification cannot switch `gl` on for the lock), `celestina/scripts/verify-production.sh` (verifies all three binaries, checks `unlock_probe` symbols are absent from the shipped `celestina-lock`, runs T-D1's closure check on the production command, records `ldd` expectations), `celestina/scripts/deploy-production.sh` (installs the two binaries beside `celestina`; does not install the PAM text), `celestina/scripts/status-production.sh` and `celestina/scripts/smoke.sh` (two more binaries; assertions only), `celestina/README.md` (a section "Installing the lock's PAM service" with the author's one command and the recovery path of spec §8), `celestina/AGENTS.md` (the lock's copy file line, if not already added in Task 2)
- Test: `scripts/test-production-artifacts.sh`-style checks only where the existing repo tests cover these entries; the scripts are run in the worktree **only in dry or check modes**; never the production entries

- [ ] **Step 1: Failing check first.** Extend the existing smoke/verify expectations (assertions, not behaviour) so they fail until the scripts know three binaries.
- [ ] **Step 2: Write `lock-rehearsal.sh`** per spec §9.2: runs inside `dev-session.sh --own-bus` with the system bus pointed at B's fake logind on a private bus; refuses to start if `/etc/pam.d/celestina-lock-rehearsal` exists; refuses to start if any `$XDG_RUNTIME_DIR/celestina/lock-*.state` other than the nest's own display names a live pid; tears the nest down by the pid its own `.kdl` run recorded; **no `pkill`, no `killall`, no name-based kill anywhere** (a shellcheck-style scan test greps the script). Steps R-1…R-9 as in the spec table.
- [ ] **Step 3: Run R-1…R-9 in the nest** and record each with captures and the time to `locked`: R-1 (cover then prompt within 1.5 s), R-2 (second `celestina-lock` exits 10), R-3 (SIGKILL by pid: relaunch, niri logs "replacing existing dead lock"; stop the shell; `swaylock` against the nest's display only), R-4 (kill between `requested` and `locked`), R-5 (`celestina quit` while locked, start again: untouched lock, `lock suspend` refused, no `Suspend` on the fake logind), R-6 (`/proc/<pid>/environ` and `mem` unreadable by the user), R-7 (only if Q7 said a virtual keyboard reaches the nest's lock; `--rehearsal` service only), R-8 (`unlock_probe` built with `fake-verifier`: `locked`, `unlocked`, exit 0, niri logs "unlocking session"), R-9 (idle with short test delays and the fake logind: screen off, lock, `Suspend` only after `locked`; input before `locked` cancels it).
- [ ] **Step 4: Verify scripts without running production:** `sh -n` on every changed script; `bash scripts/check-architecture-contract.sh`; the repo's existing script tests that cover these files (`scripts/test-production-artifacts.sh`, `scripts/test-production-common.sh`); the standard set. Commit `celestina-maintenance: Add the lock to the production entries and a nest rehearsal`.

**Exit criteria:** the PAM text equals the spec's; no script installs, edits or reads `/etc` beyond the rehearsal's read-only refusal check; R-1…R-9 recorded (or R-7 dropped by Q7 with the reason); the scripts contain no name-based process kill.

### Task 8: ADRs, documents, validation rows, evidence (Sonnet 5.5 for ADRs; Haiku 5.5 for rows)

**Files:**
- Create: `celestina/docs/evidence/2026-10-XX-cel-2-h-a.md`, the ADR files under `celestina/docs/decisions/` (next free numbers at landing, after whatever CEL-2-I has taken; draft H-1…H-18 from spec §14, one ADR per group the author prefers, default one file `00NN-session-lock.md`)
- Modify: `celestina/{README,STATUS,ROADMAP,VALIDATION}.md`, `celestina/docs/handover.md`, `celestina/docs/decisions/README.md` (index), the H-a ledger row (`planned` → `active`)

- [ ] **Step 1: Evidence record.** A table with every test id of spec §9.1 and its result and the command that ran; R-1…R-9 with captures and the time to `locked`; `cargo tree -e normal` of both new packages; `ldd` of `celestina-lock-verify` (only `libpam`, `libc` and dependencies) and of `celestina-lock` (no `libEGL`, no `libfontconfig`); the count and location of `unsafe` blocks (all in `pam.rs` and `egl.rs`); the spike answers; the residuals the author must accept (TM-13, F24, F33, the X2 limit); what did **not** run (the live session, real PAM, real logind, hotplug). Contains the exact line ``- **Resolved architecture debt:** `path` `` only if a baseline row actually shrinks.
- [ ] **Step 2: `VALIDATION.md` rows (author only, non-blocking):** `VAL-CEL2-LOCK-PAM`, `-LIVE`, `-SUSPEND`, `-HOTPLUG`, `-RECOVERY`, `-NOTICES`, `-COEXIST` exactly as spec §9.4, plus rollback ("switch `duties.lock` off, leave `swaylock` on `Mod+Shift+A`").
- [ ] **Step 3: Documents.** README (product and structure), STATUS (volatile truth: duty off, PAM not installed), ROADMAP (the unit's evidence), handover (how to run the nest rehearsal; the recovery path of spec §8).
- [ ] **Step 4: Ledger.** Confirm the H-a row says `active`, its text matches the Ledger section above, and it stays contiguous with the table.
- [ ] **Step 5: Verify:** the standard set, with special attention to `bash scripts/check-documentation-contract.sh` and `python3 scripts/check-language-contract.py`. Commit `celestina-maintenance: Add the lock evidence, decisions and live validation rows`.

**Exit criteria:** every spec §6 item and every §7 row maps to a test, a rehearsal step or a structural reason in the evidence table; the documentation and language contracts pass.

### Task 9: Review A over H-a (Opus 5.5, high; not the implementer)

Not an implementation task. A fresh Opus 5.5 subagent at high reasoning receives the whole unit diff against `origin/main`, the spec and this plan, and the prompt says: read-only, no commits, no `git stash`, no process kills by name.

- [ ] **Step 1:** §4.4 and §4.5 transition by transition against the code and tests; every §7 row against a test or rehearsal step; every §6 item against a test, guard or structural reason; the root invariants (no production `unwrap`/`expect`/`panic!`, no blocking IO on a loop, typed errors, bounded input, joined workers); the language contract. Written report appended to the evidence record.
- [ ] **Step 2:** Exit criterion: no open finding of severity high; each finding fixed with a test (Task 11) or accepted in writing by the author (P5).

### Task 10: Review B, adversarial security, over H-a only (Opus 5.5, highest reasoning, clean context)

A clean session with no implementer or review-A context, run in addition to Task 9, never instead of it. Inputs: spec §5–§7, the H-a diff and the built debug binaries (never the live session). The reviewer tries to (1) unlock without the password, (2) obtain the password, (3) get the machine asleep unlocked, (4) lock the author out beyond §8. It reads every `unsafe` block against its invariant, the FFI constants and allocator pairing, panics across FFI, the bundle and line parsers with hostile input, signals, pid reuse, attempt races (double Enter, verdict after output hotplug, unlock during `PrepareForSleep`), environment and argv.

- [ ] **Step 1:** the written report is linked from H-a's evidence record.
- [ ] **Step 2:** Exit criterion: zero open findings for (1) and (3); (2) and (4) fixed or accepted in writing as residuals (P5). A fix after either review is reviewed again by the same reviewer.

### Task 11: Fixes and re-review (the implementer's model; the same reviewer)

- [ ] **Step 1:** for each finding: a failing test first, then the fix, then the standard set; one commit per coherent fix, `celestina-maintenance: <imperative>`.
- [ ] **Step 2:** the same reviewer re-reviews the fix. Close when both reviews have no open finding.
- [ ] **Step 3:** final standard set; `celestina --check`; smoke from the debug build in the nest; update the evidence record. **Stop (P4):** the author lands with `python3 scripts/land-unit.py unit/celestina/CEL-2-H-a --kind milestone --summary "<first own commit subject>"`. The landing builds, verifies and deploys; the duty stays off; the live shell is not replaced; the PAM service is not installed.

---

## Unit CEL-2-H-b: the presentation

Open after P4: `scripts/worktree.sh open celestina CEL-2-H-b` (from the new `origin/main`); set the H-b ledger row to `active`. **H-b may not change the bundle format, the status lines, the verifier contract, or any row of spec §4.4 and §4.5 except the clock's redraws.** If a task seems to need it, stop, record it, and reopen H-a's Review B (spec §0, §10).

### Task B1: Wallpaper thumbnails and the bundle's pixel blocks (Sonnet 5.5, medium)

**Files:**
- Modify: `celestina/src/wallpaper/surface.rs` and `celestina/src/providers/wallpaper.rs` (derive on the wallpaper worker, from each successful decode, a thumbnail of at most 640 px a side with a three-pass box blur, kept in the app's wallpaper state at ≤ 1.6 MiB per output; no thumbnail when `duties.wallpaper` is off), `celestina/src/wallpaper/mod.rs` as needed, `celestina/src/lock/bundle.rs` (copy the thumbnails into the bundle at spawn time: the first 8 outputs by connector name, each block per the frozen format; RGBA8 opaque), `celestina/src/lock/supervisor.rs` (read the offset afresh per spawn: calls `read_utc_offset()`, see B3; until B3 lands in the same unit, keep `offset_known = 0`)
- Test: `celestina/tests/wallpaper_thumbnail.rs`, extend `celestina/tests/lock_supervisor.rs` and the bundle tests

- [ ] **Step 1: Characterise** `celestina/tests/wallpaper_decode.rs` and the wallpaper tests (unchanged and green).
- [ ] **Step 2: Failing tests:** `thumbnail_is_at_most_640_px_a_side_and_1_6_mib`, `three_pass_box_blur_matches_a_reference_on_a_fixture`, `a_failed_decode_yields_no_thumbnail_and_a_black_backdrop`, `no_thumbnail_exists_with_the_wallpaper_duty_off`, `the_bundle_carries_one_block_per_output_up_to_8_by_connector_name`, `a_ninth_output_gets_no_block` (F40), `the_bundle_still_validates_with_the_h_a_decoder_unchanged` (proves the format is untouched: the proto crate has zero diff), `eight_maximal_outputs_fit_16_mib`.
- [ ] **Step 3: Implement.** Bounded work on the wallpaper worker, never on the loop; thumbnails dropped when the output disappears.
- [ ] **Step 4: Verify** (`git diff --stat celestina-rs/crates/celestina-lock-proto` is empty; standard set). Commit `celestina-maintenance: Keep a blurred thumbnail per output and send it to the lock`.

**Exit criteria:** the proto crate is byte-identical to H-a's; T-Z1 still passes.

### Task B2: The lock's backdrop worker and the scrim (Sonnet 5.5, medium)

**Files:**
- Modify: `celestina/crates/celestina-lock/src/backdrop.rs` (the pixel part: validated blocks → one upscaled base per output on the `backdrop` worker, cached; a bundle invalid or slow means black), `celestina/crates/celestina-lock/src/view.rs` (the scrim token composited over the backdrop; the field's contrast does not depend on the picture), `celestina/crates/celestina-lock/src/paint.rs` as needed
- Test: unit tests in `backdrop.rs`, `view.rs`

- [ ] **Step 1: Failing tests:** `an_upscaled_base_is_computed_once_per_output_and_cached`, `only_the_damaged_rectangle_is_redrawn_over_the_cached_base`, `an_invalid_or_slow_bundle_keeps_black_and_the_prompt`, `outputs_without_a_block_stay_black_with_the_scrim`, `the_worker_reads_within_2_s_and_16_mib` (reuse the header-only reader from Task 4), `t_t1_field_and_status_text_over_the_scrim_composited_over_pure_white_still_reach_4_5_to_1_over_a_white_backdrop`, `no_decoder_is_linked` (closure unchanged: T-D1 still equals `closure.allow`), `the_lock_opens_no_user_file_for_the_backdrop` (source scan).
- [ ] **Step 2: Implement.** The upscale is the one full-output operation and runs on the worker; per-frame cost stays damage-limited. Do not change the machine's transitions.
- [ ] **Step 3: Verify** (`closure.allow` unchanged; `ldd` unchanged; standard set). Commit `celestina-maintenance: Draw the dimmed backdrop under the lock prompt`.

**Exit criteria:** T-D1 passes with an unchanged allowlist; contrast holds over the brightest possible backdrop.

### Task B3: The clock and the date (Sonnet 5.5, medium)

**Files:**
- Create: `celestina/crates/celestina-lock/src/clock.rs` (a `timerfd` on `CLOCK_REALTIME`, absolute at the next minute boundary, `TFD_TIMER_CANCEL_ON_SET`, re-armed on `ECANCELED`; formats from `SystemTime` and the bundle's `utc_offset_s`; spawns no `date`, reads no time-zone file)
- Modify: `celestina/src/island/clock.rs` (adds an uncached `read_utc_offset()` with the same `date +%z` parse; the Island keeps its cached call; its staleness is noted for a separate bug unit, not changed), `celestina/src/lock/supervisor.rs` (calls `read_utc_offset()` on the lock worker on every spawn, off the loop; a failed read sets `offset_known = 0`), `celestina/crates/celestina-lock/src/view.rs` (clock and date lines; no clock when `offset_known = 0`, the date and field remain), `celestina/crates/celestina-lock/src/copy.rs` (weekday and month names), `celestina/crates/celestina-lock/src/machine.rs` only to receive the clock tick as a redraw event
- Test: unit tests in `clock.rs`, `view.rs`; `celestina/tests/lock_supervisor.rs`

- [ ] **Step 1: Failing tests:** `the_timer_fires_at_the_next_minute_boundary`, `a_clock_step_reports_ecanceled_and_rearms`, `resume_from_suspend_redraws` (simulated realtime jump), `format_uses_the_bundle_offset_and_no_time_zone_file`, `offset_known_0_hides_the_clock_and_shows_the_date`, `read_utc_offset_is_uncached_and_fails_to_none`, `the_island_keeps_its_cached_offset` (characterisation of `island/clock.rs` unchanged), `at_rest_one_frame_a_minute_and_one_per_second_only_during_a_countdown`, `t_t1_the_large_clock_text_over_the_scrim_reaches_3_to_1` (extends the earlier contrast test), `the_machine_transitions_are_unchanged_by_the_clock` (T-M1 still passes without edits).
- [ ] **Step 2: Implement.** A daylight-saving change during one lock shows the offset read at its spawn (accepted, spec §4.8).
- [ ] **Step 3: Verify** (standard set; `closure.allow` unchanged). Commit `celestina-maintenance: Add the lock clock and date on a realtime timer`.

### Task B4: Rehearsal R-10, captures and validation rows (Sonnet 5.5 medium; Haiku 5.5 low for rows)

**Files:**
- Modify: `celestina/scripts/lock-rehearsal.sh` (R-10 only), `celestina/VALIDATION.md` (`VAL-CEL2-LOCK-BACKDROP`, `VAL-CEL2-LOCK-CLOCK` as spec §9.4), `celestina/README.md` if it names the black backdrop
- Test: the rehearsal itself, nest only

- [ ] **Step 1: Run R-10** in the nest at scale 1.5 on the 4K window with a real wallpaper thumbnail: p50 and p99 of a key press and of the minute redraw; captures of the dimmed backdrop with the clock; if p99 exceeds 16 ms record it for the author (H-15's revisit), do not change the painter.
- [ ] **Step 2: Rows.** Add the two rows; note in the backdrop row that the wallpaper duty off (coexistence with Noctalia) leaves the backdrop black.
- [ ] **Step 3: Verify** (standard set; `sh -n`; no name-based kill in the script). Commit `celestina-maintenance: Rehearse the lock backdrop and add its live validation rows`.

### Task B5: Ledger row, evidence, STATUS (Haiku 5.5, low)

**Files:** Create `celestina/docs/evidence/2026-10-XX-cel-2-h-b.md`; modify `celestina/STATUS.md`, `celestina/ROADMAP.md`, the H-b ledger row (`planned` → `active`).

- [ ] **Step 1:** evidence: T-F1 (unchanged), T-T1 (extended), T-Z1, T-D1 (allowlist unchanged), the new tests by name, R-10 and the captures, a statement with the command output that `git diff origin/main -- celestina-rs/crates/celestina-lock-proto` is empty and that no §4.4/§4.5 row changed.
- [ ] **Step 2:** standard set; commit `celestina-maintenance: Add the lock presentation evidence`.

### Task B6: Review A over H-b (Opus 5.5, high; not the implementer)

- [ ] **Step 1:** the unit's whole diff against the spec; additionally verify that H-b changed none of what spec §0 freezes (the bundle format, the status lines, the verifier contract, any row of §4.4, §4.5 except the clock's redraws). Report appended to the evidence.
- [ ] **Step 2:** If H-b touched anything §0 freezes, **Review B runs again over that change** before the unit closes (spec §10). Exit: no open high finding.

### Task B7: Fixes and re-review

- [ ] Same method as Task 11 (failing test, fix, standard set, same reviewer). Then **stop**: the author lands with `python3 scripts/land-unit.py unit/celestina/CEL-2-H-b --kind milestone --summary "<first own commit subject>"`.

---

## Self-review

- **Coverage of the spec:** §4.1/4.2 in Tasks 4, 6, 7; §4.3 in Task 1 (codec, frozen from H-a) with the H-b payload in B1/B2; §4.4 in Tasks 1 and 6; §4.5 in Task 4; §4.6 in Tasks 3 and 5; §4.8 in B3; §4.9 in Task 6; §4.10 across Tasks 4 and 6; §4.11 in Task 2 and Task 7 (PAM text); §4.12 in Task 7 (README) and `VAL-CEL2-LOCK-COEXIST`; §4.13/4.14 in Tasks 2, 4 and 7 (T-D1); §5 by the guards in Tasks 4 and 6; §7 by the rehearsal in Task 7 and the evidence map of Task 8; §8 in the README, handover and `VAL-CEL2-LOCK-RECOVERY`; §9.1 test ids: T-P1…T-P10, T-I1, T-X1 (Task 1, 6), T-B1…T-B4, T-N1 (Task 1), T-C1, T-G1, T-G2, T-Z1, T-S1, T-S2 (Task 6), T-M1…T-M6, T-K1, T-O1…T-O4, T-U1, T-V6, T-E1, T-E3, T-H1, T-H2, T-L1, T-D1 (Task 4), T-V1…T-V8 (Task 2), T-W1…T-W3 (Task 3), T-F1, T-T1 (Task 5, extended in B2/B3); §9.2 R-1…R-9 in Task 7 and R-10 in B4; §10 reviews in Tasks 9, 10, B6; §11 models in the table above; §12 Q-questions in Task S (Q1, Q2, Q4, Q6, Q7, Q10, Q11, Q16, Q17, Q18, Q19), Q9 in Task 4 (T-E3), Q8 and Q12 (time to `locked`) in Tasks 5/7 and the author's live row, Q14/Q15 are the author's, Q3 in the Task S dead-client capture, Q5 and Q13 moot; §13 in P1; §14 in Task 8.
- **Order argument:** the toolkit (3) and the verifier (2) are independent and may run in parallel in separate files; the lock core (4) needs both; the shell integration (6) needs the policy (1) and a real child (4); S precedes 3 because Q1/Q4/Q6 may reshape the profile; H-b opens only after P4 so the frozen format is real.
- **Not planned on purpose:** logind `Lock`/`Unlock` handling, other authentication factors, AT-SPI on the lock screen, a keyboard-layout indicator, EGL painting, any edit to `/etc`, `~/.config/niri` or `swaylock`'s binding, any change to the Island's cached offset, any live-session run.
