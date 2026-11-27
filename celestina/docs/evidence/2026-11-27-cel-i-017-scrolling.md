# Evidence: the launcher scrolls (CEL-I-017)

- **Date:** 2026-11-27
- **Scope:** `CEL-I-017`: `celestina/src/launcher/{scrolling,state,selector,view,mod}.rs`, the ledger row and `VAL-CEL-I-017`
- **Environment:** worktree `celestina-CEL-I-017`, rebased on `main` at `4a90c58b` (celestina 1.32.0, with the key repeat of `CEL-I-018`); debug and release builds in private target directories; the nest ran on its own bus
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the launcher preview the author approved on 2026-11-26, private artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>; [design](../../../docs/superpowers/specs/2026-11-26-celestina-launcher-design.md) §5.4, [plan](../../../docs/superpowers/plans/2026-11-26-celestina-cel-i-017-plan.md)

## Change

- **Rows:** the launcher answers with every row instead of eight; the selector keeps eight (`SELECTOR_MAX_ROWS`).
- **Layout:** the rows sit in a clipped area eight rows high, inside an inner column offset by a `celestina_ui::Scroll`. Only the rows in view, plus four on each side, are built.
- **Wheel:** the wheel over the list moves the offset at once. The tree slides nothing for it.
- **Keyboard:** a keyboard move brings the selected row into view with the least change, sliding over 200 ms; a wrap goes to the other end. Typing and clearing put the list back at its top; a new index keeps the offset while the selected row is still there.
- **Scroll bar:** 4 px wide, 2 px from the right edge, in `Role::ControlHover` (the paint crate has no strong-hairline role). It shows while the list moves and for 800 ms after, then fades.
- **Focus ring:** it is drawn only for a row in view, clipped to the list area.

## Departures from the plan

- **Wheel slide.** The wheel's jump stops every slide in flight, not only the inner column's. With the column alone marked still, a keyboard slide in flight kept going after a wheel step.
- **Wheel target.** The area takes the wheel only while the list scrolls. A short list does not light up its empty part under the pointer.
- **Ring.** It follows the row's painted place, so it moves with the row during the slide.
- **Commit subjects.** Two subjects use "Build" and "Make", because the commit guard does not accept the verbs "Lay" and "Scroll".

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked --no-fail-fast   # the shell's library: 2016 passed, 0 failed (126 in launcher::)
```

- **The one failure.** The workspace run fails only `celestina-lock`'s load-sensitive `a_sigterm_right_after_the_spawn_does_not_kill_the_lock`.
- **Red step.** The new tests failed to compile before the change.
- **Review Focus coverage:**
  - the ring out of view: `the_ring_is_none_for_a_row_out_of_view_and_clipped_for_a_half_visible_one`;
  - typing and a new index: `typing_resets_the_offset_but_a_new_index_keeps_it`;
  - the wheel outside the list: `a_wheel_over_the_field_or_the_tab_row_does_not_scroll`;
  - a half-hidden row: `a_click_on_a_half_hidden_row_launches_that_row`;
  - held Down: `holding_down_through_three_hundred_rows_restarts_the_slide_each_time`.
- **The nest:** the guarded kit, nested niri at scale 1.5, `LANG=es_US.UTF-8` and the author's own desktop files, about 50 of them without his Flatpaks. The probe scrolled with the virtual pointer's wheel (120 px), pressed Down twelve times, held Down for 1.4 s (the key repeat of `CEL-I-018`) and typed a letter.
- **The smoke:** `smoke.sh --binary <release> --only launcher` on a release build in a private target: `smoke: OK (only: launcher)`.

## Result

- [top.png](./2026-11-27-cel-i-017-scrolling/top.png): the list at its top, eight rows, no bar.
- [wheel.png](./2026-11-27-cel-i-017-scrolling/wheel.png): after the wheel, three rows further down, with the bar at the right edge. The lit row is the one under the pointer.
- [wheel-bar-gone.png](./2026-11-27-cel-i-017-scrolling/wheel-bar-gone.png): 1.3 s later, the bar has faded.
- [down12.png](./2026-11-27-cel-i-017-scrolling/down12.png): after twelve Downs, the selected row is at the bottom of the area, with its ring inside the card.
- [held-down.png](./2026-11-27-cel-i-017-scrolling/held-down.png): after Down was held, the selection walked further down the list and stayed in view.
- [typed.png](./2026-11-27-cel-i-017-scrolling/typed.png): one letter typed, the list back at its top.
- **Review:** 1 round (`/code-review` at high reasoning, Opus; the implementers were Sonnet). Nothing blocking. Noted:
  - every keystroke clones each matching entry, which is cheap at today's index size;
  - the hover highlight stays on the old row after a wheel jump until the pointer moves, as in the sheets;
  - Flatpak entries matched by their long `Exec` now reach the list's tail (`CEL-I-015`'s follow-up).

## Limits

- **Motion.** The slide and the fade were seen in single shots, not frame by frame.
- **Wheel steps.** The real mouse's wheel steps were not used; the virtual pointer sent one 120 px step.

## Follow-up

- Match only a Flatpak entry's command name at the lowest search tier (carried from `CEL-I-015`).

## Landing

- **Base revision:** `44bf26a0b18ff24e7dc35728ad06a35f686393be`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases launcher only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:95426f6c9241651d154735b52d23a4b202049b4084e190bf78870cf5cd1b5058, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
