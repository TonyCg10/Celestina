# CEL-I-006: session menu, implementation plan

> **For agentic workers:** you are the one implementer of this whole unit. Do not dispatch subagents. Work test-first. Report to the file the controller names.

**Goal:** A `session` piece at the end of the bar (a `power` glyph). Its primary click opens a menu under it.
- **Immediate rows:** Lock, Suspend.
- **Rows that confirm inline** in a danger card: Log out, Restart, Power off. Each is confirmed or cancelled there.

**Approved preview:** variant A of the session-menu board, approved by the author on 2026-11-25. It is a design canvas private to the author. The values below are the ones it used.

## Global constraints

- **Worktree and branch.** Open the worktree from the canonical checkout: `scripts/worktree.sh open celestina CEL-I-006 --from unit/celestina-style/STYLE-ICONS-1`. That branch adds the `lock`, `moon` and `log-out` glyphs. Never edit the canonical checkout.
- **Read first.** Run `python3 scripts/agent-context.py celestina/src` and read the "Read now" documents. Read `celestina/AGENTS.md` fully.
- **Language.** English in code, comments and tests. Spanish product copy goes only in `celestina/src/copy.rs`.
- **Checks.** Run all of these from `celestina/`; each must pass:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`: sum every `test result` line yourself.

  Also run the repository guards from the worktree root:
  - `bash scripts/check-architecture-contract.sh`
  - `bash scripts/check-documentation-contract.sh`
  - `python3 scripts/check-language-contract.py`
- **Commits.** Subjects read `celestina-maintenance: <Verb> …`. The verb must be one `scripts/commit_scope.py` accepts: Add, Fix, Update, Replace or Remove (not "Open" or "Show").
- **Never touch sensitive paths:**
  - `src/lock`, `src/polkit`, `celestina/scripts/` and `.githooks`;
  - any `AGENTS.md`;
  - `docs/contracts|governance|standards`.

  Call the lock only through the existing verb path. `tests/lock_guards.rs` TM-25 bans the word `unlock` anywhere in `src`.
- **No live session.** Never run `celestina` or `celestina-lock` against the author's session. Never run session verbs for real.

## Decisions (controller's calls, final)

1. **Module.** Add a new sibling module `celestina/src/session_menu/`, made of `mod.rs` and `view.rs`. Do not add a `Kind::Session` to `osd/popup.rs`.
   - Use `island/menu.rs` (`MenuCore`/`Menu`, `ChoiceSlot`, Up/Down mapped to Tab) as the template.
   - Use `resize_wanted` with `ctl.resize_popup` from `LevelPopup` for the size change.
   - Placement is `AnchorEdge::Bottom` and `Gravity::Bottom` with offset `(0, spacing.sm)`, so the top edge stays fixed when the menu grows.
2. **Types:**
   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Eq)]
   pub enum SessionAction { Lock, Suspend, LogOut, Reboot, PowerOff }
   impl SessionAction {
       pub const ROWS: [SessionAction; 5];
       pub fn needs_confirm(self) -> bool;   // LogOut | Reboot | PowerOff
       pub fn verb(self) -> Verb;            // Lock→Verb::Lock, Suspend→Verb::LockAndSuspend,
                                             // LogOut|Reboot|PowerOff→Verb::Session(PowerAction::…)
       fn icon(self) -> &'static str;        // lock, moon, log-out, rotate-ccw, power
   }
   pub type SessionSlot = Rc<Cell<Option<SessionAction>>>;
   pub const MENU_W: f32 = 240.0;
   ```
   Never map Suspend to `Session(Suspend)`, which is refused by design. Suspend is lock-and-suspend, exactly what the author's own niri binding `Mod+Shift+Escape` runs (outside the repository).
3. **Sizes and styles** (theme tokens only, no literals except these):

   | Element | Values |
   |---|---|
   | Card | width 240, padding `spacing.sm`, radius `radius_md`, the same haze/glass card as the level popup |
   | Rows | height `row_height` (40), radius `radius_button` (10), `padding_xy(spacing.md, 0)`, gap `spacing.md` |
   | Row icon | 18 |
   | Row label | `type_scale.body`, Regular, TextHi; rows are `.interactive().focusable()`, so hover, press and the focus ring come from the toolkit |
   | Divider | after Suspend: 1 px of `Divider`, margin `spacing.xs` above and below |

   **Confirm card.** It replaces the chosen row.
   - Fill `DangerFill`, a 1 px inset stroke of `DangerBorder`, radius `radius_button`, padding `spacing.md`.
   - The question text is `body`, `DangerFillInk`, measured with the painter's wrap (like the `fit` closure in `LevelPopup::new`).
   - Below it, a right-aligned row with gap `spacing.xs`:
     - **Cancel:** filled `ControlFill`, TextHi.
     - **The action:** filled `Danger`, `DangerInk`, with the row's own label.

     Use `island::view::button_filled`.
4. **Behaviour:**
   - Lock and Suspend close the menu with their choice.
   - A confirm row moves to the confirm card and the menu grows; its top edge stays put. Keyboard focus goes to Cancel.
   - Cancel returns to the list and the menu shrinks back.
   - The action button closes the menu with that choice.
   - Escape on the card returns to the list. Escape on the list closes with no choice.
   - A press outside closes with no choice.
   - Arrows move through the rows and skip the divider. Enter activates.
5. **Acting.** The app acts only after `PopupClosed`, so no popup grab is held while the lock maps.
   - In `input()` under `if closed`, `if let Some(a) = self.session_choice.take() { self.post(Message::Command(a.verb())); }`.
   - This is the same path as `celestina lock` and `celestina session …` (`bus.rs` → `App::command`).
   - Failures stay journal-only, as the verbs already do.
6. **Theme roles.** Add `DangerInk`, `DangerFill`, `DangerBorder` and `DangerFillInk` to `celestina-paint::Role`, mapped from `celestina_tokens::Scheme` (the fields already exist). Add them to `ALL` and to the per-role tests.
7. **Toolkit stroke.** Add `stroke: Option<Role>` to `ViewKind::Box` and a `View::stroke(role)` builder.
   - `paint.rs` draws it after the fill with `stroke_rounded`: the rect inset 0.5, radius minus 0.5, width 1.0.
   - Fix every `ViewKind::Box` literal or pattern the compiler flags.
   - Add a test: `a_box_stroke_is_kept_and_other_kinds_ignore_it`.
8. **Hover on filled buttons.** On main, `celestina-ui/src/highlight.rs` `box_fill` replaces any interactive fill with `ControlHover` or `ControlPressed`, the accent fill included, so a hovered danger button would lose its colour, against the preview. Change it so that an interactive Box whose fill is not `ControlFill` keeps that fill under hover and press, with the standard wash on top (or the closest the painter offers). `ControlFill` keeps today's behaviour. Test a Danger and an Accent box (fill kept) and a `ControlFill` box (lifted as today).
9. **Piece.**
   - `celestina/src/pieces/session.rs`, id `session`, node keys `session` and `session-icon`, glyph `power`. Use `const GLYPH: f32 = 16.0` (as media and weather do), drawn through `conn_glyph::tap`.
   - `Gesture::Primary` gives `PieceAction::SessionMenu`. `page()` gives `None`.
   - Its accessible name is `copy::PIECE_SESSION`.
   - Register it last in `PIECES`, and append `"session"` last to `BarLayout::default().end` in `celestina-rs/crates/celestina-shell/src/settings.rs`. Update `the_bar_defaults_are_todays_layout` and `network_and_bluetooth_reach_files_that_never_chose_an_end_zone`.
10. **Island and app.**
    - `PieceAction::SessionMenu` in `pieces/mod.rs`.
    - `IslandAction::OpenSessionMenu { anchor: Rect }`, and its arm in `app.rs` built like the `OpenLevelPopup` arm: the `SessionMenu` with a fresh `SessionSlot`.
    - In `bar_input`, `PieceAction::SessionMenu => out.extend(self.session_menu_action(key))`, shaped like `popup_action`.
    - A field `session_menu: Option<PopupId>`, and `open_session_menu(anchor, size, popup, ctl) -> Option<PopupId>`. It takes the same guard as `open_level_popup`, and is added to:
      - the `open_level_popup` and `open_tray_menu` guards;
      - the PointerPress guard;
      - `close_menu`, which polkit calls;
      - `menu_closed`.
    - The app gets a field `session_choice: SessionSlot`, reset to `None` when the menu opens.
11. **Copy.** Product copy goes in `copy.rs` only, as constants: `PIECE_SESSION` (the piece's name, "Session"); `SESSION_LOCK`, `SESSION_SUSPEND`, `SESSION_LOG_OUT`, `SESSION_REBOOT`, `SESSION_POWER_OFF` (the row labels: lock, suspend, log out, restart, power off); `SESSION_CANCEL` (cancel); `SESSION_ASK_LOG_OUT` (asks whether to log out, warning that every application closes), `SESSION_ASK_REBOOT` and `SESSION_ASK_POWER_OFF` (ask whether to restart or power off now). The exact Spanish is the approved preview's and is pinned by a copy test in `copy.rs`.
12. **Out of scope.** niri shows its own confirm dialog after Log out (`Quit { skip_confirmation: false }`). Leave it, and record it in the evidence under Follow-up.

## Tests (write first, see them fail)

**`session_menu`:**
- `the_rows_are_lock_suspend_divider_log_out_reboot_power_off_in_order`: y order, size `(240, list_h)`, each row 40 tall.
- `lock_and_suspend_close_at_once_with_their_choice`.
- `a_dangerous_row_asks_first_and_grows_keeping_its_top`: `resize_wanted` returns the new size once and then `None`; placement edge and gravity are Bottom.
- `confirm_sends_the_action_and_cancel_returns_to_the_list`.
- `escape_in_the_confirm_card_cancels_and_escape_in_the_list_closes`.
- `the_keyboard_moves_through_the_rows_and_enter_chooses`: Down skips the divider; Enter on a confirm row focuses Cancel.
- `each_action_is_the_verb_its_bus_word_names`: `Verb::parse` of the words `session` and `log-out` (as `String`s) == Ok(SessionAction::LogOut.verb())`, the same for reboot, power-off and lock; Suspend is `Verb::LockAndSuspend`.
- `the_confirm_card_uses_the_danger_roles`.
- `the_icons_it_names_exist_in_the_icon_set`: copy the approach from `pieces/weather.rs`.

**Elsewhere:**
- `pieces/session.rs`: `the_piece_is_a_power_glyph_that_opens_the_menu`.
- `pieces/mod.rs`: add `"session"` to `the_table_yields_the_pieces_and_the_tab_order`.
- `island/mod.rs`: `the_session_piece_asks_for_its_menu_anchored_at_its_node` and `the_session_menu_is_one_of_the_islands_popups`.
- The settings, theme, toolkit, hover and copy tests named in the decisions.

## Documents (in the same branch)

- **Ledger.** In `celestina/docs/plans/active/2026-11-25-cel-tracks.md`, add a row `CEL-I-006`, status `active`, with Files `pending` and Diffstat `pending`. Its intended change is one line, with "preview approved 2026-11-25, variant A". Evidence links `../../evidence/2026-11-25-cel-i-006-session-menu.md`; validation is `VAL-CEL-I-006`.
- **Build order.** Change it to:
  1. `CEL-I-004` (done);
  2. `STYLE-ICONS-1`, then `CEL-I-006`, `CEL-D-001` and `CEL-D-006`;
  3. `CEL-D-002` to `CEL-D-005`;
  4. `CEL-I-003`.
- **Backlog.** Add a line that the author asked for the session menu on 2026-11-25.
- **Validation.** Add `VAL-CEL-I-006` to `celestina/VALIDATION.md`, in the same field format as `VAL-CEL-I-004`, including `Result: not run` and `Evidence`. Its procedure:
  - open the menu;
  - Lock locks;
  - Suspend locks and suspends;
  - each dangerous row asks first, and Cancel and Escape go back;
  - confirming Restart reboots. Tell the author to save their work first.
- **Evidence.** Write `celestina/docs/evidence/2026-11-25-cel-i-006-session-menu.md`, an ordinary record: scope, the preview approval, the procedure commands, results with test counts, limits (no nested screenshot unless you took one; verbs were never run for real), and a Follow-up that includes the niri second confirm.
- **The author's layout.** Their `~/.config/celestina/shell.json` sets `bar.end` explicitly. Write in the evidence the line they must change:

  `"end": ["bubbles","sysmon","tray","network","bluetooth","volume","brightness","clipboard","media","battery","power-profile","session"]`

## Smoke

Do not change any smoke script; they are sensitive. The landing will run `--smoke bar`. If your environment allows it, run `timeout -k 60 2700 sh celestina/scripts/smoke.sh --binary <debug binary> --only bar` and record the result. If not, write "not run".
