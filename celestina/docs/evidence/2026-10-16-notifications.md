# Evidence: CEL-1-F — notifications: the server, banners, history and do-not-disturb

- **Date:** 2026-10-16
- **Scope:** unit CEL-1-F on `unit/celestina/CEL-1-F`, stacked on CEL-1-D
  (base `84362c57`), five implementation commits `d0cc510d`, `8eea6067`,
  `7450abdb`, `3b93849c` and `ba3a7e42`, the smoke commit `7b1df8ed`, the
  documentation commit `f4552f66` and the fix round that raises the smoke's
  `--exit-after` from 3 to 4 seconds (cold-nest margin) and syncs the banners
  after an output is added:
  `celestina-rs/crates/celestina-shell/src/notifications.rs` (expiry defaults,
  `PausableClock`, `dismiss_all`), `celestina/src/providers/notifications.rs`
  (the bus server, salvaged by copy from
  `celestina-qt/src/provider_adapter/notifications.rs` at
  `c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413`), `celestina/tests/notifications_bus.rs`,
  `celestina/src/notifications/` (state, `banners.rs`, `history.rs`),
  `celestina/src/island/pages/notifications.rs` and `island/mod.rs`,
  `celestina/src/{app,main,message,lib,copy}.rs`, `celestina/clippy.toml`
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins; the private-bus test uses a
  `dbus-daemon` started by the test itself with its own config and socket. No
  run touched the session bus, and `org.freedesktop.Notifications` was never
  claimed outside that private bus.
- **Artifact:** none for this record. The release build, the smoke and the
  production entries are the landing's and are pending (see Result).

## Procedure

From `celestina/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
sh scripts/smoke.sh --binary <target>/release/celestina
```

From `celestina-rs/` (the pure crate was edited by the first commit):

```sh
cargo fmt --all --check
cargo clippy --locked -p celestina-shell --all-targets -- -D warnings
cargo test --locked -p celestina-shell
```

From the worktree root:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

## Result

- **Commit-time results, as the implementation reports record them:**
  - `celestina-rs`: `cargo fmt`, `cargo clippy -D warnings` and
    `cargo test -p celestina-shell` pass (86 lib tests) after the first commit.
  - `celestina`: `cargo fmt --all --check`, `cargo clippy --workspace
    --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`
    pass after each commit. The last recorded count is 153 lib tests. The
    server commit reports 10 new unit tests plus the private-bus integration
    test, the wiring commit 7, the banners commit 13 (11 banner and 2 history
    tests) and the page commit the rest of the 153; the integration test passed
    on each re-run the reports record.
  - Each commit passed the pre-commit hook (language contract, staged-unit
    check) and `commit_scope.py --check`.
- **Verify and smoke:** pending. `complete-production.sh`,
  `verify-production.sh`, the nested-niri `scripts/smoke.sh` run and the
  deploy were not run by this unit's sessions; the controller's landing runs
  them on the tree it commits.
- **Guards:** `check-documentation-contract.sh` and
  `check-language-contract.py` exit 0 on the documentation commit of this unit
  (the documentation guard prints unbounded-pathspec errata for another unit's
  immutable inventory and still reports `Documentation contract: OK`). `check-architecture-contract.sh`
  and `version_tool.py check` were not run by this unit; the landing runs them.

## What was delivered

- **Server.** The worker `notifications` claims `org.freedesktop.Notifications`
  with `DoNotQueue` only, and only after the object is served. If another
  program holds the name, the worker describes the holder (unique name plus
  `/proc/<pid>/comm`, bounded and control characters stripped; `?` when the bus
  refuses), drops its connection and command queue, sends the owner to the
  app, never retries and idles until shutdown. Nothing is served under the
  shell's unique name in that case.
- **Interface.** `Notify` (eight flat arguments, as the specification fixes
  them), `CloseNotification` (emits `NotificationClosed` with reason 3),
  `GetCapabilities` and `GetServerInformation`. `NotificationClosed` and
  `ActionInvoked` are declared, so introspection lists them.
- **Hostile input.** Bodies are bounded to the machine's prefix; actions are
  capped; an `image-path` is kept as text only and is never read; raw pixel
  hints are ignored; long paths are not copied.
- **Expiry.** Low 5 s, normal 10 s, critical never. Hovering the stack pauses
  every notification: the worker's clock does not advance while hovered.
- **Owner on the page.** When another program holds the name, the
  Notifications page shows the owner line with the holder's unique name and
  process, its switch is disabled, and nothing is claimed. The Tray page keeps
  its own owner line (CEL-1-G).
- **Banners.** At most three, newest first, at the top edge of the output
  focused by niri (or the first Island's output when none is). A banner shows
  the icon, the summary on one line, the body excerpt on two lines, an icon-only
  dismiss button, and one button per offered action except `default` (at most
  four). The stack is created with the first banner and destroyed when no toast
  remains.
- **Clicks.** A click on a body runs the `default` action when offered;
  otherwise it opens the Notifications page on that output's Island and closes
  the banner. A button runs its action. The dismiss button closes with reason
  dismissed.
- **Hover.** Entering a banner sends `Hover(true)`; leaving the banners, or the
  pointer leaving the surface, sends `Hover(false)`. A surface that goes away
  while hovered releases the hover, so the clock never stays paused.
- **Actions.** `Invoke` emits `ActionInvoked`, then closes the notification with
  reason dismissed and emits `NotificationClosed` with reason 2. An unknown key
  does nothing.
- **History page.** Live rows first, then the last twenty ended ones, newest
  first. Each row shows the time (`HH:MM` on the same local day, else
  `DD/MM HH:MM`), two lines of body, and the actions of live rows. The page
  offers dismiss-all and the do-not-disturb switch. Viewing the page marks the
  unread rows read.
- **Do-not-disturb.** The setting is persisted through the existing
  `ShellSettings`, sent to the worker at start and on change. While on, banners
  are hidden and only critical notifications stay; history is kept.
- **Commands and verbs.** The existing Shell1 page verbs `celestina island open
  notifications` and `celestina island toggle` open and toggle the page on the
  Island (the toggle is decided by a pure function). The do-not-disturb setting
  is changed from the overflow menu and from the page switch.
- **Command queue.** `NotificationsCommand` (`Close`, `Invoke`, `DismissAll`,
  `SetQuiet`, `ClearHistory`, `Hover`, `MarkRead`) travels over a `SyncSender`
  bounded at 32. A full queue, a stopped worker or a worker that holds no name
  gives the command back; the app logs one warning and then stays quiet.

## Design rulings (from the controller)

- **R1.** `NotificationsEvent` is `Owner(Option<String>)` or
  `Snapshot(NotificationsSnapshot)`. The snapshot is sent whenever the state
  changes, coalesced per tick. The owner string names the holder
  (`:1.42 (noctalia)`), because `copy::notifications_owner` prefixes it.
- **R2.** Commands are `Close(u32)`, `Invoke(u32, String)`, `DismissAll`,
  `SetQuiet(bool)`, `ClearHistory`, `Hover(bool)` and `MarkRead`. `Close` and
  `DismissAll` use reason dismissed; `Invoke` emits `ActionInvoked` and then
  closes with reason dismissed.
- **R3.** Expiry defaults are low 5 s, normal 10 s and critical never. Hover
  pauses the whole stack through the pure `PausableClock` in `celestina-shell`.
- **R4.** `NotificationsSnapshot` carries `toasts` (newest first; the banners
  show at most three), `live`, `history` (the history page's bound), `quiet`,
  `unread` and the wall-clock epoch used for the time labels.

## Limits

Nothing was checked in a real session. The unit tests are pure or run against a
private `dbus-daemon`; the smoke starts the shell and counts frames. Not
exercised: the real Wayland layer and its exclusive zone, the GL pass, the blur
behind a banner, the hover on a real pointer, the click and the dismiss on
a real surface, the keyboard reach of the page, and any coexistence with
Noctalia in the author's session. These are the author's rows VAL-CEL-GLASS,
VAL-CEL-BANNERS and VAL-CEL-NOCTALIA in [VALIDATION.md](../../VALIDATION.md).

## Rulings and known limitations

- **No layer margin.** `LayerSpec` has no margin and the runtime sets zero
  margins. The banner surface uses `exclusive_zone: -1` (it ignores other
  zones) and paints its stack at the Island zone plus `spacing.sm`. The empty
  band above the stack is input-transparent and receives no blur. It may overlap
  another program's exclusive zone, which a real margin would also do. The
  shared `celestina-wayland` was not changed.
- **No exit fade for the last banner.** The surface is destroyed as soon as no
  toast remains, because the frame path has no runtime context to destroy it
  later. Banners that leave while others remain do fade out.
- **Banners are fixed at creation.** Surface size, theme and text scale are
  read when the surface is created and not after. A larger text scale set while
  banners show can clip text inside the fixed heights; nothing overflows the
  input region.
- **The banners stay on their output.** They do not follow focus until the
  stack empties.
- **Dismiss is icon-only.** `BANNER_DISMISS` and the page's dismiss-all label
  exist in `copy.rs`, but the toolkit has no accessible-name channel yet, so
  neither reaches assistive technology.
- **No page button for `default`.** Rows are not clickable as a whole; the
  `default` action is reachable only from a banner body.
- **Epoch label staleness.** Time labels are computed when the snapshot is
  applied. A label `HH:MM` stays until the next change, so it is stale after
  midnight. The arrival time is the epoch plus the posted time, which excludes
  time spent paused by hover.
- **Bus-originated changes arrive on the tick.** A change that comes from the
  bus (a `Notify` or a `CloseNotification`) reaches the loop within one 250 ms
  tick. Commands from the shell are applied at once.
- **Clippy threshold.** `celestina/clippy.toml` sets
  `too-many-arguments-threshold = 9` for the whole workspace, because the
  specification fixes eight flat `Notify` arguments. Folding them into a struct
  would publish a different introspection signature. The alternative is a
  per-item attribute, which the repository forbids.
- **Owner detection is first-come.** The worker claims the name once. If the
  holder exits later, the shell does not retry the claim; a restart does.
- **The page's do-not-disturb switch** is enabled when the shell owns the name
  and disabled while another program holds it.

## Follow-up

- The author's rows VAL-CEL-GLASS, VAL-CEL-BANNERS and VAL-CEL-NOCTALIA in
  [VALIDATION.md](../../VALIDATION.md).
- The tray's owner detection in CEL-1-G.
- A layer margin in `celestina-wayland` if the author wants the stack
  positioned by the surface rather than by the zone offset; a dismiss name when
  the toolkit gains an accessible-name channel.

## Landing

- **Base revision:** `84362c57bf5b42f8760f4ae34716b8cc741dbb49` (CEL-1-D).
  The branch is not based on `main` at `fa1546d3`, where CEL-1-E landed; the
  landing must rebase or order the units.
- **Check:** pending; the landing runs `production_artifact.py check
  celestina --require-verified`.
- **Build:** pending; `verify-production.sh` runs on the landing's tree.
- **Deploy:** pending; `deploy-production.sh` and `status-production.sh` run
  after the push.

## Landing

- **Base revision:** `fa1546d39039957a3570887fc082f7a65aa20aa0`
- **Check:** `production_artifact.py check celestina --require-verified` exit 0: artifact: celestina current; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh
- **Build:** celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:fd1c2b88e8475a462fc5b062a43538e4f75770f7ce93251f64f2f52120fb2876, verification_fingerprint sha256:ffe02560bbcc1003b001fc4e776c8f389698c16a98a1740b1077d4ef1bbed893
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
