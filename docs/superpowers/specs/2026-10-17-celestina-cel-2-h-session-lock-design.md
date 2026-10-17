# Celestina session lock (CEL-2-H) — design

- **Date:** 2026-10-17
- **Status:** drafted from the author's decisions of 2026-10-17 (restated in
  §2, not reopened) and revised after two reviews; OD-1 and OD-2 decided by
  the author on 2026-10-17 (§0). **Approval of the complete specification is
  pending.** Nothing here is implemented, and no implementation starts before
  the author approves this document, the prerequisite commit of §13 and the
  model table of §11.
- **Product:** the shell's own session lock: an `ext-session-lock-v1` client
  that covers every output, asks for the password, and is the only thing that
  can end the lock, after PAM says yes; plus the lock-and-suspend chain that
  CEL-2-B left inert.
- **Identifiers:** project `celestina`, plan `cel-2-parity`, units
  `CEL-2-H-a` and `CEL-2-H-b` (two ledger rows replacing `CEL-2-H`, §0),
  commit prefix `celestina:` for both, duty key `duties.lock`, validation rows
  `VAL-CEL2-LOCK-*`. Decisions are numbered **H-1 … H-18** in this text:
  the program's list ends at 15 and CEL-2-I is designed in another session,
  so global ADR numbers are assigned when each unit lands (§14).
- **Builds on:** [the parity design](2026-10-16-celestina-parity-program-design.md)
  §4.4 (privileged units), §5 row H, §7; `celestina/docs/decisions/0014-lock-and-polkit-carry-over.md`;
  the halted shell's `celestina-qt/docs/decisions/0004-first-party-session-lock.md`
  (carried over and re-reviewed in §3); CEL-2-B (the idle chain, the logind
  worker, the session verbs) and CEL-2-E (per-output wallpaper selection and
  bounded decoding).
- **Verified against:** the worktree at `a0cdaf18`; `smithay-client-toolkit`
  0.21.1 and `wayland-protocols` 0.32.12/13 in the local registry; niri's
  source in `/home/toni/CODIGO/NIRI-MELIBEA` (HEAD `e4062688`, whose lock code
  is unchanged since `8ed0da4`, the commit the installed `/usr/bin/niri`
  26.04 reports); the machine's `/etc/pam.d`, Linux-PAM 1.7.3 headers, logind
  and swap configuration (read only). Every claim that could not be checked
  this way is marked **[verify]** and listed in §12.
- **Revised:** 2026-10-17, after review A (Sonnet 5.5, findings `S-*`) and an
  adversarial pass (Opus 5.5, findings `X-*`). Each finding was checked
  against the code before it was applied; the disposition of every finding
  is in §16.

## 0. Decisions taken by the author

On 2026-10-17 the author answered the two questions the reviews raised:

**OD-1 — two units** (review A, `S-I7`). CEL-2-H is delivered as two units of
the `cel-2-parity` ledger, both under the prefix `celestina:`, in this order
(decision H-18):

| Unit | Delivers | Security review |
|---|---|---|
| **CEL-2-H-a**, the security core | black covers, the verifier, the shell integration (supervisor, sleep worker, energy rewiring, verbs, notices), the suspend chain, the recovery path, the CPU painter with the field, status, Caps Lock and the fade; the backdrop bundle **format v1 is frozen and reserved here**: `celestina-lock-proto` implements and tests the whole codec, the shell sends a header-only bundle (`outputs = 0`, `offset_known = 0`), and the lock validates it and keeps `prior_failures` | review A **and** Review B |
| **CEL-2-H-b**, the presentation | the dimmed wallpaper (thumbnails on the wallpaper worker, the bundle's pixels, the lock's backdrop reader and upscaled base, the scrim over it), the clock and the date (realtime timerfd, the offset read per spawn) | review A only; it changes no security decision and no protocol |

H-b may not change the bundle format, the status lines, the verifier
contract or any row of §4.4, §4.5 except the clock's redraws; if it needs to,
it reopens H-a's review. Until H-b lands, the lock is black with the field,
and that is a complete, validatable lock.

**OD-2 — option (b)** (review A, `S-I11`). Sonnet 5.5 at high reasoning
implements T1–T4 and T6; review A is Opus 5.5 at high reasoning over both
units; Review B is Opus 5.5 at its highest reasoning, in a clean context,
over CEL-2-H-a only. Both reviews thus come from a model other than the
implementer of the security code (§10, §11).

The complete specification still awaits the author's approval (Status).

## 1. Goal and scope

The session must be lockable by Celestina before Noctalia stops (parity
design §5: H is required before M). Today `Mod+Shift+Escape` runs Noctalia's
`session lock-and-suspend`, `Mod+Shift+A` runs `swaylock -f -c 000000 -u`, and
the idle chain's second step is refused as `NoLocker` (CEL-2-B decision D1).

In scope (§0 says which unit delivers what):

- `celestina-lock`, a separate binary that owns the `ext-session-lock-v1`
  object, a black cover on every output, a dimmed wallpaper, a clock and a
  password field;
- `celestina-lock-verify`, a short-lived PAM child per attempt;
- the shell's side: launching the lock, its confirmation, the `lock` verb,
  wiring the idle chain's second step, a logind delay inhibitor for every
  suspend, and the `duties.lock` switch;
- the PAM service text and its installation instructions (installed by the
  author, never by an agent);
- the recovery path and its lifetime.

The scope is delivered in two units, CEL-2-H-a and CEL-2-H-b (§0). Out of
scope for both: any factor but the password, logind's `Lock`
signal, keyboard-layout display, assistive technology on the lock screen
(§4.8), and every visual refinement (group 2).

## 2. The author's decisions (restated)

| # | Decision |
|---|---|
| 1 | Password only. The verifier contract leaves room for other factors without breaking its protocol; fingerprint is out of scope and on the never list |
| 2 | Each output's wallpaper (CEL-2-E selection) dimmed and blurred, plus a clock and the field. A failed image degrades to solid black and never prevents covering. Clock and field on the output with the focus; the others show only the dimmed backdrop. Zero user content |
| 3 | A separate binary `celestina-lock`, its own `ext-session-lock-v1` client, surviving a shell crash or restart, launched by the shell, confirmed by a `locked` line (or a better channel, justified); a third process, a short PAM child with an argv/stdin/exit-code contract |
| 4 | PAM hanging or dying keeps the screen locked: a deadline per attempt (about 15 s), the child killed, the field cleared, a message distinct from "wrong password", retry allowed. Never unlocks |
| 5 | No grace period. A progressive delay after consecutive failures (2/4/8… up to 30 s) that lives in the lock and resets on unlock |
| 6 | PAM service `celestina-lock`, installed by hand by the author; no fallback to `login`; a missing file falls to `other` (deny) and the lock says how to install it |
| 7 | Triggers: (a) the `celestina lock` verb over the CLI and `org.celestina.Shell1`; (b) the idle chain's second step, lock then suspend only after confirmation; (c) any suspend through a logind `sleep`/`delay` inhibitor released once locked. logind's `Lock` signal is not served |
| 8 | `duties.lock = off|on` in `shell.json`, default off while Noctalia locks; a rule for coexistence |
| 9 | `swaylock` stays installed and on `Mod+Shift+A` until the author's live validation; a documented exit from a stuck lock (another VT, kill by pid only, a second client taking over) |

## 3. What exists, measured, and what the reference teaches

### 3.1 Facts this design stands on

| Fact | Source |
|---|---|
| `IdleChain::set_locker_available` exists; `LockAndSuspend` is refused as `Refusal::NoLocker` while it is false; `IdleSlot::sync` sends `false` once | `celestina-rs/crates/celestina-shell/src/idle.rs`, `celestina/src/energy/mod.rs` |
| `perform_idle(LockAndSuspend)` only logs; it runs on the idle worker's thread | `celestina/src/energy/mod.rs` |
| logind's `Suspend` is refused with `NO_LOCKER` while `LogindConfig::locker_available` is false (fixed at spawn) | `celestina/src/energy/logind.rs` |
| Verbs answer `(true, "")` once queued and cannot return text | `celestina/src/energy/verbs.rs`, `celestina/src/bus.rs` |
| `unsafe` is denied workspace-wide; the only exception is `crates/celestina-wayland/src/egl.rs`, approved in the shell design §5.2 | `celestina/Cargo.toml`, `celestina/AGENTS.md` |
| The toolkit runtime always binds layer shell, the data device (paste), text input (IME) and background effect, and its SIGINT/SIGTERM source stops the loop | `celestina/crates/celestina-wayland/src/runtime.rs` |
| The runtime logs `?ev` (the whole `InputEvent`, a key's `utf8` included) at debug when input reaches a surface that is gone | `runtime.rs`, `Runtime::deliver` |
| `Modifiers` carries ctrl, alt, shift and logo; no Caps Lock | `celestina-wayland/src/input.rs` |
| sctk 0.21.1 has `session_lock` (`SessionLockState::lock`, `create_lock_surface`, `SessionLock::unlock`); dropping a locked `SessionLock` sends `destroy`, a protocol error that fails secure | registry `smithay-client-toolkit-0.21.1/src/session_lock/mod.rs` |
| `wp_single_pixel_buffer_v1` and `wp_viewporter` are in `wayland-protocols` (staging) and served by niri | registry; niri `src/niri.rs` (`SinglePixelBufferState`, `ViewporterState`) |
| niri refuses a second lock while one is locking, or locked with a live client; a refused locker is answered `finished` (smithay's `SessionLocker` drop) | niri `Niri::lock`; smithay `ff5fa7d` `session_lock/mod.rs` |
| niri **replaces a dead lock**: a new client's lock is accepted at once when the locked client is gone ("replacing existing dead lock") | niri `Niri::lock` |
| niri waits at most 1 s for every output's lock surface to be mapped, then redraws and confirms; an output without a lock surface is drawn in `CLEAR_COLOR_LOCKED` (dark red) | niri `Niri::lock`, `continue_to_locking`, `CLEAR_COLOR_LOCKED` |
| niri **unlocks** when it fails to render a locked frame on an output while `Locking`; the dropped confirmation answers `finished`. So `finished` after `lock()` does not always mean "another client holds the lock": it can mean "the compositor gave up and the session is unlocked" | niri, the `Locking` branch after an output's render (`self.unlock()` when `lock_render_state == Unlocked`) |
| niri gives keyboard focus to the lock surface of the output under the cursor, else the active output, else the first | niri `lock_surface_focus` |
| While locked niri still runs `Quit`, `ChangeVt`, `Suspend`, `PowerOff/OnMonitors`, `SwitchLayout`, `ToggleKeyboardShortcutsInhibit` and binds marked `allow-when-locked` (the author's media, volume and brightness keys). `Quit` ends the session (no exposure); a bind to it works while locked | niri `allowed_when_locked`; `~/.config/niri/config.kdl` |
| The nest (`dev-session.sh`) shares the desktop's `XDG_RUNTIME_DIR`: a file a nested lock writes there is visible to the live shell | `celestina/scripts/dev-session.sh` (`runtime=${XDG_RUNTIME_DIR:-/tmp}`) |
| The `faillock` tally `/run/faillock/<user>` is owned by the user (mode 0660, group root): the user can read it and reset it **without privilege** (`faillock --user <name>` ran unprivileged) | `ls -l /run/faillock`, `faillock --user` |
| `celestina-paint` builds its fonts with `FontSystem::new()` (system fonts through fontconfig, which reads the user's `fonts.conf`) and resolves themed icons through `freedesktop-icons`; `tiny-skia` 0.12, `cosmic-text` 0.19, `fontdb` 0.23 and `swash` 0.2 are in the lockfile | `celestina/crates/celestina-paint/src/text.rs`, `Cargo.toml`, `Cargo.lock` |
| No input method (fcitx, ibus) runs in the author's session today | `pgrep -a 'fcitx|ibus'` |
| logind `InhibitDelayMaxUSec` is 5 000 000 (5 s) | `busctl get-property … InhibitDelayMaxUSec` |
| `/etc/pam.d/system-auth` has `pam_faillock` (preauth, authfail, authsucc), `pam_systemd_home`, `pam_unix … nullok`; `/etc/security/faillock.conf` has no active line, so the defaults apply (deny 3, unlock time 600 s) | `/etc/pam.d/system-auth`, `/etc/security/faillock.conf` |
| `login` adds `auth requisite pam_nologin.so`; `swaylock` is `auth include login`; `waylock` is `auth include system-auth`; `other` is `pam_deny` + `pam_warn` for every type | `/etc/pam.d/*` |
| `unix_chkpwd` is setuid root: `pam_unix` in an unprivileged process needs it | `ls -l /usr/bin/unix_chkpwd` |
| `PAM_MAX_RESP_SIZE` is 512, `PAM_MAX_NUM_MSG` 32; the constants this design uses have the values in §4.11 | `/usr/include/security/_pam_types.h` |
| Swap is `/dev/zram0` only (no disk swap, no hibernation image); `kernel.yama.ptrace_scope` = 1; core dumps go to `systemd-coredump`; `RLIMIT_MEMLOCK` 8 MiB | `swapon`, `/proc/sys` |
| `rustix` 1.1.5 offers safe `set_dumpable_behavior`, `pidfd_open`, `pidfd_send_signal`, `kill_process_group`, `timerfd_create/settime`, `memfd_create`, `fcntl_add_seals`, `flock` | registry `rustix-1.1.5/src` |
| The shell is started with `setsid -f` (activation) and later from niri's autostart (CEL-2-M); it is not a systemd unit | `celestina/scripts/activate-production.sh` |
| The Island clock reads the UTC offset once per process with `date +%z` and ticks on a monotonic deadline | `celestina/src/island/clock.rs` |

### 3.2 The halted reference, critiqued

The halted shell's lock (`celestina-qt/src/lockcontroller.cpp`,
`lockauthenticator.cpp`, `lockverify/main.cpp`, at `c7fb0b5d`) was read, not
ported. What this design keeps and changes:

| Reference decision | Here | Why |
|---|---|---|
| Lock surface in its own process, outliving the shell | **kept** (H-1) | a shell crash must not end or weaken the lock |
| PAM in a third, short-lived process, exit code is the verdict | **kept**, contract versioned (H-4) | a PAM or module crash dies in the child |
| `locked` line on stdout as the only "covered" signal | **kept**, extended to a small versioned line set (§4.3) | an anonymous pipe has one reader and EOF is liveness |
| Default service `login`, overridable by `--service` | **changed**: compiled `celestina-lock`, no fallback (H-7) | `login` brings `pam_nologin` (a shutdown notice locks the person out) and the author chose no fallback |
| User from `$USER` | **changed**: from the real uid (H-13) | the environment is not an identity |
| `CELESTINA_LOCK` and `CELESTINA_LOCK_VERIFY` environment overrides | **removed**; binaries beside the shell, pinned by descriptor (H-13) | "which program may say authenticated" must not be chosen by an environment |
| Every echo-on and echo-off prompt answered with the password | **changed**: the first echo-off prompt only (H-4) | answering a username or OTP prompt with the password sends it where it does not belong |
| Trailing CR/LF stripped from stdin | **changed**: the secret is the exact bytes up to EOF | no silent transformation of a secret |
| 4096-byte bound | **changed**: 512 bytes, this design's own cap | the header calls `PAM_MAX_RESP_SIZE` 512 "mostly arbitrary" and Linux-PAM handles longer responses, so 512 is not PAM's limit; it is a bound we choose, far above any real password, equal to that constant for clarity |
| `pam_authenticate(handle, 0)` | **changed**: `PAM_DISALLOW_NULL_AUTHTOK`; empty field never submitted | `system-auth` has `nullok` |
| Wallpaper paths on the lock's stdin, decoded by the lock (QML `Image`) | **changed**: no decoder and no user file in the lock; pre-scaled pixels in a sealed memfd (H-2) | a decoder bug inside the process that can unlock is the worst place for it |
| Blocking `bus.call(…, 2000)` and `waitForStarted(2000)` on the GUI thread | **changed**: every bus call and spawn on a worker (§4.10) | blocking IO never runs on the loop |
| On sleep with a lock that will not start: release and let logind sleep | **kept**, logged as an error (H-8) | a `block` inhibitor could wedge the machine awake |
| No timer that suspends anyway | **kept** (H-8) | an unconfirmed lock may be uncovered |
| `finished()` cannot tell unlock from death | **changed**: `unlocked` line plus exit 0 distinguishes them, and a dead lock is relaunched (H-12) | niri accepts a replacement for a dead lock |
| Volatile-pointer wipes | **changed**: `zeroize` buffers, no own `unsafe` (§4.13) | the same effect without a second `unsafe` exception |

## 4. Design

### 4.1 Processes and ownership (H-1)

```text
niri ── (Wayland) ── celestina (shell)                     owns: policy, triggers, logind
                       │  spawn, process group of its own
                       │  stdin  = sealed memfd (backdrop bundle)
                       │  stdout = pipe  (status lines, lock → shell)
                       ▼
niri ── (Wayland) ── celestina-lock                         owns: the lock object, covers, prompt,
                       │  spawn per attempt, process group     the only unlock call
                       │  argv  = --protocol 1 --service celestina-lock
                       │  stdin = pipe (the secret, then EOF)
                       ▼
                     celestina-lock-verify                  owns: one PAM conversation; no Wayland,
                       └── pam_unix → unix_chkpwd (setuid)   no bus, no files of its own
```

| Process | Lifetime | Holds | Never holds |
|---|---|---|---|
| shell | the session | the duty switch, the lock policy, the delay inhibitor, the suspend call | the lock object, the password |
| `celestina-lock` | from the request to an authenticated unlock | the `ext_session_lock_v1` object, lock surfaces, the typed secret, the backoff | a D-Bus connection, a decoder, any user file, the clipboard |
| `celestina-lock-verify` | one attempt (≤ 15 s) | the secret for one `pam_authenticate` | a Wayland connection, a log line with PAM text |

The shell spawns the lock with `CommandExt::process_group(0)` (a group of its
own, so a signal to the shell's group does not reach it) and never kills it.
The lock's stderr is inherited from the shell (M14): under `systemd-cat` it
reaches the journal tagged `celestina`, and it keeps writing there after the
shell dies (the descriptor is the journal's stream, not the shell's). Its
tracing filter is fixed at `info` and no line carries a secret (TM-11). The
verifier's stdout and stderr are `/dev/null`.
When the shell exits or crashes the lock keeps running; its stdout write
fails with `EPIPE`, which it ignores (Rust ignores `SIGPIPE` by default).

### 4.2 Launch, discovery and environment

**Finding and pinning the binaries.** When `duties.lock` turns on, the
shell's lock worker opens `celestina-lock` and `celestina-lock-verify` beside
its own executable (`std::env::current_exe()`), never through `PATH` or an
environment variable, and keeps the lock's descriptor. Each pinned file must
pass `fstat`: a regular file, owned by the user or root, not writable by
group or others, executable. The lock is executed from its descriptor
(`/proc/self/fd/N`, the path glibc's `fexecve` uses; **[verify]** Q9), so the
bytes that run are the bytes that were checked, and a deploy that replaces
the file on disk changes nothing until the duty restarts: the shell runs the
lock that was deployed with it. The lock pins the verifier the same way at its
own start, and runs every attempt from that descriptor.

**Availability.** The lock is *available* when the duty is on and both
binaries are pinned. Availability is re-checked on the lock worker before
every spawn and published to the idle chain (`set_locker_available`) on every
change. **A missing PAM service file does not make the lock unavailable**
(the author's decision 6): the lock still covers, shows
`LOCK_SERVICE_MISSING` from the start, and every attempt is denied, so the
session is never left asleep or uncovered because of it. The shell checks
the file (`/etc/pam.d/celestina-lock` or `/usr/lib/pam.d/celestina-lock`; the
path list is one constant of `celestina-lock-proto`, also used by the
verifier) when the duty turns on and logs, and posts a notice (§4.9), naming
the installation section of `celestina/README.md`, so the author learns it
before the first lock rather than at it.

**Environment.** The lock is spawned with `env_clear()` and an allowlist of
exactly `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` and `MESA_SHADER_CACHE_DISABLE=true`
(the last only matters if the EGL alternative of H-15 is ever chosen). No
`HOME`, no `XDG_*` beyond the runtime directory, no `LANG`/`LC_*` (the copy is
compiled Spanish and the clock is formatted in process), no `LD_*`, no
`RUST_LOG`, no `__EGL_*`/`MESA_*` loader variables. Without `HOME` and
`XDG_CONFIG_HOME` nothing in the process can find the user's `fonts.conf`,
icon themes or caches. The verifier is spawned with `env_clear()` and
`PATH=/usr/local/sbin:/usr/local/bin:/usr/bin` only.

**Not traced at start.** Right after `PR_SET_DUMPABLE 0` (§4.11, H-13), the
lock and the verifier read `TracerPid` from `/proc/self/status`; a non-zero
tracer (one attached in the moment before the call, which non-dumpable does
not detach) makes the lock exit 15 **before** `requested`, and the verifier
exit 6. No secret exists yet at that point.

**State file: the pid for the author, nothing more.** The lock writes
`$XDG_RUNTIME_DIR/celestina/lock-<WAYLAND_DISPLAY>.state` (mode 0600, the
display name sanitised to `[A-Za-z0-9._-]`) with `pid <n>`, under a
best-effort `flock`. It is **never** a source of truth: nothing reads it to
decide whether the session is covered, whether to lock, whether to release
the inhibitor or whether to suspend (review X-H1). The compositor is the only
authority: every decision to lock asks it (`ext_session_lock_manager_v1.lock`)
and every "covered" comes from its `locked` event. Failing to create or lock
the file is logged and changes nothing else; a second `celestina-lock` is
not refused by the file, the compositor refuses it. The per-display name
keeps a nested lock's file (the nest shares `XDG_RUNTIME_DIR`) apart from the
live session's; the rehearsal refuses to run if a file for a display other
than the nest's names a live pid (§9.2).

**Why a pipe, not a socket or a bus name.** The anonymous pipe has exactly
one reader, needs no authentication, and its EOF is the lock's death. A
socket or a bus name would let any process of the session talk to the
process that can unlock.

### 4.3 Protocols, versioned and bounded

**(a) Shell → lock: the backdrop bundle, on stdin as a sealed memfd (H-2).**
The shell writes the bundle into a `memfd_create` file, adds
`F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE | F_SEAL_SEAL` (the file is
created with `MFD_ALLOW_SEALING | MFD_CLOEXEC`; `std` sets the child's stdin
with `dup2`, which clears close-on-exec for it only), and passes it as
the child's stdin (`Stdio::from(OwnedFd)`, safe). The bundle is written with
positioned writes (`pwrite` at explicit offsets, `rustix::io::pwrite`), so
the descriptor's file offset stays 0 and the child's first `read` starts at
the magic; the writer asserts the offset is 0 before the spawn (M4: a plain
`write` would leave the offset at the end and the child would read EOF).
Nothing is written to a pipe the child may not read, nothing travels in argv,
and no path is sent. Little-endian:

```text
magic           4 bytes  "CLKB"
version         u16      1
prior_failures  u8       counted failures since the last unlock (H-6)
offset_known    u8       1 when utc_offset_s was read for this spawn, 0 when it could not be (§4.8)
utc_offset_s    i32      the local UTC offset, read afresh for this spawn
outputs         u8       0..=8
per output:
  name_len      u8       1..=64, then the connector name, UTF-8
  width, height u16 each 1..=640
  pixels        width × height × 4, RGBA8, opaque
```

| Bound | Value | On violation |
|---|---|---|
| file size (`fstat`) | ≤ 16 MiB and exactly the size the header implies. The bounds above cap it below that: 8 outputs × (640 × 640 × 4 + 69) + 13 bytes ≈ 12.5 MiB (M3) | whole bundle ignored: black, `prior_failures` 0 |
| outputs beyond 8 | — | not sent by the shell (the first 8 by connector name); the lock shows those outputs black |
| seals | all four present (`fcntl_get_seals`) | ignored |
| header | magic, version 1 | ignored (a newer shell's bundle is not guessed at) |
| an output block | name UTF-8, sides in range | that block and the rest ignored; the header's fields kept |
| read time | 2 s on the backdrop worker | ignored |
| stdin not a memfd (`/dev/null`, a tty) | — | no backdrop, nothing waited for |

**(b) Lock → shell: status lines on stdout.** ASCII, one line per event,
`\n`-terminated, at most 64 bytes; the shell reads with a 4 KiB bounded
buffer and drops an overlong line with a log. The first line names the
version; an unknown line is logged and ignored (forward compatible).

```text
hello 1               connected, ext_session_lock_manager_v1 bound
requested             lock() sent, a surface created for every output
locked                ext_session_lock_v1.locked received (compositor presented a locked frame everywhere)
attempt <kind>        refused | account | unavailable | timeout | unsupported | service-missing
unlocked              unlock_and_destroy sent and a wl_display.sync round trip returned
```

The final exit status completes the line set:

| Exit | Meaning | Shell's reading |
|---|---|---|
| 0 | `unlocked` was printed | unlocked by PAM |
| 1 | a recoverable Rust error reached `main` before `requested` (logged) | unavailable |
| 2 | usage (bad argv) | not locked; bug |
| 10 | `finished` before `locked`: **either** another live client holds or is taking the lock, **or** the compositor gave up locking (niri unlocks when it cannot render a locked frame) | ambiguous: never suspend; retry within the bound of §4.4, then report |
| 11 | no `ext_session_lock_manager_v1` | unavailable |
| 12 | no Wayland connection | unavailable |
| 13 | `finished` after `locked` (compositor policy ended the lock) | logged as an error; not relaunched |
| 15 | the lock's own setup failed before `requested` (hardening, a tracer attached, the verifier pin) | unavailable |
| 16 | the Wayland connection was lost after `requested` (the compositor quit or crashed: logout, `Quit`, a crash) | the session it protected is gone; not relaunched; logged |
| a signal, or any other status after `requested` | the lock died | relaunch (H-12) |

There is no exit for "another `celestina-lock` runs": the compositor
answers that (exit 10), not a file (§4.2).

**(c) Lock → verifier: argv, stdin, exit code (H-4).**

```text
argv    --protocol 1 --service celestina-lock     (rehearsal: --service celestina-lock-rehearsal)
stdin   the secret: 1..=512 bytes, no NUL, exactly as typed, then EOF
stdout  /dev/null      stderr  /dev/null
env     PATH only
```

| Exit | Verdict | Counts for the delay (H-6) |
|---|---|---|
| 0 | `Authenticated`: `pam_authenticate` and `pam_acct_mgmt` both `PAM_SUCCESS`, `pam_end` `PAM_SUCCESS` | — (unlocks) |
| 1 | `Refused`: `PAM_AUTH_ERR`, `PAM_USER_UNKNOWN`, `PAM_MAXTRIES`, `PAM_CRED_INSUFFICIENT`, `PAM_PERM_DENIED` in auth | yes |
| 2 | `Unavailable`: `pam_start` failed, `PAM_SYSTEM_ERR`, `PAM_BUF_ERR`, `PAM_ABORT`, `PAM_AUTHINFO_UNAVAIL`, `PAM_CONV_ERR` not caused by an unsupported prompt, any other code | no |
| 3 | `AccountRefused`: auth succeeded, `pam_acct_mgmt` returned `PAM_NEW_AUTHTOK_REQD`, `PAM_ACCT_EXPIRED`, `PAM_PERM_DENIED` or `PAM_AUTH_ERR` | yes |
| 4 | `Unsupported`: the stack asked a second secret prompt or an echo-on prompt | no |
| 5 | `ServiceMissing`: neither service path exists (production service only) | no |
| 6 | `BadRequest`: unknown protocol, bad argv, empty, oversized or NUL-bearing secret, uid 0, a rehearsal service file that exists | no |
| a signal, any other code, a timeout | `Unavailable` | no |

`--service` is a closed enum of `celestina-lock-proto` with exactly two
values, `Production` (`celestina-lock`) and `Rehearsal`
(`celestina-lock-rehearsal`); any other word is 6, and no string from argv
ever reaches `pam_start`. The presence rules (production file must exist,
rehearsal file must not) are a pure function of the service and a list of
existing paths, `service_check(service, &[existing]) -> Result<(), Verdict>`,
whose caller passes what `stat` found in the compiled directory list; the
tests feed it lists directly, so "the rehearsal file exists → 6" is tested
without root (M5).

The door to other factors is the version: a protocol-2 verifier could
forward prompts as framed messages on stdout and read answers on stdin. A
protocol-1 lock never asks for it; a verifier must keep answering every
version a deployed lock may speak, and refuses an unknown one with 6.

### 4.4 The shell's lock policy (pure, `celestina-shell::lock`)

The policy is a state machine without IO, run on the loop's thread; the
workers of §4.10 feed it events and carry out its actions.

| State | Meaning |
|---|---|
| `Unavailable(why)` | duty off, or a binary not pinned (a missing PAM file is **not** a reason, §4.2) |
| `Idle` | available, no lock child of this shell runs |
| `Spawning { since }` | a spawn was handed to the lock worker |
| `Requested { since }` | the child printed `requested` |
| `Locked` | **this shell's** child printed `locked` and has not exited |
| `Ending { since }` | the child printed `unlocked`; waiting for its exit |

There is no "foreign lock" state (review X-H1, which replaces S-C1): the
policy never believes a file or another process about coverage. A shell
restarted under a running lock is `Idle`; its next lock request asks the
compositor, which refuses (exit 10) while that lock lives, and that refusal
never suspends.

Side state: `intent: Option<Suspend { origin: Idle | Verb, deadline }>`,
`sleep: Awake | Preparing`, `inhibitor: Held | Released | Unavailable`,
`relaunches` and `refusal_retries` (timestamps in the last 60 s), `failures`
(counted `attempt` lines since the last `unlocked`). `Tick` is a one-second
timer that exists only while the state is not `Idle` or `Unavailable`, or an
intent is pending.

| Event | From | To | Actions |
|---|---|---|---|
| `Lock` (verb) | `Idle` | `Spawning` | `Spawn` |
| `LockAndSuspend(origin)` | `Idle` | `Spawning` | `Spawn`; `intent = Suspend(origin, now + 10 s)` |
| `LockAndSuspend(origin)` | `Spawning`, `Requested` | same | set the intent |
| `LockAndSuspend(origin)` | `Locked` | same | `Suspend(LockedProof)` |
| `LockAndSuspend(origin)` | `Ending` | same | set the intent; it is served by the next lock (below) |
| `Lock` | `Spawning`, `Requested`, `Locked` | same | nothing (already locking) |
| `Lock` | `Ending` | same | remember one pending request; `Spawn` after the exit |
| `Lock`, `LockAndSuspend(Verb)` | `Unavailable(why)` | same | log; `Notice(why)` (§4.9) |
| `LockAndSuspend(Idle)` | `Unavailable(why)` | same | idle already refused it (`NoLocker`); log |
| a spawn failed | `Spawning` | `Idle` | log; drop the intent with `Notice` if its origin is `Verb`; `ReleaseInhibitor` if `Preparing` |
| line `hello 1` | `Spawning` | same | — |
| line `requested` | `Spawning` | `Requested` | — |
| line `locked` | `Spawning`, `Requested` | `Locked` | `ReleaseInhibitor`; if an intent is pending and not expired: `Suspend(LockedProof)` |
| line `attempt refused|account` | `Locked` | same | `failures += 1` |
| line `unlocked` | `Locked` | `Ending` | `failures = 0`; if `Preparing`, remember one pending request (a sleep is under way and the session was just uncovered): the exit spawns again at once |
| exit 0 after `unlocked` | `Ending` | `Idle` | `HoldInhibitor` only if `Awake`; serve a pending request or intent with `Spawn` |
| exit 10 | `Spawning`, `Requested` | `Spawning` or `Idle` | retry after 500 ms while fewer than 2 retries in 60 s (it may have been the compositor giving up, not another client); then `Idle`, log "the compositor refused the lock", drop the intent with `Notice` if `Verb`, `ReleaseInhibitor` if `Preparing` |
| exit 1, 11, 12, 15 | `Spawning` | `Idle` | log; drop the intent (`Notice` if `Verb`); `ReleaseInhibitor` if `Preparing` |
| exit 16 | any | `Idle` | log; no relaunch (the compositor is gone; the shell's own connection ends with it) |
| exit 13 | `Locked` | `Idle` | log an error; `HoldInhibitor` if `Awake`; no relaunch |
| death after `requested` without `unlocked` | `Requested`, `Locked` | `Spawning` | `Spawn(prior_failures)` while fewer than 3 relaunches in 60 s, else `Idle` with an error log and a `Notice` |
| `Tick`: `Spawning` older than 5 s without `requested` | `Spawning` | `Idle` | kill the child through its pidfd (it never asked to lock, so killing it uncovers nothing); log; as a spawn failure |
| `Tick`: `Requested` older than 10 s without `locked` | `Requested` | same | do **not** kill (it may be about to lock); drop the intent (`Notice` if `Verb`); `ReleaseInhibitor` if `Preparing`; log an error once |
| `Tick`: `Ending` older than 5 s without an exit | `Ending` | `Idle` | log (the unlock already happened by protocol); `HoldInhibitor` if `Awake` |
| `Tick`: intent deadline passed | any | same | drop the intent, log "not suspended: the lock was not confirmed", `Notice` if `Verb` |
| `IdleResumed` | any | same | drop an intent whose origin is `Idle` (someone is back) |
| `PrepareForSleep(true)` | `Locked` | same, `Preparing` | nothing (released by the invariant) |
| `PrepareForSleep(true)` | `Idle` | `Spawning`, `Preparing` | `Spawn` (the compositor decides; a running foreign lock answers with exit 10, then `ReleaseInhibitor`) |
| `PrepareForSleep(true)` | `Spawning`, `Requested`, `Ending` | same, `Preparing` | keep the inhibitor until `locked` or a failure; in `Ending`, the exit spawns again |
| `PrepareForSleep(true)` | `Unavailable` | same, `Preparing` | log an error: the session sleeps uncovered (duty off or no binary) |
| `PrepareForSleep(false)` | any | same, `Awake` | `HoldInhibitor` unless `Locked`; log a warning if the sleep went ahead before `locked` |
| duty off | any | `Unavailable` | `ReleaseInhibitor`; the running lock is **not** touched (only PAM ends it) |

Invariant: the delay inhibitor is held **iff** the duty is on, the lock is
available, the state is not `Locked`, and `sleep` is `Awake`. During
`Preparing` the policy never takes a new inhibitor (one taken after logind
announced the sleep would not delay it, **[verify]** Q17), it only releases
the one it holds; the re-take happens on `PrepareForSleep(false)`.

`LockedProof` is a type with a private constructor in `celestina-shell::lock`,
minted only in `Locked`. It is carried by the lock worker, which checks the
child's pidfd is still not readable (the child is alive) immediately before
handing the proof to the logind worker's `suspend` (§4.9); a dead child
drops the proof. A suspend that still races an unlock is caught by the delay
inhibitor, which unlocking re-takes.

### 4.5 The lock process's state machine (pure, `celestina-lock/src/machine.rs`)

| State | Entered by | Leaves on |
|---|---|---|
| `Connecting` | start | globals bound → `hello 1`, `lock()` and one surface per present output in the same dispatch → `Requesting`; no manager → exit 11 |
| `Requesting` | `lock()` sent → `requested` | `locked` → `Prompt`; `finished` → exit 10 |
| `Prompt { field, status, wait_until, faded }` | `locked` (prints `locked`), or a verdict | Enter with a non-empty field and no wait → `Verifying` |
| `Verifying { attempt, deadline }` | Enter | the attempt's verdict → `Prompt` or `Unlocking` |
| `Unlocking` | `Authenticated` of the current attempt | `unlock_and_destroy`, `wl_display.sync` returns → `unlocked`, exit 0 |

| In `Prompt` | Effect |
|---|---|
| a printable key (`utf8` present, no control character) | appended only if the field's length plus the key's UTF-8 length stays ≤ 512 bytes; otherwise ignored, so the lock can never build a secret the verifier refuses as oversized (M2) |
| Backspace | removes the last `char` |
| Escape | clears the field |
| Enter, empty field | nothing (no attempt, no PAM call) |
| Enter while `wait_until` is in the future | nothing; the countdown stays |
| paste, IME, selection | do not exist (§4.7) |
| no key or pointer motion for `lock.idle_fade_after` (a theme token, proposed 30 s) | `faded = true`: every output, the prompt included, is drawn black (backdrop and text gone; OLED and onlookers); the field's content is kept |
| any key or pointer motion while `faded` | `faded = false`, redrawn; a key is also handled as above |

| Verdict | Next |
|---|---|
| `Authenticated` | `Unlocking` |
| `Refused`, `AccountRefused` | `Prompt`, field cleared, `failures += 1`, wait `min(2^failures, 30)` s |
| `Unavailable` (also timeout, crash), `Unsupported`, `ServiceMissing` | `Prompt`, field cleared, wait 2 s, `failures` unchanged |

Keys during `Verifying` are dropped (the secret already left; the next one
starts empty). A verdict carries its attempt number and is discarded unless
it is the current attempt's. `Authenticated` is a zero-sized type
constructible only in `verify.rs` from exit status 0 of the child it spawned
for that attempt; `unlock.rs` holds the single call to the toolkit's unlock,
and takes that token. A test asserts the call site is unique (§9.1).

### 4.6 Covering before anything else; outputs that come and go

- On the first `configure` of every lock surface the **toolkit** acks it and
  commits a cover: a `wp_single_pixel_buffer_v1` black pixel scaled by
  `wp_viewporter` to the configured size, or, without that global, a zeroed
  `wl_shm` XRGB8888 buffer of the configured size (a fresh memfd is zero, so
  nothing is written). No GPU, no font, no decode is on that path. Covering is
  a toolkit invariant of the lock role, not product code (H-2).
- Nothing but covers is committed before `locked`: niri continues as soon as
  every surface is mapped, so time to `locked` is the cover's time (target
  < 1.5 s, measured in the nest, Q12; logind gives 5 s).
- After `locked`: the CPU painter and the one embedded font start (§4.6.1);
  the backdrop worker reads the bundle; each output draws its backdrop (or
  stays black), the scrim token over it, and on the focused output the clock
  and the field.
- **An output that appears** gets a lock surface at once, locked or not
  (the protocol asks for it; a duplicate is a protocol error, so one per
  `wl_output` global). niri draws it in its lock colour until the cover maps;
  no unlocked content shows **[verify]** (VAL-CEL2-LOCK-HOTPLUG; the nest has
  one output).
- **An output that disappears** has its surface destroyed (the protocol's
  recommendation). If it held the prompt, the prompt moves (§4.7); the field's
  content stays.
- A surface that cannot be drawn keeps its cover; one that cannot even be
  covered is blanked by niri. Neither ever unlocks.
- If the painter cannot draw (an shm pool cannot be created or grown), the
  outputs keep their covers and the field still works blind (keys are still
  delivered); the failure is logged (F16).

#### 4.6.1 Painting: CPU into `wl_shm`, not EGL (H-15)

| | EGL with the shell's painter (`celestina-paint`, femtovg) | CPU into `wl_shm` (`tiny-skia` for shapes, `cosmic-text`/`swash` for glyphs) |
|---|---|---|
| Code inside the process that can unlock | libEGL, the Mesa driver (hundreds of thousands of lines of C), the shader compiler, its on-disk cache, femtovg, fontconfig, `freedesktop-icons`, resvg | two pure-Rust crates already in the lockfile, and the glyph rasteriser |
| Inputs it reads from the user's home | shader cache, `fonts.conf`, icon themes (unless each is switched off) | none |
| Environment that changes its behaviour | `__EGL_VENDOR_LIBRARY_FILENAMES`, `MESA_*`, `LIBGL_*` | none |
| Failure modes | GPU reset or hang (the author's nest once took the GPU down while building), context loss | out of memory for the pool |
| Cost | cheap frames | a 3840×2160 buffer is 31.6 MiB; the backdrop is upscaled once per output into a cached base, then only the damaged prompt rectangle is redrawn per frame (a key press, the minute, a countdown second); two buffers per output from one pool |
| Look | the shell's exact rendering | the same tokens, a separate rasteriser: glyph and anti-aliasing may differ slightly from the bar's |

**Chosen: CPU into `wl_shm`.** The lock draws a scrim, two rounded
capsules, a row of bullets and three lines of text, a minute apart; nothing
in that needs a GPU, and every library the GPU path would add is attack
surface and failure surface inside the one process whose compromise unlocks
the session. EGL would be justified only if a measured frame on the 4K
output exceeded one refresh (16 ms) after damage tracking; the evidence
records the measured p99 so the choice can be revisited with numbers. The
lock therefore depends on neither `celestina-paint` nor `celestina-ui`; its
view is a small module over `tiny-skia` with sizes and colours from
`celestina-tokens`. The toolkit's GL modules (`egl`, `blur`, `popup`) are
compiled behind a `gl` cargo feature (on by default, off for the lock), so
EGL code is not even linked into `celestina-lock`.

**Fonts.** One face, Inter (`celestina-style/fonts/InterVariable.ttf`),
embedded in the binary with `include_bytes!` and loaded into an **empty**
`fontdb::Database` handed to `FontSystem::new_with_locale_and_db`. The lock
never calls `load_system_fonts`, never reads fontconfig, never resolves an
icon theme, and its glyphs (bullets, digits, Spanish letters) are all in
Inter. A test asserts the database holds exactly the embedded faces
(T-F1).

**Which output.** The prompt (clock, date, field, status) is drawn on the
lock surface that holds `wl_keyboard` focus, because niri sends the keys
there (output under the cursor, else active, else first). Before any
keyboard `enter`, it follows `wl_pointer` `enter`; with neither, it is on the
first output by connector name. Moving the pointer to another output moves
niri's focus and so the prompt; nothing animates (no motion, so
`reducedMotion` is honoured trivially). The other outputs show only the
dimmed backdrop. When the seat has no keyboard capability, the status says so.

**Input.** The lock profile binds no data device, no primary selection, no
`zwp_text_input_v3`: no paste and no text-input activation. **The user's
input method** (review X-N6) is a separate client of
`zwp_input_method_v2`; niri serves that protocol (smithay's
`InputMethodHandler`), and an input method holding a keyboard grab receives
keys before the focused surface. Since the lock never activates text input,
smithay should deactivate the input method when focus moves to the lock
surface, but the exact moment the grab is released and whether a grab can
outlive the focus change is **[verify]** Q16. No input method runs in the
author's session today (measured); the risk is recorded in §5 (TM-28) and the
live row checks it if one is ever installed. Keys come from xkb with compose
(**[verify]** Q8: dead keys and accents yield the same bytes as a VT login).
Caps Lock is shown from the xkb modifier state (the toolkit gains
`Modifiers::caps_lock`). The field shows one bullet per character.

**Copy.** Every word is product copy in `celestina/crates/celestina-lock/src/copy.rs`
(`language-contract: product-copy`). Proposed literals (the author may
reword them):

```text
LOCK_FIELD              "Contraseña"
LOCK_CHECKING           "Comprobando…"
LOCK_REFUSED            "Contraseña incorrecta"
LOCK_UNAVAILABLE        "No se ha podido comprobar la contraseña. Vuelve a intentarlo."
LOCK_WAIT               "Espera {s} s para volver a intentarlo"
LOCK_ACCOUNT            "La cuenta no permite entrar ahora (por ejemplo, la contraseña ha caducado). Entra desde otra consola."
LOCK_UNSUPPORTED        "La autenticación pide algo más que la contraseña, y este bloqueo solo pide la contraseña."
LOCK_SERVICE_MISSING    "Falta el servicio PAM celestina-lock: instálalo como explica celestina/README.md y entra desde otra consola."
LOCK_FAILLOCK_HINT      "Tras varios intentos fallidos la cuenta puede quedar bloqueada unos minutos."
LOCK_CAPS               "Bloq Mayús activado"
LOCK_NO_KEYBOARD        "No hay ningún teclado conectado"
LOCK_REHEARSAL          "Ensayo: este bloqueo no se desbloquea"
weekday and month names for the date line
```

The shell's notices (§4.9) are product copy of the shell, in
`celestina/src/copy.rs`:

```text
NOTICE_LOCK_TITLE           "Bloqueo de sesión"
NOTICE_LOCK_UNAVAILABLE     "No se puede bloquear: {motivo}"
NOTICE_LOCK_REFUSED         "El compositor no ha concedido el bloqueo; no se ha suspendido."
NOTICE_LOCK_NOT_CONFIRMED   "El bloqueo no se ha confirmado a tiempo; no se ha suspendido."
NOTICE_LOCK_RELAUNCH_LIMIT  "El bloqueo se ha cerrado varias veces seguidas; usa otra consola para recuperarlo."
NOTICE_PAM_MISSING          "Falta el servicio PAM celestina-lock: el bloqueo cubrirá la pantalla pero no podrá desbloquearse. Instálalo como explica celestina/README.md."
```

`LOCK_FAILLOCK_HINT` appears from the third consecutive counted failure,
because `pam_faillock` (deny 3) refuses even the right password during its
lockout and PAM cannot tell the lock why (§4.11).

**Nothing of the person's.** No notifications, media, clipboard, window
titles, application names or user name. The lock process has no D-Bus
connection and no access to the shell's model, so this is structural; a test
pins its dependency list (§9.1).

**Accessibility.** The prompt is fully keyboard-operable. The suite's Rust
shell has no AT-SPI bridge yet, and this unit does not add one to the
privileged process; the gap is recorded as `VAL-CEL2-LOCK-LIVE` notes and as a
deferred item (§6, 16).

### 4.8 The clock, and waking from suspend (H-10)

`CLOCK_MONOTONIC` does not advance during suspend, so a monotonic minute
timer shows the old time for up to a minute after resume (the Island's clock
has this behaviour today; noted for a separate bug unit, not changed here).
The lock arms a `timerfd` on `CLOCK_REALTIME`, absolute at the next minute
boundary, with `TFD_TIMER_CANCEL_ON_SET`: it fires as soon as the machine
resumes (the realtime clock jumped past it) and on any clock step (the read
reports `ECANCELED`; re-arm). Each frame formats the time from `SystemTime`
and the bundle's `utc_offset_s`; the lock spawns no `date` and reads no
time-zone file.

**The offset** (M1). The Island's `island::clock::utc_offset()` is a
`OnceLock` filled once per shell process from `date +%z`, falling back to UTC
with one log line: after a daylight-saving change it stays wrong until the
**shell** restarts, and a failed `date` shows UTC with nothing on screen to
say so (verified in `celestina/src/island/clock.rs`). The lock does not
inherit that: the module gains an uncached `read_utc_offset()` (the same
`date +%z` parse, without the cell; the Island keeps its cached call, its own
staleness noted for a separate bug unit), and the lock worker calls it on
every spawn, off the loop. A failed read sets `offset_known = 0` in the
bundle, and the lock then shows the date line and the field but **no
clock**, rather than a time that may be an hour off. A daylight-saving change
during one lock still shows the offset read at its spawn (accepted). At rest
the lock draws one frame a minute and one per second only during a
countdown.

### 4.9 Triggers (H-8)

| Trigger | Path | Suspends? |
|---|---|---|
| `celestina lock` (CLI and `Shell1`) | `Verb::Lock` → policy `Lock` | no |
| `celestina lock suspend` | `Verb::LockAndSuspend` → policy `LockAndSuspend(Verb)`; refused with a notice while the lock duty is off | after `locked`, through `LockedProof` |
| idle step 2 (`duties.idle` on) | `perform_idle(LockAndSuspend)` sends `Message::Lock(LockAndSuspend(Idle))` to the loop; never waits | after `locked`, unless input came back first |
| lid, power button, `systemctl suspend`, anything | logind `PrepareForSleep(true)` while our `sleep`/`delay` inhibitor is held | logind proceeds when we release (locked or failed) or after its 5 s |
| `celestina session suspend` | **always** refused, reason "suspend goes through `lock suspend`" (one behaviour whatever the duty, M6; it was refused already under CEL-2-B D2) | no |
| logind `Lock` (`loginctl lock-session`) | **not served** (§6, 5) | — |

Changes to CEL-2-B, each removing the old path in the same unit:

- `LogindConfig::locker_available` and `NO_LOCKER` go; `Request::Power(Suspend)`
  without a proof is refused ("suspend goes through the lock");
  `LogindHandle::suspend(LockedProof)` is the one way to suspend.
- `IdleSlot::sync` sends the policy's availability, again on every change,
  instead of a constant `false`.
- `IdleChain::resumed` returns `IdleAction::Resumed` when the period had
  handled `LockAndSuspend`, so the wiring can drop an idle-origin intent.
  Explicit verbs are not cancelled by input (pressing the binding is input).
- The smoke's fifth run expects the new refusal text.

**A suspend that races an unlock** (M11). The proof was minted, then the
person unlocked before the logind worker called `Suspend`. Re-taking the
delay inhibitor at unlock does not cover this: a delay inhibitor taken after
logind sent `PrepareForSleep(true)` does not delay that sleep (Q17), and the
call may land after the unlock but before the new inhibitor exists. So each
`LockedProof` carries the lock **generation** (a counter the policy bumps on
every `unlocked`, on every child exit and on duty off), published through a
shared `AtomicU64` that only the policy writes. The logind worker compares the
proof's generation with the current one immediately before `Suspend` and
drops a stale proof with a log; the remaining window (between that compare
and logind acting on the call) is the same one any locker has, and the
pidfd check of §4.4 narrows the child-death case further.

**Refusals the author can see** (review S-I2). A log line is invisible at
the moment it matters. Every refusal of an **explicit** request (`lock`,
`lock suspend`), every suspend dropped because the lock
was not confirmed, the relaunch limit, and a missing PAM file at duty-on are
also posted as a desktop notification: a `Notify` call to whoever owns
`org.freedesktop.Notifications` (Noctalia during coexistence, the shell
itself afterwards), made on the lock worker with `CALL_TIMEOUT`, best effort,
at most one identical notice per 10 s. Idle-origin refusals stay in the log
(nobody is there). The notices name no file, no user and no secret.

**Who may ask** (review X-N2). `org.celestina.Shell1` is on the session bus,
so any process of the user can ask for `lock` or `lock suspend`. Locking is
harmless. Suspending is a mild denial of service, but not a new capability:
logind already lets the active session's user suspend without a password
(`CanSuspend` answers `yes` for the author's session, measured with
`busctl call … CanSuspend`); the shell adds no rate
limit beyond "one intent at a time", and logs each request's origin
(verb or idle).

### 4.10 Threads and blocking IO

| Thread | Process | Owns | Bounded by |
|---|---|---|---|
| loop | shell | the pure policy, routing | no IO |
| `lock` | shell | pinning, spawn, stdout reader, `pidfd` wait, the liveness check before a suspend proof, the memfd bundle, the offset read, the PAM-file check, the notices | one blocking `poll` on stdout, the pidfd and an `eventfd` that the command sender and shutdown signal (M12: no periodic wake-up; the bounded `sync_channel` of 4 is drained when the eventfd fires), join `JOIN_WAIT` |
| `lock-sleep` | shell | its own system-bus connection, the delay inhibitor fd, the `PrepareForSleep` subscription; woken by the bus or an `eventfd`, never by a periodic poll | `energy::logind`'s connect helper, `CALL_TIMEOUT`, `CONNECT_TIMEOUT` (reused, made `pub(crate)`, not copied) **[verify]** Q10 for unblocking the signal iterator on shutdown |
| `logind` (CEL-2-B) | shell | the `Suspend` call with a proof | unchanged |
| `wallpaper` (CEL-2-E) | shell | one thumbnail per decoded picture | unchanged bounds |
| loop | lock | Wayland, the realtime timerfd, the delay and fade timers, the channel, painting the damaged rectangle | no IO beyond the Wayland socket; the one full-output upscale per output and backdrop is done on the `backdrop` worker into the cached base |
| `backdrop` | lock | reading and validating the bundle | 2 s, 16 MiB |
| `verify` | lock | one attempt: spawn from the pinned fd, write the secret, close, `pidfd` poll to the 15 s deadline, then `pidfd_send_signal(SIGKILL)` and `kill_process_group` on the child's group (its own: `process_group(0)`), reap | one attempt at a time. The verdict `Unavailable` is sent to the loop **as soon as** the kill is sent (M10), so the prompt returns without waiting for the reap; the reap then waits at most 2 s on the pidfd, and a child still not gone is logged and left to the next attempt's spawn check (no second attempt starts while the previous pidfd is unreaped) |

Kill by pid only: every kill targets a pidfd or a process group the
process itself spawned and has not reaped (so neither can be reused).

### 4.11 The PAM child (H-3, H-4)

**Language.** Three ways were compared:

| Option | Own `unsafe` | Supply chain | Build | Memory safety outside FFI | Verdict |
|---|---|---|---|---|---|
| Rust, one FFI module with hand-written `libpam` declarations | one module, about 80 lines, every block with its invariant | `libc` (types, allocator) and `nix` (`User::from_uid`), both already in the lockfile, and `zeroize` | Cargo only; links `-lpam` | yes | **recommended** |
| C, the reference ported | the whole program | none | a C toolchain step in a Cargo-only production entry | no | rejected: more unchecked code, a second build system |
| A crate (`pam`, `pam-client`, `nonstick`) | none of ours | third-party code deciding the unlock; their conversation helpers answer every prompt **[verify]** maintenance and behaviour if reconsidered | Cargo | depends | rejected: delegates the one decision this unit exists to keep narrow |

The recommendation is a **second documented `unsafe` exception** (H-3),
modelled on `egl.rs`: `celestina/crates/celestina-lock-verify/src/pam.rs` is
the only module with `#![allow(unsafe_code)]` and `#![deny(unsafe_op_in_unsafe_fn)]`;
every block states its invariant; the rest of the crate forbids `unsafe`.
`celestina/AGENTS.md` "Local boundary" is extended in the unit's first task,
after the author approves this document (the approval is the "prior" the
root rule asks for, as §5.2 of the shell design was for EGL). Panics cannot
unwind out of the `extern "C"` conversation function (they abort since Rust
1.81; the toolchain is 1.89).

**What `pam.rs` declares:** `pam_start`, `pam_authenticate`,
`pam_acct_mgmt`, `pam_end`, `struct pam_conv`, `struct pam_message`,
`struct pam_response`, and the constants `PAM_SUCCESS 0`, `PAM_SYSTEM_ERR 4`,
`PAM_BUF_ERR 5`, `PAM_PERM_DENIED 6`, `PAM_AUTH_ERR 7`,
`PAM_CRED_INSUFFICIENT 8`, `PAM_AUTHINFO_UNAVAIL 9`, `PAM_USER_UNKNOWN 10`,
`PAM_MAXTRIES 11`, `PAM_NEW_AUTHTOK_REQD 12`, `PAM_ACCT_EXPIRED 13`,
`PAM_CONV_ERR 19`, `PAM_ABORT 26`, `PAM_PROMPT_ECHO_OFF 1`,
`PAM_PROMPT_ECHO_ON 2`, `PAM_ERROR_MSG 3`, `PAM_TEXT_INFO 4`,
`PAM_DISALLOW_NULL_AUTHTOK 0x0001`, `PAM_MAX_NUM_MSG 32`,
`PAM_MAX_RESP_SIZE 512`. A test reads `/usr/include/security/_pam_types.h`
and fails if any value differs.

**Conversation (pure policy in `conversation.rs`, FFI shell in `pam.rs`):**

- `num_msg` outside `1..=32` → `PAM_CONV_ERR`.
- The first `PAM_PROMPT_ECHO_OFF` of the call gets a `malloc`'d,
  NUL-terminated copy of the secret (PAM frees it with `free`, so `libc`'s
  allocator is used).
- A second echo-off prompt, or any echo-on prompt → `PAM_CONV_ERR`, and the
  child exits 4 (`Unsupported`).
- `PAM_ERROR_MSG` and `PAM_TEXT_INFO` are answered with an empty response and
  counted; their text is never copied, printed or forwarded.
- On any allocation failure every response already built is freed and
  `PAM_BUF_ERR` is returned.

**The rest of the child:** user from the real uid (`getuid` → `User::from_uid`),
refusing uid 0; `PR_SET_DUMPABLE 0` and `RLIMIT_CORE 0` first thing; the
secret read into a `Zeroizing<Vec<u8>>` with capacity 512 reserved up front
(no reallocation leaves a copy), refusing more, a NUL, or none; the service
presence check (§4.2) before `pam_start` for the production service, and
refusal if the rehearsal service file *does* exist; `pam_authenticate(h,
PAM_DISALLOW_NULL_AUTHTOK)`, then `pam_acct_mgmt(h, 0)`; the secret dropped
(zeroed) before `pam_end`. No `pam_setcred`, no `pam_open_session`, no
`pam_chauthtok`. **`PR_SET_NO_NEW_PRIVS` is never set** in the lock or the
child: `pam_unix` runs the setuid `unix_chkpwd`, and that bit would make
every attempt `Unavailable`.

**The PAM service** (`celestina/contrib/pam.d/celestina-lock`, text only;
installation in `celestina/README.md`; the author runs
`sudo install -m 0644 -o root -g root celestina/contrib/pam.d/celestina-lock /etc/pam.d/celestina-lock`):

```text
#%PAM-1.0
# Celestina session lock: the password, through the system stack.
auth      include   system-auth
account   include   system-auth
```

What that brings, measured on the author's machine:

| Module | Effect on the lock |
|---|---|
| `pam_faillock` (preauth, authfail, authsucc) | the tally shared with `sudo`, `swaylock` and VT login: 3 failures in 15 min lock the account for 10 min, during which the right password is refused too. Kept: against a person at the keyboard it is the real brute-force bound (§5). The tally file is owned by the user (measured, 0660), so a same-user process can reset it: it is no bound against X2. The author may tune `/etc/security/faillock.conf` |
| `pam_systemd_home` | ignored for a non-homed account |
| `pam_unix … nullok` | password check through `unix_chkpwd`; `nullok` neutralised by `PAM_DISALLOW_NULL_AUTHTOK` |
| `pam_env` (auth) | harmless |
| `account pam_unix`, `pam_time` | expiry and time rules → `AccountRefused` |
| **not** `pam_nologin` | `login` and `system-login` add it; `/run/nologin` (written before a scheduled shutdown) would refuse every unlock. `swaylock`'s `include login` has that defect |

Without the file the lock **still covers** (the author's decision 6): it
shows `LOCK_SERVICE_MISSING` from the start, the verifier answers 5 without
calling PAM (PAM itself would fall to `other` and deny), and the shell warned
at duty-on with `NOTICE_PAM_MISSING` (§4.2). The session is locked out, never
let in and never left asleep uncovered; §8 is the way back. There is no
fallback to any other service.

### 4.12 Coexistence with Noctalia and `swaylock` (H-11)

`ext-session-lock-v1` admits one lock: whichever client asks second is
answered `finished`. So two lockers can race but never both hold the session,
and no rule of ours can make that worse. The rules:

- `duties.lock` ships **off**. With it off nothing of this unit runs: no
  worker, no inhibitor, idle step 2 refused as today.
- The author switches it on after turning off Noctalia's idle lock
  (Noctalia's own idle settings, as CEL-2-B's `VAL-CEL2-IDLE` already asks)
  and rebinding `Mod+Shift+Escape` to `celestina lock suspend` (the author's
  edit; CEL-2-M makes it permanent). The shell logs one reminder line when
  the duty turns on.
- If Noctalia, `swaylock` or any client already holds the lock, ours exits 10,
  is retried within its bound (§4.4) and then reported as "the compositor
  refused the lock", never a suspend. The wording does not claim another
  client: the same answer comes from niri giving up on a failed render.
- If ours holds it, theirs is refused by niri; what they do then is theirs,
  and the session is locked.
- `PrepareForSleep` with a foreign lock holding: ours is refused and the
  inhibitor released after the retries; the machine sleeps locked by the
  other client (or, in the render-failure case, uncovered: F33).

### 4.13 Files and modules

```text
celestina-rs/crates/celestina-lock-proto/           NEW crate, no dependencies at all (review S-I4)
  src/lib.rs         Verdict and the verifier's exit codes, the lock's exit codes, the status-line
                     codec, the bundle codec, Backoff, the PAM service name and its paths, the
                     protocol versions: the one owner both binaries and the shell share
celestina-rs/crates/celestina-shell/src/
  lock.rs            NEW pure: LockPolicy (§4.4) and LockedProof, over celestina-lock-proto
  idle.rs            CHANGED: IdleAction::Resumed
  settings.rs        CHANGED: duties.lock (lenient, default off)
  lib.rs             CHANGED: module list
celestina/crates/celestina-wayland/src/              (toolkit, knows nothing about PAM)
  lock.rs            NEW: the lock profile: binds the session-lock manager, single-pixel buffer, shm,
                     viewporter, fractional scale, seat; never layer shell, xdg_wm_base, data device,
                     primary selection, text input, background effect; Role::Lock; the cover on first
                     configure; Locked/Finished events; the unlock, reachable only through
                     `UnlockCapability` (M7): the toolkit's lock runtime hands exactly one
                     non-cloneable capability to its shell at `locked`, and
                     `UnlockCapability::unlock(self)` consumes it; there is no public
                     `unlock_session()` any other consumer of the toolkit could call
  runtime.rs, handlers.rs, surface.rs   CHANGED: profile-dependent globals and signal policy
                     (the lock profile does not stop on SIGINT/SIGTERM/SIGHUP; it logs them)
  Cargo.toml, lib.rs CHANGED: a `gl` feature (default on) gating egl, blur, popup and their crates
  input.rs           CHANGED: Modifiers::caps_lock; Debug of Key redacts keysym and text
celestina/crates/celestina-lock/                     NEW package, bin celestina-lock
  src/main.rs        argv (--rehearsal), hardening, tracer check, pin the verifier, pid file, run
  src/machine.rs     the lock-side state machine (§4.5), tested without Wayland
  src/verify.rs      one attempt: spawn, pidfd deadline, kill, Authenticated
  src/unlock.rs      the single unlock call
  src/backdrop.rs    bundle reader, validation and the per-output upscaled base (worker)
  src/clock.rs       realtime timerfd, formatting from the bundle's offset
  src/paint.rs       the CPU painter over tiny-skia and cosmic-text, the embedded font (H-15)
  src/view.rs        prompt, secret field, scrim, fade, per-output placement, damage
  src/state_file.rs  best-effort pid file per display
  src/copy.rs        product copy
  src/bin/unlock_probe.rs    rehearsal only: the same core with a fake verifier, a [[bin]] with
                             `required-features = ["fake-verifier"]`; the production entries never
                             pass that feature, and verify-production checks the shipped binary
                             contains no probe symbol (review S-I8)
celestina/crates/celestina-lock-verify/              NEW package, bin celestina-lock-verify
  src/main.rs        argv, stdin, uid, service check, exit code
  src/conversation.rs  which prompt gets the secret (pure)
  src/pam.rs         the FFI module, the second unsafe exception
celestina/src/lock/                                  NEW shell provider
  mod.rs             LockHandle: duty on/off, availability → idle, routing of the policy's actions
  supervisor.rs      worker `lock`
  sleep.rs           worker `lock-sleep`
  bundle.rs          the sealed memfd from the wallpaper thumbnails
celestina/src/energy/{mod.rs,logind.rs,verbs.rs}     CHANGED (§4.9)
celestina/src/wallpaper/surface.rs (and the worker)  CHANGED: keep a ≤ 640 px box-blurred thumbnail per output
celestina/src/{command.rs,app.rs,message.rs}         CHANGED: Verb::Lock, Verb::LockAndSuspend, Message::Lock
celestina/contrib/pam.d/celestina-lock               NEW text, never installed by a script
celestina/scripts/{build,verify,deploy}-production.sh, smoke.sh   CHANGED: two more binaries
celestina/scripts/lock-rehearsal.sh                  NEW: the nest rehearsal (§9.2)
celestina/AGENTS.md                                  CHANGED: the unsafe exception and the lock's copy file
```

**Prerequisite outside the unit's prefix** (review S-I7): `docs/projects.toml`
(the celestina entry's `artifact_paths`, `source_roots`, `commit_roots` and
`production_inputs` gain the two binaries and `celestina-rs/crates/celestina-lock-proto`)
and the `celestina-rs/Cargo.toml` member list are registry files. They are
changed by the author's `suite-maintenance:` commit before CEL-2-H-a's
first commit, together with the salvage edit of §13 (one commit serves both
units), so that HEAD already
authorises the unit's paths (the commit guard reads HEAD and INDEX).

Files by unit: CEL-2-H-b owns the wallpaper changes, the bundle's pixel
blocks in `bundle.rs`, the backdrop part of `backdrop.rs` and `clock.rs`;
every other path above is CEL-2-H-a's (the bundle codec in
`celestina-lock-proto` included, complete from H-a).

The thumbnail is derived on the wallpaper worker from each successful decode
(downscale to at most 640 px a side, a three-pass box blur), kept in the
app's wallpaper state (≤ 1.6 MiB per output), and copied into the bundle at
spawn time. With the wallpaper duty off there is no thumbnail and the lock is
black. That is the normal case during coexistence (M15): while Noctalia paints
the wallpaper, `duties.wallpaper` is off, so the lock shows a black backdrop
until the author moves the wallpaper duty to the shell; the live row says so. The scrim (dimming) is the lock's, from a theme token, so the field's
contrast does not depend on the picture.

### 4.14 Dependencies

| Package | New dependency | Justification |
|---|---|---|
| `celestina-lock-proto` | none | the shared vocabulary must not pull anything into the binaries |
| `celestina-lock` | `celestina-wayland` (`default-features = false`: no `gl`), `celestina-lock-proto`, `celestina-tokens` | the Wayland runtime without EGL, the shared vocabulary, the theme's values |
| | `tiny-skia` 0.12, `cosmic-text` 0.19 (no default features), `fontdb` 0.23 | the CPU painter and text (H-15); all in the lockfile |
| | `rustix` (`fs`, `process`, `time`, `event`) | dumpable, rlimit, pidfd, process-group kill, timerfd, flock, seals; safe API, in the lockfile |
| | `zeroize` 1.x | zeroing secret buffers without an own `unsafe`; small and widely used; **new to the lockfile** |
| | `tracing`, `tracing-subscriber`, `thiserror` | as the shell; the subscriber's filter is fixed at `info` (no environment filter) |
| | **not** `celestina-shell`, `celestina-paint`, `celestina-ui`, `zbus`, `image`, `serde_json`, `niri-ipc`, `khronos-egl`, `femtovg`, `freedesktop-icons`, `resvg` | no bus, no decoder, no compositor IPC, no GPU, no system fonts or icon themes |
| `celestina-lock-verify` | `celestina-lock-proto` | the exit-code vocabulary |
| | `libc` | C types and the allocator PAM frees with; in the lockfile |
| | `nix` (`user`) | `User::from_uid` without own `unsafe`; 0.31.3 in the lockfile |
| | `zeroize`, `rustix` (`process`, `fs`) | as above |
| | system `libpam` | linked, not bundled; hand-written declarations, no bindgen |
| `celestina` | `celestina-lock-proto` | the policy's and the supervisor's vocabulary |

T-D1 pins the **transitive** normal-dependency closure of both binaries:
the evidence and the test compare `cargo tree -e normal --prefix none -p <pkg>`
(deduplicated names) with a checked-in allowlist, so a crate that arrives
through a feature or a transitive edge fails the test. Cargo unifies
features across the packages of **one** invocation, so building `celestina`
and `celestina-lock` together would switch `celestina-wayland`'s `gl` and
`cosmic-text`'s `fontconfig` on for the lock too. `build-production.sh`
therefore builds the two lock binaries in an invocation of their own
(`cargo build --release --locked -p celestina-lock -p celestina-lock-verify`),
and T-D1 checks the closure of that exact command; **[verify]** that the
shared `target/` keeps both feature sets of the toolkit without thrashing the
cache (Q18).

## 5. Threat model

**Assets.** A1 the content of the unlocked session (screen and input);
A2 the password; A3 the integrity of the unlock decision; A4 the invariant
"never asleep unlocked"; A5 the author's ability to get back into their own
session.

**Adversaries.**

| Id | Adversary | In scope for |
|---|---|---|
| X1 | a person at the keyboard and the outputs, without code execution | everything |
| X2 | a process running as the same user | A2 through our channels; no new unlock interface; **not** A3 (below) |
| X3 | the compositor misbehaving or the lock client dying (faults) | A1, A3, A4 |
| X4 | a hostile wallpaper file | A1, A3, A5 |
| X5 | a manipulated environment (launcher, `systemd --user` environment) | A2, A3 |
| X6 | brute force and timing | A3 |
| X7 | denial of service against the author | A5 |

Out of scope: root, the kernel, a malicious compositor (it is the trusted
base: it decides what is shown), boot-time and cold-boot attacks, DMA.

**The honest limit for X2.** On niri, any client of the user's Wayland socket
can replace a *dead* lock and then unlock it (verified in `Niri::lock`). A
same-user process can `SIGKILL` our lock (same uid; non-dumpable does not
prevent a kill) and run its own client. A same-user attacker with code
execution can therefore end any lock on this compositor, ours or `swaylock`.
This unit adds no interface that makes it easier, and keeps the password from
that process; it cannot make niri's replacement policy stricter. Recorded so
no review assumes otherwise (decision H-12 "Revisit when").

**Nothing outside the compositor is trusted about coverage** (review X-H1).
An earlier draft let a flock-held state file stand for "a lock is up": a
nested lock in the shared `XDG_RUNTIME_DIR`, a stale or forged file, or a
lock still covering could then have released the inhibitor or minted a
suspend proof for an uncovered session. Now only this shell's own child's
`locked` line, which only the compositor's `locked` event produces, counts;
the file carries a pid for the author and nothing else (§4.2, §4.4).

| # | Threat | Mitigation | Test or evidence |
|---|---|---|---|
| TM-1 | X1 unlocks without the password through an error path | unlock reachable only with `Authenticated` of the current attempt (§4.5); every other verdict, timeout, crash and error returns to `Prompt` | T-M1, T-M2, T-U1, §7 table, Review B |
| TM-2 | X1 races a stale verdict (old attempt) into an unlock | verdicts carry the attempt number; one attempt at a time | T-M3 |
| TM-3 | X1 sees content while the lock comes up or an output is plugged | covers on first configure, nothing else before `locked`; late outputs get a surface at once; niri blanks the rest | T-W1, R-1, VAL-CEL2-LOCK-HOTPLUG |
| TM-4 | X3 the lock crashes after `locked` | the compositor keeps the session locked; the shell relaunches (≤ 3/60 s), niri accepts the replacement | R-3, T-P5 |
| TM-5 | X3 the lock crashes before `locked` | niri finishes locking after its 1 s deadline **[verify]** Q1; the shell relaunches after `requested` | R-4, T-P5 |
| TM-6 | X3 the shell crashes or restarts | the lock does not depend on the shell: own process group, ignores `EPIPE`; the new shell trusts no file about it and asks the compositor when it next needs a lock | T-S2, R-5 |
| TM-6b | a file or another compositor's lock taken as "covered" (X-H1, S-C2) | no state file is read for any decision; the pid file is per display | T-P8 (no event from a file exists in the policy), R-5 |
| TM-7 | X4 a hostile image crashes or subverts the lock | the lock never decodes; the shell decodes under CEL-2-E's bounds; the bundle is validated (sizes, seals, UTF-8, total) | T-B1…T-B4 (seeded random fuzz of the parser), dependency pin T-D1 |
| TM-7b | X2 or X5 a hostile font, `fonts.conf`, icon theme or shader cache parsed inside the lock (X-H2) | one embedded font in an empty database; no fontconfig, no icon lookup, no GPU, no `HOME`/`XDG_*` | T-F1, T-E1, T-D1 |
| TM-8 | X5 an environment variable picks the verifier, the lock, a library or the PAM service | binaries beside the shell, pinned by fd; `env_clear` + allowlist; service compiled in | T-E1, T-E2, T-V7 |
| TM-9 | X2 or X5 replaces the verifier on disk while the screen is locked | executed from the descriptor pinned at lock start, after `fstat` checks | T-E3 (replace the file, the pinned bytes still run; a script is refused) |
| TM-10 | the password in argv or environment (world-readable `/proc`) | stdin pipe only; verifier env is `PATH` | T-V1 (fake verifier inspects its own cmdline and environ) |
| TM-11 | the password in a log or the journal | no secret is formatted; `Key` Debug redacts; the lock's subscriber is `info`; the verifier writes nothing; PAM text never forwarded | T-L1 (capture every tracing event at TRACE through a full attempt and assert the typed bytes never appear), T-W3 |
| TM-12 | the password in a core dump or through `ptrace` / `/proc/<pid>/mem` | `PR_SET_DUMPABLE 0` and `RLIMIT_CORE 0` first thing in lock and verifier; then `TracerPid` must be 0, else exit before `requested` (lock) or 6 (verifier). Non-dumpable refuses a later attach whatever Yama says, but cannot detach a tracer attached in the instant before the call: closing that window relies on `kernel.yama.ptrace_scope` ≥ 1 (measured 1: only an ancestor may attach) **and** the tracer check (X-N3). No secret exists in that window | T-H1, T-H2 (a traced start exits), R-6 |
| TM-13 | the password in swap | swap is zram only (measured); zeroized buffers with reserved capacity; residual: transient per-key strings from xkb and the toolkit are freed, not zeroed | evidence note; residual accepted by the author |
| TM-14 | the password through the clipboard or an input method | the lock profile binds no data device, primary selection or text input | T-W2 (the lock profile's bound globals) |
| TM-15 | keylogging by other clients | the compositor routes keys only to the lock surface; `allow-when-locked` binds see chords, not text (the author's are media keys) | VAL-CEL2-LOCK-LIVE note |
| TM-16 | a PAM module echoes the secret in a message | messages are counted, never copied or printed; stderr is `/dev/null` | T-V3 |
| TM-17 | the secret sent to a prompt it does not belong to (OTP, username) | only the first echo-off prompt is answered | T-V2 |
| TM-18 | X6 brute force by a person at the keyboard | `pam_faillock` (3 per 15 min, then 10 min) plus the lock's own 2/4/8/16/30 s, carried across a relaunch. Against X2 neither holds (the tally is user-writable, §4.11), which the X2 limit above already concedes | T-M4, T-P6 |
| TM-19 | X6 timing differences between "wrong" and "right" | the only answer is the verdict; `pam_unix` adds its own failure delay; the delay is applied after the verdict | review only |
| TM-20 | X7 the author locked out by a missing or wrong PAM service | warned at duty-on (log and `NOTICE_PAM_MISSING`); the lock still covers and says why (decision 6: locked out, never let in); no `pam_nologin` in the recommended stack; distinct messages; §8 | T-P2, T-V5, VAL-CEL2-LOCK-PAM |
| TM-21 | X7 PAM hangs | 15 s deadline, child and group killed, retry after 2 s | T-V6 (fake verifier that sleeps) |
| TM-22 | X7 crash loop of the lock | relaunch bounded to 3 per 60 s, then the recovery path of §8 | T-P5 |
| TM-23 | X7 `faillock` lockout by an X1 typing wrong guesses | inherent to the shared tally; the hint from the third failure; recovery in §8 | VAL-CEL2-LOCK-RECOVERY |
| TM-24 | sleep before the lock is up | delay inhibitor held while unlocked; our own suspends wait for `locked`; logind's 5 s cap logged when hit | T-P3, T-P4, T-S1 (private bus) |
| TM-25 | an unlock via logind (`loginctl unlock-session`) or a shell verb | neither exists | T-C1 (`Verb::parse` refuses `unlock`), T-G1 (no `Unlock` subscription) |
| TM-26 | X2 asks `Shell1` for `lock` or `lock suspend` (X-N2) | locking is harmless; suspending is a mild denial of service that logind already grants the active user without a password, so nothing new; one intent at a time, origin logged; no further rate limit (§4.9) | T-P1 (one intent), review |
| TM-27 | stack injection through the rehearsal service | the rehearsal service name is compiled, may only be passed by `celestina-lock --rehearsal`, the lock never unlocks in rehearsal, and the verifier refuses if the rehearsal file exists | T-V8, T-M5 |
| TM-28 | the user's input method sees the password (X-N6) | the lock never activates text input; no input method runs today (measured); whether a grab can outlive the focus change is Q16 | Q16, VAL-CEL2-LOCK-LIVE |
| TM-29 | an onlooker reads the clock, the backdrop or the bullet count on an idle lock; OLED burn-in (S-I9) | fade to black after `lock.idle_fade_after` without input; niri's screen-off still applies | T-M6, R-1 |
| TM-30 | X3 niri gives up locking after `lock()` (render failure: it unlocks and sends `finished`) (X-H3) | exit 10 is ambiguous: bounded retry, never a suspend, reported as a refusal; the sleep case is F33 | T-P9, Q19 |

## 6. What the unit never does

1. Never unlocks except after `Authenticated` from the verifier child spawned
   for the current attempt (exit status 0, normal exit, observed through that
   child's pidfd).
2. Never unlocks on a timeout, crash, signal, error, missing service,
   configuration value, environment variable, file, verb, D-Bus call,
   compositor event, duty switch-off or shell restart.
3. Never has a grace period or an "unlocked recently" shortcut.
4. Never offers an unlock verb, method or signal handler.
5. Never serves logind's `Lock` signal in this unit (deferred: during
   coexistence Noctalia may answer it too, and a third trigger path deserves
   its own review), and **never** serves `Unlock`, which any process of the
   session may emit without a password.
6. Never authenticates with anything but the password: no fingerprint, smart
   card, OTP or PIN.
7. Never answers a PAM prompt other than the first echo-off prompt of a call.
8. Never puts the password in argv, the environment, a file, a log line, the
   status pipe, D-Bus, a core dump or the screen; never formats a key event's
   text or keysym.
9. Never forwards a PAM module's text to the screen or a log.
10. Never reads or offers the clipboard or the primary selection; never
    enables an input method in the lock.
11. Never shows notifications, media, clipboard, window titles, application
    content or the user name.
12. Never decodes an image or opens a user file in the lock process.
13. Never finds the lock or the verifier through `PATH` or an environment
    variable, and never executes a path that can change after it was checked.
14. Never falls back to another PAM service (`login`, `other`,
    `system-auth`).
15. Never forces a suspend when the lock is not confirmed, never uses a
    `block` inhibitor for sleep, never keeps the machine awake.
16. Never adds an accessibility bridge, a keyboard-layout indicator or any
    feature not listed in §4 (deferred, not refused).
17. Never kills a process by name; only pidfds and process groups it spawned,
    and the author kills by pid (§8).
18. Never installs or edits anything in `/etc`, never edits `~/.config/niri`,
    never uninstalls or rebinds `swaylock`.
19. Never opens a PAM session, sets credentials or changes a password.
20. Never sets `PR_SET_NO_NEW_PRIVS` on the lock or the verifier.
21. Never runs as root or authenticates uid 0.
22. Never kills or displaces another client's lock.
23. Never rehearses in the author's live session, never types the author's
    password, and never runs a wrong-password rehearsal against the
    production service (it would count toward the author's `faillock`).
24. Never takes a file, a pid, or anything but its own child's `locked`
    line (the compositor's `locked` event) as proof that the session is
    covered.
25. Never loads a font, an icon or a shader from the system or the user's
    home in the lock, and never initialises a GPU there.

## 7. Failure modes: always locked or denied

| # | Failure | Resulting state | The person sees |
|---|---|---|---|
| F1 | lock or verifier binary missing, or fails the pin | **denied**: not locked, no suspend; idle step 2 `NoLocker` | nothing new; log |
| F2 | PAM service file missing (at duty-on, at a request, or vanishing while locked) | **locked** and every attempt denied (verdict `ServiceMissing`); warned at duty-on | `NOTICE_PAM_MISSING` at duty-on; `LOCK_SERVICE_MISSING` on the lock; recovery §8 |
| F3 | (merged into F2) | — | — |
| F4 | no Wayland connection or no session-lock global | **denied**; our suspends do not happen | log; `Notice` for a verb |
| F5 | exit 10: another client holds the lock, or niri gave up | after 2 retries: locked by that client, or unlocked by niri (F33); ours never suspends | the other locker, or the notice `NOTICE_LOCK_REFUSED` |
| F6 | the lock dies before `locked` | **locked** by niri after ≤ 1 s with a dead client [verify Q1]; relaunched | dark red, then the prompt |
| F7 | the lock dies after `locked` | **locked** (dead client); relaunched, accepted as replacement | dark red, then the prompt |
| F8 | relaunch limit reached | **locked** (dead client) | dark red; `NOTICE_LOCK_RELAUNCH_LIMIT` after the unlock; recovery §8 |
| F9 | verifier hangs past 15 s | **locked**; child and group killed | `LOCK_UNAVAILABLE`, retry after 2 s |
| F10 | verifier crashes or exits with an unknown code | **locked** | `LOCK_UNAVAILABLE` |
| F11 | verifier cannot be spawned | **locked** | `LOCK_UNAVAILABLE` |
| F12 | wrong password | **locked**, delay 2/4/8/16/30 s | `LOCK_REFUSED`, `LOCK_WAIT` |
| F13 | account refused after a right password | **locked** | `LOCK_ACCOUNT` |
| F14 | the stack asks something else | **locked** | `LOCK_UNSUPPORTED` |
| F15 | `faillock` lockout | **locked**; even the right password is refused | `LOCK_REFUSED`, then `LOCK_FAILLOCK_HINT` |
| F16 | the CPU painter cannot draw (shm pool cannot be created or grown) | **locked**; covers stay, typing blind still works | black screens |
| F17 | bundle missing, invalid, slow | **locked**; black backdrop | black backdrop with the prompt |
| F18 | output plugged while locked | **locked**; new surface | blank, then cover |
| F19 | output with the prompt unplugged | **locked**; prompt moves with focus | prompt elsewhere |
| F20 | no keyboard | **locked** | `LOCK_NO_KEYBOARD` |
| F21 | shell crashes while locked | **locked**; the lock is unaffected | nothing |
| F22 | shell crashes with a suspend intent pending | **not suspended** | nothing |
| F23 | logind unreachable | our suspends fail (locked, awake); external sleep not gated (logged at duty-on) | log |
| F24 | an external sleep and the lock not confirmed within logind's 5 s | logind sleeps; the lock covers on resume; logged as an error | possibly a glimpse on resume (VAL measures time-to-locked) |
| F25 | our suspend intent not confirmed within 10 s | **not suspended**; the lock continues | the prompt |
| F26 | input returns before `locked` (idle origin) | locked, **not suspended** | the prompt |
| F27 | `SIGTERM`, `SIGINT`, `SIGHUP` to the lock | ignored and logged; still **locked** (at logout the compositor's exit ends it first, F38) | nothing |
| F28 | `SIGKILL` to the lock | as F7 | dark red, then the prompt |
| F29 | the lock dies between `unlock_and_destroy` and the sync | either unlocked by PAM's verdict, or killed by a protocol error first and still **locked**, then relaunched | either the desktop or the prompt again |
| F30 | the compositor sends `finished` after `locked` (exit 13) | the compositor ended the lock by its own policy; never on niri (verified source); not relaunched | the desktop; logged as an error |
| F31 | the pid file cannot be created | **locked**; only the author's pid lookup is lost (no decision depends on it) | log; §8 falls back to reading the pid from `ps -eo pid,args` (listing, never killing, by name) |
| F32 | sctk drops a locked `SessionLock` without `unlock` (bug) | protocol error, client killed, **locked** (dead) | as F7 |
| F33 | niri fails to render a locked frame while `Locking` and unlocks (verified source), during a `PrepareForSleep` | ours exits 10 and is retried; if every retry fails, the inhibitor is released and logind sleeps **uncovered**; logged as an error | the desktop on resume; `NOTICE_LOCK_REFUSED` |
| F34 | no input for `lock.idle_fade_after` | **locked**, faded to black | black; a key brings the prompt back |
| F35 | a tracer is attached when the lock starts | **denied**: exit 15 before `requested`; the shell logs and notices | `NOTICE_LOCK_UNAVAILABLE` |
| F36 | the child stays in `Requested` past 10 s | not killed; the intent dropped; the inhibitor released if a sleep is pending; whatever niri does then (locks or not) | log; `NOTICE_LOCK_NOT_CONFIRMED` for a verb |
| F37 | the child hangs before `requested` past 5 s | killed through its pidfd (it never asked to lock): **denied** | log; notice for a verb |
| F38 | the compositor quits or crashes while locked (logout, `Quit`, a crash) | the lock's Wayland connection breaks and it exits 16 at once (M8), so a logout is never held up waiting for systemd's `SIGKILL` of a lock that ignores `SIGTERM`; the session is gone, nothing is exposed | the greeter |
| F39 | no output at all when locking (all off or unplugged) | niri locks at once (verified: "no outputs, lock the session right away"); the lock has no surface and no keyboard focus, prints `locked`; the first output that appears gets a surface, its cover and the prompt (M9) | nothing until an output appears, then the prompt |
| F40 | more than 8 outputs | the 9th and later get covers and the scrim but no backdrop (§4.3) | black backdrop there |

F24, F30 and F33 are the only rows where the session can end uncovered, and
none is caused by this unit's code: F24 is logind's cap on delay inhibitors,
F30 and F33 the compositor's decisions. All are logged as errors and
measured live.

## 8. Recovery path and its lifetime (H-12)

**Stays installed:** `swaylock` and its `/etc/pam.d/swaylock`, bound to
`Mod+Shift+A`, unchanged by this unit, until every `VAL-CEL2-LOCK-*` row has
passed **and** the author decides to remove it in a separate change. No
agent proposes its removal; CEL-2-M does not include it.

**From a stuck lock (alive but not unlocking):**

1. `Ctrl+Alt+F3` (niri runs `ChangeVt` while locked) and log in there.
2. `cat "$XDG_RUNTIME_DIR/celestina/lock-wayland-1.state"` gives `pid <n>`
   (`XDG_RUNTIME_DIR` is `/run/user/<uid>`; the display name is the
   session's, see `ls /run/user/<uid>/wayland-*`). Without the file, read the
   pid of `celestina-lock` from `ps -eo pid,args`.
3. `kill -KILL <n>` — by that pid only, never by name. `SIGTERM` is ignored
   on purpose.
4. The session stays locked with a dead client. If the shell runs, it
   relaunches the lock within a second and `Ctrl+Alt+F<session>` shows the
   prompt. Otherwise, from the VT:
   `WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/<uid> swaylock -c 000000`
   (the display name is the session's; check `ls /run/user/<uid>/wayland-*`):
   niri accepts it as the replacement of the dead lock and `swaylock`'s own
   PAM service unlocks. Both steps are rehearsed in the nest (§9.2, R-3)
   **[verify]** Q2.

**When the user's own password path is closed** (reviews S-I10, X-N1). The
VT login (`login` → `system-local-login` → `system-login`), `swaylock`
(`include login`) and `sudo` all go through `system-auth`'s `pam_faillock`,
and the first two also through `pam_nologin`. So during a `faillock`
lockout, or while `/run/nologin` exists (a shutdown has been scheduled), steps
1 and 4 are closed too, for the same user. The last resort is **root**:

- `faillock` lockout: wait the unlock time (10 min by default); or log in as
  root on a VT, or over `ssh` as root where the author allows it, and run
  `faillock --user <name> --reset`. Root is not locked by the default
  configuration (no `even_deny_root`). Because the tally file is the user's
  own (measured, 0660), an already open shell of that user (an `ssh` session
  opened with a key, which skips PAM's `auth`) can reset it too.
- `/run/nologin`: cancel the scheduled shutdown (`shutdown -c` as root, or
  wait for it).
- Neither path can be prepared by this unit: it is the author's choice to
  keep a root login (VT or `ssh` key) available while `swaylock` is the
  fallback. The live row checks the author knows which one they have.

**Retirement:** the relaunch and the pid file stay as long as the lock
exists; `swaylock` is the author's to retire.

## 9. Verification

### 9.1 Automated tests (each traced from §5)

| Id | Where | What |
|---|---|---|
| T-P1…T-P7 | `celestina-shell::lock` | every row of §4.4; the inhibitor invariant over random event sequences (seeded), including that no inhibitor is taken while `Preparing`; `LockedProof` only in `Locked`; relaunch bound; failure carry-over; a missing PAM file does not change availability; idle-origin intent dropped by `Resumed`, verb intent not; `Notice` exactly for the explicit refusals of §4.9 |
| T-P8 | `celestina-shell::lock` | the policy has no event or input derived from a file or a pid: a shell restarted under a running lock is `Idle`, and only a `locked` line of its own child mints a proof |
| T-P9 | `celestina-shell::lock` | exit 10 retries twice within 60 s, never suspends, releases the inhibitor during `Preparing` after the last retry; the `Tick` watchdogs of `Spawning`, `Requested` and `Ending` |
| T-P10 | `celestina::lock::supervisor` | the proof is dropped when the child's pidfd is readable (dead) at hand-off |
| T-I1 | `celestina-shell::idle` | `Resumed` only after a handled `LockAndSuspend`; B's tests unchanged |
| T-B1…T-B4 | `celestina-lock-proto` codec | round trip; each bound of §4.3 (a); header kept when a block fails; seeded random bytes never panic and never allocate past the declared size |
| T-C1 | `celestina::command` | `lock`, `lock suspend` parse; `unlock` and any other word refused; `session suspend` is refused the same way with the lock duty on or off (M6) |
| T-M1…T-M5 | `celestina-lock::machine` | every transition of §4.5; stale verdicts discarded; delay schedule 2, 4, 8, 16, 30, 30; empty Enter does nothing; rehearsal never reaches `Unlocking` |
| T-M6 | `celestina-lock::machine` | fade after the token's delay without input; a key or motion restores; the field's content survives the fade |
| T-F1 | `celestina-lock::paint` | the font database holds exactly the embedded Inter faces; no system or fontconfig font is ever loaded; no icon lookup exists |
| T-U1 | `celestina-lock`, `celestina-wayland` | `UnlockCapability` is created once per lock, is neither `Clone` nor `Copy`, and is consumed by `unlock`; the toolkit exposes no other unlock path; `unlock.rs` is the one place that consumes it, and `Authenticated` has no public constructor (type tests plus a source scan, as the language guard scans) |
| T-K1 | `celestina-lock::machine` | a key whose UTF-8 would take the field past 512 bytes is ignored, so a submitted secret is never refused as oversized (M2) |
| T-O1…T-O3 | `celestina-lock::machine` with a fake runtime | outputs added before and after `locked` each get exactly one surface (a second for the same `wl_output` would be `duplicate_output`); a removed output's surface is destroyed and the prompt moves; zero outputs at `locked`, then one appears (F39) (M14) |
| T-O4 | `celestina-lock` | the Wayland connection breaking after `requested` exits 16 (F38) |
| T-X1 | `celestina-shell::lock` | the first draft's scenarios (S-C1, S-C2): a pid file of another display, a stale file, and a shell restarted under a running lock all leave the policy `Idle` with no proof (M14) |
| T-T1 | `celestina-lock::view` | the field and clock text tokens against the scrim token composited over pure white reach 4.5:1 (large clock 3:1) (M13) |
| T-N1 | `celestina-lock-proto` | `service_check` over given path lists: production missing → 5, rehearsal present → 6, both rules without touching `/etc` (M5) |
| T-G2 | `celestina` | a proof whose generation is older than the current one is dropped by the logind worker before `Suspend` (M11) |
| T-Z1 | `celestina::lock::bundle` | the memfd's offset is 0 at spawn and the child's first read returns the magic (M4); a bundle with 8 maximal outputs fits 16 MiB (M3); a missing offset sets `offset_known = 0` and the lock hides the clock (M1) |
| T-L1 | `celestina-lock` | a full attempt with a capturing subscriber at TRACE: the typed bytes appear in no event |
| T-D1 | `celestina-lock`, `celestina-lock-verify` | the transitive normal-dependency closure of the production build command (`cargo tree -e normal`) equals the checked-in allowlist of §4.14; `unlock_probe` is absent without `fake-verifier` |
| T-V1…T-V8 | `celestina-lock-verify` | conversation policy (first echo-off only, echo-on refused, messages never copied, `num_msg` bounds, allocation failure frees); argv/stdin/exit table; secret bounds; uid 0 refused; constants against `_pam_types.h`; the real binary with `--service celestina-lock-rehearsal` and no such file answers 1 (via `other`, outside `faillock`); with the file present answers 6 |
| T-V6 | `celestina-lock::verify` | a fake verifier that sleeps: killed at the (shortened) deadline, group included, verdict `Unavailable`; a fake that exits 0 yields `Authenticated` only for its own attempt |
| T-E1…T-E3 | `celestina-lock`, `celestina::lock` | lock and verifier get only the allowlisted environment; no override variable is read; a pinned binary replaced on disk still runs the pinned bytes, a script is refused |
| T-H1 | both binaries | `prctl(PR_GET_DUMPABLE)` (`rustix::process::dumpable_behavior`) reports not dumpable after hardening, and the core limit is 0 |
| T-H2 | both binaries | a start with a non-zero `TracerPid` (parsed from a fixture of `/proc/self/status`) exits 15 (lock, before `requested`) or 6 (verifier) |
| T-W1…T-W3 | `celestina-wayland::lock` | first commit of a lock surface is the cover; the lock profile binds exactly its list; `Key`'s Debug shows no keysym or text |
| T-S1 | `celestina::lock::sleep` with B's private-bus logind fixture | `Inhibit("sleep", …, "delay")`; released on `locked`; re-held after unlock; `Suspend false` recorded only after `locked` |
| T-S2 | `celestina::lock::supervisor` with a fake lock binary (prints lines, exits) | line parsing bounds, exit table, relaunch, `EPIPE` tolerance, notices rate-limited to one per 10 s |
| T-G1 | `celestina` | no subscription to logind `Lock`/`Unlock` (source scan) |

### 9.2 Rehearsal in the nest only

`celestina/scripts/lock-rehearsal.sh` runs inside `dev-session.sh` (its own
session bus, never `--session`, the system bus pointed at B's fake logind
on a private bus, so no real inhibitor is taken), refuses to start if
`/etc/pam.d/celestina-lock-rehearsal` exists, refuses to start if any
`$XDG_RUNTIME_DIR/celestina/lock-*.state` other than the nest's own display
names a live pid (a lock of the live session; the nest shares the runtime
directory), and tears the nest down by the pid its own `.kdl` run recorded.
It never runs in the author's session.

| Step | Expectation |
|---|---|
| R-1 | `celestina lock` in the nest: `hello 1`, `requested`, `locked` within 1.5 s (time recorded, Q12); niri logs "locking session"; a capture shows the cover and then the prompt |
| R-2 | a second `celestina-lock` started by hand: exit 10, the first unaffected |
| R-3 | `SIGKILL` the lock by its pid-file pid: niri's lock colour, the shell relaunches, niri logs "replacing existing dead lock"; then kill again, stop the shell, and start `swaylock` against the nest's display: it takes over (Q2) |
| R-4 | kill between `requested` and `locked`: the nest ends locked, relaunch covers (Q1) |
| R-5 | `celestina quit` while locked, start the shell again: the lock is untouched; the new shell is `Idle`; `celestina lock suspend` is refused (exit 10 after the retries), with no `Suspend` on the fake logind |
| R-10 | a full lock with the CPU painter at scale 1.5 on the nest's 4K window: per-frame time of a key press and of the minute (p50, p99) recorded for H-15 |
| R-6 | the lock's `/proc/<pid>/environ` and `/proc/<pid>/mem` are owned by root and unreadable to the user (the kernel's effect of a non-dumpable process) |
| R-7 | `celestina-lock --rehearsal` and wrong passwords typed into the nest (only if a virtual keyboard reaches the nest's lock surface, Q7): `LOCK_REFUSED`, the 2 s delay; the rehearsal service never unlocks |
| R-8 | `unlock_probe` (built with `fake-verifier`): `locked`, then `unlocked`, exit 0, niri logs "unlocking session" |
| R-9 | idle on in the nest with short test delays and the fake logind: screen off, lock, `Suspend` only after `locked`; input before `locked` cancels it |

### 9.3 Guards and evidence (program §9)

`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`
(workspace), `celestina --check`, `smoke.sh`, `lock-rehearsal.sh`,
`bash scripts/check-architecture-contract.sh`, the language and
documentation contracts. Production artifacts (now three binaries) are
built, verified and deployed by the landing, never in the worktree. The
evidence records (each unit its own): every test id above with its result; R-1…R-9 (H-a) and R-10 (H-b) with
captures and the time to `locked`; `cargo tree -e normal` of the two new
packages; `ldd` of `celestina-lock-verify` (only `libpam`, `libc` and their
dependencies) and of `celestina-lock` (no `libEGL`, no `libfontconfig`); the
count and location of `unsafe` blocks (all in `pam.rs` and `egl.rs`); the
residuals the author accepted (TM-13, F24, F33, the X2 limit).

### 9.4 Author validation (queued, non-blocking)

| Row | Unit | What the author checks live |
|---|---|---|
| `VAL-CEL2-LOCK-PAM` | H-a | install `/etc/pam.d/celestina-lock` from the repo text; `pamtester celestina-lock <user> authenticate` if installed |
| `VAL-CEL2-LOCK-LIVE` | H-a | duty on, Noctalia's idle lock off; `celestina lock`: three outputs covered black, prompt on the focused output and following the pointer, right password unlocks, wrong one refused with the delay, Caps Lock shown, a password with accents and dead keys, contrast of the field over black, scale 1.5 and 1.0 outputs; no AT-SPI (known gap) |
| `VAL-CEL2-LOCK-SUSPEND` | H-a | idle 240 s: lock then suspend; lid, power button and `systemctl suspend`: locked before sleeping; screen-off while locked |
| `VAL-CEL2-LOCK-HOTPLUG` | H-a | plug and unplug an output while locked: nothing of the session shows |
| `VAL-CEL2-LOCK-RECOVERY` | H-a | §8 from a VT: kill by pid, relaunch, `swaylock` takeover; the `faillock` lockout message; which root path (VT or `ssh`) the author keeps for a lockout |
| `VAL-CEL2-LOCK-NOTICES` | H-a | a refused `lock suspend` and a missing PAM file at duty-on reach the notification owner (Noctalia during coexistence); the fade to black after the token's delay |
| `VAL-CEL2-LOCK-COEXIST` | H-a | with Noctalia running: ours refused while Noctalia's lock holds, and the reverse |
| `VAL-CEL2-LOCK-BACKDROP` | H-b | with `duties.wallpaper` on: each output's own picture, dimmed and blurred; the field readable over the brightest wallpaper; black with the wallpaper duty off (coexistence) |
| `VAL-CEL2-LOCK-CLOCK` | H-b | the clock and date right at lock time, right on resume from suspend, hidden when the offset could not be read; a daylight-saving change during one lock shows the offset read at its spawn |

## 10. Reviews

| Review | Units | Model | Scope | Exit criteria |
|---|---|---|---|---|
| **A**, correctness and contract | CEL-2-H-a and CEL-2-H-b, each before its landing | Opus 5.5, high reasoning (OD-2 (b): a model other than the implementers, who are Sonnet 5.5 and Haiku 5.5) | the unit's whole diff against this document; §4.4 and §4.5 transition by transition; every §7 row against a test or rehearsal step; every §6 item against a test, a guard or a structural reason; the root invariants (no production `unwrap`/`expect`/`panic!`, no blocking IO on a loop, typed errors, bounded input, join on shutdown); the language contract. For H-b also: that it changes none of what §0 freezes | no open finding of severity high; each finding fixed with a test, or accepted in writing by the author |
| **B**, adversarial security only | **CEL-2-H-a only** | Opus 5.5 at its highest reasoning (the author's explicit request; Fable is unavailable), a clean session with no implementer or review-A context; in addition to A, not instead of it | given §5–§7, the H-a diff and the built binaries, try to: (1) unlock without the password; (2) obtain the password; (3) get the machine asleep unlocked; (4) lock the author out beyond §8. Read every `unsafe` block against its invariant, the FFI constants and allocator pairing, panics across FFI, the bundle and line parsers with hostile input, signals, pid reuse, attempt races (double Enter, verdict after output hotplug, unlock during `PrepareForSleep`), environment and argv | zero open findings for (1) and (3); (2) and (4) fixed or accepted in writing as residuals; a written report linked from H-a's evidence |

A fix after either review is reviewed again by the same reviewer before the
unit closes. If H-b's review A finds that H-b touched anything §0 freezes,
Review B is run again over that change.

## 11. Tasks and the model table

Heuristic estimates under the author's global rule; the models follow the
author's choice OD-2 (b) of 2026-10-17. No subagent's `effort` is fixed
except Review B's, which the author asked for.

**CEL-2-H-a, the security core**

| Task | Content | Model | Reasoning | Why |
|---|---|---|---|---|
| T0 | the author: approve this document and this table; make the `suite-maintenance:` prerequisite commit of §13 | — | — | gate |
| TS | spikes for Q1, Q2, Q4, Q6, Q7, Q10, Q11, Q16, Q17, Q18, Q19: throwaway clients and scripts in the nest, nothing kept but the evidence notes; each answer may change §4 before T3 starts | Opus 5.5 | high | protocol behaviour that decides the design; produces notes, not reviewed code |
| T1 | `celestina-lock-proto` (the whole bundle codec included, format v1 frozen), `celestina-shell::lock` (policy), `IdleAction::Resumed`, `duties.lock`; characterisation of B's idle tests first | Sonnet 5.5 | high | security state machine (OD-2 (b)) |
| T2 | `celestina-lock-verify` with `pam.rs`, the `celestina/AGENTS.md` exception line | Sonnet 5.5 | high | the `unsafe` FFI and the verdict (OD-2 (b)) |
| T3 | the toolkit's lock profile, the `gl` feature, cover invariant, `UnlockCapability`, signal policy, `caps_lock`, redacted `Debug`; characterisation of the shell profile first | Sonnet 5.5 | high | protocol and fail-secure invariants (OD-2 (b)) |
| T4 | `celestina-lock` core: machine, verify thread, unlock site, pid file, header-only bundle reading (`prior_failures`), hardening and tracer check, rehearsal flag, `unlock_probe` behind `fake-verifier` | Sonnet 5.5 | high | the unlock decision lives here (OD-2 (b)) |
| T5a | the CPU painter, the embedded font, the prompt view (field, status, Caps Lock), fade, placement, copy | Sonnet 5.5 | medium | presentation on a clear contract |
| T6 | shell integration: supervisor, sleep worker, energy rewiring, verbs, availability, notices, the header-only sealed bundle | Sonnet 5.5 | high | concurrency and suspend ordering (OD-2 (b)) |
| T8a | production entries (the separate lock build invocation), deploy, PAM text and README section, `smoke.sh`, `lock-rehearsal.sh`, running R-1…R-9 | Sonnet 5.5 | medium | scripts against existing contracts |
| T9a | ADRs from H-1…H-18, the H-a `VALIDATION.md` rows, `ROADMAP.md`, `STATUS.md`, the CEL-2-H-a ledger row, evidence | Sonnet 5.5 for the ADRs; Haiku 5.5 for the rows | medium / low | prose with strict contracts; mechanical rows |
| T10a | Review A over H-a | Opus 5.5 | high | a model other than the implementer |
| T11 | Review B over H-a | Opus 5.5 | highest | adversarial security pass, clean context |
| T12a | fixes, then re-review | the implementer's model; the same reviewer | as the task | — |

**CEL-2-H-b, the presentation** (opens after CEL-2-H-a lands)

| Task | Content | Model | Reasoning | Why |
|---|---|---|---|---|
| T7 | wallpaper thumbnails on the wallpaper worker and the bundle's pixel blocks | Sonnet 5.5 | medium | bounded data path over a frozen format |
| T4b | the lock's backdrop worker: validation already in the codec, the upscaled base per output, the scrim over it | Sonnet 5.5 | medium | presentation inside the lock, no security decision |
| T5b | the clock and the date: realtime timerfd, `read_utc_offset()` per spawn, `offset_known`, contrast test T-T1 | Sonnet 5.5 | medium | presentation on a clear contract |
| T8b | rehearsal R-10 and the backdrop/clock captures; the H-b `VALIDATION.md` rows | Sonnet 5.5; Haiku 5.5 for the rows | medium / low | — |
| T9b | the CEL-2-H-b ledger row, evidence, `STATUS.md` | Haiku 5.5 | low | mechanical rows |
| T10b | Review A over H-b, including the §0 freeze | Opus 5.5 | high | a model other than the implementer |
| T12b | fixes, then re-review | the implementer's model; the same reviewer | as the task | — |

Order: T0 → TS → T1 → T2 and T3 (independent) → T4 → T5a → T6 → T8a → T9a →
T10a → T11 → T12a, landing CEL-2-H-a; then T7 → T4b → T5b → T8b → T9b →
T10b → T12b, landing CEL-2-H-b. Each unit is one ledger row, one inventory
and one landing.

## 12. Questions to settle in the nest before implementing

| Q | Question | How |
|---|---|---|
| Q1 | Does niri end in `Locked` with a dead client when the lock dies between `lock()` and `locked`? (`continue_to_locking` does not check liveness) | a throwaway sctk client in the nest, killed after `lock()`; niri log and capture |
| Q2 | Does niri accept a new client after our lock was killed, and does `swaylock` take over and unlock? | R-3 |
| Q3 | Is an output without a lock surface drawn in `CLEAR_COLOR_LOCKED` until our cover maps? | nest capture for the dead-client case; hotplug only live |
| Q4 | Is a single-pixel buffer with a viewport destination accepted as a lock surface's first buffer (no `dimensions_mismatch`)? Otherwise the shm cover is the only path | throwaway client in the nest |
| Q5 | (moot with H-15: covers and frames are both `wl_shm` or single-pixel buffers; reopened only if EGL is chosen) | — |
| Q6 | Does `wl_keyboard.enter` reach the lock surface under the pointer and move with it? | nest (one output), multi-output live |
| Q7 | Does a virtual keyboard (`wtype` against the nest's socket, its path checked against the nest's environment file) reach the nest's lock surface? | nest; if not, R-7 is dropped and T-V covers the path |
| Q8 | Do xkb compose and dead keys give the bytes `passwd` stored (precomposed UTF-8)? | live only, the author typing (VAL-CEL2-LOCK-LIVE) |
| Q9 | Does `std::process::Command` execute `/proc/self/fd/N` (CLOEXEC descriptor) through its posix_spawn path, and refuse a script? | unit test T-E3 |
| Q10 | How does a blocking `zbus` 5 signal iterator end within `JOIN_WAIT` on shutdown? | prototype against B's private bus |
| Q11 | Does `ext-idle-notify` report `resumed` for input while locked? | nest with short delays |
| Q12 | Time from spawn to `locked` | R-1, target < 1.5 s |
| Q13 | (moot with H-15: no mesa in the lock) Does `PR_SET_DUMPABLE 0` disturb anything the lock uses? | R-1 with hardening on |
| Q14 | `InhibitDelayMaxUSec` stays 5 s, and `faillock.conf` stays at defaults; does the author want either tuned? | the author |
| Q15 | Can `faillock` be read and reset without privilege? **Answered:** yes, the tally is the user's own file (0660, measured); recorded in §4.11, §5 and §8. Still open: does the author keep a root VT or `ssh` login for a lockout? | the author |
| Q16 | With an input method running (fcitx5), can its keyboard grab receive keys typed on the lock surface (at the focus change, or later)? | nest with fcitx5 started inside it only; never in the live session |
| Q17 | Does logind wait for a delay inhibitor taken after `PrepareForSleep(true)` was sent? (the policy assumes not, and never takes one then) | B's private-bus fixture cannot answer it; logind's documentation and source, then VAL-CEL2-LOCK-SUSPEND |
| Q18 | Do the two feature sets of `celestina-wayland` (with and without `gl`) coexist in `target/` without rebuilding each other on every production run? | two consecutive production-shaped builds in the worktree, timing the second |
| Q19 | How often, if ever, does niri unlock on a failed locked frame (the render-failure branch), and what does it log? | nest log during R-1…R-10; live journal during VAL |

## 13. The salvage edit of the root `AGENTS.md`

Nothing of the halted tree is **copied** by this unit. The program's row says
"`session.rs` as reference only"; the halted
`celestina-qt/src/provider_adapter/session.rs` turned out to be the
connectivity and session adapter (no lock content), so it is not needed. The
references this design actually used are the lock's own files. Because the
halt allows reading halted files only as the list names them, the author's
edit of "Halted projects" before H's first commit should add:

```text
- CEL-2-H (CEL-2-H-a and CEL-2-H-b): as reference only, nothing copied:
  `celestina-qt/docs/decisions/0004-first-party-session-lock.md`,
  `celestina-qt/src/lockverify/main.cpp`, `celestina-qt/src/lockauthenticator.cpp`,
  `celestina-qt/src/lockcontroller.cpp` and `celestina-qt/src/lock/`, for
  `celestina/crates/celestina-lock`, `celestina/crates/celestina-lock-verify`
  and `celestina/src/lock`.
```

No copy head comments are needed, since nothing is copied. The edit is the
author's (a `suite-maintenance:` change, as for B and E), made in the same
prerequisite commit as the registry changes of §4.13 (`docs/projects.toml`,
the `celestina-rs` member list), and in the same commit the parity ledger's
`CEL-2-H` row is replaced by two `planned` rows, `CEL-2-H-a` and `CEL-2-H-b`,
both `celestina:` (the parity program's row H and its §4.3 "before that
unit's first commit" then apply to H-a; H-b needs no further salvage edit,
since it reads no halted file). CEL-2-H-a's first commit must come after it. This document was written after reading those files on the author's
explicit instruction for this design.

## 14. Decisions recorded here (to become `celestina/docs/decisions/`)

Numbered H-n; each gets the next free ADR number when CEL-2-H lands, after
whatever CEL-2-I has taken by then.

- **H-1. Three processes.** The shell supervises; `celestina-lock` owns the
  lock and the only unlock; `celestina-lock-verify` answers one PAM question.
  The lock outlives the shell; status travels on a pipe; only the
  compositor's `locked`, relayed by the shell's own child, counts as covered,
  and a per-display pid file serves the author only (§4.1, §4.2, §4.4).
- **H-2. Cover first, never decode in the lock.** The toolkit commits a black
  cover on each lock surface's first configure; the lock receives
  pre-scaled, pre-blurred pixels in a sealed memfd and opens no user file
  (§4.3, §4.6).
- **H-3. The verifier is Rust with one FFI module,** the project's second
  documented `unsafe` exception after `egl.rs`, with constants checked
  against the installed header (§4.11).
- **H-4. Password only, versioned contract.** Only the first echo-off prompt
  is answered; other factors would come as protocol 2 (§4.3, §4.11).
- **H-5. Unlock only on `Authenticated` of the current attempt;** every
  other outcome stays locked (§4.5, §7).
- **H-6. Delay 2^n up to 30 s on refusals and account refusals;**
  unavailable outcomes do not count (they say nothing about the guess) but
  wait 2 s; the count survives a relaunch and resets on unlock (§4.5).
- **H-7. PAM service `celestina-lock`, installed by the author, no fallback;
  while it is missing the lock still covers and denies, and the shell warns at
  duty-on;** the recommended stack includes `system-auth` and excludes
  `pam_nologin` (§4.2, §4.11).
- **H-8. Suspend only after `locked`,** through a typed proof; every other
  sleep gated by a `delay` inhibitor held while unlocked; logind `Lock` not
  served, `Unlock` never (§4.9).
- **H-9. The prompt follows keyboard focus;** other outputs show the dimmed
  backdrop only (§4.7).
- **H-10. The lock's clock runs on an absolute realtime timer** that fires on
  resume and on clock steps (§4.8).
- **H-11. `duties.lock`, off by default; coexistence rests on the protocol's
  single lock;** a refused lock never suspends (§4.12).
- **H-12. Recovery:** `swaylock` stays until the author retires it after the
  live rows pass; the shell relaunches a dead lock (≤ 3 a minute); the
  author kills a stuck lock by its pid from another VT; when `faillock` or
  `pam_nologin` closes the user's own paths, root (VT or `ssh`) is the last
  resort. *Revisit when* niri offers a way to restrict who may replace a dead
  lock (§5, §8).
- **H-13. Hardening:** non-dumpable lock and verifier with a tracer check,
  cleared environments with allowlists (no `HOME`, no `XDG_*` but the runtime
  directory), binaries pinned by descriptor beside the shell, identity from
  the uid, zeroized secret buffers, redacted key events, never
  `NO_NEW_PRIVS` (§4.2, §4.11).
- **H-14. ADR 0004 of the halted shell is superseded for this project** by
  H-1…H-13 and H-15…H-18, which keep its boundary (the lock owns the surface, PAM owns the
  verdict, failure is locked) and change its mechanisms as §3.2 lists.
- **H-15. The lock paints on the CPU into `wl_shm`,** with one embedded font
  and no GPU, fontconfig or icon theme in the process; EGL only if a measured
  frame says so (§4.6.1).
- **H-16. The lock's vocabulary lives in `celestina-lock-proto`,** a crate
  without dependencies, and the closure of both binaries is pinned by a test
  (§4.13, §4.14).
- **H-17. Explicit requests that fail are told to the author** as a desktop
  notification; idle refusals stay in the log; the lock fades to black when
  nobody is there (§4.5, §4.9).
- **H-18. CEL-2-H is delivered as two units** (the author's decision of
  2026-10-17): CEL-2-H-a, the security core with the backdrop bundle format
  frozen, reviewed by A and B; then CEL-2-H-b, the backdrop and the clock,
  reviewed by A, which may not change what H-a froze (§0, §11).

## 15. Delivery of this document

This is a design document only, covering both units. It is committed, on the
author's explicit authorization, as `suite-maintenance:` in the same way as
the program's design, possibly in the prerequisite commit of §13; the ledger
row CEL-2-H-a moves to `active` when that unit opens, and CEL-2-H-b only
after CEL-2-H-a has landed. Before T1 starts, the author approves the
complete specification and §11's tables and makes the commit of §13.

## 16. Review dispositions

`S-*` are review A's findings (Sonnet 5.5), `X-*` the adversarial pass's
(Opus 5.5). Each accepted finding was checked against the code or the machine
first; the check is named.

| Finding | Disposition | Reason and check |
|---|---|---|
| S-C1 `Locked(Foreign)` had no way out; re-probe before minting a proof | **accepted, superseded by X-H1** | the foreign state is gone; a proof comes only from this shell's child's `locked`, and the lock worker checks the child's pidfd before handing it on (§4.4, T-P10) |
| S-C2 one state file for every compositor | **accepted** | verified: `dev-session.sh` uses the desktop's `XDG_RUNTIME_DIR`; the pid file is now per display and the rehearsal refuses to run beside a live session's lock (§4.2, §9.2) |
| X-H1 the flock file must not decide coverage, locking or the inhibitor | **accepted** | verified in the first draft's §4.4 (probe on `PrepareForSleep`, `Locked(Foreign)`, exit 14): all removed; the compositor decides; recorded in §5 and as never-item 24 |
| X-H2, S-I5 fonts, icons and caches parsed inside the lock | **accepted** | verified: `celestina-paint` uses `FontSystem::new()` (fontconfig) and `freedesktop-icons`; the lock embeds Inter in an empty `fontdb`, has no icon lookup, gets no `HOME`/`XDG_*` but the runtime directory, and `MESA_SHADER_CACHE_DISABLE` (§4.2, §4.6.1, T-F1) |
| X-H3, S-I3 `finished` after `lock()` may be niri giving up; missing watchdogs and rows; the `Preparing` contradiction; allowed actions | **accepted** | verified: niri calls `unlock()` when a locked frame fails to render while `Locking`, and `allowed_when_locked` also lists `Quit` and `ToggleKeyboardShortcutsInhibit`. Bounded retry on exit 10, `Tick` watchdogs for `Spawning`/`Requested`/`Ending`, the missing rows (`Lock` while locking, requests during `Ending`), no inhibitor taken while `Preparing`, F33–F37, §3.1 updated |
| S-I1 the author's decision 6: a missing PAM file must still lock | **accepted** | the first draft refused to lock, contradicting decision 6; now the lock covers, says why, and denies; the shell warns at duty-on (§4.2, F2, H-7) |
| S-I2 refusals of explicit requests must be visible | **accepted** | verified: the shell has no internal notice API (`celestina/src/notifications` exposes none), so a bounded `Notify` call to the owner of `org.freedesktop.Notifications`; copy in §4.7 (§4.9, H-17) |
| S-I4 a dependency-free crate for codes, verdicts and codecs; pin the closure | **accepted** | `celestina-lock-proto` with no dependencies; the lock no longer depends on `celestina-shell`; T-D1 compares the transitive `cargo tree` closure; verified `celestina-tokens` has no dependencies (§4.13, §4.14, H-16) |
| S-I6 compare a CPU painter with EGL | **accepted** | comparison in §4.6.1; CPU into `wl_shm` chosen (H-15); the toolkit's GL code gated by a `gl` feature; verified `tiny-skia`, `cosmic-text`, `fontdb`, `swash` in the lockfile and `FontSystem::new_with_locale_and_db` in cosmic-text 0.19; F16 is now the shm failure |
| S-I7 spikes first, T7 before T6, registry prerequisite; split the unit | **accepted**; the split decided by the author (OD-1: two units, H-18). T7 moved to H-b, so "T7 before T6" became "T6 writes a header-only bundle in H-a" | task TS, the new order, and the author's prerequisite commit (§4.13, §11) |
| S-I8 the probe as a feature-gated binary, not an example | **accepted** | `src/bin/unlock_probe.rs` with `required-features = ["fake-verifier"]`; T-D1 and verify-production check it is absent (§4.13) |
| S-I9 fade to black without input | **accepted** | `lock.idle_fade_after` token, T-M6, F34, TM-29 (§4.5) |
| S-I10, X-N1 `swaylock` and the VT are under `faillock` and `pam_nologin` too | **accepted** | verified: `login` has `auth requisite pam_nologin.so` and includes `system-local-login` → `system-login` → `system-auth` (`pam_faillock`); `swaylock` includes `login`. §8 now ends at root (VT or `ssh`); Q15 answered by measurement: the tally `/run/faillock/<user>` is user-owned (0660) and readable and resettable without privilege |
| S-I11 Review B uses the implementer's model | **decided by the author: OD-2 (b)** | Sonnet 5.5 high implements T1–T4 and T6; review A is Opus 5.5 high; Review B Opus 5.5 highest in a clean context, over H-a only (§0, §10, §11) |
| S-I12 F33 for niri's render-failure unlock, `Quit` and `ToggleKeyboardShortcutsInhibit` in §3.1 | **accepted; already applied with X-H3** | verified in niri (`allowed_when_locked` in `src/input/mod.rs`, the `Locking` branch that calls `unlock()`); F33 and §3.1 carry both |
| X-N2 `Shell1` lets any process ask for `lock suspend` | **accepted; rate limit rejected** | TM-26 corrected (it is a mild denial of service, not "harmless"); no rate limit beyond one intent, because logind already grants the active user suspend without a password (verified: `CanSuspend` answers `yes`) (§4.9) |
| X-N3 TM-12 depends on `ptrace_scope` ≥ 1 | **accepted with a correction** | the kernel refuses a later `ptrace` of a non-dumpable process whatever Yama says, so TM-12 depends on Yama only for the instant before `PR_SET_DUMPABLE 0`, where a tracer already attached would stay; added the `TracerPid` check (exit before `requested`, T-H2); measured `ptrace_scope` = 1; no secret exists in that window |
| X-N6 the user's input method | **accepted as a recorded risk** | verified: niri serves `zwp_input_method_v2`; no input method runs today (measured); TM-28 and Q16 (an input-method grab outliving the focus change) |
| M1 the offset is a per-process `OnceLock` | **accepted** | verified in `island/clock.rs` (`utc_offset()`, once per process; on a failed `date` it logs one warning and shows UTC, so "without warning" holds for the screen, not the journal). The lock worker reads it afresh on every spawn through an uncached `read_utc_offset()`; `offset_known = 0` hides the clock (§4.3, §4.8, T-Z1) |
| M2 512 is not PAM's limit; a multibyte key could overflow it | **accepted** | verified: the header calls the 512 limits "mostly arbitrary". 512 is now this design's own cap (§3.2), and a key is appended only if the result stays ≤ 512 bytes (§4.5, T-K1) |
| M3 16 outputs × 1.6 MiB exceed 16 MiB | **accepted** | 16 × 640 × 640 × 4 ≈ 25 MiB; outputs capped at 8 (≈ 12.5 MiB), the rest black (§4.3, F40) |
| M4 a sealed memfd as stdin is read from its offset | **accepted** | positioned writes keep the offset at 0, asserted before spawn (§4.3, T-Z1) |
| M5 `--service` must be a closed enum; T-V8 needs root | **accepted** | two-value enum in `celestina-lock-proto`; `service_check` is pure over given path lists (§4.3 (c), T-N1) |
| M6 `session suspend` changing meaning with the duty | **accepted** | `session suspend` is now always refused, pointing to `lock suspend`; one verb, one behaviour (§4.9, T-C1) |
| M7 a public `unlock_session()` in the toolkit | **accepted** | replaced by a non-cloneable `UnlockCapability` handed once to the lock's shell and consumed by `unlock` (§4.13, T-U1) |
| M8 a lock that ignores `SIGTERM` at logout | **accepted** | the lock exits 16 when its Wayland connection breaks after `requested`, so logout is not held up; policy row and F38 (§4.3, §4.4, T-O4) |
| M9 zero outputs | **accepted** | verified in `Niri::lock`: with no outputs niri locks at once; F39 and T-O3 |
| M10 the reap after `SIGKILL` | **accepted** | the verdict reaches the prompt when the kill is sent; the reap waits at most 2 s; no new attempt while the old pidfd is unreaped (§4.10) |
| M11 an inhibitor taken after `PrepareForSleep(true)` does not delay it | **accepted** | the proof carries a lock generation checked by the logind worker right before `Suspend` (§4.9, T-G2); whether logind honours a late delay inhibitor stays Q17 |
| M12 a 100 ms polling loop costs battery | **accepted** | the `lock` and `lock-sleep` workers block on `poll` with an `eventfd` for commands and shutdown (§4.10) |
| M13 contrast test of text over the scrim | **accepted** | T-T1 against pure white under the scrim |
| M14 missing tests; the lock's stderr | **accepted** | T-O1…T-O4, T-X1; stderr inherited from the shell, i.e. the journal under `systemd-cat` (§4.1) |
| M15 a black backdrop while Noctalia owns the wallpaper | **accepted** | noted in §4.13 and for the live row |
