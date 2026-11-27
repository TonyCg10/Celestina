# CEL-I-016 Implementation Plan: the launcher's calculator

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A typed math expression shows its result as the launcher's first, selected row; Enter copies the result as shown, marks it copied and closes the launcher 350 ms later.

**Architecture:**
- A pure parser in `celestina-shell` decides whether the query is an expression and formats the result.
- A new clipboard worker command offers a text as the selection and records it in the history.
- The launcher adds a calculator row, a non-closing `Copy` action and a delayed close.

**Tech Stack:** Rust; `celestina-shell`, `celestina-paint` (one new role), `celestina/src/clipboard`, `celestina/src/launcher`, `celestina/src/app.rs`.

**Spec:** `docs/superpowers/specs/2026-11-26-celestina-launcher-design.md` §5.7 and §9 calls 4 and 5.

**Branch:** `unit/celestina/CEL-I-016` from `main`, after `STYLE-ICONS-2` has landed (the `calculator` glyph). Kind `milestone`.

## Global Constraints

- **Language.** English everywhere except `celestina/src/copy.rs`. New constants: `LAUNCHER_CALC_COPY` and `LAUNCHER_CALC_COPIED`, with the preview's two Spanish words for "Copy" and "Copied" (`/home/toni/CODIGO/previews/2026-11-26-launcher/index.html`, `rowHTML`).
- **Dependencies.** No new dependency.
- **The row.**
  - glyph `calculator` 24 px, tinted `Role::TextLo`;
  - an `=` in `Role::TextLo`, then the result in `type_scale.row_title` (15), tabular, `Role::TextHi`;
  - on the right, the copy constant and an "Enter" key cap: secondary size, `Role::TextLo`, a 1 px `Role::Divider` stroke, radius 5, padding 3 by 5;
  - once copied: the glyph is `check` and the detail is the copied constant in the new `Role::Success`.
- **The close.** `COPY_CLOSE = 350 ms` after the copy.
- **Bounds.** At most 256 chars are parsed and parentheses nest at most 32 deep. Past either bound the text is not an expression.
- **Format.**
  - up to 12 significant digits, a comma as the decimal separator, no grouping, no trailing zeros;
  - exponent style (for example `1,5e20`) when the magnitude is 1e15 or more, or below 1e-9 and not zero;
  - `-0` prints `0`.
- **Scope.** The selector (`Mod+K`) and the `/` switches never show the row.
- **Safety.** Live-session safety and checks as in `celestina/AGENTS.md`; nests only.

## Review Focus

1. Typing the expression one key at a time: the row appears only when the text parses whole (`2*`, `(3+` show no row), and the selection does not jump from an application to the row and back while typing.
2. `1/0`, `0/0`, `sqrt(-1)`, `10^400`: no row (non-finite).
3. `((((…))))` 1 000 deep, or a 10 000-char query: returns at once, no row, no stack overflow.
4. Enter on the row with the clipboard duty off: nothing happens, and the launcher stays open.
5. Closing the launcher (Escape, focus loss) inside the 350 ms after a copy: the copy stands and no second close fires.

---

### Task 1: the parser

**Files:**
- Create: `celestina-rs/crates/celestina-shell/src/calc.rs` (with tests)
- Modify: `celestina-rs/crates/celestina-shell/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub fn evaluate(query: &str) -> Option<String>`: the formatted result, or `None` when the query is not an expression in the spec's sense.
  - `pub fn format(value: f64) -> Option<String>` (`None` for non-finite).
  - `pub const MAX_INPUT_CHARS: usize = 256; pub const MAX_DEPTH: usize = 32;`

**Grammar** (recursive descent with a depth counter):

```text
expr    = term (("+" | "-") term)*
term    = unary (("*" | "/" | "×" | "÷") unary | implicit)*
implicit = an atom that starts with "(" or a function name, directly after a unary (no operator)
unary   = ("-" | "+") unary | power
power   = postfix ("^" unary)?          -- right-associative; -2^2 = -(2^2)
postfix = atom "%"*                       -- each % divides by 100
atom    = number | constant | function atom | "(" expr ")"
number  = digits ([.,] digits)? | [.,] digits
constant = "pi" | "π" | "e"
function = "sqrt" | "√" | "sin" | "cos" | "tan" | "ln" | "log"
```

Notes on the grammar:
- Whitespace is ignored between tokens.
- An identifier that is not a constant or a function fails the parse.
- **The expression rule.** The query counts only when the parse consumes every token, the tokens include at least one binary operator (`+ - * / × ÷ ^` between two operands, or an implicit multiplication), a `%` or a function, and the value is finite.
- `-5`, `pi`, `2`, `2e` and `7-zip` therefore give `None`, as do `2*` and `(3+`.

- [ ] **Step 1: Write the failing tests:**
  - `2*(3+4)^2` → `98`;
  - `80*15%` → `12`;
  - `sqrt(2)/2` → `0,707106781187`;
  - `1,5+1.5` → `3`;
  - `2(3)` → `6`;
  - `2sqrt(4)` → `4`;
  - `-2^2` → `-4`;
  - `2^3^2` → `512`;
  - `1e3` is not a number literal: `None`;
  - `10/4` → `2,5`;
  - `1/3` → `0,333333333333`;
  - `2^60` → `1,15292150461e18`;
  - `1/2^40` → `9,09494701773e-13`;
  - each `None` case above and in Review Focus 2–3.
- [ ] **Step 2: Run** `cargo test -p celestina-shell --locked calc` from `celestina-rs/`. Expected: FAIL.
- [ ] **Step 3: Implement.** Format with 12 significant digits via `format!("{:.*e}", 11, v)`, then trim zeros and choose plain or exponent by the magnitude rule. Swap the decimal point for a comma.
- [ ] **Step 4: Run** the tests and clippy for the crate. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher calculator's expression parser`

### Task 2: the success role and the clipboard's `offer_text`

**Files:**
- Modify: `celestina/crates/celestina-paint/src/theme.rs` (`Role::Success`, its place in `Role::ALL`, the colour from the scheme's `success`, the role test)
- Modify: `celestina/src/clipboard/worker.rs` (`Command::Offer`, `ClipboardHandle::offer_text`, the worker's handling, tests)

**Interfaces:**
- Produces:
  - `Role::Success`, resolved from `Scheme::success` (`#8ee0b4` in the dark scheme).
  - `impl ClipboardHandle { pub fn offer_text(&self, text: String) -> Result<(), ClipboardRefused> }`, documented like `select`.
- **Worker behaviour:**
  - it builds `Content::text(text)`, and refuses with a `ClipboardEvent::Refused` when that is `None`;
  - unless `paused`, it records the content in the history (`self.history.record(content, now)`, then `self.commit(now)` as reads do);
  - it then becomes the selection owner with a fresh source offering the content's MIME types plus the worker's identity MIME, exactly as `select` does. Extract the shared part of `select` into one private `fn own(&mut self, content: Content, now: u64) -> Result<(), Lost>` that both call.

- [ ] **Step 1: Write the failing tests:**
  - the paint role test lists `Success`;
  - with the worker's existing fake transport, `offer_text("98")`:
    - makes the transport own a source offering the text MIME types and the identity;
    - puts the text at the history's front;
    - is not read back as a new selection when the fake echoes it (the identity MIME is honoured);
  - while paused it owns the selection and records nothing;
  - an over-long text (`MAX_TEXT_BYTES + 1`) is refused and owns nothing.
- [ ] **Step 2: Run** `cargo test -p celestina-paint --locked` and `cargo test -p celestina --locked clipboard::`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `Add the success role and a clipboard command that offers a text`

### Task 3: the calculator row in the launcher

**Files:**
- Modify: `celestina/src/launcher/state.rs` (`Row::Calc`), `view.rs` (the row), `mod.rs` (`LauncherAction::Copy`, the delayed close, `can_copy`), `celestina/src/copy.rs`, `celestina/src/app.rs` (open passes `can_copy`; `Copy` calls `offer_text`; a finished launcher closes)

**Interfaces:**
- Consumes: `celestina_shell::calc::evaluate`, `ClipboardHandle::offer_text`, `Role::Success`.
- Produces:
  - `Row::Calc { result: String }`;
  - `LauncherAction::Copy(String)`, for which `closes()` returns `false`;
  - `Launcher::new(.., can_copy: bool, ..)` and `Launcher::selector` unchanged;
  - `Launcher::take_finished(&mut self) -> bool`, true once 350 ms have passed since a copy, and only once.

**Behaviour:**
- `LauncherState::answer` for `Source::Apps`, when the query does not start with `/` and `evaluate(query)` is `Some`, puts `Row::Calc` first, before the application rows, so `MAX_ROWS` still bounds the total. A new query selects row 0, as today.
- **Enter or a click on the row.**
  - With `can_copy` false: it returns `None` and nothing changes.
  - Otherwise: `copied_at = Some(now)`, the view is stale, and the action is `Copy(result)`.
- **`frame`** (through the `advance` step `CEL-I-014` added, or a new one if that unit has not landed) requests redraws while a copy waits. Once `now - copied_at >= COPY_CLOSE` it sets `finished`.
- **The app.**
  - After the launcher's `frame` (app.rs ~3173) it checks `take_finished()` and closes the launcher through the same path as `LauncherAction::Close`.
  - On `Copy(text)` it calls `offer_text` on the clipboard duty's handle (`self.clipboard.get()`, as app.rs ~824 reaches it) and logs a refusal; it does not close.
  - Any close resets `copied_at`, so Review Focus 5 holds.
- **The row's entrance.** It is a new keyed node (`launcher-calc`); the tree's own entrance and the rows' slide carry it. The preview's 0→40 px height growth needs no extra code if the tree slides the rows below; record what the nest shows.

- [ ] **Step 1: Write the failing tests:**
  - state: `2*(3+4)^2` gives `[Row::Calc { result: "98" }, ..]` with `selected == 0`; `/2+2` gives the switches only; the selector never gets the row;
  - view: the row's children are the calculator glyph, the `=` text, the result text and the detail; after the copy, the `check` glyph and `Role::Success`;
  - input: Enter gives `Copy("98")`, which does not close; `take_finished` is false at +349 ms, true at +350 ms, then false again; with `can_copy` false, Enter gives `None`; Escape after the copy closes once.
- [ ] **Step 2: Run** `cargo test -p celestina --locked launcher::`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the three workspace checks from `celestina/`. Expected: PASS.
- [ ] **Step 5: Commit** `Add the launcher's calculator row that copies its result`

### Task 4: evidence in a nest

- [ ] Private target dir and one nest, as in `celestina/AGENTS.md`, with the clipboard duty on.
  - The probe types `2*(3+4)^2` and takes a shot.
  - It presses Enter and takes a shot within 350 ms, if the timing allows; otherwise it records the frame log.
  - It reads the nest's clipboard with `wl-paste` inside the nest only, and expects `98`.
- [ ] Run the launcher's and the clipboard's smoke phases (`smoke.sh --list`).
- [ ] Write the evidence.
