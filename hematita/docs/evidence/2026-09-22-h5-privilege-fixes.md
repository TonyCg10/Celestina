# The foreign-process path and the action outcome races — H5-C

- **Date:** 2026-09-22
- **Scope:** `H5-C` of
  [`../plans/archive/2026-09-22-h5-services.md`](../plans/archive/2026-09-22-h5-services.md):
  the whole-unit review's correction of `H5-B` — foreign-process actions
  reachable, outcome precedence over the ownership note, an action token
  against two actions racing on one outcome, a dead bus connection reopened,
  scope-aware `pending` wording, and the toggle-state, anchoring, `STATUS`
  and evidence corrections
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; `polkitd` running, **no authentication agent**
- **Artifact:** `hematita/target/production-artifact.toml` (verified, not
  deployed)

## What changed

1. **The privileged path was unreachable** (critical). Both action buttons in
   `ProcessTable.qml` were `enabled` only on `selectedRow().actionable`, so a
   foreign row disabled them and `privilege::signal_as_root` could never be
   called from the window. Both are now enabled whenever a row is selected:
   `actionable` only words the bar, and the hub is what decides whether a
   signal goes straight to the kernel or through a prompt. Terminar sends
   directly through the hub; Matar goes through `ConfirmDialog` with a
   question that names the owner and says an authorisation will be asked for
   (`killQuestion`).
2. **The ownership sentence shadowed every outcome** for exactly the rows
   whose outcome matters most (critical). The note now shows the outcome first
   when `processes.actionPid === row.pid` and `actionOutcome !== ""`; the
   ownership line is what it says otherwise.
3. **Two actions racing on one `actionOutcome`.** Both hubs keep a monotonic
   `action_token: u64`. Every action — and `clear_action`, and the refusal
   that answers without a thread — takes the next token; the worker captures
   the one it was asked under, and the queued closure writes the outcome only
   while `publish::still_current(token, hub.action_token)`. A `pkexec` prompt
   that stands for minutes and is answered after the person asked for
   something else is dropped rather than shown for the newer question. The
   compare helper has its own test, the same discipline `publish::accepts`
   applies to ticks.
4. **A dead bus connection was never replaced.** `list_units` now answers
   `Result<Vec<Unit>, ListFailure>`, where `ListFailure` carries the reason
   and whether the connection is worth keeping: a `MethodError` proves the
   manager replied, so the connection stays and the section is `Malformed`;
   anything else is a connection that is finished, so `section_of_bus` puts
   the slot back to `None` and the next service tick opens a new one.
5. **`pending` was worded as an authorisation wait for user units too**, which
   is not true — the person's own manager authorises nothing.
   `ServicesPage.outcomeText()` now asks `actionScope()` (the acted unit's row
   if still listed, else the selection's) and says
   `qsTr("%1: en curso")` for a user unit, keeping the authorisation wording
   for a system one.
6. **The three toggles' `checked: true` literals** are gone: the glyphs are
   written from the hub once in `Component.onCompleted`, and the toggle writes
   the hub back.
7. **`weave()` assigned the rows before re-anchoring**, so a shorter model
   could make the list emit `currentIndexChanged` and be read as the person
   moving the selection, clearing an answer nobody had seen. Both pages now
   make the assignment (and, in `ProcessTable`, the fold's re-layout) under
   `anchoring`.
8. **`STATUS.md`** said killing is asked in `KillDialog`; it names
   `ConfirmDialog` now, with the history in parentheses.
9. **The prompt window is named as a limit** below.
10. **`PENDING` moved to `publish.rs`**, imported by both hubs, beside
    `still_current`: the token that is not one of
    `hematita_core::services::Outcome`'s words lives with the other
    publication policy rather than in one hub that the other has to depend on.

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 12 ./target/release/hematita
```

## Result

- **Exit:** 0 for the build, the three release-profile checks and the
  verification; `124` (still alive) for the 12-second run.
- **Observed:** `cargo test --release --all-targets` — `20 passed; 0 failed`
  (the new `still_current` test added to `H5-B`'s nineteen).
  `clippy -D warnings` and `fmt --all --check` clean.
  `verify-production.sh`: `hematita-core` `67 passed`, captures `11 passed`,
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline
  warning(s))`, `smoke: OK — binary alive for 10 s, every section was shown,
  the first row published the CPU contract, the Sensors page published chips,
  the Services page listed system units, no QML errors, no auto-bindings`.
  The 12-second run printed `hematita-shape cpu 3 60`,
  `hematita-sensors 10 44` and `hematita-services 143 true false`, and
  `grep -cE 'TypeError|ReferenceError|Unable to assign|Cannot read property'`
  answered `0`. The two `D-Bus` lines in that log are the deliberately absent
  session bus of an isolated run, which is the state the page must render.
- One compilation failure occurred and was fixed before the cycle completed:
  moving `PENDING` left a stale `use crate::services::PENDING;` in
  `processes.rs`.

## Limits

- **No action was performed.** No `StartUnit`/`StopUnit`/`RestartUnit`, no
  signal, direct or through `pkexec`, in any run. The corrected paths are
  proven by their unit tests and by reading the code.
- **The prompt window is not covered by the re-validation.** The uid and
  start-ticks re-check against `/proc` happens *before* `pkexec` is spawned.
  A polkit prompt can stand open for minutes, and nothing re-checks the PID
  when the person finally answers: if the target died in that window and the
  kernel handed its number to another process, the `kill` that then runs as
  root would signal the new one. Closing that window needs a handle rather
  than a number — the pidfd-based signalling ADR 0010 names in its "Revisit
  when", which is not available through this path today. The action token
  (finding 3) bounds only what is *reported*, not what is signalled.
- **This session has no authentication agent**, by the author's order, so
  every privileged path can only answer `no-agent` here; that the real
  outcomes are the ones ADR 0010 names is `VAL-H5`.
- Keyboard, focus containment, hover and the row colours are unproven
  offscreen; a unit's state change after an action, and the wording of the
  foreign-row question in front of a person, are `VAL-H5`.
- Nothing was installed: that is `H5-Z`.

## Follow-up

`H5-Z` (implementation exit and 0.6.0). `VAL-H5` is recorded pending in
`VALIDATION.md` and does not block this closure.
