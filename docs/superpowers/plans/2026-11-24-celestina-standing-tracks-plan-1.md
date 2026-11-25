# Celestina standing tracks — plan 1: open the tracks and CEL-D-001 (tab icons)

> **For agentic workers:** both units are light units (`docs/standards/verification.md`, "Light units"): the session implements them itself, without subagents, and each takes one review round. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the two standing tracks (design `CEL-D`, improvements `CEL-I`) into the repository, then deliver the first design entry: the Island's tab row shows icons instead of text.

**Architecture:** Two units, landed one after the other. Unit 1 is a suite documents-only unit: the approved design, this plan, one standing Celestina plan holding both tracks, and roadmap links. Unit 2 (`CEL-D-001`) is stacked on Unit 1's branch; it adds `fn icon(&self) -> &'static str` to `PageView`, one semantic icon per page, and makes `island/pages/tabs.rs` draw that icon in each pill. It also adds the one missing Lucide icon (`calendar`) to `celestina-style/icons`.

**Tech Stack:** Rust (celestina workspace, pinned toolchain plus the CI runner's stable 1.100), `celestina-ui` views, Lucide SVG icons, the repository's `scripts/worktree.sh` and `scripts/land-unit.py`.

**Spec:** `docs/superpowers/specs/2026-11-24-celestina-standing-tracks-design.md` (committed by Unit 1).

## Revision of 2026-11-25 (after `main` moved to `ef84a451`)

The development speed plan (DEV-1, [ADR 0013](../../decisions/0013-development-speed.md)) landed after this plan was first written. These rulings change it:

- **One standing plan, not two.** The documentation guard allows exactly one active plan per roadmap, and `celestina/ROADMAP.md` names one active checkpoint. Both tracks live in one plan, `celestina/docs/plans/active/2026-11-25-cel-tracks.md` (Plan ID `cel-tracks`, checkpoint `CEL-TRACKS`). It has one ledger and one backlog per track.
- **Unit 1 is a suite unit hosted by a standing suite plan**, `docs/plans/active/2026-11-25-cel-tracks-docs.md` (Plan ID `cel-tracks-docs`, checkpoint `TRK-1`). It hosts the documents-only units that commit the tracks' design notes and task plans under `docs/superpowers/`, outside the shell's commit roots. The landing finds a unit's open row only in an active plan.
- **Light lane.** ADR 0013 amends the design's "each entry is one ordinary unit" so that a qualifying entry uses the light lane. For each unit that means:
  - the session implements it with no subagents;
  - a short evidence record from `docs/templates/evidence-light.md`;
  - one review round. The reviewer is a model other than the implementer's, with high reasoning.
- **Commit subjects** use the guard's form: `suite-maintenance: …` and `celestina-maintenance: …`.
- **Nested checks:**
  - the smoke runs in phases under the host-wide nest lock;
  - the session runs only the phases of its area, and the landing runs the full smoke;
  - bound a run with `timeout -k 60 2700`, never `timeout -s KILL`;
  - nests start with `CELESTINA_DDC=0` and `CELESTINA_BACKLIGHT=0`.

## Global Constraints

- Development text is English; the author's conversation is Spanish.
- Each unit gets:
  - one session worktree opened with `scripts/worktree.sh`;
  - one ledger row;
  - one landing with `scripts/land-unit.py`.

  Never edit the canonical checkout.
- Every landing needs the author's direct authorisation in the coordinating chat; landings are asked for in batches.
- CI must stay green on the runner's stable Rust (1.100 today), not only the pinned toolchain: run `cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings` as well.
- Never run `celestina` or `celestina-lock` against the author's live session. Use only a nested niri through `scripts/dev-session.sh --own-bus` or `smoke.sh`, with a clean environment.
- Unit ids are `CEL-D-NNN` (design) and `CEL-I-NNN` (improvements), zero-padded to three digits.
- New icons come from Lucide (the licence is already in `celestina-style/icons/LICENSE-lucide.txt`), drawn in the set's stroke style:
  - `stroke="#fff"`, `stroke-width="2.5"`;
  - round caps and joins;
  - 24×24 viewBox.

## Review Focus

1. **A page added later without an icon:** the code must not compile (the trait method has no default), so no tab can silently render empty.
2. **An icon name with no SVG file:** a test must fail, so no blank square appears at runtime.
3. **The open tab must still read as selected without text:** it keeps the accent fill and gets an accent-ink tint on the icon.
4. **Icon pills must fit the row** at the narrowest page width (388 px) without overlapping.
5. **The page's title must stay visible somewhere** once tabs lose their text: the page frame's title row keeps it, unchanged.

---

## Unit 1 — `CEL-TRACKS-OPEN` (suite, light, documents only)

Worktree: `suite-CEL-TRACKS-OPEN`, on `ef84a451`.

**Files:**
- Create `docs/superpowers/specs/2026-11-24-celestina-standing-tracks-design.md` as a copy of the approved draft, with these edits:
  - set **Status:** approved 2026-11-24;
  - drop the "Draft location" line;
  - add §2.2 with the revision rulings above (one plan, the light lane per ADR 0013).
- Create `docs/superpowers/plans/2026-11-24-celestina-standing-tracks-plan-1.md` (this plan).
- Create `docs/plans/active/2026-11-25-cel-tracks-docs.md`. This is the standing suite plan:
  - Plan ID `cel-tracks-docs`, checkpoint `TRK-1`;
  - its first row is `CEL-TRACKS-OPEN`, `active`.
- Create `docs/evidence/2026-11-25-cel-tracks-open.md` from the light template.
- Also commit the CEL-I-004 design note and plan, as `docs/superpowers/specs/2026-11-25-celestina-cel-i-004-audio-design.md` and `docs/superpowers/plans/2026-11-25-celestina-cel-i-004-audio-plan.md`.
- Modify the root `ROADMAP.md` and `STATUS.md` (TRK-1 active) and `docs/plans/active/README.md`.

**Ruling after review (2026-11-25):** a landing changes exactly one active plan, so this unit does not open the shell's plan. `CEL-I-004`, the shell's first entry, creates `celestina/docs/plans/active/2026-11-25-cel-tracks.md` (checkpoint `CEL-TRACKS`) with itself as the open row. The same commit edits `celestina/ROADMAP.md`, `celestina/STATUS.md` and the shell's active-plans index. The plan's other rows, in order, all `planned`:
  - `CEL-D-001` to `CEL-D-005`;
  - `CEL-I-003`.

  Its backlog records:
  - `CEL-I-001` and `CEL-I-002` as landed in the CEL-2 ledger;
  - `CEL-I-005` as merged into `CEL-I-004`.

  The draft is `/home/toni/CODIGO/2026-11-25-celestina-cel-tracks-plan-draft.md`.

- [ ] Write the files; size the unit (`git diff --numstat origin/main...` without `*.md`: expected 0 lines).
- [ ] Run `bash scripts/check-architecture-contract.sh`, `scripts/check-documentation-contract.sh` and `python3 scripts/check-language-contract.py`. All must pass.
- [ ] Commit: `suite-maintenance: Add the standing design and improvement tracks of the shell and their first batch`.

## Unit 2 — `CEL-D-001` (celestina, light): icons in the tab row

Worktree: `scripts/worktree.sh open celestina CEL-D-001` once `CEL-I-004` has landed (it opens the shell's plan).

**Open point, settled when this unit starts:** `celestina-style/icons/` is outside the `celestina:` commit roots, so `calendar.svg` needs its own `celestina-style` unit to stack on, or the clock tab uses an existing icon.

**Files:**
- Create `celestina-style/icons/calendar.svg`. It is Lucide "calendar" with the set's stroke attributes:
  - `<path d="M8 2v4" />`;
  - `<path d="M16 2v4" />`;
  - `<rect width="18" height="18" x="3" y="4" rx="2" />`;
  - `<path d="M3 10h18" />`.
- Modify `celestina/src/island/pages/mod.rs`:
  - `PageView::icon(&self) -> &'static str` with no default body;
  - `pub fn page_icon(page: PageId) -> &'static str` beside `page_title`;
  - the impls `Absent` and the test `Counts`.
- Modify every `impl PageView` in `celestina/src/island/pages/{activity,bluetooth,clipboard,clock,media,network,notifications,phone,power,sysmon,tray}.rs`.
- Modify `celestina/src/island/pages/tabs.rs`:
  - add `pub const TAB_ICON: f32 = 18.0`;
  - draw the child `view::icon(Key::indexed("tab-icon", index(page)), page_icon(page), TAB_ICON).tint(ink)`;
  - keep the pills' `.grow()` and `min_size.width = 0`;
  - delete `tab_label` and `label_style`.
- Modify `celestina/src/copy.rs`: remove the constants only the tab labels used.
- Modify the `CEL-D-001` row in the standing plan.
- Create the evidence record `celestina/docs/evidence/2026-11-25-cel-d-001-tab-icons.md` from the light template.
- Add a `VAL-CEL-D-001` row to `celestina/VALIDATION.md`.

Icon mapping (exact):

| Page | Icon |
|---|---|
| activity | `view-grid` |
| notifications | `bell` |
| tray | `system-tray` |
| sysmon | `gauge` |
| clock | `calendar` (new) |
| clipboard | `clipboard-paste` |
| network | `wifi` |
| bluetooth | `bluetooth` |
| power | `zap` |
| phone | `phone` |
| media | `music` |
| `Absent` | `app-window` |

Tests (write first, see them fail, then implement):
- `every_page_icon_has_a_file` (in `pages/mod.rs`): for every `PageId::all()`, `page_icon(page)` matches the table and `../celestina-style/icons/<name>.svg` exists, resolved from `CARGO_MANIFEST_DIR`.
- `tabs_draw_the_page_icon_not_its_title`: each tab's single child is `ViewKind::Icon` with `IconKey::Semantic(page_icon(page))` and size `TAB_ICON`, and no `ViewKind::Text` appears anywhere in the row.
- `the_open_tab_tints_its_icon_accent_ink`: with `open = PageId::CLOCK`, the clock tab's tint is `Some(Role::AccentInk)` and every other tab's is `Some(Role::TextLo)`.
- `icon_tabs_share_the_row_equally_and_fit_388`, which replaces the two width tests. At width 388:
  - every pill sits on one row;
  - none overlaps;
  - the last pill ends at 388 (±0.01);
  - the widths are equal (±1.0).
- Keep and adapt these tests:
  - `tabs_show_every_page_with_the_open_one_selected`;
  - `tabs_name_their_pages`;
  - `the_phone_and_media_pages_never_have_a_tab_and_opening_them_selects_none`.

- [ ] Size the unit before starting (expected well under 150 lines outside documents) and again before the review.
- [ ] Implement test-first.
- [ ] Run the checks from `celestina/`. All must pass.
  - `cargo fmt --all -- --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`;
  - the same clippy on `+1.100`;
  - `cargo test --workspace --locked`;
  - the three repository guards.
- [ ] Nested look. Build the debug binary, then run `timeout -k 60 2700 sh scripts/smoke.sh --binary PATH --only <the Island/bar phase>`, choosing the phase with `--list`. Then take one nested screenshot of `celestina island open clock` showing nine icon pills with the clock pill filled.
- [ ] Write the evidence and the validation row ("open each tab from the row; every icon names its page; the selected tab is obvious"), then commit: `celestina-maintenance: Show each Island tab as its page icon instead of its title`.

## Review and landing

- [ ] Review the two branches once:
  - Unit 1's diff;
  - Unit 2's diff against Unit 1.

  The reviewer is Fable 5.2 with high reasoning, a model other than the implementer's, using `/code-review`. The session fixes defects in what the unit set out to do. Any other finding becomes a backlog line or a `Follow-up` line.
- [ ] Ask the author to authorise both landings, as one batch, in this chat.
- [ ] Land in dependency order: `python3 scripts/land-unit.py unit/suite/CEL-TRACKS-OPEN --kind maintenance`, then `python3 scripts/land-unit.py unit/celestina/CEL-D-001 --kind maintenance`. Both run from the canonical checkout.
- [ ] After landing, the author restarts the shell and runs `VAL-CEL-D-001` live.

## What comes next

`CEL-D-002` (anchored panel and motion) gets its own plan next, written against the code as it stands then. It is the foundation for 003, 005 and the three improvements, and it is an ordinary milestone unit, not a light one.
