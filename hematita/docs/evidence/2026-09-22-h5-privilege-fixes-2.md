# The polkit interaction flag, the action timeouts and the outcome mapping — H5-D

- **Date:** 2026-09-22
- **Scope:** `H5-D` of
  [`../plans/archive/2026-09-22-h5-privilege-fixes.md`](../plans/archive/2026-09-22-h5-privilege-fixes.md):
  the eleven findings of the `H5` whole-branch review that the controller did
  not defer, the amendment to [ADR
  0010](../../../docs/decisions/0010-one-shot-privilege-through-polkit.md)
  they rest on, and 0.6.1
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; `polkitd` running, **no authentication agent**
- **Artifact:** `hematita/target/production-artifact.toml` (verified and
  deployed to `~/.local/bin/hematita` at 0.6.1)

## What changed

1. **The interaction flag** (critical). `call_unit` sent its call with empty
   `MethodFlags`, and `AllowInteractiveAuth` (`0x4`) is opt-in: systemd
   passes that message flag to polkit as `AllowUserInteraction`, and without
   it polkit never consults an agent — it refuses with
   `InteractiveAuthorizationRequired` even on a session that has one. Every
   system-unit action would therefore have been refused before anybody was
   asked, and `outcome_of_dbus_error` mapped that very name to `NoAgent`, so
   the defect would have read on the page as the session's known missing
   agent. The call is now
   `proxy.call_with_flags::<_, _, OwnedObjectPath>(method,
   MethodFlags::AllowInteractiveAuth.into(), &(name, REPLACE))`. ADR 0010's
   Consequences gained the sentence naming the flag (its Decision section is
   untouched), in its own `suite-maintenance` commit.
2. **The action timeout.** `call_with_flags` answers
   `Result<Option<R>>`, and a `None` body is a call that was sent but never
   answered as one: `outcome_of_reply` maps `Ok(Some(job))` to `Done`,
   `Ok(None)` to `Failed` and every error through the unchanged
   `outcome_of_call`. The 25-second default reply timeout is a bus timeout,
   not a human one — a system unit's call does not return until the person
   has answered polkit — so each action builds its own connection through
   `zbus::blocking::connection::Builder::{system, session}()?
   .method_timeout(ACTION_TIMEOUT).build()` with `ACTION_TIMEOUT` at five
   minutes. **Which API:** zbus 5.19.0's blocking *proxy* builder exposes no
   timeout (`src/blocking/proxy/builder.rs` has destination/path/interface
   and the caching pair only); the timeout lives on the *connection* builder
   (`method_timeout`, `src/blocking/connection/builder.rs:300`), which is
   what both call sites use.
3. **`NotAuthorized` is `Denied`.** In `hematita-core::services`,
   `org.freedesktop.PolicyKit1.Error.NotAuthorized` now reads as `Denied` —
   polkit asked and was told no, or refused outright, which is an answer
   (ADR 0010: "the person cancelled or was not authorised"). Only
   `InteractiveAuthorizationRequired` remains `NoAgent`. Its test was
   updated.
4. **The listing cannot freeze the sampler.** The two listing connections are
   built with `LISTING_TIMEOUT = 2 s`; a timeout is not a `MethodError`, so
   the existing classification already answers `Unavailable` with
   `keep_connection: false`, and the next service tick opens a new
   connection.
5. **`AGENTS.md`**: the bullet claiming signals leave through `rustix` alone
   is replaced by one that names both paths — `rustix` for the person's own
   process, `pkexec` through `privilege.rs` for somebody else's — and the
   gate they share (listed in the latest snapshot; `/proc` still showing the
   same start time and owner; never PID 1 or this process). The ADR bullet
   is unchanged.
6. **The process page's `no-agent` sentence** no longer talks about system
   units; its `qsTr()` string now says that acting on another person's
   process is what needs an agent.
7. **`actionScope` is published by the hub** (a new `QString` property
   written with the other three action properties and cleared with them), so
   the page no longer guesses the acted unit's manager by matching a name
   against the visible rows. `noteText()` now prefers the outcome when there
   is one and the acted scope's bus is available, and says the
   bus-availability sentence otherwise.
8. **`Refusal::Foreign`** is a unit variant; its unused `start_ticks` payload
   is gone and the classification test with it.
9. **The hub is the authority on what exists.** `act` refuses a unit name the
   latest listing of that manager does not carry, before the kind check and
   before any thread, rather than sending it to systemd to be answered with
   `NoSuchUnit`.
10. **The foreign kill question names the prompt window**: its `qsTr()`
    string now warns that if the process ends while the person is answering
    the prompt, the signal could reach another process with the same PID.
11. **`VAL-H5`** requires 0.6.1 and its pass condition now says that, with an
    agent, the polkit dialog actually appears for a system unit and for a
    foreign process.

## Deviation from the plan, recorded

The H5 plan named `terminateForeign` and `killForeign` as separate
invokables. They were folded into `terminate` and `kill`: the hub already
classifies the PID (own, foreign, not allowed) and must do so anyway to
decide whether a signal is legitimate at all, so a second pair of entry
points would have let the page assert a classification the hub would then
have to re-derive and could contradict. One signal path, one decision, and
the page only says which signal it wants. `H5-D` also corrected the
consequence of that fold that the review found: the buttons must not be
disabled for the rows the folded path exists to serve (fixed in `H5-C`).

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
sha256sum target/release/hematita ~/.local/bin/hematita
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 12 ./target/release/hematita
```

## Result

- **Exit:** 0 for `complete-production.sh` and `status-production.sh`;
  `124` (still alive) for the 12-second run.
- **Observed:** the completion ran the release build, the artifact tests, the
  architecture contract, `fmt --check`, `clippy -D warnings`, `cargo test`
  for both crates (`hematita` `22 passed`, `hematita-core` `67 passed`,
  captures `11 passed`, doc-tests `0`), `qmllint-production: OK —
  org.celestina.hematita (0 non-fatal baseline warning(s))`, `smoke: OK —
  binary alive for 10 s, every section was shown, the first row published the
  CPU contract, the Sensors page published chips, the Services page listed
  system units, no QML errors, no auto-bindings`, then the deploy and
  `artifact: hematita current and verified`, `installed: OK
  /home/toni/.local/bin/hematita`.
- **The binaries are the same bytes:**
  `55608527054aa64a6c80427be603afd8c67ba3a42246b9a04a839cae904d780d` for both
  `target/release/hematita` and `~/.local/bin/hematita`. `Cargo.toml` reads
  `version = "0.6.1"`.
- The 12-second offscreen walk printed `hematita-shape cpu 3 60`,
  `hematita-sensors 10 44` and `hematita-services 143 true false`, and the
  error-pattern grep counted `0`.

## Limits

- **No action was performed and no signal was sent** in any run: nothing
  called `StartUnit`/`StopUnit`/`RestartUnit`, and no `pkexec` process was
  spawned. The flag, the timeouts and the mapping are proven by their unit
  tests and by reading the code and zbus 5.19.0's source; that the flag makes
  a real polkit dialog appear is exactly what `VAL-H5` now requires, and it
  cannot be shown here.
- **This session has no authentication agent**, by the author's order, so
  every privileged path can still only answer `no-agent`/`denied` locally.
  The critical defect this unit fixes is one that a session without an agent
  physically cannot distinguish from correct behaviour — which is why it
  survived `H5-B` and `H5-C`.
- The prompt window stays open as a named limit: the uid and start-ticks
  re-validation happens before the `pkexec` spawn, so a PID recycled while
  the prompt stands could be signalled. Closing it needs pidfd-based
  signalling, ADR 0010's stated revisit; the confirmation question now warns
  about it in words.
- `ACTION_TIMEOUT` bounds the wait at five minutes: a prompt left standing
  longer reports `failed`, which is a truthful "no answer arrived", not a
  claim about what the person wanted.
- Keyboard, focus, hover, the row colours and every Spanish sentence in front
  of a person remain `VAL-H5`.

## Follow-up

`VAL-H5` is recorded pending in `VALIDATION.md` against 0.6.1 and does not
block this closure. Deferred by the controller and untouched: `revision` is
bumped on every service tick whether or not any unit changed.
