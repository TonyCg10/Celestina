# Evidence: the launcher's quick clear (CEL-I-014)

- **Date:** 2026-11-27
- **Scope:** `CEL-I-014`: `celestina/crates/celestina-ui/src/field.rs`, `celestina/crates/celestina-ui/src/input.rs`, `celestina/src/launcher/mod.rs`, `celestina/src/launcher/view.rs`, the launcher's frame call in `celestina/src/app.rs`, the ledger row and `VAL-CEL-I-014`
- **Environment:** worktree `celestina-CEL-I-014` from `main` at `0900b8bc` (celestina 1.29.1), rebased on `82851050`; debug and release builds in private target directories; the nest ran on its own bus
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the launcher preview the author approved on 2026-11-26 (hold Backspace with the clear button), private artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>, source `/home/toni/CODIGO/previews/2026-11-26-launcher/`; [design](../../../docs/superpowers/specs/2026-11-26-celestina-launcher-design.md) §5.3, [plan](../../../docs/superpowers/plans/2026-11-26-celestina-cel-i-014-plan.md)

## Change

- **Word deletion:** `EditOp::DeleteWordBack` and `EditOp::DeleteWordForward` in `celestina-ui`'s field, on Ctrl+Backspace and Ctrl+Delete, for every field of the shell. They delete the selection when there is one, else back to the previous word start or forward to the next word end.
- **Clear button:** a 28 px round button with the `x` glyph at the launcher pill's right end. It fades in with the first character and takes no click while hidden, and a click empties the field and the query. The selector has none.
- **Hold:**
  - Backspace's first press deletes one grapheme. Held for 600 ms (`HOLD_DELAY`), or at a first repeat if one comes sooner, it starts the sweep: a wash of `Role::AccentSoft` that grows from the text's end over 450 ms (`SWEEP`), after which the query empties.
  - Repeats are swallowed, and releasing Backspace cancels the sweep and keeps the text.
  - A Backspace pressed during an input-method composition never sweeps.
  - The selector never sweeps.

## Departure from the plan

- **The hold is timed by the launcher.** The plan started the sweep on the first key repeat, but the shell has no key repeat: `celestina-wayland` creates its keyboard with `get_keyboard(.., None)`, so `repeat_key` never fires. A nest run with `wtype -P a` held for 1.5 s typed a single `a`. The launcher now times the hold itself from the press, with niri's default repeat delay. The design's §3 and §5.3 are amended in `CEL-TRACKS-DOCS-10`.
- **The button only fades.** The preview scaled it in from 0.8, but `View` has no scale.
- **The wash colour.** The wash is `AccentSoft` (accent at 14 %), against the preview's 20 %: the paint crate has no 20 % role.

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked          # 2752 passed, 0 failed
```

- **Test counts.** The counts are the sum of every `test result` line. The new tests failed before their code (RED: missing items at compile time).
- **The new tests:**
  - field: `delete_word_back_removes_to_the_previous_word_start`, `delete_word_forward_removes_to_the_next_word_end`, `a_word_deletion_with_a_selection_deletes_the_selection_only`, `ctrl_backspace_and_ctrl_delete_map_to_word_deletions`;
  - clear button: `the_clear_button_fades_with_the_query_and_the_selector_has_none`, `the_clear_button_empties_the_query_and_keeps_the_launcher_and_the_focus`;
  - hold: `holding_backspace_sweeps_then_clears`, `a_hold_with_no_repeats_sweeps_after_the_delay`, `releasing_before_the_end_keeps_the_text_and_a_new_query_answers`, `repeats_never_delete_letters`, `another_keys_release_keeps_the_hold`, `a_ctrl_backspace_is_not_a_hold`, `a_preedit_blocks_the_hold`, `shift_backspace_deletes_one_and_a_single_press_never_sweeps`, `focus_loss_during_the_sweep_leaves_no_hold_behind`, `a_repeat_on_an_empty_query_starts_no_sweep`, `any_other_key_ends_the_hold`, `the_selector_has_no_sweep`;
  - sweep geometry: `the_sweep_is_a_wash_that_grows_from_the_texts_end_inside_the_pill`.
- **The nest.** It used CEL-I-012's guarded kit:
  - `env -i`, `--own-bus`, fake `wpctl` and `pactl`, and the nest lock;
  - nested niri at scale 1.5 on a 2560 by 1440 logical output;
  - keys typed with `wtype` (held with `-P` and `-p`), the pointer moved by the kit's virtual-pointer client.
  
  The shots are the launcher's pill, cropped.
- **The smoke:** `smoke.sh --binary <release> --only launcher` on a release build in a private target: `smoke: OK (only: launcher)`; the launcher was configured at the whole output, opened and closed.

## Result

- [typed-with-clear.png](./2026-11-27-cel-i-014-quick-clear/typed-with-clear.png): text in the field and the clear button.
- [sweep-mid.png](./2026-11-27-cel-i-014-quick-clear/sweep-mid.png): Backspace held 850 ms; one letter deleted and the wash sweeping the rest.
- [cleared-after-hold.png](./2026-11-27-cel-i-014-quick-clear/cleared-after-hold.png): the field empty after the hold, the button gone.
- [ctrl-backspace.png](./2026-11-27-cel-i-014-quick-clear/ctrl-backspace.png): `alpha beta` after Ctrl+Backspace is `alpha `.
- [released-early.png](./2026-11-27-cel-i-014-quick-clear/released-early.png): Backspace let go at 750 ms; the text stays.
- [clear-button-hover.png](./2026-11-27-cel-i-014-quick-clear/clear-button-hover.png) and [after-clear-click.png](./2026-11-27-cel-i-014-quick-clear/after-clear-click.png): the button's hover plate, then the empty field after a click.
- **Review:** 1 round (`/code-review` at high reasoning, Opus; the implementers were Sonnet):
  - fixed: another key's release ended the hold (key rollover);
  - fixed: the wash could run past the field on a long query with the caret moved left;
  - fixed: a dead assignment;
  - deferred: the shell's missing key repeat (below), and whether the tree animates the wash's width between frames (the nest shots show it tracking).

## Limits

- **What the nest covers.** One output, the nest's keyboard through `wtype`, the author's real keyboard not used; the author's session is `VAL-CEL-I-014`.
- **Frames.** The wash's motion was seen in single shots, not frame by frame.

## Follow-up

- **Key repeat.** The shell has no key repeat anywhere (arrows, letters, Backspace in every field and in the Island). That calls for its own `CEL-I` entry, which asks `celestina-wayland` for the keyboard with repeat and checks the lock's handlers.

## Landing

- **Base revision:** `82851050eb1cd705f9f22cd1dd4e6c72ac6c9502`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases launcher only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:bba02dc7c7cffdfd849505ce71ad0d34397f13f8e5e8a52a028526683e289eea, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
