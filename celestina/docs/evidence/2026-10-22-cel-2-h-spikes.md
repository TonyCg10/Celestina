# Evidence: CEL-2-H Task S — the spikes Q1, Q2, Q4, Q6, Q7, Q10, Q11, Q16, Q17, Q18, Q19

- **Date:** 2026-10-22
- **Scope:** Task S of the CEL-2-H plan
  ([plan](../../../docs/superpowers/plans/2026-10-17-celestina-cel-2-h-session-lock.md),
  [spec](../../../docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md)
  §3, §9.2, §12), unit CEL-2-H-a, worktree `celestina-CEL-2-H-a` on
  `unit/celestina/CEL-2-H-a` at `3f37f329`. P1 checked: `AGENTS.md` carries
  the CEL-2-H salvage entry, the ledger has two `planned` rows `CEL-2-H-a` and
  `CEL-2-H-b`. No repository file other than this note was changed; the spec
  is not edited (stop point P2).
- **Environment:** Linux 7.2.10-1-cachyos; toolchain 1.97.1
  (`celestina/rust-toolchain.toml`); nest compositor
  `~/.local/lib/celestina/niri` 26.04 (v26.04-19-gf4b5347b; its lock, input
  and idle code is unchanged from the release `8ed0da4` that `/usr/bin/niri`
  reports, checked with `git diff 8ed0da4 f4b5347b` in
  `/home/toni/CODIGO/NIRI-MELIBEA`); smithay `ff5fa7d` (niri's pin, cargo git
  checkout); smithay-client-toolkit 0.21.1, wayland-protocols 0.32.13,
  wayland-protocols-misc/-wlr 0.3.12, zbus 5.19.0 (versions of
  `celestina/Cargo.lock`); systemd 262 (262.1-1-arch); dbus-daemon 1.16.2;
  swaylock and wtype from the system; fcitx5 **not installed**.
- **Artifact:** not applicable. The probes were a throwaway crate outside the
  repository (scratchpad: `lockprobe`, `busprobe`, a driver script); nothing
  of them is kept. No production entry ran.

## Contradictions with spec §4 (read first)

1. **Q4 — niri does not serve `wp_single_pixel_buffer_manager_v1`.** The
   global exists in niri only under `#[cfg(test)]` (`src/niri.rs`, the
   `SinglePixelBufferState::new` call; commit `d38bfc4a` "Add test-only
   single-pixel-buffer support", present in `8ed0da4`, `f4b5347b` and
   `e4062688`). The nest's registry lists 42 globals and no single-pixel
   buffer. Spec §3.1 (the row "`wp_single_pixel_buffer_v1` and `wp_viewporter`
   … served by niri") is wrong for single-pixel; §4.6, §4.13 and T-W1's
   primary cover path cannot run on this compositor. The zeroed `wl_shm`
   XRGB8888 cover works and is the only path (plan Task 3 step 5 already says
   to delete the single-pixel path in that case).
2. **Q16 — an input method can take every key typed on the lock screen.** In
   smithay `ff5fa7d`, `zwp_input_method_v2.grab_keyboard` installs a seat
   keyboard grab at once, whatever the activation or the focus
   (`src/wayland/input_method/input_method_handle.rs`, `GrabKeyboard`); the
   grab forwards every forwarded key to the input method and none to the
   focused surface (`input_method_keyboard_grab.rs`, `KeyboardGrab::input`);
   it ends only when the client destroys it. niri forwards keys through that
   grab while locked (`src/input/mod.rs`, `on_keyboard` →
   `KeyboardHandle::input` → `input_forward` → `with_grab`). In the nest, a
   throwaway input-method client got its grab's keymap (the grab installed)
   **while the session was locked**, and a grab taken before the lock
   survived it (only `deactivate`/`done` arrived). Any unrestricted client of
   the user can bind the global (niri's filter is `client_is_unrestricted`).
   Spec §4.6.1 "Input" ("smithay should deactivate the input method …") and
   §5 TM-15 ("the compositor routes keys only to the lock surface") and TM-28
   do not hold on this compositor: an X2 process obtains A2 (the password)
   through the compositor, without touching our channels. Related, found
   while reading the same path: in a session instance niri serves
   `org.freedesktop.a11y.KeyboardMonitor` on the session bus
   (`src/dbus/mod.rs`, inside `is_session_instance`), and `process_key`
   (`src/dbus/freedesktop_a11y.rs`) emits every key's keysym and character to
   any client that called `WatchKeyboard`, with no lock check
   (`a11y_process_key` runs before the key reaches any surface). Neither path
   can be closed by the lock; the options are a niri change (the author builds
   a patched niri) or a written X2 residual next to "the honest limit for X2".
   Physical key delivery itself was not exercised (see Limits).

Everything else below agrees with §4 or refines it without changing a
decision.

## Procedure

The nest was started once, on its own session bus, without the host-side
window placement (`--monitor none`, so nothing ran against the host's IPC
socket; the nested output kept scale 1.5), with a placeholder in place of the
shell; every client ran with `env -i WAYLAND_DISPLAY=<nest> XDG_RUNTIME_DIR=…
PATH=/usr/bin`, after re-reading `$XDG_RUNTIME_DIR/celestina-dev-session.env`
and checking, before each client, that the display is not `wayland-1`, that
the listener of that socket (`ss -xlp`) is the niri pid named in the recorded
`NIRI_SOCKET`, and that this niri is the child of the launcher pid the run
recorded. Every kill was `kill -KILL <pid>` of a pid the driver itself
started. The nest was closed with `niri msg action quit --skip-confirmation`
against its own recorded socket; its placeholder, orphaned by the quit, was
stopped by pid after its `environ` showed the nest's display.

```sh
# from the worktree root, launcher pid recorded first
sh -c 'echo $$ > runs/launcher.pid; exec celestina/scripts/dev-session.sh --own-bus --monitor none --binary <scratch>/nest-hold.sh'
# nest record: WAYLAND_DISPLAY=wayland-2, NIRI_SOCKET=/run/user/1000/niri.wayland-2.1300795.sock,
#   niri 1300795 child of launcher 1300776 (dbus-run-session); the live session is wayland-1, niri 1435
bash <scratch>/nest-probes.sh          # two passes; pass 1 used the single-pixel cover and is kept for Q1/Q4
faillock --user toni                   # read only, before and after both passes
```

`lockprobe lock --cover shm|spb|none [--unlock-after-ms N] [--exit-after-commit] [--idle-ms N]`
locks with sctk 0.21.1 `session_lock`, one surface per output, and prints one
line per event; keys are counted, never decoded or printed. `lockprobe idle`
holds an `ext_idle_notification_v1`; `lockprobe vptr` sends
`zwlr_virtual_pointer_v1` motions; `lockprobe im --grab` binds
`zwp_input_method_v2` and calls `grab_keyboard`.

## Q1 — the lock dies between `lock()` and `locked`

```sh
lockprobe lock --cover none &      # prints "requested", then: kill -KILL <its pid>
grim -s 0.25 q1a-after-death.png   # 1.5 s later, against the nest
lockprobe lock --cover shm --unlock-after-ms 1000
lockprobe lock --cover shm --exit-after-commit   # exits right after the cover commit
```

```text
18:26:12.918 INFO niri::niri: locking session                                   (q1a)
18:26:14.478 INFO niri::niri: locking session (replacing existing dead lock)     (its replacement)
18:26:17.251 INFO niri::niri: locking session                                   (q1b, exit 9 after commit)
18:26:18.811 INFO niri::niri: locking session (replacing existing dead lock)
q1a-after-death.png, q1b-after-death.png: 95.7 % of pixels (77,25,25)  = CLEAR_COLOR_LOCKED (0.3, 0.1, 0.1)
```

Pass 1 gave the same result for a third shape: a client that received its
`configure` and exited without committing anything (the single-pixel path,
global missing) left niri locked with a dead client; the next lock was
"replacing existing dead lock".

**Conclusion:** yes. niri finishes locking after its 1 s deadline with the
dead client and stays `Locked`, drawing the lock colour (dark red) where no
lock surface exists; the next client replaces the dead lock. This confirms
TM-5 and R-4 and answers Q3 for the dead-client case. Two details for Task 3
and Task 4: on a replacement niri sends `locked` **before** the new client's
`configure` and cover (`q1a-repl`: `locked` +253 ms, then `configure`, then
the commit), so the toolkit must cover on a `configure` that arrives after
`locked`; and a client refused while another is locking receives `finished`
and then still a `configure` for a surface niri ignores ("ignoring lock
surface from an unrelated client"), which must not be treated as an error.

## Q2 — a second client and `swaylock` after the locked client is killed

```sh
lockprobe lock --cover shm &   # waits for "locked", then kill -KILL <its pid>
lockprobe lock --cover shm --unlock-after-ms 1000            # q2a-repl
lockprobe lock --cover shm &   # locked, then kill -KILL <its pid>
swaylock -e -c 000000 &        # no key is ever sent while it runs; killed by its pid (and its PAM child's) after 5 s
lockprobe lock --cover shm --unlock-after-ms 1000            # q2b-repl
```

```text
q2a-dead-lock.png: 95.7 % (77,25,25), the lock colour, after the SIGKILL
18:26:23.777 INFO niri::niri: locking session (replacing existing dead lock)   q2a-repl: locked, unlocked, exit 0
18:26:25.731 INFO niri::niri: unlocking session
18:26:28.736 INFO niri::niri: locking session (replacing existing dead lock)   swaylock
driver: swaylock pid 1305364 children: 1305369; swaylock alive after 2.5 s (not refused)
q2b-swaylock.png: 100 % (0,0,0), swaylock's black
18:26:35.732 INFO niri::niri: locking session (replacing existing dead lock)   q2b-repl after swaylock was killed
18:26:37.731 INFO niri::niri: unlocking session
faillock --user toni: empty before and after both passes
```

**Conclusion:** yes for the takeover: a second client's `lock()` is accepted
at once over a dead lock, and that client's `unlock_and_destroy` unlocks;
`swaylock` is accepted as the replacement and covers. That `swaylock` then
unlocks with the right password was **not executed** (it needs the author's
password; no key was sent to it, `-e` ignored any empty submission, and the
`faillock` tally stayed empty): it stays in `VAL-CEL2-LOCK-RECOVERY`. §8
step 4 and R-3 hold as far as the nest can show.

## Q4 — a single-pixel buffer with a viewport as the first buffer

```sh
WAYLAND_DEBUG=1 lockprobe idle --run-ms 100     # registry of the nest
lockprobe lock --cover shm --unlock-after-ms 1500
```

```text
hello globals: single-pixel=false viewporter=true outputs=1 seat=true
registry: … ext_session_lock_manager_v1 v1 … wl_shm v2 … wp_viewporter v1 … (42 globals, no wp_single_pixel_buffer_manager_v1)
q4b: requested, configure 628x673, committed cover=zeroed-shm-xrgb8888, locked (+915 ms), unlocked, exit 0
```

**Conclusion:** the question cannot arise on niri: the single-pixel global
is test-only there (contradiction 1). smithay's dimension check would have
accepted it (`src/wayland/session_lock/surface.rs`, the pre-commit hook uses
the viewport destination when one is set), but no client can create the
buffer. The zeroed shm XRGB8888 buffer of the configured logical size, buffer
scale 1, maps and locks without a protocol error at scale 1.5 (the surface
was told preferred scale 2; a 1:1 logical buffer is still the size the
configure names). Changes §3.1, §4.6, §4.13 and T-W1.

## Q6 — keyboard focus on the lock surface

```text
q6lock: requested, configure, committed cover, kbd-enter on-lock-surface=true (+1 ms), ptr-enter on-lock-surface=true, locked
q16lock, q1a-repl, q2a-repl: kbd-enter on-lock-surface=true in every run, before or with `locked`
```

**Conclusion:** yes on the nest's single output: `wl_keyboard.enter` reaches
the lock surface as soon as niri is `Locking` (before `locked`), and again on
a replacement. niri recomputes the lock focus on every refresh from the output
under the cursor, else the active output, else the first
(`src/niri.rs` `lock_surface_focus`, called from `update_keyboard_focus` in
`refresh`), so the focus follows the pointer between outputs by
construction; that half is multi-output and stays live only
(`VAL-CEL2-LOCK-LIVE`). Agrees with §4.6.1 "Which output".

## Q7 — a virtual keyboard against the nest's lock surface

```sh
wtype abc     # env -i WAYLAND_DISPLAY=wayland-2 (verified), while q6lock was locked
```

```text
q6lock: kbd-key-press count=1, 2, 3 (+2619 … +2627 ms)
```

**Conclusion:** yes. `zwp_virtual_keyboard_v1` keys reach the lock surface:
smithay sends them straight to the focused surface's `wl_keyboard`
(`src/wayland/virtual_keyboard/virtual_keyboard_handle.rs`, `Key` →
`for_each_focused_kbds`), bypassing niri's input path, its binds and any
keyboard grab. R-7 can run as planned (rehearsal service only). The same
path lets any same-user client type guesses into the lock; that is the X2
limit and TM-18 already concede it.

## Q10 — ending a blocking zbus 5 signal reader within `JOIN_WAIT`

```sh
busprobe <short dir>   # its own dbus-daemon (fixture config, unix:path=…), a fake
                       # org.freedesktop.login1.Manager with Inhibit and PrepareForSleep
```

```text
Inhibit returned an fd: OwnedFd { … fd: 10 }; fake holds 1 read end(s)
A drop-clone: iterator STILL BLOCKED after 3.00 s (> JOIN_WAIT)
B close(): close() returned Ok after 111 µs; iterator ENDED after 234 µs
B' close() with the bus stopped (SIGSTOP of its own dbus-daemon pid): close() Ok after 39 µs; ENDED after 127 µs
C waker+channel: worker ENDED after 35 µs (wake-ups 2 over 1.5 s idle); join + drop 145 µs
C' waker+channel with the bus stopped: ENDED after 23 µs; join + drop 78 µs
```

**Conclusion:** dropping the other connection clones never ends a blocking
`MessageIterator` (it holds its own reference). Two ways end it in well
under a millisecond, also with a hung bus: (a) `Connection::close()` on a
clone from the stopping thread, the repository's existing pattern
(`celestina/src/providers/magnetita.rs`, `session`); (b) polling the
`MessageStream` by hand with a `Waker` that writes to the same bounded
channel the stop path writes to, and blocking on `recv()` with no timeout
(the shape of `celestina/src/providers/tray.rs` `signal_worker`, minus its
periodic `recv_timeout`). (b) wakes only for traffic (two wake-ups: the
match-rule reply and the signal). Neither needs an `eventfd`: §4.10's "woken
by the bus or an eventfd" can be a channel; no §4 decision changes.

## Q11 — `ext-idle-notify` while locked

```sh
lockprobe idle --idle-ms 1500 --run-ms 11000 &        # a separate client, like the shell's idle worker
lockprobe lock --cover shm --unlock-after-ms 9000 --idle-ms 1500 &
wtype abc                                             # +2.6 s after locked
lockprobe vptr --moves 10                             # +4.6 s
```

```text
q11idle: idle-idled +1500 | (wtype at +2.6 s: nothing) | idle-resumed +4666 (the first virtual-pointer motion) | idle-idled +7438 | idle-resumed +9020 (the unlock) | idle-idled +10520
q6lock:  locked +1 | idle-idled +1500 | idle-resumed +4651 | ptr-motion count=1…10 | idle-idled +7424 | unlocked +9006
```

**Conclusion:** yes: input that goes through niri's input path while locked
(here `zwlr_virtual_pointer_v1`, which niri feeds to `process_input_event`
→ `notify_activity`) reports `resumed` to every idle notification, the lock's
own and another client's. Virtual-keyboard input does **not** count as
activity (it bypasses that path), and niri also reports `resumed` at unlock
(`SessionLockHandler::unlock` calls `notify_activity`). Physical keys and
the pointer take the counted path. Agrees with §4.4 `IdleResumed` (dropping
an idle-origin intent); R-9's "input before `locked` cancels it" must use
pointer input, not `wtype`.

## Q16 — an input method's keyboard grab and the lock surface

fcitx5 is not installed and installing it needs root: the fcitx5 run is
**not executable**. It was replaced by a throwaway `zwp_input_method_v2`
client, which is the protocol fcitx5 uses on niri.

```sh
lockprobe im --grab --run-ms 9000 &       # grab before the lock
lockprobe lock --cover shm --unlock-after-ms 6000 &
wtype ab
kill -KILL <im pid>; lockprobe im --grab --run-ms 2000   # grab requested while locked
```

```text
q16im1: im-created, im-grab-requested, im-grab-repeat-info, im-grab-keymap (grab installed) +0 | im-deactivate, im-done +67 (the lock took focus)
q16lock: kbd-enter on-lock-surface=true, locked | kbd-key-press count=1, 2 (wtype, virtual keyboard)
q16im2 (session locked): im-grab-repeat-info, im-grab-keymap (grab installed) +0
```

**Conclusion:** contradiction 2. A grab outlives the focus change (the lock
only causes `deactivate`) and can be created while locked. The keys `wtype`
sent reached the lock surface because virtual-keyboard input bypasses grabs;
physical keys go through the grab (source above), so with a grab held they
reach the input method and not the lock surface. Changes §4.6.1 "Input",
§5 TM-15 and TM-28.

## Q17 — does logind wait for a delay inhibitor taken after `PrepareForSleep(true)`?

Primary sources, systemd tag `v262` (commit `8cc40e0c5e92…`, the installed
262.1): `src/login/logind-dbus.c`, `src/login/logind-inhibit.c`,
`src/login/logind.c`, `docs/INHIBITOR_LOCKS.md`, and the installed
`org.freedesktop.login1(5)`.

- `bus_manager_shutdown_or_sleep_now_or_later` emits `PrepareForSleep(true)`
  (`send_prepare_for`) and, in the same call, decides whether to delay by
  asking `manager_is_inhibited(…, MANAGER_IS_INHIBITED_CHECK_DELAY …)` about
  the inhibitors that exist at that instant; with none it starts the sleep
  job at once (`execute_shutdown_or_sleep`), before any client has read the
  signal.
- While a delayed or running sleep is pending (`m->delayed_action` set,
  from the delay until the job finishes), `method_inhibit` refuses a new
  delay lock for that operation with `org.freedesktop.login1.OperationInProgress`
  (the comment above the check says the lock would give a false impression).
- `manager_dispatch_delayed` (each loop iteration, `logind.c` `manager_run`)
  re-checks the current delay inhibitors and executes as soon as none is
  left, or when the `InhibitDelayMaxUSec` timer started at the signal fires;
  that timer is not restarted.
- `PrepareForSleep(false)` is emitted from the job-removed handler right
  before `delayed_action` is cleared in the same handler, so an `Inhibit`
  sent after receiving it is accepted. `INHIBITOR_LOCKS.md` "Taking Delay
  Locks" gives exactly that scheme (take, release on true, re-take on false).

**Conclusion:** no. A delay inhibitor requested after the signal is not
merely ineffective: logind refuses it (`OperationInProgress`). The §4.4
invariant (never take one while `Preparing`, re-take on
`PrepareForSleep(false)`) is right. Refinement for Task 6: the sleep worker
must treat `OperationInProgress` as the expected answer of a race, not as a
bus failure. `VAL-CEL2-LOCK-SUSPEND` still measures the live ordering.

## Q18 — two feature sets of the toolkit in the shared `target/`

The lock packages do not exist yet. The stand-in is the real feature split
of today's tree: `-p celestina-wayland` alone resolves different features of
shared crates than `-p celestina` (`cargo tree -e normal,features`: `bytemuck`,
`log` and two `syn` versions differ), so cargo builds a second
`celestina-wayland` unit. Debug profile only, in the worktree's shared target
(`CELESTINA.worktrees/.cargo-target`).

```sh
python3 q18.py runs celestina celestina-wayland celestina celestina-wayland celestina
# each step: cargo build --locked --offline -p <pkg>, from celestina/
```

```text
build 1: -p celestina          wall 8.79 s  compiled 10 (this worktree's path crates)
build 2: -p celestina-wayland  wall 0.76 s  compiled 1  (celestina-wayland, second unit)
build 3: -p celestina          wall 0.06 s  compiled 0
build 4: -p celestina-wayland  wall 0.04 s  compiled 0
build 5: -p celestina          wall 0.06 s  compiled 0
debug/deps: libcelestina_wayland-3dbcddb546d6ed54.rlib and libcelestina_wayland-9696640e86562286.rlib side by side
```

**Conclusion:** yes: cargo keys each unit by its resolved features, both
units stay in `target/`, and alternating builds rebuild nothing (second
build of each set 0.04–0.06 s). Not yet shown: the release profile and the
real `gl` feature with `celestina-lock`/`celestina-lock-verify`; **repeat in
Task 3 step 6** (release builds there are the plan's own step).

## Q19 — niri unlocking on a failed locked frame

```sh
sed 's/\x1b\[[0-9;]*m//g' runs/nest.log | grep -E 'locking session|unlocking session| (ERROR|WARN) '
```

```text
pass 2: 8 "unlocking session" lines, 8 unlock_and_destroy requests by the probes (q0, q4b, q6lock, q16lock, q1a-repl, q1b-repl, q2a-repl, q2b-repl)
pass 1 (all probes died): 0 "unlocking session" lines
ERROR or WARN lines in the whole nest log: 0
```

**Conclusion:** none in 34 lock requests (`locking session` or `refusing lock` lines) across both passes. niri logs
the render-failure unlock with the same info line, "unlocking session", as a
client's unlock (`src/niri.rs`, the `Locking` branch after an output's render
calls `self.unlock()`, which only logs that line), so the log alone cannot
tell them apart; a supervisor tells them apart only by whether its own child
printed `unlocked`. The shell sees the render-failure case as exit 10
(`finished` before `locked`), as §4.3 and TM-30 already assume. The live
journal check stays in VAL.

## Result

| Q | Answer | Changes spec section? |
|---|---|---|
| Q1 | niri ends `Locked` with the dead client after its 1 s deadline, shows the lock colour, and the next client replaces the dead lock; on a replacement `locked` precedes the new cover's `configure` | no (confirms TM-5, R-4; Task 3 must cover on a post-`locked` configure) |
| Q2 | a second client replaces a dead lock at once and its unlock works; `swaylock` is accepted as replacement and covers; its password unlock not executed (author's password) | no (unlock half stays in VAL-CEL2-LOCK-RECOVERY) |
| Q4 | the single-pixel global is test-only in niri; the zeroed shm XRGB8888 cover is the only path and works at scale 1.5 | **yes: §3.1, §4.6, §4.13, T-W1** |
| Q6 | `wl_keyboard.enter` reaches the lock surface already in `Locking`; focus follows the output under the cursor by construction (multi-output live only) | no |
| Q7 | `wtype` against the nest reaches the lock surface (virtual keyboard bypasses binds and grabs) | no (R-7 runs) |
| Q10 | dropping clones never ends the iterator; `close()` or a hand-polled stream with a shared channel waker ends it in < 1 ms, also with a hung bus; no eventfd needed | no (§4.10 may use a channel instead of an eventfd) |
| Q11 | counted input while locked reports `resumed` (virtual pointer shown; physical by source); virtual-keyboard input does not; unlock itself reports `resumed` | no (R-9 must use pointer input) |
| Q16 | an input-method grab outlives the lock's focus change, can be taken while locked, and receives physical keys instead of the lock surface; niri's a11y `KeyboardMonitor` also emits keys while locked; fcitx5 run not executable | **yes: §4.6.1 "Input", §5 TM-15, TM-28 (and the X2 limit)** |
| Q17 | no: logind refuses a delay inhibitor during a pending sleep (`OperationInProgress`) and, with none held at the signal, sleeps at once | no (refines Task 6 error handling) |
| Q18 | both feature sets coexist; alternating debug builds rebuild nothing | no (repeat in release with the real `gl` split in Task 3 step 6) |
| Q19 | zero render-failure unlocks in 34 lock requests; niri logs them with the same "unlocking session" line as a client unlock | no |

## Limits

- **Physical keys were never sent.** The only key source an agent may use is
  the virtual keyboard, which bypasses the paths Q16 and Q11 are about; the
  physical behaviour of Q16 rests on smithay's and niri's source. The author
  can confirm it in the nest with a throwaway input-method client and any
  non-password text, never in the live session.
- **fcitx5 is not installed**; installing it needs root (not executed).
- **Timings are not representative.** The nest window was left where the
  host put it (`--monitor none`); niri's loop in it answered requests on
  roughly one-second ticks (for example `unlock_and_destroy` seen by niri at
  18:25:51.730 and its round trip returning at 18:25:52.730), so the
  0.25–0.98 s "time to `locked`" values above are not Q12 measurements. Q12
  stays with R-1.
- One output only: Q6's "moves with the pointer", Q3 for a hotplugged output,
  and multi-output Q19 stay live (`VAL-CEL2-LOCK-LIVE`, `-HOTPLUG`).
- `swaylock`'s unlock and every PAM authentication: not executed (no
  password typed, no PAM `authenticate` call; `faillock` tally empty before
  and after). `swaylock`'s PAM child did start (its normal start-up) and was
  killed by its pid.
- Q17 is from source and documentation; the live ordering is
  `VAL-CEL2-LOCK-SUSPEND`.
- Q18 used the debug profile and today's feature difference, not the future
  `gl` feature.
- Nothing touched the live session: no client used `wayland-1`, the live
  session bus or the system bus (the busprobe used its own `dbus-daemon`; the
  nested niri, as in every nest run, watches logind and locale1 properties on
  the system bus by itself).

## Follow-up

- Stop point P2: the author accepts, amends or rejects the two contradictions
  (Q4: shm-only cover; Q16: the input-method grab and the a11y keyboard
  monitor, a niri change or a recorded X2 residual) before Task 3.
- Task 3: shm cover only; cover on a `configure` that arrives after `locked`;
  tolerate a `configure` after `finished`; repeat Q18 in release (step 6).
- Task 6: handle `OperationInProgress` from `Inhibit` during `Preparing`;
  pick `close()` or the channel-waker reader for `lock-sleep`.
- Task 7 (R-9): drive idle input with pointer motion, not `wtype`.
