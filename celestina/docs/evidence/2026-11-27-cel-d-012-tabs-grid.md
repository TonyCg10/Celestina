# Evidence: the launcher's tabs and grid (CEL-D-012)

- **Date:** 2026-11-27
- **Scope:** `CEL-D-012`: `celestina/src/launcher/{state,view,mod}.rs`, `celestina/src/app.rs`, `celestina/src/copy.rs`, `celestina/src/providers/settings.rs`, `celestina-rs/crates/celestina-shell/src/settings.rs`, `celestina/crates/celestina-ui/src/tree.rs` (one lookup rule), the ledger row and `VAL-CEL-D-012`
- **Environment:** worktree `celestina-CEL-D-012` from `main` at `dd28ad85` (celestina 1.34.0); debug and release builds in private target directories under `~/.cache`; the nest ran on its own bus
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** the launcher preview the author approved on 2026-11-26 (tabs variant A), private artifact <https://claude.ai/artifact/QoGCyzhoqoM5P4pH2ASHWB>; [design](../../../docs/superpowers/specs/2026-11-26-celestina-launcher-design.md) §5.1, §5.2, §5.5, §5.8 and §9, [plan](../../../docs/superpowers/plans/2026-11-26-celestina-cel-d-012-plan.md)

## Change

- **Card:** the launcher's card is 560 × 444: the field's pill, a 36 px tab row, then the 320 px area. The selector keeps 560 × 400 and has no tab row (`card_size` takes the source).
- **Tabs:** Recents, All, then one tab per group that has an application, in the design's order. Each tab is a 32 px pill with an 18 px glyph. The open tab has the control fill and its label (`LAUNCHER_TAB_RECENT`, `LAUNCHER_TAB_ALL`, `LAUNCHER_GROUP_*`) beside the glyph; the other tabs show the glyph in the low text role and lift under the pointer. The view button sits at the row's right end (`view-grid` in the list, `view-list` in the grid).
- **Answers:** an empty query (or one with nothing to match) answers from the open tab:
  - Recents: the applications with a use score, best first, at most 50; with none, one line, `LAUNCHER_RECENT_EMPTY`;
  - All: every application by folded display name;
  - a group: its applications by name.
  
  A typed query searches every application as before. The tab row dims to 45 % while it does, and its tabs still work. The launcher always opens on Recents.
- **Switching tabs:** a click on a tab, Ctrl+Tab and Ctrl+Shift+Tab (they wrap), or Alt+1 to Alt+9. A switch empties the field and the query and shows the tab from its top. A click on the open tab while a query is typed empties the query. A group that empties while it is open falls back to All, from the top.
- **Grid:** Ctrl+G or the view button switches between the list and a six-column grid. Each cell is 80 px high with a 40 px icon and the name at 12 px under it, cut to the cell's width with an ellipsis at the end (measured with the text system). The calculator row spans the width above the cells; a `/` query lists its switches as rows in either view. Up and Down move by a line; Left and Right move by a cell while the field is empty and move the caret otherwise; every arrow wraps. The scroll, the reveal of the selection and the focus ring use the grid's line heights.
- **Memory:** the choice is written to `shell.json` as `launcher.view` (`"list"` or `"grid"`) only when the author switches it. An absent or unknown value reads as the list. A refused write is logged and the open launcher keeps the view it shows. The launcher's view type is the settings' own `LauncherView`.
- **Motion:** a tab or view switch crossfades the content. The label of the open tab fades in where it lands while the other tabs slide to their new places.
- **Toolkit:** `Tree::node_by_key` now treats a node under a leaving node as leaving, so during a crossfade the lookups of the list, its rows and the focus ring find the new content, not the old one fading out.

## Departures from the plan

- **No slide on a tab switch, no scale on a view switch.** The tree cannot start an inserted node from an offset, so the content only crossfades. The 12 px slide and the 0.985 scale are not done.
- **The label is built on the open tab only.** A label at opacity 0 on every tab would still take its width. The cost is a short overlap during a switch (see Result).
- **Weight.** The label is regular weight; the preview's tabs use 500, which the toolkit does not have.
- **Names in the grid.** They are measured and cut at the end, like the preview's `text-overflow`. The first implementation cut names at twelve characters, and wide names ran out of their cells in the nest, so the measured cut replaced it.
- **Commit subjects.** The guard's verb list has no "Switch", so the fourth task's commit says "Support switching".

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                                       # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked --no-fail-fast   # 2932 passed, 1 failed (below); 159 in launcher::
```

- **The one failure.** `providers::brightness::tests::after_the_piece_leaves_a_redetection_reads_nothing` failed in the full runs made while other sessions loaded the machine (load average about 7). It passes alone, waits on a 5 s timeout, and does not touch the launcher. The two full runs before the review's fixes passed whole (2932 and 2933 passed, 0 failed).
- **`celestina-shell`:** `cargo test -p celestina-shell --locked settings`: 71 passed.
- **Review Focus coverage:**
  - a group emptied while open: `state::tests` (`update`'s fallback) and `launcher::tests` (the scroll back at the top);
  - Alt+digit beyond the tabs, and Alt with a letter still reaching the field: `launcher::tests`;
  - a tab clicked while typing: `launcher::tests`;
  - grid arrows with the calculator row, and Left and Right with text: `state::tests` (`move_grid`) and `launcher::tests`;
  - an unwritable `shell.json`: `ShellSettings` keeps a protected file in memory and writes nothing; the app logs a refused write and goes on.
- **The nest:** the guarded kit, nested niri at scale 1.5, `LANG=es_ES.UTF-8`. It read the author's own desktop files, his Flatpaks included (`XDG_DATA_DIRS` with the Flatpak exports), and nothing was launched. `usage.json` was seeded from the preview's list of recent applications, so Recents matches the preview's order. The probe ran each of the preview's ten states with `wtype` and the virtual pointer, once in the list and once in the grid (Ctrl+G; `shell.json` then held `"launcher":{"view":"grid"}`), and caught a tab switch at 70 ms and a Ctrl+Tab.
- **The preview:** the approved preview's ten states, served locally and captured at 1:1, list and grid.
- **The smoke:** `smoke.sh --binary <release> --only launcher` on a release build of the branch's last commit, in a private target: `smoke: OK (only: launcher)`. The landing runs the full smoke (the series' last unit).

## Result

Each picture is the preview's state on the left and the nest on the right.

| State | List | Grid | Differences |
|---|---|---|---|
| 1. On open | [list](./2026-11-27-cel-d-012-tabs-grid/list-01-open.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-01-open.png) | None beyond the backdrop (the nest has no wallpaper behind the glass) and the caret in the field |
| 2. All, scrolled | [list](./2026-11-27-cel-d-012-tabs-grid/list-02-all-scrolled.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-02-all-scrolled.png) | The nest scrolled two wheel steps (240 px), not the preview's 260 or 120 px; the row or cell under the pointer is lit |
| 3. Arrows | [list](./2026-11-27-cel-d-012-tabs-grid/list-03-arrows.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-03-arrows.png) | None: the same row and cell carry the ring |
| 4. Pointer over a row | [list](./2026-11-27-cel-d-012-tabs-grid/list-04-hover.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-04-hover.png) | One game of the preview is not in the nest's index and another has no icon there; the rest match |
| 5. The browser word | [list](./2026-11-27-cel-d-012-tabs-grid/list-05-navegador.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-05-navegador.png) | Zen's detail shows its localized generic name |
| 6. A typo | [list](./2026-11-27-cel-d-012-tabs-grid/list-06-krtia.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-06-krtia.png) | Krita comes first, then seven Flatpak entries matched by letters in order in their long `Exec` (`CEL-I-015`'s follow-up) |
| 7. A sum | [list](./2026-11-27-cel-d-012-tabs-grid/list-07-sum.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-07-sum.png) | None |
| 8. Copied | [list](./2026-11-27-cel-d-012-tabs-grid/list-08-copied.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-08-copied.png) | None |
| 9. No results | [list](./2026-11-27-cel-d-012-tabs-grid/list-09-none.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-09-none.png) | None |
| 10. Clearing | [list](./2026-11-27-cel-d-012-tabs-grid/list-10-clearing.png) | [grid](./2026-11-27-cel-d-012-tabs-grid/grid-10-clearing.png) | The search answers eight applications, not the preview's three. The sweep's wash reaches past the caret: `main` does the same (Follow-up) |

- **Switching:** [list, 70 ms into a switch](./2026-11-27-cel-d-012-tabs-grid/list-11-switch-mid.png), [grid, same](./2026-11-27-cel-d-012-tabs-grid/grid-11-switch-mid.png): the old and the new content crossfade. For that moment the leaving label of Recents overlaps the All tab's glyph and label, where the preview unrolls the label's width instead. [list after Ctrl+Tab](./2026-11-27-cel-d-012-tabs-grid/list-12-ctrl-tab.png), [grid after Ctrl+Tab](./2026-11-27-cel-d-012-tabs-grid/grid-12-ctrl-tab.png): the next tab with its label.
- **Recents with no use:** not shot; the line reads `LAUNCHER_RECENT_EMPTY` (tested in `view::tests`). Its text is new copy for the author to accept.
- **Review:** 1 round (`/code-review` at high reasoning, Opus; the implementers were Sonnet). Fixed: the content keyed by the tab's position (a group appearing would crossfade the list), a panic path for a non-application row in a cell, and the grid's names measured again on every rebuild. Left: the label overlap during a switch (above) and the sweep's overshoot, which `main` has.

## Limits

- **Motion.** The crossfades were seen in single shots, not frame by frame.
- **Real input.** The keys came from `wtype` and the pointer from a virtual pointer; the author's keyboard and mouse were not used.

## Follow-up

- The hold-Backspace sweep's wash reaches past the caret, more on a longer text with a space: the binary built from `main` at `dd28ad85` shows it too (`CEL-I-014`).
- Match only a Flatpak entry's command name at the lowest search tier (carried from `CEL-I-015`).

## Landing

- **Base revision:** `dd28ad85c2c648176d97383cfffd1f3e727ec7e3`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2224230dc0ed734848b5ae066f01c946c5701f6f358536ae2fce555da97fe15c, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cf5d9d21cf327cda22a8dbafd40c0729fbab7d086f998823f320b6096c9dbb0a, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
