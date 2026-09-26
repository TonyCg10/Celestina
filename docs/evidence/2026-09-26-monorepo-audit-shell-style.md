# Evidence: the shell and shared style audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): the Celestina shell (`celestina/`, the lock, the polkit agent, the helper and `celestina-shell-core`) and `celestina-style/`, on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Area SH audit: Celestina shell, lock, polkit agent and CelestinaStyle".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py celestina
python3 scripts/agent-context.py celestina-style
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
cargo test -p celestina-shell-core --offline
cargo test --manifest-path celestina/Cargo.toml --offline
```

Contrast ratios in SH-6 were computed with the method of
`scripts/check-contrast-contract.py`, not measured on a display.

## Result

- **Exit:** the architecture guard printed OK, including the QML visual and contrast contracts; the language guard printed OK (148 ratcheted); the documentation guard printed OK with the pre-existing Siderita inventory errata; `celestina-shell-core`: 349 passed; the helper crate failed to resolve `niri-ipc` offline, so its tests did not run.
- **Observed:** 25 findings: 0 Critical, 8 Important, 17 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Checkout: `main` at 9d022dd, read-only. Auditor area IDs: `SH-<n>` (shell), `STY-<n>` (style module).

### Executive summary

1. The lock's security core is sound. `ext-session-lock-v1` is used correctly, PAM runs in a separate short-lived child (`celestina-lock-verify`), the verdict is only the exit status, every error path stays locked, and the uncover timer can delay an unlock but can never cause one.
2. The polkit agent keeps its promised boundary. Verification is delegated to `libpolkit-agent-1` in `celestina-polkit-converse`, only polkitd's bus owner may call `BeginAuthentication`, and every failure denies.
3. Most helper IO is bounded, coalesced and kept off the GUI thread: the provider runtime, Melibea, clipboard receive, wallpaper import (atomic, source kept), notifications and the host `ProtocolDecoder`. Shell-core has 349 passing tests, and the architecture, language and documentation guards all pass.
4. Top risk (SH-1): `celestina msg lock` reports `confirmed` as soon as the lock process starts, before the compositor has covered anything. This breaks the shell's own "accepted is not confirmation" rule on the one verb where a false confirmation matters most.
5. Top risk (SH-2): the shell, the lock and the chooser all re-point one shared `CelestinaStyle` import symlink. If `XDG_RUNTIME_DIR` is unset it sits in world-writable `/tmp` with no ownership check. Because QML can emit `lockAuthenticator.answered(...)`, any QML that gets loaded into the lock can unlock it.
6. Top risk (SH-3): the polkit prompt shows polkit's `message` with the default `Text.AutoText`. pkexec builds that message from a path that any unprivileged caller chooses, so the caller can inject styled markup or remote `<img>` fetches into a password dialog. Everywhere else the shell forces `PlainText` for foreign strings.
7. Accessibility gaps on the lock (SH-4, SH-6, SH-19). It never binds `CelestinaTheme.reducedMotion`. Its date line and its error message drop to about 1.3:1 contrast over a bright wallpaper, and the contrast contract does not check this composition. The passphrase field has no accessible name.
8. The GUI thread still has synchronous waits: logind `Inhibit` (2 s), polkit registration (5 s), `serviceOwner` on every prompt, and `waitFor*` on the conversation children (SH-5). Clipboard re-offer writes block the Wayland thread, the same wedge that was already fixed on the receive side (SH-7).
9. Documentation is stale. README and STATUS still say `lock` is refused and that lock and polkit are "not built yet", while both are wired in `main.cpp` (SH-8). STATUS also says no checkpoint is active while ROADMAP names SURF-1.
10. The style module is healthy: `qmldir`, CMake and the file tree agree, there is no hard-coded colour, and the modal layer contains and restores focus. It has small gaps (STY-1..STY-4): an English accessible name, a literal swatch anatomy the guard cannot see, untested controls, and one duplicated slider in Siderita.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| SH-1 | Important | 1 Correctness | celestina/src/shellservice.cpp:517-520 | `lock` verb reports `confirmed` when the process starts, before the compositor confirms | S | celestina |
| SH-2 | Important | 2 Security | celestina/src/lock/main.cpp:122-136; celestina/src/main.cpp:235-253 | Shared, racy, `/tmp`-fallback style symlink; QML inside the lock can emit the unlocking signal | M | celestina |
| SH-3 | Important | 2 Security | celestina/qml/PolkitPrompt.qml:184, 237 | Polkit message and PAM text rendered as AutoText (markup and remote images from the pkexec caller) | S | celestina |
| SH-4 | Important | 6 QML/a11y | celestina/src/lock/main.cpp (no binding); LockScreen.qml:127,157,175 | Lock never binds `CelestinaTheme.reducedMotion`, so the recession, blur and scale always animate | S | celestina |
| SH-5 | Important | 1 Correctness (thread) | lockcontroller.cpp:168,227; polkitagent.cpp:193,232; polkitconversation.cpp:98-138 | Blocking D-Bus and process waits on the shell GUI thread | M | celestina |
| SH-6 | Important | 6 QML/a11y | LockScreen.qml:345,378,486; check-contrast-contract.py | Lock text over `scrim` and a bright wallpaper is about 1.3:1 (date, error); the contract does not check it | M | celestina |
| SH-7 | Important | 1 Correctness | celestina/src/provider_adapter/clipboard.rs:399 | Re-offered clipboard entry is written with a blocking `write_all` inside Wayland dispatch | S | celestina |
| SH-8 | Important | 7 Docs | celestina/README.md:102-104,173,283; STATUS.md:33-43,114,811,827 | README and STATUS say lock and polkit are refused or unbuilt; STATUS contradicts the active SURF-1 | S | celestina |
| SH-9 | Minor | 1 Correctness | polkitagent.cpp:284-293; polkitpromptcontroller.cpp (AlreadyShowing) | One wrong password aborts the privileged action; a concurrent request is cancelled | M | celestina |
| SH-10 | Minor | 1 Correctness | lockauthenticator.cpp:65,122; polkitconversation.cpp:67,136 | A crashed child emits two verdicts; `cancel()` drops the `busy` and `errorOccurred` connections | S | celestina |
| SH-11 | Minor | 1 Correctness | celestina/src/lockverify/main.cpp:121,190,206 | Expired password (`NEW_AUTHTOK_REQD`) shows as "wrong password" with no way out; the passphrase answers ECHO_ON prompts | S | celestina |
| SH-12 | Minor | 2 Security | celestina/src/lockauthenticator.cpp:56 | Lock account taken from `$USER` rather than `getuid()` (the polkit agent already uses `getuid`) | S | celestina |
| SH-13 | Minor | 1 Correctness | lockcontroller.cpp:279-280; lock/main.cpp:265-266 | A lock that dies before confirming during sleep keeps the inhibitor held; the lock exits 0 without unlocking if still unconfirmed | S | celestina |
| SH-14 | Minor | 2 Security (bounds) | niri_adapter.rs:805,874,401; provider_adapter/settings.rs:41 | Niri event stream and settings read are unbounded; settings path is duplicated | S | celestina |
| SH-15 | Minor | 4 Reuse | celestina/src/polkitconversation.cpp:148 | Child stdout buffer is unbounded and re-implements `ProtocolDecoder` | S | celestina |
| SH-16 | Minor | 4 Reuse | LockScreen.qml:426-480 vs qml/BackdropTextField.qml | Hand-copied backdrop field recipe that has already drifted | M | celestina |
| SH-17 | Minor | 4 Reuse | main.cpp:44 + 6 controllers | `CELESTINA_REDUCED_MOTION` policy read in 7 places | S | celestina |
| SH-18 | Minor | 3 Performance | provider_adapter/tools.rs:335-341; launcher.rs:309; celestina-launcher.sh:14-15 | One reaper thread per launched app for its whole life; hard-coded `kitty`; `LD_LIBRARY_PATH` leaks to apps | M | celestina |
| SH-19 | Minor | 6 QML/a11y | celestina/src/lock/LockScreen.qml:432-490 | Passphrase field has no `Accessible.name`; lock and polkit error text is not announced | S | celestina |
| SH-20 | Minor | 8 Quick win | celestina/src/lock/LockScreen.qml:250,260 | Clock claims per-minute ticks but wakes every second on a locked machine | S | celestina |
| SH-21 | Minor | 1 Rust hygiene | provider_adapter/clipboard.rs:136; melibea.rs:222; celestina-shell-core/src/nightlight.rs:70 | Production `expect` without a local invariant; cosmetic `#[allow(non_snake_case)]` | S | celestina (+ celestina-shell-core) |
| STY-1 | Minor | 6 QML/a11y + language | celestina-style/CelestinaScrollBar.qml:85 | English, non-`qsTr` accessible names read aloud in Spanish products | S | celestina-style |
| STY-2 | Minor | 6 QML tokens | celestina-style/GlassMenuItem.qml:67,77,82; scripts/check-style-contract.sh | Swatch anatomy literals (14 px, 11 px, alpha 0.24); guard misses literal `withAlpha` | S | celestina-style |
| STY-3 | Minor | 5 Tests | celestina-style/tests/ | No Qt Quick tests for Switch, TextField, Capsule, RowHighlight, GlassCard, GlassContextMenu | M | celestina-style |
| STY-4 | Minor | 4 Reuse / a11y | siderita/qml/components/SizeRow.qml:35-70 | Re-implements the CelestinaSlider anatomy with no focus ring and no accessible name | S | siderita |

---

### SH-1: `lock` reports `confirmed` before the screen is covered (Important, correctness)

Evidence, `celestina/src/shellservice.cpp:509-521`:

```cpp
if (!m_lock->lock()) { return refuse(...); }
const qulonglong requestId = ++m_lastRequestId;
// Started, not covered: the compositor confirms the cover later, and
// `lock` deliberately does not wait for it ...
reportOutcome(requestId, verb, QStringLiteral("confirmed"), QString());
```

`LockController::lock()` also returns `true` when a lock process is already running but not yet confirmed (`lockcontroller.cpp:218-219`).

Why it matters:
- `celestina msg lock` exits 0 and prints `confirmed` even when the lock then fails. For example, `celestina-lock` exits 2 when the compositor has no `ext-session-lock`, or 3 when it is refused.
- A person or script walks away believing the session is locked.
- This contradicts the shell contract ("`accepted` is not confirmation"), the Qt standard ("signals describe confirmed state transitions"), and README's own session-verb promise.

Fix: report `pending`, then `confirmed` on `lockedChanged` with `isLocked()`, or `failed` when the process finishes unconfirmed. This is the same tracker pattern `lockAndSuspend` already uses at `lockcontroller.cpp:305-321`. Add a `shellservice_test` case for "started but never confirmed".

Effort S. Prefix `celestina:`.

### SH-2: shared style symlink in the runtime or temp dir, and a QML-callable unlocking signal (Important, security)

Evidence, `celestina/src/lock/main.cpp:122-136`. `celestina/src/main.cpp:235-253` is identical and also runs for `--pick-output`.

```cpp
if (runtime.isEmpty()) runtime = QDir::tempPath();
const QString importRoot = QDir(runtime).filePath("celestina-shell-import");
QDir().mkpath(importRoot);
QFile::remove(styleLink);
QFile::link(qEnvironmentVariable("CELESTINA_STYLE_PATH", CELESTINA_STYLE_DIR), styleLink);
```

`main.cpp:138` exposes `lockAuthenticator` to QML, and `main.cpp:269-275` unlocks on its `answered(Authenticated)` signal. A QML file can emit a C++ signal by calling it as a method. The type is registered (`QML_ELEMENT`, `Q_ENUM(Verdict)`), so `lockAuthenticator.answered(LockAuthenticator.Authenticated)` would reach `uncover->begin()`.

Why it matters:

(a) The `/tmp` fallback has no ownership or mode check.
- Another local user can pre-create `/tmp/celestina-shell-import` and later swap the `CelestinaStyle` link to their own module.
- That code then runs in the lock, where it can emit the verdict. This breaks ADR 0004's "reachable from exactly one place".

(b) The shell, every `celestina-lock` start and every `--pick-output` run (spawned by xdg-desktop-portal-wlr) delete and recreate the same link.
- A running shell that lazily loads a CelestinaStyle component during that window fails to load it.
- A development build and the production bundle re-point each other's style. This is the "wrong CelestinaStyle" the comment says the relink prevents.

(c) If a non-symlink entry already sits at that path, `QFile::remove` does not remove it. The lock then never loads, returns `NotLocked`, and every lock or suspend is refused.

Fix:
- Use a per-process private directory: a `QTemporaryDir` under `XDG_RUNTIME_DIR` with mode 0700, and refuse (lock: `NotLocked`) when `XDG_RUNTIME_DIR` is absent instead of using `/tmp`. Better still, have the bundle ship a directory already named `CelestinaStyle` so no link is needed.
- Separately, route the unlock decision through a C++-only path (a `std::function` callback or a private object), and give QML a narrow facade that exposes only `authenticate`, `busy` and the last verdict.

Effort M. Prefix `celestina:`.

### SH-3: polkit prompt renders caller-influenced text as rich text (Important, security)

Evidence, `celestina/qml/PolkitPrompt.qml:183-189`:

```qml
Text {
    width: parent.width
    text: prompt.message
    color: backdropInk.primary
    ...
    wrapMode: Text.WordWrap
}
```

The Text at `:233-243` (`prompt.problem` / `prompt.notice`, which is PAM text) also has no `textFormat`. `MenuHeader.qml:62,100,113`, `GlassMenuItem.qml:138` and the toasts all set `Text.PlainText`, with comments explaining the `<img src=…>` fetch risk.

Why it matters:
- polkitd fills the action message from the request. For `org.freedesktop.policykit.exec`, the message template is "…run `$(program)` as the super user", and `$(program)` is a path that any unprivileged pkexec caller chooses.
- A filename such as `<b>…</b>` or `<img src=http://…>` passes Qt's `mightBeRichText` heuristic. It then restyles or hides parts of the authorization dialog and makes the shell fetch a URL.
- The actionId Text is monospace and elided but also AutoText.

Fix: set `textFormat: Text.PlainText` on every Text in `PolkitPrompt.qml`, and add a `tst_polkitprompt.qml` case that renders `<b>x</b>` literally.

Effort S. Prefix `celestina:`.

### SH-4: the lock ignores reduced motion (Important, accessibility)

Evidence:
- `celestina/src/lock/main.cpp` never sets `CelestinaTheme.reducedMotion` or passes a `reducedMotion` property.
- `LockScreen.qml` has three reduced-motion branches (`:127`, `:157`, `:175`) and `Behavior` gating (`:471`), but in the lock process they are dead code because the singleton defaults to `false` (`CelestinaTheme.qml:771`).
- The shell reads `CELESTINA_REDUCED_MOTION` (`main.cpp:44`). The lock inherits that variable and never reads it.

Why it matters: the lock always plays a full-screen scale from 1.06 to 1 and a blur cross-fade on entry and retreat, on every output. That is exactly the spatial motion the standard requires to become instant. The tested QML branches give false confidence.

Fix: read the same variable in lock `main.cpp`, through the single owner proposed in SH-17. Set it before `component.create`, either as a required root property or by assigning the singleton, and add a test.

Effort S. Prefix `celestina:`.

### SH-5: synchronous D-Bus and process waits on the shell GUI thread (Important, thread affinity)

Evidence:
- `lockcontroller.cpp:168`: `bus.call(call, QDBus::Block, 2000)` in `takeSleepInhibitor()`. It runs from the constructor, after every wake (`prepareForSleep(false)`) and after every unlock (`finished()`).
- `lockcontroller.cpp:227`: `waitForStarted(2000)`.
- `polkitagent.cpp:193`: `m_bus.call(call, QDBus::Block, 5000)` at startup and on every polkitd restart.
- `polkitagent.cpp:232`: `serviceOwner(authorityService).value()`, a synchronous `GetNameOwner` on every `BeginAuthentication`.
- `polkitconversation.cpp:98,106,128,138`: `waitForStarted`, `waitForBytesWritten` ×2 and `waitForFinished(1000)`.
- `lockauthenticator.cpp:98,108,124`: the same waits in the lock process.

Why it matters:
- "Blocking D-Bus, filesystem, process … never runs on the GUI thread."
- A slow logind or polkitd stalls every panel output, menu and OSD for up to 5 s, and this happens during resume, which is when logind is busiest.

Fix:
- Use `asyncCall` with `QDBusPendingCallWatcher` for Inhibit and Register, and store the fd in the callback.
- Check the polkit caller against a cached owner kept current by the existing `QDBusServiceWatcher`.
- Replace the `waitFor*` calls with `started` / `bytesWritten` / `finished` signals.

Effort M. Prefix `celestina:`.

### SH-6: lock text contrast over a bright wallpaper is untested and fails (Important, accessibility)

Evidence:
- `LockScreen.qml:345`: `color: CelestinaTheme.scrim`, which is `#73000000` (`CelestinaTheme.qml:427`), over a blurred wallpaper.
- The date (`:378`) uses `textMuted` (`textLo #9ba3af`).
- The error (`:486`) uses `danger` (`#ff746d`) inside a ContextualVeil card, which lightens: `glassHighlight` at `glassContextualVeilStrength` 0.12.

Measured against the contract's own hostile-white backdrop method, the scrim over white is about rgb(140,140,140):

| Text | Contrast | Standard |
|---|---|---|
| `textHi` (clock) | 3.17:1 | passes only as large text |
| `textLo` (date) | 1.32:1 | fails |
| `danger` (the Spanish wrong-password message) | 1.28:1 | fails |

`check-contrast-contract.py` checks glass tints and the media scrim, but not `scrim` or the lock composition.

Why it matters: the failure message on the one surface that must be read under stress can be close to illegible on a light wallpaper.

Fix:
- Give the lock a dedicated, token-backed wash (for example `lockScrim`) or a dense backing behind the date and message.
- Add the lock pairs (`text`, `textMuted`, `danger` over wash plus white or black) to `check-contrast-contract.py` in the same unit. That script is owned by celestina-style, so it goes in a separate `celestina-style:` unit or into this one if it is cross-suite.

Effort M. Prefix `celestina:` (plus `celestina-style:` for the contract row).

### SH-7: clipboard re-offer blocks the Wayland thread (Important, correctness)

Evidence, `celestina/src/provider_adapter/clipboard.rs:395-401`:

```rust
source_proto::Event::Send { fd, .. } => { ...
    let mut file = File::from(fd);
    let _ = file.write_all(text.as_bytes());
```

Entries can be up to `MAX_ENTRY_BYTES` = 256 KiB (`celestina-shell-core/src/clipboard.rs:20`), which is larger than a 64 KiB pipe buffer.

Why it matters:
- The receive path (`:423-470`) was bounded after "parked this thread inside a Wayland event handler for good". The send path has the identical hazard.
- A paste target that requests and then reads late, or never reads (a frozen app, or one that reads on the same thread only after a round trip), blocks the clipboard thread indefinitely. History, selection and every queued request then stop for the rest of the session.
- The error is also discarded silently.

Fix: set the fd non-blocking and write under a deadline, as `receive_text` does, or hand the write to a short-lived bounded worker. Record failures.

Effort S. Prefix `celestina:`.

### SH-8: README and STATUS contradict the checkout (Important, documentation)

Evidence:
- README `:102-104`: "`lock` and `lock-and-suspend` are accepted vocabulary and refused in practice: this shell has no locker provider yet".
- README `:173`: "`lock`, `lock-and-suspend` | nothing yet — these are refused on purpose".
- README `:283`: "Screen lock and the polkit agent are on the list and not built yet."
- STATUS `:811` and `:827`: the same claims about refusal and "suspend refused while no locker exists".

The code wires `LockController` (`main.cpp:455-456`) and `PolkitAgent` (`main.cpp:477-478`). ROADMAP marks R6 and R8 complete, and STATUS `:933` says R6 closed.

Separately:
- STATUS `:114` says "No implementation checkpoint is active", while ROADMAP `:4` and the active plan name `SURF-1` (STATUS `:33` agrees).
- The SURF-1 bullet is truncated mid-sentence ("Activation and `VAL-SURF-1` are the author's") before `- **Design direction:**`.

Why it matters: an author following README binds keys to `swaylock` or believes suspend is refused, and agents reading STATUS get two opposite checkpoint states.

Fix:
- Rewrite the README session-verb and handover paragraphs, and the Niri binding table, to describe the first-party lock (with `VAL-R6` still unrun).
- Drop the stale STATUS bullets, set the checkpoint section to SURF-1, and finish the truncated sentence.

Effort S. Prefix `celestina:`.

### SH-9: a polkit typo aborts the privileged action; concurrent requests are cancelled (Minor, correctness)

Evidence:
- `polkitagent.cpp:284-293` finishes the request on any verdict, including `Refused`: "A verdict that is not an authorization still ends the request normally".
- `polkitpromptcontroller.cpp` `promptRefusal(... alreadyShowing ...)` dismisses (cancels) a second `BeginAuthentication`.

Why it matters:
- A single mistyped password makes pkexec, package installs or mounts fail outright. polkit keeps the cookie valid until the agent replies, so standard agents re-initiate a new `PolkitAgentSession` with the same cookie and let the person retry.
- A second request arriving while one is shown fails instead of queueing.

Fix:
- On `Refused`, keep the request open, show the "wrong password" state and start a fresh conversation for the same cookie, up to a bounded number of attempts.
- Queue at most N pending requests and show them in order.

Effort M. Prefix `celestina:`.

### SH-10: double verdicts and lost connections in the child-process wrappers (Minor, correctness)

Evidence:
- `lockauthenticator.cpp:65-71` and `polkitconversation.cpp:67-73` emit `answered(Unavailable)` from `errorOccurred` whenever `state()==NotRunning`.
- For a crashed child, Qt emits `errorOccurred(Crashed)` after the state has become `NotRunning`, then `finished(CrashExit)`, and `finished` emits `Unavailable` again. The ADR's "exactly one per accepted authenticate" is therefore violated.
- `lockauthenticator_test.cpp:134-148` (`aCrashedVerifierNeverAuthenticates`) does not assert `spy.count()==1`.
- `cancel()` (`lockauthenticator.cpp:122`, `polkitconversation.cpp:136`) calls `m_process->disconnect(this)`. This also removes the `stateChanged` lambda (which emits `busyChanged`) and the `errorOccurred` lambda, and only `finished` (plus `readyRead`) is reconnected. After one cancel, `busy` never notifies again.

Fix: emit from `errorOccurred` only for `FailedToStart`. Replace `disconnect(this)` with a generation counter or a connection handle for `finished` alone. Assert the single emission in tests.

Effort S. Prefix `celestina:`.

### SH-11: verifier PAM handling can lock out an expired account (Minor, correctness)

Evidence, `lockverify/main.cpp`:
- `:186-190`: `pam_authenticate` then `pam_acct_mgmt`.
- `:198-206`: anything that is not ABORT, BUF_ERR or SYSTEM_ERR returns `Refused`. That includes `PAM_NEW_AUTHTOK_REQD`, which `LockScreen.qml` shows as the Spanish wrong-password message
- `:119-128`: every `PAM_PROMPT_ECHO_ON` prompt (an OTP, a username) is answered with the passphrase.

Why it matters:
- A password that expires while the machine is locked turns the right passphrase into a "wrong password" loop. The only way out is a TTY. This is the "locked out" direction ADR 0004 names as its revisit trigger.
- Replaying the passphrase to echo-on prompts can leak it to a module that logs or echoes that field.

Fix:
- Treat `PAM_NEW_AUTHTOK_REQD` after a successful `pam_authenticate` as a distinct exit code, shown as "password expired". Decide by ADR whether it unlocks, as swaylock/hyprlock do by skipping acct_mgmt, or refuses with an explicit message.
- Answer ECHO_ON prompts with `PAM_CONV_ERR` rather than the secret.

Effort S. Prefix `celestina:`.

### SH-12: the lock's account comes from `$USER` (Minor, security hardening)

Evidence: `lockauthenticator.cpp:55-56` initializes `m_user` from `QProcessEnvironment::systemEnvironment().value("USER")`. `celestina-lock-verify` accepts any `--user`. `polkitagent.cpp:95` correctly uses `getpwuid(getuid())`.

Why it matters: the header promises "the only account a session lock may accept", but an inherited environment decides it. A wrong `USER` makes the lock unlockable or, with stacks that do not tie PAM to the caller's uid, accepts another account's password.

Fix: resolve the name from `getuid()` in both the lock and the verifier. Have the verifier refuse a `--user` that does not match its real uid.

Effort S. Prefix `celestina:`.

### SH-13: lock sequencing edge cases (Minor, correctness)

Evidence:
- `lockcontroller.cpp:279-280`: `finished()` returns early when `!m_confirmed`. After `prepareForSleep(true)` started a lock that then dies unconfirmed, `m_sleepPending` stays `true` and the delay inhibitor stays held until logind's `InhibitDelayMaxSec`. Unlike the start-failure path (`:209-212`), nothing logs that the machine will sleep uncovered.
- `lock/main.cpp:264-266`: `lock->release(); QGuiApplication::exit(Unlocked);`. `release()` is a no-op when not confirmed (`locksession.cpp:20`), yet the process exits 0 ("Unlocked").

Fix:
- In `finished()`, when unconfirmed and `m_sleepPending`, log it critically, release the inhibitor and clear the flag.
- In the lock, only `exit(Unlocked)` when `release()` actually unlocked (return a bool). Otherwise keep running.

Effort S. Prefix `celestina:`.

### SH-14: unbounded reads in the Niri adapter and a duplicated settings path (Minor, bounds and reuse)

Evidence:
- `niri_adapter.rs:805` and `:874`: `reader.read_line(&mut line)` on the compositor socket with no cap. The same file uses `read_bounded_line` for host input (`:1150`), and Melibea caps its socket at `MAX_MESSAGE_BYTES` (`melibea.rs:239`).
- `niri_adapter.rs:401`: `std::fs::read(path.join("celestina").join("settings.json"))` reads the whole file before `Settings::from_bytes` checks `MAX_FILE_BYTES`.
- The same path is built again in `provider_adapter/settings.rs:41`.

Fix: read the event stream through a bounded reader with a generous cap (for example 16 MiB, logged and reconnected on overflow). Expose one `settings_path()` and a bounded `read_settings()` from the settings owner and call them from both helpers.

Effort S. Prefix `celestina:`.

### SH-15: polkit conversation buffer is unbounded and duplicates ProtocolDecoder (Minor, reuse)

Evidence, `polkitconversation.cpp:148`: `m_pending.append(m_process->readAllStandardOutput());` grows without limit until a newline arrives. `protocoldecoder.cpp` already implements bounded, oversized-line-dropping framing for the shell.

Fix: feed the child's stdout through `ProtocolDecoder` (or a sized instance of it).

Effort S. Prefix `celestina:`.

### SH-16: the lock's passphrase field is a drifted copy of BackdropTextField (Minor, reuse)

Evidence: `LockScreen.qml:426-432`: "The recipe is written out here rather than imported … a change to one has to be made in the other." The copies already differ:

| State | Lock (`:452-461`) | `BackdropTextField.qml:23-30` |
|---|---|---|
| Rest fill | `CelestinaTheme.glassHighlight` | `ink.materialTint` |
| Focus fill | `badgeAccentFill` | `ink.selectedRestFill` |
| Focus border | `CelestinaTheme.text` | `ink.focus` |

Fix: move the backdrop-field specialization, plus the `BackdropInk` roles it needs, into a small QML module that both the shell and `celestina-lock` link. Alternatively promote it to CelestinaStyle, since the lock and the polkit prompt are now two consumers with identical semantics.

Effort M. Prefix `celestina:`.

### SH-17: the reduced-motion input has seven readers (Minor, reuse)

Evidence: `main.cpp:44` defines `reducedMotionRequested()`, but `overlaycontroller.cpp:100`, `osdcontroller.cpp:230`, `polkitpromptcontroller.cpp:188`, `toastcontroller.cpp:346` and `panelmenucontroller.cpp:380,667,…` each call `qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")` directly. The lock reads it nowhere (SH-4).

Fix: add one small header, for example `shellmotion.h` with `bool reducedMotionRequested()`, used by every controller and by the lock. This is the single place to later add a platform source.

Effort S. Prefix `celestina:`.

### SH-18: launch plumbing costs (Minor, performance)

Evidence:
- `provider_adapter/tools.rs:335-341` spawns one `reaper` OS thread per launched application, and it blocks in `child.wait()` for that application's whole lifetime.
- Launched apps stay children of the provider helper, in its process group and cgroup scope.
- `launcher.rs:309` hard-codes `"kitty"` for `Terminal=true` entries.
- `celestina-launcher.sh:14-15` exports `LD_LIBRARY_PATH=$bundle` to the whole tree. Every app started from the launcher or `xdg-open` inherits it, as does the non-setuid PAM verifier.

Fix:
- Spawn through the compositor (`niri msg action spawn`), or double-fork or `setsid` so there is nothing to reap.
- Read the terminal from settings, falling back to `xdg-terminal-exec`.
- Use `$ORIGIN` RPATH instead of `LD_LIBRARY_PATH`.

Effort M. Prefix `celestina:`.

### SH-19: lock and prompt accessibility gaps (Minor, accessibility)

Evidence:
- `LockScreen.qml` contains no `Accessible.*` at all. The passphrase `CelestinaTextField` (`:432-441`) relies on its placeholder. `PolkitPrompt.qml:229` sets `Accessible.name` to the `qsTr` Spanish word for password for the same field.
- The verdict Text (`:482-492`) and the polkit problem Text (`:233-243`) are plain `Text` items, so a screen reader is not told that an attempt failed.

Fix: add `Accessible.name` to the lock field. Expose the message with `Accessible.role: Accessible.AlertMessage` and a name bound to the message, in both surfaces.

Effort S. Prefix `celestina:`.

### SH-20: the lock clock wakes every second (Minor, quick win)

Evidence, `LockScreen.qml:250-262`: the comment says "It ticks to the minute … a locked machine should not be waking for a second hand nobody asked for", but the code is `interval: 1000`, `repeat: true`.

Fix: schedule the next tick at the next minute boundary (a single-shot `Timer` re-armed with `60000 - now % 60000`).

Effort S. Prefix `celestina:`.

### SH-21: Rust production hygiene (Minor)

Evidence:
- `provider_adapter/clipboard.rs:134-136`: `REQUESTS.set(sender).expect("clipboard::spawn is called exactly once")`. The invariant is asserted, not demonstrated. A second call panics the aggregate helper; siblings use `let … else` plus `eprintln!`.
- `melibea.rs:222`: `ProviderId::new(NAME).expect(...)`, while every other provider handles the same call fallibly.
- `celestina-shell-core/src/nightlight.rs:70`: `#[allow(non_snake_case)] let TEMPERATURE = temperature;`, a lint allowance with no reason whose only purpose is capitalizing a local.

Fix: return an error or log and skip for the first two. Rename the local and drop the `#[allow]`.

Effort S. Prefix `celestina:`; `celestina-shell-core:` for nightlight.

### STY-1: English accessible names in the scrollbar (Minor, accessibility and language)

Evidence, `celestina-style/CelestinaScrollBar.qml:85`: `Accessible.name: root.horizontal ? "Horizontal scroll" : "Vertical scroll"`. A screen reader reads this to the user, so it is product copy. Every other name in the module uses `qsTr` in Spanish (for example `CelestinaTreemap.qml:68`, `CelestinaUsageList.qml:96`).

Fix: `qsTr("Desplazamiento horizontal")` / `qsTr("Desplazamiento vertical")`.

Effort S. Prefix `celestina-style:`.

### STY-2: GlassMenuItem swatch anatomy bypasses tokens (Minor, QML tokens)

Evidence, `GlassMenuItem.qml:67-68` (`width: 14; height: 14`), `:76-77` (`CelestinaTheme.withAlpha(CelestinaTheme.text, 0.24)`) and `:82` (`width: 11`). `check-style-contract.sh` catches `Qt.rgba` but not a literal alpha passed to `withAlpha`/`multiplyAlpha`, so this is the only literal colour derivation outside the theme and no guard sees it.

Fix: add `compMenuSwatchSize`, `compMenuSwatchSlash` and a `swatchOutline` role to `CelestinaTheme`. Extend the guard pattern to `(withAlpha|multiplyAlpha)\([^,]+,\s*[0-9.]` (0 other hits today, so no ratchet churn).

Effort S. Prefix `celestina-style:`.

### STY-3: untested shared controls (Minor, tests)

Evidence: `celestina-style/tests/` covers button and icon button, glass surface, icon catalog and gradient, input shield, line gutter, menu item, modal, scrollbar, slider, treemap and usage list. There are no tests for:
- `CelestinaSwitch`: toggle by Space, `visualFocus` ring, reduced-motion duration 0.
- `CelestinaTextField`: `visualFocus` only on Tab, Backtab or Shortcut.
- `CelestinaCapsule`, `CelestinaRowHighlight`, `GlassCard`, `GlassContextMenu`, `ListSection`, `CelestinaSectionLabel`.

The local contract requires every interactive control to cover keyboard, `visualFocus` and reduced motion.

Fix: add `tst_switch.qml` and `tst_textfield.qml` at minimum.

Effort M. Prefix `celestina-style:`.

### STY-4: Siderita duplicates the slider control (Minor, reuse and accessibility)

Evidence, `siderita/qml/components/SizeRow.qml:35-70`: a raw `QtQuick.Controls Slider` with its own track, fill and handle anatomy built from `compLinearTrackHeight` / `compSliderHandleSize`. It has no focus ring and no `Accessible.name`. Siderita already uses `CelestinaSlider` elsewhere (`dialogs/MediaPreview.qml:136,200`), as do Fluorita and the shell.

Fix: replace it with `CelestinaSlider { value; from; to; step: 0.1; onMoved }` plus an accessible name from `sizeRow.label`.

Effort S. Prefix `siderita:`.

## Limits

- `cargo test --manifest-path celestina/Cargo.toml --offline` failed to
  resolve `niri-ipc` offline, so the helper's unit and integration tests
  did not run.
- There is no Qt 6 SDK in the container: no CTest, no `qmllint`, no Qt
  Quick tests and no Qt source to cross-check Qt internals. SH-10's
  double emission is reasoned from `QProcessPrivate::processFinished`
  ordering and should be confirmed with a `count()==1` assertion.
- No real session: no `ext-session-lock`, polkit, PAM, clipboard
  hand-off or AT-SPI check, and SH-6's ratios are computed, not measured.
- Not deep-read, given size: `panelmenucontroller.cpp` (1633 lines),
  `wallpaper.rs` (2031, only the import path), the `nightlight.rs`
  adapter (1296), the `network.rs` core (1342), `ControlCentre.qml`
  (1020) and the tray stack beyond its async-call pattern.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-10 | `SURF-1-E` | `celestina/docs/plans/active/2026-08-20-persistent-carriers.md` | SH-1, SH-2, SH-3, SH-11, SH-12, SH-13 |
| P-15 | `SID-H1-C` | `siderita/docs/plans/active/2026-09-26-hardening.md` | STY-4 |
| P-16 | `STYLE-G7-N` | `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md` | SH-6, STY-1, STY-2, STY-3 |
| P-17 | `SURF-1-F` | `celestina/docs/plans/active/2026-08-20-persistent-carriers.md` | SH-4, SH-5, SH-6, SH-7, SH-8, SH-9, SH-10, SH-14, SH-15, SH-17, SH-19, SH-20, SH-21 |

Unscheduled backlog (Minor; taken when the file is next touched): SH-16, SH-18.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Ordered by value divided by effort.

1. **`celestina-bug`: make the lock verb and lock surface truthful and accessible.** `lock` reports pending then confirmed or failed. The lock reads reduced motion through one owner. The passphrase field gets a name and failures are announced as alerts. The clock ticks per minute. Unconfirmed-exit and inhibitor edge cases are fixed. Covers SH-1, SH-4, SH-13, SH-17, SH-19 (lock half), SH-20. Effort M.
2. **`celestina-bug`: close the lock's in-process unlock path and the shared import symlink.** Use a private per-process import root (no `/tmp` fallback) and a C++-only unlock route with a narrow QML facade. Resolve the lock account from `getuid()`. Add the verifier PAM fixes. Covers SH-2, SH-12, SH-11. Effort M.
3. **`celestina-bug`: harden the polkit prompt and conversation.** Plain-text rendering, retry on refusal and a bounded queue, single verdict, a decoder-backed buffer, alert role. Covers SH-3, SH-9, SH-10, SH-15, SH-19 (prompt half). Effort M.
4. **`celestina-bug`: take synchronous D-Bus and process waits off the GUI thread.** Async Inhibit and Register, a cached polkit owner, signal-driven child IO. Covers SH-5. Effort M.
5. **`celestina-bug`: bound the helper IO edges.** Non-blocking clipboard send with a deadline, bounded Niri stream, one settings reader, launch plumbing (no per-app reaper, configurable terminal, RPATH), production `expect` cleanup. Covers SH-7, SH-14, SH-18, SH-21. Effort M. `nightlight.rs` goes in a separate `celestina-shell-core-maintenance` atomic commit.
6. **`celestina-maintenance`: documentation truth.** README and STATUS lock and polkit sections, the checkpoint section and the truncated SURF-1 bullet. Covers SH-8. Effort S.
7. **`celestina-style-bug`: style module gaps.** Spanish scrollbar names, swatch tokens plus the `withAlpha` guard pattern, lock-composition contrast rows (the contract half of SH-6), Switch and TextField tests. Covers STY-1, STY-2, STY-3, SH-6 (contract). Effort M. The lock's darker wash and dense backing for SH-6 follow in unit 1 or a small `celestina:` unit once the token exists. SH-16 (shared backdrop field) fits here if the recipe is promoted to CelestinaStyle.
8. **`siderita-bug`: use CelestinaSlider in SizeRow.** Covers STY-4. Effort S.
