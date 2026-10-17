# Celestina polkit agent (CEL-2-I) — design

- **Date:** 2026-10-17
- **Status:** drafted from the author's answers in brainstorming and revised
  after two reviews (§10.2); awaiting the author's review
- **Product:** the unit that gives the Rust shell its own polkit
  authentication agent, so that actions needing authorization (`pkexec`,
  `systemctl` on system units, Hematita's service and process actions) can be
  answered in the author's niri session once Noctalia stops being the agent
- **Identifiers:** project `celestina`, plan `cel-2-parity`
  ([the ledger](../../../celestina/docs/plans/active/2026-10-16-cel-2-parity.md)),
  unit `CEL-2-I`, commit prefix `celestina:`
- **Builds on:** [the parity program](2026-10-16-celestina-parity-program-design.md)
  (§4.4 privileged units, §5 row I, §7), [the shell design](2026-10-14-celestina-rust-shell-design.md),
  the suite's [ADR 0010](../../decisions/0010-one-shot-privilege-through-polkit.md),
  the shell's [ADR 0014](../../../celestina/docs/decisions/0014-lock-and-polkit-carry-over.md)
  and its ADR 0009 (one process, one loop). The halted Qt shell's ADR 0005 and
  its C++ agent are read-only references; nothing is ported unread (§9)
- **Privileged unit:** it follows the program's §4.4: this design, a threat
  model and a list of what the unit never does (§6), a review by a model other
  than the implementer plus an adversarial security-only pass (§10.2), a
  recovery path (§7) and a failure mode of "denied", never "granted" (§6.4)

## 1. Goal and scope

When a program asks polkit to authorize an action that needs a password,
polkitd calls the *authentication agent registered for the person's session*.
Today that agent is Noctalia's; with the Celestina shell as the session's
shell there would be none, and every such action would fail at the prompt
(ADR 0010). This unit adds that agent, as one duty of the shell:

1. **Registration** as the agent of this session — and only of this session —
   with polkitd, on the system bus.
2. **A prompt surface**: a dimmed whole-output overlay with a centred card
   that holds the keyboard exclusively and says what is being authorized,
   as polkit worded it.
3. **The helper conversation**: the password goes from the card to polkit's
   own `polkit-agent-helper-1`, which runs PAM as root and tells polkitd the
   verdict itself. The shell never verifies a password and never reports a
   success.

Out of scope, for good: verifying anything itself; remembering anything (no
"remember for this session", no password caching); fingerprint or token
conversations beyond showing their text; choosing among several users; a
system-wide agent; a polkit action file of the shell's own; anything on the
lock screen (CEL-2-H); an input method in the password field (§6.5).

## 2. Decisions the author made in brainstorming

| Decision | Choice | Alternatives considered |
|---|---|---|
| Another agent already holds the session | **Refuse and say so**: the registration fails, the shell logs it and neither retries nor evicts; the author turns Noctalia's agent off and switches the duty on again | retry in the background; replace the other agent (rejected up front: an agent that evicts another can impersonate the prompt) |
| Wrong password | **Two wrong answers, then the request is cancelled** (`MAX_ATTEMPTS = 2`). Chosen over three because this host's `pam_faillock` has `deny = 3` and `unlock_time = 600`: a third failure inside one prompt would lock the account for ten minutes, for login and `sudo` too | three (the author's first answer, replaced once the lockout was known); one |
| The prompt | **A card centred over a dimmed whole-output overlay, keyboard exclusive** | a card without dimming; a page of the Island (rejected: the Island takes no keyboard at rest and a privileged prompt must not share a surface with the bar) |
| Where it lives | **Inside the shell process** (ADR 0009); the password lives in one zeroizing buffer that is moved, never copied, from the card to the helper's thread | a separate `celestina-polkit-agent` binary (rejected: a second deployable and a second Wayland runtime; the residual is recorded in §6.2) |

Choices the design made on its own and the reviews tested, all listed with a
proposed default in §12: the keyboard proof, the protocol spoken in Rust, the
queue and backoff, the idle timeout, the `/etc/passwd` lookup and the shared
text sanitiser.

## 3. Verified facts

Marks: **[src]** polkit `main`, read on 2026-10-17
(`polkitagentsession.c`, `polkitagenthelper-pam.c`,
`polkitbackendinteractiveauthority.c`, `pkexec.c`); **[host]** this machine
(polkit 127-3.1, 2026-10-17); **[repo]** this checkout; **unverified** is
listed in §10.3 and must be closed before an invariant relies on it.

**polkitd and the agent**

1. The agent calls `RegisterAuthenticationAgent(subject, locale, object_path)`
   on the **system** bus, with `subject = unix-session{session-id}` [src].
   Registration is an object path at polkitd, **not a bus name**: nothing on
   the bus says who holds the slot except the refusal of one's own attempt.
   The caller's session must equal the subject, and a second agent for the
   same subject fails with "An authentication agent already exists for the
   given subject" [src].
2. polkitd then calls, on interface
   `org.freedesktop.PolicyKit1.AuthenticationAgent` at the registered path,
   `BeginAuthentication(action_id s, message s, icon_name s, details a{ss},
   cookie s, identities a(sa{sv}))` and `CancelAuthentication(cookie s)`. The
   timeout is `G_MAXINT`: a request stays open until the agent ends it [src].
3. A normal reply to `BeginAuthentication` yields polkitd's own
   `session->is_authenticated`, which **only the helper's**
   `AuthenticationAgentResponse*` sets: uid 0 only, a cookie bound to the
   agent's `creator_uid`, an identity among `session->identities`. An error
   whose name maps to `POLKIT_ERROR_CANCELLED` marks the request "dismissed";
   any other error does not [src]. **The agent cannot grant.**
4. When an agent's unique bus name vanishes polkitd cancels that agent's
   sessions and removes it [src].
5. polkitd expands groups into users before calling and falls back to uid 0
   for an empty list, so `unix-group` identities do not reach the agent in
   practice. The details carry `polkit.caller-pid` and `polkit.subject-pid`
   [src].
6. The `message` is attacker-influenced. For `org.freedesktop.policykit.exec`
   pkexec builds it from the caller's argv, joined by spaces, unsanitised and
   cut at 80 bytes; the full argv and the program path travel in `details`
   [src]. Any local user can run pkexec with any argv.
7. Of 280 installed action ids, 59 contain capitals or `_`
   (`org.freedesktop.NetworkManager.*`, `id.waydro.Initializer.Init`, …)
   [host]; the longest is 65 characters.
8. Bus policy: only user `polkitd` may own `org.freedesktop.PolicyKit1`, and
   only it may send on the agent interface; the bus is `dbus-broker` [host].
   Whether another `system.d` file grants uid 0 a wider `send_destination` is
   unverified (U-1).

**The helper**

9. If `/run/polkit/agent-helper.socket` exists the library connects a Unix
   stream socket to it and writes `<username>\n<cookie>\n`; otherwise it
   spawns the setuid `polkit-agent-helper-1 <username>` and writes the cookie
   to its stdin [src]. On this machine the binary is mode 0755, **not
   setuid**, so only the socket works. `/run/polkit` is root-owned; the socket
   is `srw-rw-rw- root` **by design** and is served by the sandboxed one-shot
   `polkit-agent-helper@.service` (`Accept=yes`, `--socket-activated`)
   [host]. After `connect`, `SO_PEERCRED` names the listener (systemd, uid 0),
   not the per-connection helper.
10. Helper → agent: one line per event, `g_strescape`-escaped (octal `\ooo`
    for non-printables, so UTF-8 text arrives as octal): `PAM_PROMPT_ECHO_OFF
    <text>`, `PAM_PROMPT_ECHO_ON <text>`, `PAM_ERROR_MSG <text>`,
    `PAM_TEXT_INFO <text>`, then `SUCCESS` or `FAILURE`. After a prompt the
    helper reads **one line with `fgets` into a 512-byte buffer**: an answer
    longer than 510 bytes of content is split and its tail answers the next
    PAM prompt [src, host]. Libpolkit treats any other line, EOF or a hang-up
    as failure [src]. The helper sets no timeout of its own [src].
11. **The helper reports the success, not the agent**: on success it tells
    polkitd (`AuthenticationAgentResponse2/3`, with the caller's uid and
    pidfd from the socket) *before* printing `SUCCESS` [src]. Cancelling is
    closing the socket.
12. The PAM stack is `polkit-1 → system-auth` with `pam_faillock`
    (`preauth`, `authfail`, `authsucc`); `deny = 3`, `unlock_time = 600`, and
    the lock applies to every PAM service [host].

**This machine and this repository**

13. `GetSessionByPID` of the shell **fails** ("PID does not belong to any
    known session": niri runs under `user@1000.service`, outside a session
    scope). `loginctl show-user toni -p Display` gives `Display=11`, which is
    how polkitd itself resolves such processes (`sd_uid_get_display`)
    [host]. `XDG_SESSION_ID` is also set, and is what the C++ reference used.
14. `kernel.yama.ptrace_scope = 1`; swap is zram; core dumps go to
    `systemd-coredump` [host].
15. zbus 5.19: `#[interface]` methods are spawned as tasks and "may not be
    run in the order in which they were called"; the macro deserialises the
    body before the handler runs; `Builder::system()` honours
    `DBUS_SYSTEM_BUS_ADDRESS`; the blocking connection offers
    `reply`/`reply_error` [repo, vendored source]. `smoke.sh` sets that
    variable to a dead socket in every nested run so a smoke never touches
    the real logind [repo].
16. The tray already recognises `NameOwnerChanged` only when the sender is
    the bus driver and the message is a broadcast (`src/tray/item.rs`,
    `recognise`) [repo].
17. Toolkit: `unsafe` is denied by the workspace and permitted only in
    `celestina-wayland/src/egl.rs`; `InputEvent::Key.utf8` is an
    `Option<String>` and `InputEvent` derives `Debug`, and `runtime.rs:1198`
    logs `?ev` at `debug` for input to a surface that is gone — **a password
    key could reach the journal**; `text_input.rs` never sends
    `set_content_type`; `celestina_ui::Field` is a `String` editor with an
    IME preedit, selection and paste; the launcher already uses
    `KeyboardInteractivity::Exclusive`; the release profile unwinds [repo].
18. `fluorita-core::claimed_text` is the suite's "one sanitiser" for
    attacker-sized strings; it already strips U+00AD, U+180E and U+2061–2064
    [repo]. `docs/projects.toml` gives the prefix `celestina:` the roots
    `celestina/` and `celestina-rs/crates/celestina-shell/`; `scripts/` is
    outside them [repo].

## 4. Design

### 4.1 Pieces and their homes

| Responsibility | Home |
|---|---|
| Request state, queue, backoff, attempt counter, helper-line decoding, identity choice, `Cookie` — pure, tested without a bus | `celestina-rs/crates/celestina-shell/src/polkit.rs` |
| The `polkit` duty (`off` default) | `celestina-shell/src/settings.rs`, `Duties` |
| The text sanitiser | shared owner, see §6.2 (D) and §8 |
| The agent's bus thread: dedicated system connection, registration, owner cache, `BeginAuthentication`/`CancelAuthentication` intake, `ReplyHandle` | `celestina/src/providers/polkit/{mod,agent,session}.rs` |
| The helper conversation: socket checks, line framing, bounded reads | `celestina/src/providers/polkit/helper.rs` |
| Loop-side state: the active request, the surfaces, the guards of §5 | `celestina/src/polkit/{mod,state}.rs` |
| The card and the dimming | `celestina/src/polkit/view.rs` |
| `Secret` and `SecretField` (zeroizing, fixed capacity, no preedit, no selection) | `celestina/crates/celestina-ui/src/secret.rs` |
| Keyboard enter/leave per surface; redacting `Debug` for key events | `celestina/crates/celestina-wayland` |
| Spanish copy | `celestina/src/copy.rs` |

Dependencies point down: the providers and the loop module depend on the
pure domain, never the reverse; the toolkit components know nothing about
polkit, and CEL-2-H's lock reuses `Secret` and `SecretField`.

### 4.2 Threads, dispatch and replies

The `#[interface]` macro is **not** used for the agent object (fact 15):
its tasks have no order, so a `CancelAuthentication` could overtake the
`BeginAuthentication` it cancels, a `NameOwnerChanged` could be processed
after the message of the owner it announces, and the body is deserialised
before any sender check could run.

- **The bus thread** (`Worker`, named `polkit-bus`) owns a dedicated system
  connection that exports nothing and reads one blocking `MessageIterator`
  in arrival order. For each message it (1) reads the **header** and drops
  anything that is not a method call to the agent path or a broadcast
  `NameOwnerChanged` from the bus driver (the tray's recogniser, extracted
  to one shared owner in this unit, the tray delegating); (2) for a method
  call, compares the sender with the cached unique owner of
  `org.freedesktop.PolicyKit1` *before* deserialising the body, answering
  `AccessDenied` otherwise; (3) deserialises and validates; (4) forwards
  `Begin` or `Cancel` to the loop as a `Message::Polkit` and goes on reading.
  It never waits for the person, so a `Cancel` and an owner change are seen
  while a request is pending.
- **Replies** are sent by whoever ends the request, through a `ReplyHandle`
  (a clone of the connection plus the call's header) with the blocking
  connection's `reply`/`reply_error`. The handle is a drop guard: dropped
  unanswered it sends `Cancelled`; answered once by move, it cannot answer
  twice (N-23).
- **The owner cache** installs the `NameOwnerChanged` match **before** the
  seeding `GetNameOwner`, so a restart in the gap is not missed; the cache
  holds a *unique* name, which a bus never reuses, so comparing against it is
  free of time-of-check races.
- **A helper thread** (`Worker`) per active conversation owns the socket,
  reads with a bounded timeout and looks at the shutdown flag; it receives
  the secret through a `sync_channel(1)` (a move) and ends by closing the
  socket.
- **Shutdown and panics.** Closing the connection wakes the blocking
  iterator, so the join is bounded. On a panic of either worker the
  connection is dropped, so the unique name vanishes and polkitd cancels the
  sessions itself (fact 4): a reply that can no longer be sent never leaves a
  request hanging.
- **The loop** owns the request table (the pure `Queue`) and the UI; it never
  does blocking IO (N-22).

### 4.3 Data flow of one request

```text
polkitd ──BeginAuthentication(action, message, icon, details, cookie, identities)──▶ bus thread
    header: sender == cached polkitd owner?  no → AccessDenied, nothing else happens
    body: cookie valid and new? action id valid? identity usable? queue room? duty on? not locked?
    no → error reply (§6.4)      yes → Message::Polkit(Begin) carrying the ReplyHandle ──▶ loop
loop:  queue → when next: close the shell's other keyboard surfaces, create the prompt surfaces (§5),
       wait for wl_keyboard.enter on the card's surface (KEYBOARD_PROOF), then the guard window
       proof fails → Cancelled and a visible notice (§5.3)
helper thread: connect, write "<user>\n<cookie>\n"
       ◀── PAM_PROMPT_ECHO_OFF <text> ──▶ the card asks for the password
       Enter on a non-empty field moves the Secret to the helper thread; it is written and wiped there
       ◀── SUCCESS ── the helper has already told polkitd; ReplyHandle replies normally; the card closes
       ◀── FAILURE ── an answer was sent: attempt n of MAX_ATTEMPTS; n < max → new conversation, same
                       cookie; n == max → Cancelled. No answer was sent → Failed (§6.4 F-12)
polkitd ──CancelAuthentication(cookie)──▶ bus thread ──▶ loop: that request ends, whatever its state
```

### 4.4 Why the agent speaks the helper's protocol itself

The halted shell linked `libpolkit-agent-1` in a separate C++ child because
the helper's transport (a setuid binary or a socket) is polkit's decision. The
Rust shell denies `unsafe` and would need FFI. The protocol is five line types
over a Unix stream socket (facts 9–11). Speaking it directly keeps the shell
free of FFI and of a second process.

The cost and its bounds: the transport choice becomes ours. The agent
(a) uses **only** the socket and never spawns the setuid binary (dead code
here, and a spawned child raises argv and environment questions); (b) before
connecting checks that `/run/polkit` is a directory owned by uid 0 and not
writable by group or others, and that the socket path is a socket owned by
uid 0 — it does **not** require a mode on the socket, which is 0666 by
design — and after connecting that `SO_PEERCRED` says the listener is uid 0
(this proves "a root listener", not "the helper"); (c) treats an unknown
line, a line over `MAX_HELPER_LINE`, more than `MAX_HELPER_LINES` in one
conversation, EOF or a silence over `HELPER_SILENCE` as a failed
conversation; (d) decodes every helper line strictly (`\\ \" \n \t \r \b \f
\v \a` and octal `\o`, `\oo`, `\ooo`; anything else is refused) and
validates UTF-8 **after** decoding; (e) matches `SUCCESS` and `FAILURE` only
as whole lines. A canary test with a fixture helper and the live row
`VAL-CEL2-POLKIT-LIVE` mean that a polkit release that changes the protocol
shows up as "denied", never as a grant. Revisit in §11.

### 4.5 Registration and its lifecycle

- The duty `polkit` is `off` by default. Switched on, the shell opens its
  dedicated system connection and resolves the **session id** from logind:
  `GetUser(uid)` → `Display`, then checks the session (`Uid` is the shell's,
  `Class=user`, `Type=wayland`, `Remote=false`); `XDG_SESSION_ID` is only a
  hint that must agree. If no session resolves, the duty reports
  `unavailable` and says why (`GetSessionByPID` is not used: fact 13).
  It registers `unix-session{session-id}` with `RegisterAuthenticationAgent`,
  the object path `/org/celestina/PolkitAgent1`, and the locale of
  `LC_MESSAGES`, else `LANG`, bounded and validated, else `C` (polkitd
  translates `message` by it).
- **A refused registration is final until the person acts**: the error is
  logged once as "another agent holds this session", the duty reports
  `refused`, and nothing retries or evicts. Switching the duty off and on is
  the retry.
- **polkitd absent** at activation: `RegisterAuthenticationAgent` is sent to
  a bus-activatable name, so the first call may start it; if the name still
  has no owner the duty reports `waiting` and registers once when the owner
  appears.
- **polkitd restarting** drops the registration. On an owner change or loss
  the shell **tears down every request at once** (helper socket closed,
  secret wiped, prompt closed, no reply to the old owner) and, for a new
  owner, registers once; a refusal then (Noctalia took the slot meanwhile) is
  `refused` with no retry.
- **Switching the duty off**, or a clean exit, cancels every request,
  unregisters and closes the prompt. A crashed shell is cleaned up by polkitd
  when the connection drops. Either way the session returns to "no agent" and
  actions fail closed (§7).

### 4.6 Identity and text

- The agent offers the **person's own account** if it is among the
  `unix-user` identities polkitd sent, and **otherwise refuses** (Q11).
  Group identities that still arrive are skipped. The uid becomes the name
  the helper wants through a bounded read of `/etc/passwd`; an absent or
  control-character-bearing name fails closed. (NSS-only accounts are out of
  scope, §11.)
- The card shows polkit's `message` as the description and the `action_id` on
  its own line, both as polkitd gave them after the sanitiser (§6.2, D). The
  `details` map and the `icon_name` are not shown or used: the icon is always
  the shell's own lock glyph. The shell composes no description of its own.
- The text of PAM prompts (`PAM_PROMPT_ECHO_OFF`, `PAM_ERROR_MSG`,
  `PAM_TEXT_INFO`) is shown decoded, sanitised and bounded, as PAM's words and
  never worded as "the password of X" (a prompt can be a one-time code).

## 5. The prompt surface

### 5.1 Surfaces

One layer surface per output, namespace `celestina-polkit-prompt`, layer
`Overlay`, anchored to all four edges, exclusive zone −1. The surface on the
**target output** (the focused output; else the first) holds the card and asks
for `KeyboardInteractivity::Exclusive`; the surfaces on the other outputs only
dim and take no keyboard, and an output that appears while a prompt is up gets
one too. Colour and opacity of the dimming and the card's anatomy are
semantic theme tokens. With `reducedMotion` the card appears and goes without
a fade. The namespace is one literal, cited by a test, because the author's
niri layer rule depends on it.

When a prompt becomes active the shell **closes its other surfaces that hold
or request the keyboard** (the launcher, an open Island page) and refuses to
open them until the request ends: two `Exclusive` overlays leave the choice
to the compositor and would starve the proof.

### 5.2 The card

A column: the lock glyph, a title (Spanish copy), polkit's message, the
action id, the account being asked for, the secret field, a line for PAM's
words and the failure line, and two controls (authenticate, cancel; Spanish
copy). Enter submits a non-empty field, Escape cancels, Tab moves between the
field and the controls. A click outside the card does nothing (a stray click
must not deny). Everything is operable from the keyboard.

The **secret field** is a toolkit component, not `celestina_ui::Field`: no
selection, no copy or paste, no word motions, no preedit, no undo. It never
enables `zwp_text_input_v3` for the surface (S-3), so a person who needs an
input method cannot use this prompt (Q8); dead keys work because the keyboard
stack composes them in the client. It shows **no per-character glyphs**: the
field is "empty" or, once anything is typed, a fixed-width row of dots. The
cost is that the person cannot count their keystrokes; the field's accessible
state says only "has content", and the card's text says what Enter does.

### 5.3 Keyboard proof, loss and the guard window

`KeyboardInteractivity::Exclusive` is a request; layer-shell has no "granted"
reply. The old agent trusted it. This one does not:

- **Proof.** The card is not ready, and no helper connection is opened, until
  its surface has received `wl_keyboard.enter`, within `KEYBOARD_PROOF` of
  the first commit after configure. Otherwise the request ends `Cancelled`,
  and the shell raises a visible, non-stealing notice (through its own
  notification path) that an authorization prompt could not be shown — so
  that a client suppressing the real prompt does not also hide that it
  happened.
- **Guard window.** After the enter the card ignores keys for
  `FOCUS_GUARD`, and ignores any key whose press began before the enter and
  an Enter on an empty field: the person may be typing in another window
  when the prompt appears, and a held Enter must not become an attempt. The
  same silence applies between two queued requests.
- **Loss.** On `wl_keyboard.leave` the partial secret is wiped at once and
  typing is not accepted until the keyboard returns; if it does not return
  within `FOCUS_GRACE` the request is `Cancelled`. A closed surface, a vanished
  target output or a locked session ends it immediately. Keys typed before a
  loss are never kept, so a prompt that lost the keyboard cannot be typing the
  rest of a password into another client.
- **Honest residual.** A virtual-keyboard client injects into the focused
  surface, which is this one: no leave event, no wipe. It can burn the
  attempts or press Escape; §6.2 records it and recommends withholding the
  virtual-keyboard and input-method protocols from untrusted clients in the
  author's niri configuration.

### 5.4 Queue and backoff

`BeginAuthentication` calls arrive from polkitd one per pending check. The
shell shows **one request at a time**; up to `MAX_QUEUED` wait, and at most
`MAX_PROMPTS_PER_MINUTE` are shown. A request beyond that, any request when
the duty is off, and any request while the session is locked is refused with
`Cancelled` at once and never shown.

Consent fatigue is an attack (§6.2, B): after `BACKOFF_DISMISSALS` prompts in
a row ended without an attempt (Escape, idle timeout, proof failure), the
shell enters a cool-down of `BACKOFF_COOLDOWN`: further requests are answered
`Cancelled` without being shown, with one notice, and a prompt never takes the
keyboard again within `REGRAB_GAP` of the previous one ending. A
`CancelAuthentication(cookie)` removes a waiting request or ends the active
one. An active request ends `Cancelled` at `IDLE_TIMEOUT` after it was
mapped — an absolute cap that typing (or injected typing) does not extend.

**Locked sessions.** The loop owns one `locked` state, read synchronously
before any polkit surface is mapped, failing closed when unknown; the lock
(CEL-2-H) layers above. The mechanism is fixed here and its source is wired
when CEL-2-H lands; until then the flag is always false and nothing can
lock.

## 6. Security

### 6.1 Assets and trust boundaries

| Asset | May exist | Never reaches | Lifetime |
|---|---|---|---|
| **Secret** (the PAM `ECHO_OFF` answer) | the field's fixed buffer → moved to the helper thread → the helper socket | journal at any level, `Debug`, `Message`, `org.celestina.Shell1`, any property, argv/env/path, clipboard, IME, accessibility, disk, a second request | first keystroke to the end of the socket write, or to the end of the request |
| **Cookie** | the request record → the helper socket line | journal, `Shell1`, argv/env, the UI layer (surfaces name a request by a local id) | the request |
| **Chosen identity** | the request record, the card | never synthesised: always one of the identities polkitd sent | the request |
| **Verdict** | the helper's `SUCCESS`/`FAILURE` | never produced by the agent | the request |
| **Prompt text** | the card, after decode and sanitising | never composed by the shell as the action's description | the request |
| **Keyboard / surface** | the card's surface while mapped | other clients, the IME | while mapped |

Trust, least to most: other Wayland clients; same-uid processes (partly out of
scope); text relayed by polkitd (hostile input: fact 6); polkitd by its
*current unique name*; the helper over the root-owned socket (trusted for the
verdict, and the only party that reports to polkitd); the compositor (trusted;
it cannot be defended against).

### 6.2 Threat model

Each row ends in the invariants (N-n, §6.3), failure rows (F-n, §6.4) and
secret rules (S-n, §6.5) that defend it, and says what is left.

| # | Attacker and vector | Defence | Residual, stated honestly |
|---|---|---|---|
| **A** | **A forged authority on the system bus.** A process calls `BeginAuthentication`/`CancelAuthentication` on the agent, races polkitd's name during a restart, or **unicasts a forged `NameOwnerChanged`** to poison the owner cache (which would make every real request `AccessDenied`) | bus policy (fact 8) is the first wall; N-12 is the second: header check before the body, against a cached unique owner kept only by broadcasts from the bus driver (the tray's recogniser), match installed before the seeding query; N-13: dedicated connection, nothing else exported; F-1, F-8; integration tests for a forged caller and a forged signal | a same-uid attacker who controls the shell's **launch environment** owns the agent: `DBUS_SYSTEM_BUS_ADDRESS` (honoured by zbus and by `smoke.sh` on purpose, fact 15) can point it at a bus of theirs, and `WAYLAND_DISPLAY` at a compositor that sees every key in clear. **Launch-environment control is out of scope** for the whole unit, and §6.5's hygiene is not claimed against it. A forged request could never *grant* (the cookie is unknown to polkitd); the damage is phishing |
| **B** | **Exhaustion and consent fatigue.** A local user loops `pkexec <argv of their choice>`; each shown prompt dims the output and takes the keyboard; one weary approval of a real prompt authorizes their `policykit.exec` action | N-27, F-5: one active, `MAX_QUEUED` waiting, `MAX_PROMPTS_PER_MINUTE`, a cool-down after `BACKOFF_DISMISSALS` dismissals, `REGRAB_GAP`; the card shows the polkit text and action id unaltered (D) | a determined user can still show prompts at the cap before the cool-down; the message can be persuasive within 80 bytes |
| **C** | **Same uid: `/proc`, ptrace, core dumps, swap.** | nothing secret in argv/env (N-2); `ptrace_scope=1` [host]; non-dumpable window while a request is open (S-9); zeroize on every path (S-8); swap is zram | an ancestor of the shell, a process with `CAP_SYS_PTRACE`, a replaced binary or environment. `mlock` needs `unsafe`, so pages are not locked (hibernation: U-5). Kernel socket buffers hold the secret until the helper reads it. The keyboard stack (sctk, xkb) frees key-text `String`s without zeroing, and the dumpable window does not cover those freed blocks (S-6). **The unit defends against an attacker who arrives later and reads leftovers, not one already active** |
| **W** | **Wayland clients.** (a) cover the prompt with an exclusive overlay to steal the keyboard; (b) draw a fake prompt; (c) capture the screen; (d) inject keys with a virtual keyboard; (e) read the IME stream | (a) F-6 (proof, with a visible notice), F-7 (loss wipes), N-24, §5.1 closes the shell's own keyboard surfaces; (b) none is complete — Wayland has no trusted path; the real prompt always shows action id and account on fixed lines; (c) no exact-length echo (S-4); a niri `layer-rule` with `block-out-from` for the namespace is proposed (Q7; whether niri applies it to layer surfaces is unverified); (d) the attempt cap, the guard window, the burst limits; (e) no IME (S-3) | phishing by a look-alike surface (narrowed by the notice of §5.3, not closed); injected keystrokes (burn the attempts, press Escape, trip faillock for the account — client-side undefendable; withhold the virtual-keyboard protocols from untrusted clients in the niri configuration); capture of the dimmed card; **a malicious or buggy compositor sees every key and pixel and cannot be defended against** |
| **D** | **Hostile text** in `message`, `details`, `icon_name`, the action id and PAM text: pkexec argv with newlines, RLO and bidi controls, zero-width and tag characters, homoglyphs, ANSI, 10 kB strings, invalid UTF-8; PAM text escaped as octal (`\033`, `\000`, the bytes of U+202E) | one sanitiser for every shown string: it removes C0/C1 controls (a newline becomes a space), bidi embeddings, overrides and isolates, directional marks, zero-width and invisible-format characters (at least `fluorita-core::claimed_text`'s set), tag characters U+E0000–E007F, variation selectors, U+FFF9–FFFC and the Hangul fillers, replaces invalid UTF-8, and cuts at `MAX_MESSAGE`/`MAX_PROMPT_TEXT`/`MAX_ACTION_ID` with an ellipsis. **Order: decode, then sanitise**, for all helper text (polkitd's strings are not escaped and are sanitise-only). `icon_name` and `details` are ignored (N-20). The sanitiser is the suite's existing one, moved to a shared owner and extended (§8), not a second recipe | homoglyphs and persuasive wording; a legitimate action whose own message misleads. Forcing left-to-right paint has no API in `celestina-paint` and is not claimed: the action id sits on its own line and the sanitiser removes the controls that matter |
| **H** | **A substituted, hostile or hanging helper:** something else at the socket path; a helper that floods, sends unknown lines, never answers, or sends `SUCCESS` without a prompt | N-16 (constant path; directory, type and owner checks; `SO_PEERCRED` uid 0), N-17 (bounds, strict decode, whole-line verdicts), F-11…F-14. `SUCCESS` is only *displayed*: polkitd decides on its own state, so a lying peer grants nothing, and a non-root peer is refused before the first byte | a root-level attacker who owns the helper owns polkit anyway |
| **E** | **Another agent in the session** (Noctalia): slot conflict, eviction | decision 1; N-14; F-24 | none for security; the function is absent until the author changes the agent |
| **R** | **The shell dies or restarts; polkitd restarts;** a worker panics | the socket closes (PAM fails), the unique name vanishes (polkitd cancels, fact 4); a clean exit answers every request `Cancelled` then unregisters; F-8 tears everything down on an owner change; a panic drops the connection (§4.2); the buffer is wiped by unwinding | leftovers in a core dump if a crash falls inside the non-dumpable window's edges |

### 6.3 The unit NEVER…

Each item names its enforcement: **T** unit test, **I** integration test on a
private bus with a fake polkitd or helper, **G** a guard written as a test
under `celestina/tests/` that scans the polkit modules' sources (the shared
architecture scanner lives outside the prefix's `commit_roots`, §8).

**Secret and cookie**

- N-1. Never stores the secret or the cookie on disk, in settings, in the
  journal or in a `tracing` field at any level. T: a `TRACE` capture through a
  full fake conversation never contains a known secret, cookie or typed
  character. T/G: `Secret` and `Cookie` implement no `Debug`, `Display`,
  `Serialize` or `Clone`; every `InputEvent` variant that can carry typed text
  has a redacting `Debug`, with a test on the `runtime.rs` log site.
- N-2. Never passes the secret or cookie in argv, the environment, a path,
  `Shell1`, the loop's `Message` enum or any provider channel other than the
  one-slot move of S-7. G: `rg` forbids the types in `message.rs`, `bus.rs`,
  `command.rs`; T: type-level assertions written locally.
- N-3. Never copies the secret to a selection, never reads a selection into
  it, never exposes its content or length to accessibility. T: copy, cut,
  paste and select-all are no-ops on the field. G: the field is not
  `celestina_ui::Field`.
- N-4. Never sends the secret to an input method. T: text input is never
  enabled on the prompt's surface; preedit and commit events are dropped and
  wiped.
- N-5. Never holds the secret in a `String`, a `format!` result or a growable
  buffer that reallocates. T: capacity is constant across inserts up to the
  cap. G: no `String`/`format!`/`push_str` on secret paths.
- N-6. Never holds the secret past the end of its request, or across
  requests. T: after every terminal event of §6.4 a test probe sees the
  buffer all-zero.
- N-7. Never sends a secret containing `\n`, `\r` or NUL, or longer than
  `MAX_SECRET_BYTES` (510 bytes of content, so content plus the newline fits
  the helper's 512-byte `fgets`). The field refuses those keys and never
  truncates. T: the 511th byte is refused.

**Authority and verdict**

- N-8. Never authenticates without the person: no stored answer, no
  auto-submit, no "remember"; an attempt starts only from an explicit submit
  in the focused card, outside the guard window. T.
- N-9. Never decides a success: no branch replies "authorized", nothing calls
  `AuthenticationAgentResponse*`, nothing spawns any program. G: `rg` forbids
  `AuthenticationAgentResponse` and `std::process::Command` in the polkit
  modules.
- N-10. Never replies normally except after a helper `SUCCESS`; every other
  end is an error reply (§6.4). An error, EOF, timeout, unknown line or panic
  is never success. T: an exhaustive table test over §6.4.
- N-11. Never retries after a cancellation or a dismissal, and never opens a
  new conversation unless the previous one received an answer and ended in
  `FAILURE`. T.
- N-12. Never accepts `BeginAuthentication` or `CancelAuthentication` from
  anything but the cached current owner of `org.freedesktop.PolicyKit1`,
  checks that on the header before the body and before any logging of
  arguments, and updates the cache only from broadcast `NameOwnerChanged`
  signals sent by the bus driver. I: a fake caller gets `AccessDenied` with no
  prompt, no helper connection and no logged arguments; a peer's unicast
  forged signal leaves the cache unchanged and real requests still work.
- N-13. Never exports another interface on the agent's connection, and never
  exports the agent on the session bus. I: introspection shows only the agent.
- N-14. Never registers as the system-wide agent, never uses a fallback
  option or `RegisterAuthenticationAgentWithOptions`, never unregisters or
  evicts another agent, never retries a refused registration. I: the fake
  polkitd counts calls.
- N-15. Never chooses an identity polkitd did not offer, and never sends a
  username other than the chosen identity's name read from `/etc/passwd`, nor
  one containing `\n`, NUL or other controls. T.

**The helper**

- N-16. Never connects to the helper except at the constant socket path after
  the checks of §4.4(b), and never lets a setting or the environment choose
  that path, the PAM service or the helper binary; test seams exist only under
  `#[cfg(test)]` or a test-only feature. T.
- N-17. Never reads an unbounded helper line, never accepts an unknown line
  or a partial verdict, never sanitises before decoding. T: the helper
  scenarios of §10.1.

**Text and surface**

- N-18. Never shows text that is not polkit's or PAM's as the description of
  the action; the shell's own words are labels in `copy.rs`. G: the card builds
  its description only from the sanitised request fields.
- N-19. Never paints unsanitised text. T: a corpus of hostile strings.
- N-20. Never resolves `icon_name` or shows `details`.
- N-24. Never shows a prompt without the proven keyboard grab; never connects
  to the helper before the card has the keyboard and the guard window has
  passed; never keeps keys typed before a loss. T and I.
- N-25. Never lets another shell surface hold the keyboard while a prompt is
  active. T.

**Code and flow**

- N-21. Never uses `unsafe`; never uses production `unwrap`, `expect` or
  `panic!`. G: the workspace lint and the existing guard.
- N-22. Never runs blocking D-Bus or socket IO on the loop thread. G/review.
- N-23. Never replies twice and never forgets to reply: the `ReplyHandle` is a
  drop guard (§4.2); a worker panic drops the connection. T.
- N-26. Never consults a host-side coordinator (the lock, the idle chain)
  asynchronously: `locked` is read synchronously at map time and fails closed.
  T.
- N-27. Never shows more than the limits of §6.4 allow (queue, rate,
  cool-down, regrab gap). T.

### 6.4 Failure modes

Every row ends with the **agent** not granting; "Cancelled" is the error
`org.freedesktop.PolicyKit1.Error.Cancelled` (polkitd records "dismissed"),
"Failed" is `…Error.Failed`, "normal" is a method return. For rows after the
secret was sent, polkitd may already hold the helper's report: the card then
says "finished", never "denied".

| # | State / event | Agent action | Reply | Agent grants? |
|---|---|---|---|---|
| F-1 | Sender ≠ cached polkitd owner, or the cache is empty | nothing else | `AccessDenied` | no |
| F-2 | Cookie empty, over `MAX_COOKIE_BYTES` or not printable ASCII; action id empty or over `MAX_ACTION_ID` or outside `[A-Za-z0-9._-]` | refuse | Failed | no |
| F-3 | Cookie already pending | refuse the second call; the first continues | Cancelled for the second | no |
| F-4 | No identity resolves, none is the person's own, or `/etc/passwd` fails | refuse | Failed | no |
| F-5 | Queue full, rate limit, cool-down, locked, or duty off | refuse without showing | Cancelled | no |
| F-6 | Prompt cannot be mapped or the proof fails: no layer shell, no keyboard enter within `KEYBOARD_PROOF`, no output, surface closed by the compositor | close, wipe, raise the notice | Cancelled | no |
| F-7 | Keyboard lost while mapped | wipe now; close if not back within `FOCUS_GRACE` | Cancelled | no |
| F-8 | polkitd owner changes or vanishes | tear down every request; register once with a new owner | none (the owner is gone) | no |
| F-9 | Shell exit or duty turned off | answer all, close sockets, wipe, unregister | Cancelled | no |
| F-10 | Shell crash | none possible | polkitd cancels on name loss | no |
| F-11 | Helper socket missing, wrong type or owner, peer uid ≠ 0, connect error | close the card with a problem notice | Failed | no |
| F-12 | Helper line unknown, over `MAX_HELPER_LINE`, bad escape, bad UTF-8; or `FAILURE` when no answer was sent in that conversation | end, no retry (the helper is suspect or the account refuses prompts) | Failed | no |
| F-13 | Helper EOF or read error before a verdict | as F-12 | Failed | no |
| F-14 | No helper line within `HELPER_SILENCE` after the answer | close the socket | Failed | no |
| F-15 | `PAM_PROMPT_ECHO_ON` | refuse | Cancelled | no |
| F-16 | `FAILURE` after an answer, attempts < `MAX_ATTEMPTS` | wipe, show PAM's error text, new conversation, same cookie | pending | no |
| F-17 | `FAILURE` after an answer, attempts = `MAX_ATTEMPTS` | close | Cancelled | no |
| F-18 | `SUCCESS` (with or without a prompt before it) | wipe, close, say "done" | normal — polkitd decides on its own state | only if polkitd's state says so |
| F-19 | Escape or the cancel control | close the socket, wipe | Cancelled | no |
| F-20 | `IDLE_TIMEOUT` after the card was mapped | close | Cancelled | no |
| F-21 | `CancelAuthentication` for a pending cookie | close, wipe | Cancelled for the begin call; normal for the cancel call | no |
| F-22 | `CancelAuthentication` for an unknown cookie | ignore | normal | no |
| F-23 | Worker panic | drop guards wipe and reply; the connection is dropped | Cancelled, or none and polkitd cancels | no |
| F-24 | Registration refused (another agent) | duty `refused`, one log line, no retry | n/a | n/a |
| F-25 | No system bus, or no session resolves | duty `unavailable`, one log line | n/a | n/a |

The invariant a reviewer checks: **a normal reply to `BeginAuthentication`
exists only on F-18.** (F-21's and F-22's normal replies answer the *cancel*
call, which carries no verdict.) Constants (the values are proposals; the
author confirms them, Q4):

| Constant | Value |
|---|---|
| `MAX_SECRET_BYTES` | 510 |
| `MAX_ATTEMPTS` | 2 |
| `MAX_COOKIE_BYTES` | 256 |
| `MAX_HELPER_LINE` | 4096 bytes raw (worst case `g_strescape` is 4 bytes per input byte: 4 × 512 plus a verb) |
| `MAX_HELPER_LINES` | 64 per conversation |
| `MAX_PROMPT_TEXT` | 200 graphemes or 512 bytes |
| `MAX_MESSAGE` | 300 graphemes or 512 bytes |
| `MAX_ACTION_ID` | 128 bytes, `[A-Za-z0-9._-]` |
| `MAX_QUEUED` | 3 |
| `MAX_PROMPTS_PER_MINUTE` | 6, counted when shown |
| `BACKOFF_DISMISSALS` / `BACKOFF_COOLDOWN` / `REGRAB_GAP` | 3 / 60 s / 3 s |
| `KEYBOARD_PROOF` | 500 ms |
| `FOCUS_GUARD` | 400 ms |
| `FOCUS_GRACE` | 2 s |
| `IDLE_TIMEOUT` | 120 s, absolute |
| `HELPER_SILENCE` | 60 s |

### 6.5 Secret hygiene in Rust without `unsafe`

- **S-1. The type.** `Secret` is a newtype over `zeroize::Zeroizing<Vec<u8>>`
  created with `Vec::with_capacity(MAX_SECRET_BYTES)` and never grown;
  inserting past the cap is refused, so no stale copy is left in a freed
  block. No `Clone`, `Debug`, `Display`, `Serialize`, `PartialEq` or `Deref`.
  Its operations are `push(char)`, `pop()` (zeroing the bytes of the last
  char), `is_empty()`, `clear()` (zeroing over the used length) and
  `write_line_to(&mut impl Write)` (two `write_all`s, so no concatenated
  copy and no partial vectored write). The never-grow rule plus the explicit
  zeroing is the invariant; the crate's drop is not trusted to cover spare
  capacity.
- **S-2. The field.** `SecretField` is a view of its own; the only keys are
  character insertion, Backspace, Enter and Escape.
- **S-3. IME.** Text input is never enabled on the prompt's surface.
- **S-4. Echo.** No per-character glyphs, only "empty" or "has content".
- **S-5. Accessibility.** If the toolkit gains AT-SPI the field is a
  password-text role with no value or length; today the Rust toolkit has no
  AT-SPI (U-6), recorded as a validation gap and not claimed.
- **S-6. Keystrokes.** `InputEvent::Key` carries typed text as an
  `Option<String>`; every variant that can carry typed text gets a redacting
  `Debug` (the `runtime.rs` log site prints it today). The move to a
  non-allocating `Option<char>` is a separate optimisation that touches the
  toolkit's input, tests, launcher, tray and Island menus; it is **not**
  required and not done here. The residual — sctk and xkb allocate key text and
  free it unzeroed — is documented (§6.2, C).
- **S-7. Channel.** The card owns the `Secret`; submit **moves** it into a
  `sync_channel(1)` to the helper thread; the thread writes it and drops it;
  a send failure returns it inside the error and it is dropped. The card
  holds nothing after the move.
- **S-8. Every path wipes by ownership ending.** The release profile unwinds;
  a build-time assertion or a test of `cfg!(panic = "unwind")` guards that. No
  `catch_unwind`: unwinding is the mechanism.
- **S-9. Non-dumpable window.** While a request is active the shell sets
  `PR_SET_DUMPABLE=0` through rustix's safe `process::set_dumpable_behavior`
  and restores the **previous** value when the last request ends (nested
  windows are tested). It protects the agent's buffer, not the toolkit's freed
  key copies, and it changes `/proc/<pid>` ownership for the window (portals
  and inspection tools may notice; U-11).
- **The cookie** is a `Zeroizing<Vec<u8>>` newtype, validated (F-2), not
  `Clone`, written with the same care, and kept out of logs.

## 7. Recovery path

Without an agent, polkit fails closed: every action that needs interaction is
refused, and Hematita names it `no-agent` (ADR 0010). That is the baseline the
unit can always return to, so it installs no second recovery tool.

- **Back to Noctalia, in either order.** Switch the `polkit` duty off in
  `shell.json` (the shell cancels every request, unregisters and closes the
  prompt), or stop the shell (polkitd removes an agent whose connection
  vanished, fact 4). Then re-enable Noctalia's agent. While Noctalia holds the
  slot Celestina refuses (decision 1), so starting Noctalia first is also safe.
- **What stays installed and untouched:** `polkit` with `polkitd`, the
  socket-activated helper and its socket, and Noctalia with its agent. The
  unit installs no polkit action file, no service and no setuid file.
- **The duty is `off` by default**; the author switches it on; landing never
  replaces the live shell (program §4.1).
- **A terminal path with no graphical agent:** `sudo` talks to PAM directly
  and needs no polkit agent, so it stays the author's fallback for privileged
  work.
- **Rehearsal never touches the real session** (§10.1). `smoke.sh` never
  activates `polkit` against the real bus: it keeps its dead system bus in
  every nested run, which is the existing, intentional safeguard.

The recovery path stays in force until `VAL-CEL2-POLKIT-LIVE` passes; the
duty's default flips to `on` only in CEL-2-M, and only if that row has.

## 8. Files, tasks and wiring

Beyond §4.1, the unit changes or adds:

| Path | Change |
|---|---|
| `celestina-rs/crates/celestina-shell/src/polkit.rs`, `settings.rs` | the pure domain and `Duties.polkit` (`zeroize` for `Cookie`) |
| shared owner of the sanitiser (default: `celestina-core`, with `fluorita-core::claimed_text` delegating) | extended as in §6.2 D; **a preceding `suite-maintenance:` unit**, because `fluorita-core` and `celestina-core` sit outside the `celestina:` prefix. The alternative, recorded in Q14, is a separate polkit copy with the reason written down |
| `celestina/src/tray/item.rs` and a shared owner of the recogniser | extraction, the tray delegating (N-12) |
| `celestina/crates/celestina-ui/src/secret.rs` | `Secret`, `SecretField`, `zeroize` in its manifest |
| `celestina/crates/celestina-wayland/src/{runtime,input,handlers}.rs` | redacting `Debug` and the two log sites; keyboard enter/leave per surface |
| `celestina/src/{providers/polkit,polkit}/…`, `message.rs`, `app.rs`, `main.rs`, `copy.rs` | as §4.1; the duty's live switch; the launcher and Island close/refuse rule of §5.1 |
| `celestina/Cargo.toml` | `zeroize`; `rustix` gains the `process` and `net` features (it is already a direct dependency); each justified in the manifest |
| `celestina/tests/` | the guards of §6.3 (`G`) as source-scanning tests |
| `celestina/docs/decisions/` | the ADR this design becomes (§13); its number is assigned at landing |
| `celestina/{README,STATUS,ROADMAP,VALIDATION}.md`, `docs/handover.md`, the ledger row | state, evidence, `VAL-CEL2-POLKIT-LIVE` |

**Order of tasks** (the plan makes each a unit of work with its tests):
0 the spike that closes the checks of §10.3; 1 the preceding shared-owner
extractions; 2 the toolkit (`Debug`, enter/leave, `Secret`, `SecretField`);
3 the pure domain; 4 the bus thread; 5 the helper conversation; 6 the loop
state and the card; 7 the duty, wiring and copy; 8 guards, docs and the
validation row.

The C++ files are a **read-only reference** and nothing is copied (§9). They
are not on the root `AGENTS.md` salvage list: before the unit's first commit
the author adds `celestina-qt/src/polkit{agent,conversation,promptcontroller}.cpp`
and `celestina-qt/src/polkitconverse/main.cpp` as reference for
`celestina/src/providers/polkit`, as for CEL-2-B and CEL-2-E (program §4.3).

## 9. What was kept, changed and dropped from the halted shell

| | |
|---|---|
| **Kept** (ADR 0005 and the C++ agent were right) | the shell owns the prompt and no verification; the response is passed to the helper and dropped; the prompt is its own surface with exclusive keyboard and is refused rather than shown degraded; the description comes from polkit; every failure denies; registration is for this session only; unregister on a clean exit and register again when polkitd returns; echo-on PAM prompts are refused; the sender must be polkitd |
| **Changed** | the conversation is spoken in Rust over the socket, not through `libpolkit-agent` in a C++ child; the sender check compares a cached unique name from authentic broadcasts, runs first and covers `Cancel`; two attempts, a queue, a rate limit, backoff and an idle cap; the keyboard is *proven* and *kept*; the helper connection opens only after the card has focus; the secret is a fixed-capacity zeroizing buffer moved to the helper thread, over a toolkit whose key events no longer print their text; polkit's text goes through one sanitiser; `icon_name` and `details` are ignored; the session comes from logind's `Display`; a worker panic drops the connection |
| **Dropped** | the converse child, `CELESTINA_POLKIT_CONVERSE`, percent-encoding of the response and every blocking wait on the GUI thread |

The reference's defects, each now an invariant above: a blocking owner lookup
per call that in-process callers skipped, no sender check on `Cancel`,
requests left alive across a polkitd restart, a helper started for a request
already refused, a string wipe that cleared a detached copy, no cap on the
answer (fact 10), an unsanitised `message`, no handling of keyboard loss, one
attempt, a second concurrent request dismissed instead of queued, and a
helper path selectable by environment.

## 10. Verification and review

### 10.1 Automated evidence

Per program §9: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
`cargo test`, `celestina --check`, `smoke.sh`, the common guard
`bash scripts/check-architecture-contract.sh`, and the language and
documentation contracts. Plus:

- **Pure tests:** every state and terminal event of §6.4; the queue, rate
  limit and backoff; the attempt counter; reply-exactly-once; the sanitiser
  against a hostile corpus (RLO, newline, ESC, 10 kB, invalid UTF-8,
  combining storms, escaped octal `\033`/`\000`/RLO inside a PAM message);
  the line decoder; identity choice; `Secret`'s fixed capacity, the 511-byte
  refusal and zeroization through a test probe; nested dumpable windows;
  `cfg!(panic = "unwind")`.
- **Private bus** (a `dbus-daemon` started by the test with its own policy,
  permissive enough that a fake caller reaches the agent): a fake polkitd
  owns `org.freedesktop.PolicyKit1`, calls `BeginAuthentication` and
  `CancelAuthentication`, records replies and counts registrations; a fake
  caller gets `AccessDenied`; a unicast forged `NameOwnerChanged` changes
  nothing; a restart of the fake polkitd mid-prompt tears the request down;
  a `Cancel` and an owner change are processed while a `Begin` reply is
  pending; a fake logind answers `GetSessionByPID` with `NoSessionForPID` and
  `GetUser().Display` with the session.
- **Fake helper:** a Unix socket in a temporary directory served by a test
  thread scripting PAM lines (prompt, error, `SUCCESS`, `FAILURE`, garbage,
  flood, silence, EOF, a 4097-byte line, a line with no newline, `SUCCESS`
  before any prompt, `SUCCESS` then more lines). The seams exist only under
  `#[cfg(test)]` or a test-only feature (N-16).
- **Journal capture:** a `TRACE` subscriber through a full fake conversation
  never sees the secret, the cookie or a typed character.
- **Nested niri:** the prompt maps, holds the keyboard, closes on Escape and
  denies on focus loss, against the **private** bus. `smoke.sh` keeps its dead
  system bus; with `polkit` on and that bus the duty reports `unavailable` and
  does not panic. The nest runs in the author's real logind session, so no
  nested run registers against the real system bus.

### 10.2 Review

This design was attacked twice before the author saw it: a general review by a
second model and an adversarial security-only pass by Opus 5.5 at the highest
reasoning (the reports are in the notes folder of the session that wrote
this). Their findings are folded in: the owner-cache poisoning by a forged
signal, the dispatch model, the launch-environment boundary, consent fatigue,
the session lookup, the socket mode, the action-id charset and the faillock
lockout among them. The implementation is reviewed the same way:

1. **A review by a model other than the implementer, at high reasoning,**
   covering §6.3 and §6.4 line by line, the toolkit changes and the repo
   contracts, and answering: is there any path from `BeginAuthentication` to
   a normal reply that did not pass a helper `SUCCESS`; does every terminal
   state drop the `Secret` and the `ReplyHandle` exactly once; is any
   blocking call reachable from the loop thread; does any type that can hold
   secret bytes implement `Clone`, `Debug` or `Serialize` or reach `Message`;
   does the sender check run on the header before the body and before any
   logging; can a forged signal change the owner cache.
2. **A second pass, adversarial and security-only, by Opus 5.5 at the highest
   reasoning** (the author's brief says Fable is not available). It tries to
   break: a forged `BeginAuthentication` or `NameOwnerChanged` during an owner
   change; 1000 requests, duplicate cookies, empty or group-only identities, a
   uid without a passwd entry, a cookie containing `\n`; a 511-byte, a
   600-byte and a newline-containing secret; the hostile text corpus; every
   helper scenario of §10.1; focus lost mid-typing; the journal at
   `RUST_LOG=trace` after a scripted session; polkitd restarted mid-prompt;
   the shell killed mid-answer and its core; whether any setting or
   environment variable beyond the documented zbus one can redirect the
   socket, the helper or the logging; any `unsafe`, `unwrap`, `expect` or
   `panic!`, including in test code that ships in the library.
   **It does not pass** if any §6.3 item lacks its test or guard; if any
   §6.4 row can end in a normal reply to `BeginAuthentication` without a
   helper `SUCCESS`; if the secret or cookie is observable in logs, `Debug`,
   `Message`, `Shell1`, argv or environment; if a secret over 510 bytes or
   containing `\n` reaches the socket; if a forged caller causes any effect
   other than `AccessDenied`; if a pending request survives a polkitd owner
   change; if the helper connection opens before the card has the keyboard;
   or if any check of §10.3 that an invariant depends on is still open.
3. **Models.** The implementer is never the reviewer or the attacker. The
   table the author's global rule asks for is presented and approved before
   implementation starts.

### 10.3 Checks the plan must close first (the spike)

| # | Check | Why |
|---|---|---|
| U-1 | Whether another `system.d` file grants uid 0 a wider `send_destination` than `org.freedesktop.PolicyKit1.conf` (fact 8) | the first wall |
| U-2 | That `org.freedesktop.PolicyKit1.Error.Cancelled` maps to `POLKIT_ERROR_CANCELLED` ("dismissed") | the meaning of every cancellation |
| U-5 | Whether hibernation is configured | pages reaching disk |
| U-6 | That the Rust toolkit exposes no AT-SPI | the accessibility claim |
| U-11 | What the non-dumpable window changes for portals and `/proc`-based tools | S-9 |

Closed by this design's review: zbus honours `DBUS_SYSTEM_BUS_ADDRESS` (and
that is kept); `deny = 3`; the dispatch model needs no spike because the bus
thread reads one ordered iterator; `sudo` needs no agent.

## 11. Revisit when

- polkit changes the helper protocol, or the helper stops being
  socket-activated (§4.4): the Rust conversation then fails to "denied" and the
  feature is lost until updated.
- The author wants the requesting process or pkexec's full command line in
  the card (Q3).
- An input method must work in the prompt (Q8).
- The author's accounts leave `/etc/passwd` (NSS-only users).
- A maintained standalone agent becomes something the author prefers (ADR
  0005's own revisit); the prompt is separable from the shell.
- A separate agent process becomes worth a second deployable (§2).
- Launch-environment control stops being out of scope (§6.2, A).

## 12. Open decisions

None blocks writing the plan; each has a proposed default that the plan
implements unless the author changes it, and every way fails closed.

| Q | Question | Proposed default |
|---|---|---|
| Q1 | An agent-side fault (bad cookie, no identity, helper fault) replies `Error.Failed`; the person's Escape, the timeouts and the attempt cap reply `Cancelled`. Or always `Cancelled`? | `Failed` for faults: `Cancelled` tells polkitd "dismissed", untrue for a fault |
| Q3 | Show anything from `details` (pkexec's `program` and `command_line`, sanitised and capped), the best defence against a misleading 80-byte `message`? | no |
| Q4 | The constants of §6.4 | as written |
| Q5 | A process-wide non-dumpable window while a request is open (S-9) | yes |
| Q7 | A niri `layer-rule` with `block-out-from` for `celestina-polkit-prompt`, and withholding the virtual-keyboard and input-method protocols from untrusted clients, in the author's own niri configuration (outside the repo; whether niri applies the rule to layer surfaces is unverified) | the author decides |
| Q8 | No input method in the secret field in v1 | accept |
| Q11 | When polkitd does not offer the person's own uid: refuse | refuse |
| Q12 | Paste from a password manager into the secret field | refused in v1 |
| Q14 | Extract the suite's `claimed_text` to a shared owner as a preceding `suite-maintenance:` unit (default), or keep a polkit-local sanitiser and write down why | extract |

## 13. Decisions recorded here (to become `celestina/docs/decisions/`)

(Numbers continue the parity program's list, which ends at 15; the ADR's file
number is assigned at landing so it cannot collide with CEL-2-H's.)

16. **The shell is the session's polkit agent when the `polkit` duty is on,
    inside its own process**, owning the prompt and nothing behind it; the
    password is verified only by `polkit-agent-helper-1`, which also reports
    the verdict to polkitd (§1, §4).
17. **The agent speaks the helper's socket protocol itself**, without
    `libpolkit-agent`, FFI or a child process; the socket variant only
    (§4.4).
18. **Another agent holding the slot means the shell refuses**, never retries
    and never evicts (§2, §4.5).
19. **The unit never grants, never remembers, never logs the secret**, and
    every failure is a denial; a normal reply to `BeginAuthentication` exists
    only after the helper's `SUCCESS` (§6.3, §6.4).
20. **Keyboard ownership is proven and kept**: no helper connection before
    the card has focus, a lost focus wipes the buffer (§5.3).
21. **The secret is a fixed-capacity zeroizing byte buffer** in a toolkit
    `SecretField`, moved to the helper's thread, over a toolkit whose key
    events no longer print their text (§6.5).
22. **The agent's bus thread reads one ordered stream and does not use
    `#[interface]`**; only authentic broadcasts update the owner cache
    (§4.2).
23. **Two attempts per prompt**, below this host's `pam_faillock` limit (§2).
24. **Launch-environment control is out of scope** for the polkit agent
    (§6.2, A).

## 14. Delivery of this document

A documents unit: `suite-maintenance:`, committed in `main` only on the
author's explicit authorization in the chat of the session that wrote it. It
carries no code. After the author approves it, the implementation plan follows
in `docs/superpowers/plans/`; the author edits `AGENTS.md` for the reference
files (§8); the ledger row of CEL-2-I moves to `active` when the
implementation session opens it; and the model and reasoning table of the
author's global rule is presented and approved before any implementation.
