# CEL-I-014 Implementation Plan: the launcher's quick clear

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ctrl+Backspace and Ctrl+Delete delete a word in every shell field; holding Backspace in the launcher sweeps and then empties the query; a clear button sits in the pill while there is text.

**Architecture:** The word deletions are two new `EditOp`s in `celestina-ui`'s field. The hold is launcher state: `Launcher::handle` already sees every key before the field, so it swallows Backspace repeats and runs a 450 ms sweep that `frame` advances. The button is one more node of `view::card`.

**Tech Stack:** Rust, `celestina-ui` (field, input), `celestina/src/launcher`.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-launcher-design.md` §5.3 and §9 call 3. Approved preview: `/home/toni/CODIGO/previews/2026-11-26-launcher/index.html` (artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>).

**Branch:** `unit/celestina/CEL-I-014`, opened with `scripts/worktree.sh open celestina CEL-I-014` from `main`. Kind `milestone`.

## Global Constraints

- English everywhere except `celestina/src/copy.rs`; no Spanish in comments, tests or docs.
- No new dependency.
- Sweep: `Role::AccentSoft` (the accent at 14 %; the paint crate has no 20 % selected-wash role, and the preview's 20 % is within one step of it); 450 ms, linear; vertical insets 10 px inside the 48 px pill; it grows from the text's end towards its start.
- Clear button: 28 × 28, radius 14, glyph `x` 16 px tinted `Role::TextLo`, pointer lift as any interactive plate, at the pill's right end, its right margin −8 px. It fades in and out over `theme.motion` fast (100 ms). The preview's scale from 0.8 is not available (`View` has no scale), so the button only fades; record this in the evidence.
- The selector (`Mod+K`, `Source::Bubbles`) gets neither the button nor the sweep; only the field's Ctrl word deletions reach it.
- Escape still closes the launcher at once.
- Live-session safety: never run `celestina` or `niri msg` against the live session; nests only, through `celestina/scripts/dev-session.sh --own-bus`, as `celestina/AGENTS.md` says.
- Checks from `celestina/`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`.

## Review Focus

1. A Backspace press with Shift held still deletes one grapheme and never starts a hold (Shift is not Ctrl, Alt or Logo; the hold ignores Shift as the field does).
2. A focus loss or a close during the sweep leaves no timer behind: the next open starts with no hold.
3. A sweep over text that the input method is composing (a preedit) does not clear a half-typed composition silently: the hold does not start while `field.preedit()` is `Some`.
4. Ctrl+Backspace with a selection deletes the selection only, not the selection plus a word.
5. A query typed after a cancelled sweep answers normally (the hold resets on release).

---

### Task 1: word deletions in the field

**Files:**
- Modify: `celestina/crates/celestina-ui/src/field.rs` (`EditOp`, `Field::edit`, tests)
- Modify: `celestina/crates/celestina-ui/src/input.rs` (`key_to_edit`, tests)

**Interfaces:**
- Produces: `EditOp::DeleteWordBack` and `EditOp::DeleteWordForward`.

- [ ] **Step 1: Write the failing tests** in `field.rs`'s tests:

```rust
#[test]
fn delete_word_back_removes_to_the_previous_word_start() {
    let mut f = Field::new();
    f.set_text("alpha beta gamma");
    assert!(f.edit(EditOp::DeleteWordBack));
    assert_eq!(f.text(), "alpha beta ");
    assert!(f.edit(EditOp::DeleteWordBack));
    assert_eq!(f.text(), "alpha ");
    f.edit(EditOp::Home { select: false });
    assert!(!f.edit(EditOp::DeleteWordBack), "nothing before the cursor");
}

#[test]
fn delete_word_forward_removes_to_the_next_word_end() {
    let mut f = Field::new();
    f.set_text("one two");
    f.edit(EditOp::Home { select: false });
    assert!(f.edit(EditOp::DeleteWordForward));
    assert_eq!(f.text(), " two");
}

#[test]
fn a_word_deletion_with_a_selection_deletes_the_selection_only() {
    let mut f = Field::new();
    f.set_text("abc def");
    f.edit(EditOp::Left { select: true, word: false });
    assert!(f.edit(EditOp::DeleteWordBack));
    assert_eq!(f.text(), "abc de");
}
```


In `input.rs`'s tests: a Key event `XKB_KEY_BACKSPACE` with `ctrl: true` maps to `EditOp::DeleteWordBack`, with `ctrl: true, alt: true` to nothing, and `XKB_KEY_DELETE` with `ctrl` to `DeleteWordForward`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p celestina-ui --locked delete_word`
Expected: FAIL (no variant `DeleteWordBack`).

- [ ] **Step 3: Implement.** Add the two variants after `Delete`, documented like their neighbours ("Deletes the selection, else back to the previous word start" / "…forward to the next word end"). In `Field::edit` they end the preedit, delete the selection if any, else delete `word_before()..cursor` or `cursor..word_after()`; return whether the text changed, as `Backspace` does. In `key_to_edit` add `XKB_KEY_BACKSPACE if m.ctrl && !m.alt && !m.logo => Some(EditOp::DeleteWordBack)` and the Delete twin, before the `plain` arms.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p celestina-ui --locked`
Expected: PASS.

- [ ] **Step 5: Commit** `git commit -m "Add Ctrl+Backspace and Ctrl+Delete word deletion to the text field"`

### Task 2: the clear button

**Files:**
- Modify: `celestina/src/launcher/view.rs` (`card`, new `clear_key()`, tests)
- Modify: `celestina/src/launcher/mod.rs` (`handle`: the click)

**Interfaces:**
- Produces: `pub fn clear_key() -> Key` (`Key::named("launcher-clear")`).

- [ ] **Step 1: Write the failing tests.**
  - In `view.rs`: with an empty query the pill's last child is the clear node at opacity 0 and not interactive; after `state.set_query("ab", &index)` it is interactive at opacity 1, 28 × 28, radius 14, holding a 16 px `IconKey::Semantic("x")` tinted `Role::TextLo`. A selector state (`LauncherState::selector(..)`) has no node keyed `clear_key()`.
  - In `mod.rs`'s tests: type "ab" (the existing test helpers drive keys), deliver `UiEvent::Click(view::clear_key())` through the pointer path the tests already use for rows, then assert the field's text and `state().query.text()` are empty, the launcher did not close, and the field still holds the focus.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p celestina --locked launcher::`
Expected: FAIL.

- [ ] **Step 3: Implement.** In `card`, for `Source::Apps` only, push the button after the field: a centred box, `.size(28.0, 28.0).radius(14.0).interactive()`, `.opacity(if query empty {0.0} else {1.0})`, non-interactive while empty. In `handle`'s `UiEvent::Click(key)` arm, before the row lookup: on `view::clear_key()` set the tree field's text to "" (`tree.field_mut(view::FIELD)`), call `state.set_query("", index)`, clear `keyboard_moved`, mark stale. The fade rides the tree's opacity animation.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p celestina --locked launcher::`
Expected: PASS.

- [ ] **Step 5: Commit** `git commit -m "Add the launcher's clear button"`

### Task 3: hold Backspace to clear

**Files:**
- Modify: `celestina/src/launcher/mod.rs` (`Launcher` fields, `handle`, `frame`, tests)
- Modify: `celestina/src/launcher/view.rs` (`card` takes the sweep)
- Modify: `celestina/src/app.rs` (the `frame` call site, ~line 3173, passes `&self.index`)

**Interfaces:**
- Consumes: Task 2's clear path (reuse one private `fn clear_query(&mut self, index: &[DesktopEntry])` for both).
- Produces:
  - `pub const SWEEP: Duration = Duration::from_millis(450);` in `mod.rs`.
  - `enum Hold { Idle, Pressed, Sweeping { since: Instant } }` (private).
  - `Launcher::frame(&mut self, cx, painter, reduced, index: &[DesktopEntry])`: the new last parameter.
  - `view::card(theme, state, sweep: Option<Sweep>)` where `pub struct Sweep { pub progress: f32, pub text_end: f32 }`, and `view::launcher` gains the same parameter.

The rules (the spec's §5.3):

| Event | Hold before | Effect | Hold after |
|---|---|---|---|
| Backspace press, no Ctrl/Alt/Logo, no preedit | `Idle` | passes to the field (deletes one grapheme) | `Pressed` |
| Backspace press (a repeat) | `Pressed` | swallowed; the sweep starts | `Sweeping { since: now }` |
| Backspace press (a repeat) | `Sweeping` | swallowed | unchanged |
| Backspace release | any | none (a sweep is cancelled; the text stays) | `Idle` |
| `frame` with `now - since >= SWEEP` | `Sweeping` | `clear_query` | `Idle` |
| any other key press, focus loss, a close | any | as today | `Idle` |
| query empty when the repeat comes | `Pressed` | swallowed, no sweep | `Pressed` |

- [ ] **Step 1: Write the failing tests** in `mod.rs` (drive `handle` with `InputEvent::Key` presses and releases and a fake `now`; the existing tests already build a `Launcher` without a surface):
  - `holding_backspace_sweeps_then_clears`: type "steam", press Backspace (text "stea"), press again (a repeat): text still "stea" and the hold is sweeping; call the frame-side step with `now + SWEEP` (factor the advance into a `fn advance(&mut self, now: Instant, index: &[DesktopEntry]) -> bool` that `frame` calls, and test that) → text "" and query "".
  - `releasing_before_the_end_keeps_the_text`: as above, release at `now + 200 ms`, advance at `now + SWEEP` → text "stea".
  - `repeats_never_delete_letters`: press then five repeats → text one grapheme shorter, not six.
  - `a_ctrl_backspace_is_not_a_hold`, `a_preedit_blocks_the_hold`, `shift_backspace_deletes_one` (Review Focus 1 and 3).
  - In `view.rs`: `card(.., Some(Sweep { progress: 0.5, text_end: 120.0 }))` has a box keyed `launcher-sweep` inside the pill, absolute, 60 px wide, its right edge at the field's left + 120, top 10 and height 28, filled with the sweep role; `None` has no such node.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p celestina --locked launcher::`
Expected: FAIL.

- [ ] **Step 3: Implement.** In `handle`, add an arm before the generic `InputEvent::Key` arm for `XKB_KEY_BACKSPACE` (const `0xff08`) presses and releases, following the table. Releases arrive as `InputEvent::Key { pressed: false, .. }`. `advance` computes the progress `(now - since) / SWEEP` clamped to 0..=1, marks the view stale while sweeping, and returns whether a frame is still due. `frame` calls it first and requests a redraw while sweeping. `text_end` is the field's text width less `scroll_x()`, from `tree.field(FIELD)` (`field.run().map(|r| r.size().0)`, as `celestina-ui/src/paint.rs` measures the field). Reset the hold on `FocusLost`, on any action that closes, and in `with_state`.

- [ ] **Step 4: Run all checks**

Run from `celestina/`: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked`
Expected: all pass.

- [ ] **Step 5: Commit** `git commit -m "Add the launcher's hold-Backspace sweep that clears the query"`

### Task 4: evidence in a nest

- [ ] Build to a private target dir (`CARGO_TARGET_DIR=celestina/target/cel-i-014`), run one nest with `celestina/scripts/dev-session.sh --own-bus --once cel-i-014 --probe PROBE --binary PATH` under `env -i` with `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0` and a fake `wpctl`/`pactl`, bounded by `timeout -k 60 2700`. The probe opens the launcher (`launcher toggle`), types, holds Backspace through `wtype` or the nest's virtual keyboard, and takes `grim` shots: text with the button, the sweep half way, the empty field.
- [ ] Run `celestina/scripts/smoke.sh --binary PATH --only` with the launcher's phase ids (`smoke.sh --list`).
- [ ] Write the evidence from the template, with the screenshots under `docs/evidence/`, the preview linked, and the 0.8-scale omission stated.
