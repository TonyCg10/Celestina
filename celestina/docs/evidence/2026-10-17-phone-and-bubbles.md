# Evidence: CEL-2-J — the phone and bubbles duties

- **Date:** 2026-10-17
- **Scope:** unit CEL-2-J in the session worktree, not yet landed:
  `celestina-rs/crates/celestina-shell/src/{melibea,phone,settings,lib}.rs`,
  `celestina/src/{phone,bubbles}/mod.rs`,
  `celestina/src/providers/{melibea,magnetita,duty}.rs`,
  `celestina/src/pieces/{phone,bubbles}.rs`,
  `celestina/src/island/pages/phone.rs`, `celestina/src/launcher/selector.rs`,
  additive hunks in `celestina/src/{app,command,message,copy,main,lib}.rs`,
  the Island (`island/mod.rs`, `island/view.rs`, `island/pages/tabs.rs`),
  `celestina/scripts/{smoke.sh,fake-melibea.py}`, `celestina/Cargo.toml`,
  and the documents of this unit
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins; the cargo commands ran from
  `celestina/` and `celestina-rs/` with `--locked` and a private
  `CARGO_TARGET_DIR`. Nothing is committed; the landing computes the inventory
  and seals the artifact.
- **Artifact:** none sealed by this record; the landing builds and seals its own
  (`verify-production` is the landing's, not run here)
- **Plan:** [2026-10-17-phone-and-bubbles-plan.md](2026-10-17-phone-and-bubbles-plan.md);
  decisions in [ADR 0020](../decisions/0020-phone-and-bubbles.md)

## What was built

- **Pure domain (`celestina-shell`).** `melibea.rs` is a copy by salvage of the
  halted core's (head comment records path and commit): protocol versions 1
  and 2, the ordered projection, request building, bounded text. `phone.rs`
  reduces Magnetita's device rows to the phone to show (first connected
  `phone`, else first connected device, else absent; battery clamped; text
  bounded). `settings.rs` gained the duties `phone` and `bubbles`, off by
  default; a file from before reads with both off and an unknown value reads
  as off.
- **Workers.** `providers/melibea.rs`: guarded socket (D2), bounded line reads
  (64 KiB), subscription with snapshot and gapless changes, resync on a gap,
  backoff 0.5 s to 30 s, v1/v2 degrade (D1), a bounded action queue with
  short-lived connections, and a confirmation that waits for the authoritative
  revision. `providers/magnetita.rs`: `ListDevices`, `Changed` and owner-change
  signals on the session bus, coalesced reconciliation, `MediaAction` (and
  `Ring`, see debts) only for a device it listed, bounded dictionary parsing,
  `NameHasOwner` before any call. `providers/duty.rs`: the on/off life of one
  duty's worker with run numbers.
- **Wiring and surfaces.** `phone/mod.rs` and `bubbles/mod.rs` hold the
  handles that follow `duties.*` live; the phone piece and the Phone page of
  the Island; the bubbles piece (one bubble per window, overflow bubble,
  anchored restore); the selector as a bubbles source of the launcher's
  overlay; the verbs `bubbles select`, `bubbles minimize`, `phone media` and
  `island open phone`. All words are Spanish through `copy.rs`.
- **Smoke.** Three nested runs (metrics, verbs, absent) with both duties on
  against `fake-melibea.py` in a private runtime directory and a private bus
  where no Magnetita runs.

## Decisions as they ended

- **D1 — protocol.** v1 by default, v2 only for a request that carries a
  transition hint, degrade to v1 for the connection's life on
  `incompatible_version`. The anchor is sent whenever the bubble's rectangle
  is usable and the model does not name another output for the window. The
  window is usually not in niri's model (Melibea keeps it), so the ordinary
  case is an unknown output and the anchor goes; Melibea or niri degrade it.
  When the model does name another output, no anchor is sent (`disabled`
  under reduced motion). A reply whose status confirms nothing
  (`window_not_found`, `blocked`) is accepted with `window_id: null` (niri's
  own answer, forwarded by Melibea) or the requested id and settles as a
  refusal; a confirming status must name the window it acted on.
- **D2 — guard.** The shell reads only the socket. A symbolic link is always
  refused. Absent is a state; reconnection backs off. Journal lines are one per
  change of state, never per attempt.
- **D3 — selector.** As in the ADR. `Delete` asks the window to close only
  while the search field is empty or its caret is at the end with nothing
  selected; anywhere else it edits the text. A second `Delete` on a window
  with a pending close asks for nothing until that action settles, the
  projection drops the window, or the link is withdrawn.
- **Phone page, no tab.** The Phone page never has a tab (author's decision,
  delegated to the implementer): the nine tabs of main plus a tenth do not fit
  the row (`no_tab_label_is_wider_than_its_pill` failed with the duty on). The
  page opens from the phone piece and from `island open phone`, which is
  accepted only while `duties.phone` is on (otherwise a no-op with a journal
  line); turning the duty off while the Phone page is open moves the Island to
  Activity. With the page open no tab is selected. `shown_pages()` and
  `tabs()` lost their `phone_duty` parameter, as did `view::page`.

## Review findings applied in this pass

1. `exchange` treats `window_id: null` on a non-confirming status as a
   refusal, not a mismatch (new test with `window_id: null` for restore,
   focused minimise with no focus, and a confirming status with a null id,
   which still fails).
2. `Delete` capture rule and close dedupe (tests in `launcher/mod.rs` and
   `bubbles/mod.rs`).
3. `island open phone` gated by the duty (test in `island/mod.rs`); the Phone
   page has no tab (tests in `pages/tabs.rs` and `island/view.rs`).
4. `DutyWorker::set(true)` no longer joins on the loop: reapers are collected
   and joined only by `stop`; `run` is assigned before `start`; the module doc
   says what is true (an old and a new worker may briefly coexist, both only
   clients). Tests: off-to-on with a worker that ignores its flag returns in
   under 500 ms, a failed start, and rapid toggling.
5. Anchor: `restore_window` drops the rectangle when the model knows the
   window on another output (`bubbles::window_output`, `anchor_allowed`;
   test). Implemented, so the ADR and plan describe the code.
6. `VALIDATION.md`: VAL-CEL2-PHONE and VAL-CEL2-BUBBLES require
   `duties.bar: "on"` and the ids `bubbles` and `phone` in `bar.end` (field
   names checked against `BarChoice` in `settings.rs`); the nonexistent
   "connection row" is gone (a disconnected phone leaves no piece); the
   journal lines are the ones the code writes (`melibea: unavailable|absent`,
   `bubbles: Melibea is unavailable|absent`, `melibea: subscribed`).
7. `smoke.sh` `overlay()` uses the nest's niri binary (`CELESTINA_NEST_NIRI`,
   else the patched build, else `niri`), resolved as `dev-session.sh` does.
8. `PHONE_PREVIOUS`, `PHONE_PLAY_PAUSE` and `PHONE_NEXT` were unused and are
   removed.
9. Plan corrected (ADR 0020, three nested runs, a symlink is always refused).

## Procedure

From `celestina/` (private target directory):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --locked --release
sh scripts/smoke.sh --binary <target>/release/celestina
```

From `celestina-rs/`: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets --locked -- -D warnings`, `cargo test -p celestina-shell
--locked`. From the worktree root: `bash scripts/check-architecture-contract.sh`,
`python3 scripts/check-language-contract.py`,
`bash scripts/check-documentation-contract.sh`.

## Result

- **`cargo fmt --all --check` and `cargo clippy ... -D warnings`
  (`celestina/`):** clean. Same two in `celestina-rs/`: clean.
- **`cargo test --workspace --locked` in `celestina/`:** after the second
  rebase, onto main at `be9e0c3d` (CEL-2-C, the OSD duty), 1334 passed,
  0 failed, 1 ignored (pre-existing): the `celestina` library 1061,
  `tests/cli.rs` 3, `tests/connectivity_bus.rs` 11,
  `tests/notifications_bus.rs` 1, `tests/wallpaper_decode.rs` 26 (1 ignored),
  `celestina-paint` 51 plus 2, `celestina-ui` 98 plus 7 and 9, and
  `celestina-wayland` 65. One earlier run, before that rebase, of
  `connectivity::tests::failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life`
  failed on a timing assertion (`started() == 0` at `connectivity/mod.rs`),
  then passed in three consecutive runs; it is a connectivity test (not this
  unit's) that races a lane thread, recorded here as a possible flake.
- **`cargo test -p celestina-shell --locked` in `celestina-rs/`:** 316 passed,
  0 failed (library 313 plus 3). Its fmt and clippy are clean.
- **`check-architecture-contract.sh`:** `Architecture contract: OK`. It also
  prints token-literal lines for `celestina-qt/` and `siderita/` QML, which
  belong to other projects and are not this unit's.
- **`check-language-contract.py`:** `Language contract: OK (131 legacy
  file(s) ratcheted)`.
- **Release binary:** built from this worktree; `strings` finds the new
  `the phone duty is off` diagnostic (2 matches), so it is this tree's code.
- **Smoke:** exit 0, `smoke: OK`, on the release binary of this tree. The phone and bubbles run read `winit frames=16 rest_frames=0 late=0`, `Melibea subscribed with one window, Magnetita absent`, `bubbles selector opened and closed; island open phone and phone media answered`, `fake Melibea answered 2 requests and removed its socket` and `missing Melibea socket logged absent once`. Re-run after the rebase and the tab decision, and again after the rebase onto CEL-2-C (every run of main, the two OSD runs included, passed: `osd on: surface created and destroyed`, `osd off: both verbs rejected, no tool called`): same result for this unit's three runs; the nest's niri was the one `dev-session.sh` resolves, and `overlay()` now uses the same.
- **`check-documentation-contract.sh`:** `Documentation contract: OK`. The 59 `erratum` lines it prints are all pre-existing ones of `siderita/docs/inventories/2026-08-04-shared-reading-surface/SID-G7-D.numstat.tsv` (an inventory with an unbounded Pathspec, immutable); none concerns this unit.

## Debts and deviations

- `celestina/Cargo.toml` gained the `process` feature of `rustix` (for
  `getuid`, used by the socket's owner check and the worker's own uid).
- `providers/duty.rs` duplicates the `Slot`/`Reaper` shape of
  `energy/mod.rs`; the energy handle was not touched. Unifying them is a
  later refactor.
- The bounded line reader duplicates the one in `providers/niri.rs`; a shared
  owner is not extracted yet (two readers with different framing limits).
- The anchored transition was never run against a real niri. The tab row
  needs no measurement for the Phone page any more: it has no tab (see ADR
  0020); the nine tabs fit as on main, which the real-font test checks.
- Four percentage formats coexist in `copy.rs`: `percent` ("23%"),
  `battery_percent` (the Power page's battery row title), CEL-2-C's
  `osd_percent` (the level, a space and the sign) and this unit's
  `charge_percent` (the level, a no-break space and the sign). Unifying them
  is debt, not done here.
- Renumbering: this unit's ADR was 0017 before the rebase and is 0020 now
  (main took 0017 to 0019 for the connectivity pieces); `charge_percent` was
  `battery_percent` before the rename that avoided the collision with
  connectivity's function of that name.
- `Ring` is wired in the Magnetita worker, for a listed device only, but has
  no caller and no verb: a future capability.
- Magnetita is never activated by the shell: `NameHasOwner` is asked first and
  the repository ships no D-Bus activation file for `org.celestina.Devices1`
  (only the systemd user unit `magnetita/magnetitad.service`).
- With more than 64 minimised windows, or a snapshot over 64 KiB, the link is
  `Unavailable` in a loop with backoff. This is a deliberate bound for hostile
  input, not a truncation.
- Residual TOCTOU between the socket checks and `connect`, and `connect` has
  no timeout of its own (reads and writes do). The guard is against accidents
  of the same user, not an adversary who can replace the socket.
- `label` of pieces and the accessible names (`Piece::name`, the bubbles'
  restore names) have no assistive-technology consumer: the toolkit has no AT
  channel yet. This is prior debt of the whole suite, not new in this unit.

## Limits

Nothing was run against the real niri, the real Melibea or the real Magnetita:
every test uses a fake Melibea socket, a private bus and a fake service. Not
proven: Wayland behaviour, real keyboard handling of the selector, the focus
return after it closes, AT-SPI, the anchored transition with the patched niri,
Noctalia's widget beside the piece, and a Melibea that speaks only v1 (only a
fake). These are the author's rows VAL-CEL2-PHONE, VAL-CEL2-BUBBLES and
VAL-CEL2-BUBBLES-KEYS in [VALIDATION.md](../../VALIDATION.md).
`verify-production.sh` and the deploy were not run: the landing does.

## Landing

- **Base revision:** `be9e0c3d9f3002d999edb1db167ff7f437bed9cd`
- **Check:** `production_artifact.py check celestina --require-verified` exit 0: artifact: celestina current; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh
- **Build:** celestina-rs verify: verify-production.sh exit 0, manifest source_fingerprint sha256:83ca76f2d62d2367c4cf9d33192c03a83e61553a11cf46ec8b73116fb9c4e3ad, verification_fingerprint sha256:8880b837e2f866fc9b8b9c990af4d2e36bb8e56ed0af4a243cfcfcd4e2b51b3c
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
