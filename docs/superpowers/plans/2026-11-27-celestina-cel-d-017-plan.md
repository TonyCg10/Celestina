# CEL-D-017 Implementation Plan: a menu opens from its icon

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A capsule menu window on the end or start capsule opens next to its piece (rule B, design §5.8) instead of at the capsule's outer edge.

**Architecture:** `sheet::geometry::window` takes the piece's rect and places the window from it; every caller already holds the piece's pill in its `Anchor`. Nothing else moves: the drop opens from the piece into the new rect, and the travel goes between two such rects.

**Tech Stack:** Rust, `celestina/src/sheet/geometry.rs`, `celestina/src/sheet/mod.rs`, `celestina/src/island/mod.rs`.

**Spec:** `docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md` §5.8. Approved preview: rule B of the menu placement preview, version 1, chosen by the author on 2026-11-27 (private artifact; source `cel-d/preview-pending/d013/src18.html` in the sessions' notes).

**Branch:** `unit/celestina/CEL-D-017`, opened with `scripts/worktree.sh open celestina CEL-D-017` from `main` (celestina 1.38.0, `be89dd0a`, or later). Kind `milestone`. Size: small, executed in the session that runs it.

## Global Constraints

- **End capsule (`Edge::End`):** `x = min(max(piece.x − 16, capsule.x), capsule.x + capsule.w − w)`. A window wider than its capsule gets `capsule.x + capsule.w − w`, as today.
- **Start capsule (`Edge::Start`):** `x = max(min(piece.x + piece.w + 16 − w, capsule.x + capsule.w − w), capsule.x)`. A window wider than its capsule gets `capsule.x`, as today.
- **Centre (`Edge::Centre`):** unchanged.
- The 16 px offset is the existing `SIDE_MARGIN` value; name it `PIECE_LEAD: f32 = 16.0` beside it in `geometry.rs`.
- The screen's side margins, the narrowing of a window wider than the surface, `WINDOW_TOP` and `max_height` are unchanged.
- A non-finite piece rect counts as the capsule's outer edge (today's placement).
- No `unwrap`/`expect` in production; English only; never touch `src/lock`, `src/polkit`, `celestina/scripts/`, `.githooks`, any `AGENTS.md` or `docs/contracts|governance|standards`.
- Nested runs only through the guarded kit (`nest_guard`), `env -i`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`, fake `wpctl`/`pactl`, a private Cargo target under `~/.cache`, one nest at a time, `timeout -k 60 2700` (never `-s KILL`, never `pkill` by name). Never run a `celestina` binary or `niri msg` from the agent's own shell.

## Review Focus

1. **The end piece:** the session piece at the capsule's right end keeps today's place (the window's right edge on the capsule's). Test in Task 1.
2. **A piece that moves while its menu is open** (a level's text grows): the target follows the live pill each frame, as the anchor already does; the window glides on the existing resize, never jumps. Test in Task 1.
3. **A switch between two pieces:** the travel of `CEL-D-015` goes from the first rect to the second. Test in Task 1.
4. **The tray chevron's window** (`Edge::Centre`) is unchanged. Test in Task 1.
5. **A narrow screen:** the side margins still hold. Test in Task 1.

---

### Before Task 1 (controller)

- [ ] `scripts/worktree.sh open celestina CEL-D-017` from the canonical checkout.
- [ ] Add the ledger row after `CEL-D-013`'s in `celestina/docs/plans/active/2026-11-25-cel-tracks.md`: `| CEL-D-017 | \`celestina:\` | active | \`celestina/src/sheet\`, \`celestina/src/island\` | pending | Replace the menu window's alignment with its capsule's outer edge with a window that opens next to its piece (the author's request of 2026-11-27; menu placement preview rule B chosen 2026-11-27; [design](../../../../docs/superpowers/specs/2026-11-27-celestina-menu-drop-design.md), [plan](../../../../docs/superpowers/plans/2026-11-27-celestina-cel-d-017-plan.md)) | [evidence](../../evidence/2026-11-27-cel-d-017-menu-from-icon.md) | VAL-CEL-D-017 |`
- [ ] Evidence stub `celestina/docs/evidence/2026-11-27-cel-d-017-menu-from-icon.md` with Date, Scope, Environment, Artifact, Preview and the sections Procedure, Result, Limits.
- [ ] Commit: `celestina-maintenance: Add the CEL-D-017 ledger row and evidence stub`.

### Task 1: the window opens from its piece

**Files:**
- Modify: `celestina/src/sheet/geometry.rs` (`window`, `PIECE_LEAD`, tests)
- Modify: `celestina/src/sheet/mod.rs` (`Sheets::target_of` passes `anchor.pill`)
- Modify: `celestina/src/island/mod.rs` (the production call near line 3437 and the test helpers near 5837 and 6575)
- Test: `geometry.rs` tests; `island/tests/window.rs`

**Interfaces:**
- Produces: `pub fn window(capsule: Rect, edge: Edge, piece: Rect, body: (f32, f32), surface: (f32, f32)) -> Rect`.

- [ ] **Step 1: Write the failing tests** in `geometry.rs` (capsule `r(1376, 8, 528, 40)`, `BAR` 1920 × 560):

```rust
#[test] fn an_end_window_starts_16_px_before_its_piece()            // piece r(1392, 14, 140, 28), body (360, 300): x 1376 (piece.x − 16 = 1376); window r(1376, 56, 360, 300)
#[test] fn an_end_window_moves_inward_only_as_far_as_it_must()      // piece r(1700, 14, 28, 28), body (420, 100): right edge 1904 (capsule's), x 1484
#[test] fn the_end_piece_keeps_todays_place()                        // piece r(1872, 14, 28, 28), body (240, 100): x 1664 = 1904 − 240
#[test] fn a_start_window_ends_16_px_after_its_piece()               // start capsule r(16, 8, 600, 40), piece r(400, 14, 28, 28), body (240, 200): x 204 (428 + 16 − 240)
#[test] fn a_start_window_moves_inward_only_as_far_as_it_must()     // same capsule, piece r(40, 14, 28, 28), body (240, 200): x 16
#[test] fn a_centre_window_ignores_its_piece()                        // Edge::Centre with any piece: today's centred x (830 in the existing test)
#[test] fn a_window_wider_than_its_capsule_grows_inward_as_today()   // keep the existing assertions, each with a piece inside its capsule
#[test] fn a_non_finite_piece_falls_back_to_the_outer_edge()         // piece x NaN: today's x for End and Start
```

The existing tests that call `window` take a piece: give each the piece at its capsule's outer end, so their expected rects stay as they are.

In `island/tests/window.rs`:

```rust
#[test] fn a_menu_opens_under_its_piece()                  // the end capsule's first piece (sysmon if the fixture has it, else the leftmost): the window's x is that piece's x − 16, clamped as above
#[test] fn a_switch_travels_between_the_two_pieces_rects() // open the first piece, switch to a later one: the travel ends on the second rule-B rect
```

- [ ] **Step 2: Run them and see them fail.** From `celestina/`: `CARGO_TARGET_DIR=~/.cache/cel-d-017-tgt cargo test --workspace --locked geometry window`. Expected: FAIL (wrong arity, then wrong x).
- [ ] **Step 3: Implement** `window` with the formulas of the Global Constraints, then pass `anchor.pill` in `Sheets::target_of` and the piece's pill in the Island's call.
- [ ] **Step 4: Run** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` (also on `+1.100.0`) and `cargo test --workspace --locked`. Expected: all pass.
- [ ] **Step 5: Commit:** `celestina-maintenance: Replace the menu window's outer-edge alignment with a window that opens next to its piece`.

### Task 2: nest shots, validation and evidence

- [ ] **Shots** through the kit (copy `cel-d/kit-recovered/` to `~/.cache/cel-d-017-nest/`, point `R` at this worktree's `celestina/`): the end capsule with the menus of its first piece, a middle piece and its last piece open, at rest; one switch between two of them. Read every crop.
- [ ] **Smoke** `bar,osd,notify` (`timeout -k 60 2700`).
- [ ] **`VAL-CEL-D-017`** in `celestina/VALIDATION.md`: open CPU · RAM, a middle piece and the last piece of the end capsule, and a piece of the start capsule; pass: each menu sits next to its piece and inside its capsule's span, the end piece's menu sits where it did. Add a `**Note:**` to `VAL-CEL-D-009`: since `CEL-D-017` a window opens next to its piece.
- [ ] Fill the evidence; commit `celestina-maintenance: Add the CEL-D-017 evidence and VAL-CEL-D-017`.

## After the tasks

- [ ] `/simplify` in the session, without subagents; `stop-slop` on the evidence and `VALIDATION.md` text.
- [ ] Final review `CEL-D-017.R`: Opus, high reasoning, `/code-review` on `origin/main...unit/celestina/CEL-D-017`, at most 2 rounds.
- [ ] Landing only with the author's word: `python3 scripts/land-unit.py unit/celestina/CEL-D-017 --kind milestone --smoke bar,osd,notify --summary "Replace the menu window's alignment with its capsule's outer edge with a window that opens next to its piece (the author's request of 2026-11-27; menu placement preview rule B chosen 2026-11-27)"`.

## Model table (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| Before Task 1 | this session | n/a | Ledger row and stub |
| Task 1 | Sonnet | medium | One pure function with pinned formulas and tests, three call sites |
| Task 2 | Sonnet | medium | Guarded nest shots and evidence from the known kit |
| `/simplify` | this session | n/a | In session, no subagents |
| Final review | Opus | high | `/code-review`, a different model from the implementer; at most 2 rounds |
