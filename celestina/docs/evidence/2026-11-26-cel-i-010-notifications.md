# Evidence: removing notifications and clearing the list (CEL-I-010)

- **Date:** 2026-11-26
- **Scope:** `CEL-I-010`: `celestina/src/island/pages/notifications.rs`, `celestina/src/island`, `celestina/src/app.rs`, `celestina/src/copy.rs`, the notifications worker and domain (task 1), their tests, the ledger row and `VAL-CEL-I-010`
- **Environment:** worktree `celestina-CEL-I-010` from `main`
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** notifications preview variant A (x always visible, "Borrar todo" bar), approved by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The nested look ran through the guarded kit: `dev-session.sh --own-bus --once
--probe`, started under `env -i` with `CELESTINA_DDC=0`,
`CELESTINA_BACKLIGHT=0` and fake `wpctl` and `pactl`, a private target
directory for the binary (a shared one was overwritten by another worktree's
build between two runs; the shots below come from a binary checked to hold the
new copy). The probe ran inside the nest and refused to run on the desktop's
display, socket or bus. It posted four notifications with `notify-send` on the
nest's own bus (two with `-t 1`, which ended into the history, then two with
`-t 0`, which stay live), clicked the bell with a virtual pointer
(`zwlr_virtual_pointer_manager_v1`), sent Tab with `wtype`, and shot the nest
with `grim`; a burst of seven shots followed each removal. Shots are cropped
to the window.

## Result

- **Four rows:** [rows](./2026-11-26-cel-i-010-notifications/rows.jpg), two live and two ended, each with its x at the right of the time, the "Borrar todo" bar above them
- **Hover and focus of the x:** [hover](./2026-11-26-cel-i-010-notifications/hover-close.jpg) shows the hover plate on the x of a finished row; [focus](./2026-11-26-cel-i-010-notifications/focus-close.jpg) shows the focus ring on the x of the last row after seven Tab presses
- **Remove one:** [fold](./2026-11-26-cel-i-010-notifications/remove-fold.jpg) four frames of the press on the last row's x: the row is gone and the window's bottom edge rises; [after](./2026-11-26-cel-i-010-notifications/after-remove.jpg) the settled list
- **Clear all:** [clear-all fold](./2026-11-26-cel-i-010-notifications/clear-all-fold.jpg) (retaken after the final review): the press; then the "Borrar todo" bar already gone, the keyboard focus on the do-not-disturb switch, the last rows still fading above "Sin notificaciones"; then the window narrowing; then settled; [empty](./2026-11-26-cel-i-010-notifications/empty.jpg) the settled window at its smallest width
- **Bell:** [bell](./2026-11-26-cel-i-010-notifications/bell.jpg) the bell with its dot while the page is closed, again while the window opens over a fresh notification, and without the dot once the window is open
- **Tests:** `every_row_has_a_close_button`, `close_on_a_finished_row_sends_remove`, `clear_all_bar_shows_with_any_row`, `clear_all_asks_to_clear_the_whole_list` (the app sends one `Clear`: the worker closes what is live and empties the history in one snapshot) and `reduced_motion_removes_rows_at_once`; the pinned view snapshot of the page was updated. `cargo test --workspace --locked` passes
- **Motion:** the page uses the toolkit's own leave and slide. The removed row fades out of the layout (`Exit` step, 150 ms) while the rows below slide up (`Normal`, 200 ms, standard curve), and the window follows its content on the existing morph. Clear-all staggers the rows by 30 ms in the Island (`presented_at`, test `clear_all_staggers_the_rows_by_30_ms`).
- **Verb:** `celestina notifications clear` sends the same `Clear` as the page. Task 1 sent `ClearHistory` before `DismissAll`, which left the dismissed notifications in the history; the final review replaced both with `Clear`.

## Limits

- The preview fades a row in 100 ms; the page keeps the toolkit's 150 ms step (the controller accepted it).
- The close buttons expose their accessible name (`BANNER_DISMISS`) only through `button_name`: the repo has no accessible-name channel yet, so nothing reads it.
- Under reduced motion the surviving rows step at once, and the leaving row still fades: the toolkit keeps opacity fades. With reduced motion, or fewer than five shown rows, the clear-all bar's 150 ms fade overlaps the empty label's entry for a moment (the controller accepted it).
- When the worker refuses `Clear` (a full queue), the rows a clear-all hid stay hidden while the history holds them, and a second clear-all before the worker answers shows the first one's rows again; both need a full command queue (the controller accepted them).
- The shots show the shape of the motion, not its timing: burst frames sit about 95 ms apart; the retaken clear-all burst caught the rows mid-fade.
- Nobody shot a removed banner or a clear-all with a banner on screen; the worker's tests cover both closes.
- `connectivity::tests::failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life` failed once in a full workspace run and passed alone and in the rerun.

## Follow-up

None.

## Landing

- **Base revision:** `f3e29a4ff1e873493534f20ba9bdfd48a3e8fa97`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:1b5b84f75520b85c8d8fbb47a636bba4a7123e377bab4908265811b4b7422440, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cae970b6fc5bd637fbcbe76679413818055486aeddb1360c69c665ac3ab81ddb, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
