# Evidence: the collapsible tray (CEL-I-012)

- **Date:** 2026-11-26
- **Scope:** `CEL-I-012`: `celestina/src/pieces/tray.rs`, `celestina/src/sheet`, `celestina/src/island`, `celestina/src/tray/menu.rs`, `celestina/src/app.rs`, `celestina/src/providers/settings.rs`, `celestina/src/copy.rs`, `celestina-rs/crates/celestina-shell/src/settings.rs`, a small rotation in `celestina/crates/celestina-ui` and `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-I-012`
- **Environment:** worktree `celestina-CEL-I-012`, branch `unit/celestina/CEL-I-012`, code commits `4d6492e2` and `e0ef0621`, the fixes `de722c34` and `513ba5fa`, the cleanup `1dc7cccb`, and the final review's fixes `20a95bb2` and `fa5a1a34`; the nested look ran on the nest's own bus; debug build in a private target directory
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** approved by the author on 2026-11-26, Windows style, list G2: `/home/toni/CODIGO/previews/2026-11-26-cel-i-010-012.html` (outside the repository). The shots below are the check against it

## Procedure

```sh
cd celestina
cargo fmt --all -- --check                               # ok
cargo clippy --workspace --all-targets --locked -- -D warnings   # clean
cargo test --workspace --locked          # 2732 passed, 0 failed, 2 ignored (at fa5a1a34)
```

The counts are the sum of every `test result` line of the run.

The nested check ran through the guarded kit: `env -i`, `--own-bus`, a fake
`wpctl` and `pactl`, nested niri at scale 1.5 on a 2560 by 1440 logical output,
a `shell.json` with the `tray` and `session` pieces in the end zone, and the
nest lock. A probe inside the nest (it aborts when its display, niri socket or
bus equal the host's) started three fake `StatusNotifierItem`s on the nest's
bus. None has a dbusmenu (`Menu` is `/NO_DBUSMENU`), so the shell draws its
one-entry card for each:

- **Theme path:** `Id` `fake-themepath`, orange disc, `Active`;
- **Missing icon:** `Id` `fake-missing`, no icon anywhere (the grey window glyph), `NeedsAttention`;
- **Item is menu:** `Id` `fake-ismenu`, blue disc, `Active`.

`bar.tray_hidden` held `fake-missing` at the start (`fake-themepath` for the
last run). A virtual pointer (`zwlr_virtual_pointer_manager_v1`) moved and
clicked; `grim` shot the nest's display, several times in a row after a press
to catch the motion. The files here are the bar's right part, cropped to 1000
by 560 pixels (physical, scale 1.5). Five nests ran a binary built from
`e0ef0621`. Two more ran a binary built from `de722c34` (window and hide
sequence only) and one ran a binary built from `513ba5fa` (window only). Those
three had a fourth fake item: `Id` `fake-long`, a long `Title`,
`NeedsAttention`. No test run overlapped a nest.

## Result

- **Chevron at rest:** [chevron-rest.png](./2026-11-26-cel-i-012-collapsible-tray/chevron-rest.png): a chevron-up at the tray's left end, then the orange disc, the blue disc and the power glyph in one capsule. The 7 px accent dot sits at the chevron's top right. The hidden item is not in the bar
- **Hover:** [chevron-hover.png](./2026-11-26-cel-i-012-collapsible-tray/chevron-hover.png): a rounded 32 px box lights behind the glyph
- **Window growing:** [window-birth-1.png](./2026-11-26-cel-i-012-collapsible-tray/window-birth-1.png) and [window-birth-2.png](./2026-11-26-cel-i-012-collapsible-tray/window-birth-2.png). In the first shot the window is a dark rounded shape with no row yet and the chevron's glyph is half turned (it reads as a corner); in the second the row is drawn faded in. It is a few frames of a short motion; the window does not look smaller than its final size, so the growth is mostly a fade here
- **Window open (binary from `513ba5fa`):** [window-open.png](./2026-11-26-cel-i-012-collapsible-tray/window-open.png): the chevron points down, lit, with its dot. The window is 240 wide, its right edge on the capsule's right edge, its top 56 logical pixels from the top. One row: the grey window glyph, the title cut in the middle with an ellipsis ("Mis…on" for "Missing icon") and the whole attention label (`TRAY_NEEDS_ATTENTION`) in the accent colour
- **Long title:** [window-long-title.png](./2026-11-26-cel-i-012-collapsible-tray/window-long-title.png): two rows, "Mis…on" and "A ve…fit" (the long title ends in "fit"). Each title has an ellipsis in the middle and each attention label sits whole inside the window, one row taller. [window-two-rows.png](./2026-11-26-cel-i-012-collapsible-tray/window-two-rows.png), from `e0ef0621`, shows the label clipped before the fix
- **Menu of a hidden app:** [hidden-app-menu-show.png](./2026-11-26-cel-i-012-collapsible-tray/hidden-app-menu-show.png): right click on the row. A card with one line, an eye glyph and "Mostrar en la barra", hangs under the window; the row stays highlighted
- **Menu of a bar app:** [bar-app-menu-hide.png](./2026-11-26-cel-i-012-collapsible-tray/bar-app-menu-hide.png): right click on the orange icon. A card holding "Ocultar de la barra" with the eye-off glyph. The card holds only that entry (no separator, as the app has no menu of its own). The log of the fake item shows the host also sent `ContextMenu` for each right click
- **Hiding, during the slide (binary from `de722c34`):** [hide-slide-1.png](./2026-11-26-cel-i-012-collapsible-tray/hide-slide-1.png), [hide-slide-2.png](./2026-11-26-cel-i-012-collapsible-tray/hide-slide-2.png), [hide-slide-3.png](./2026-11-26-cel-i-012-collapsible-tray/hide-slide-3.png). This time the long-title item (the right orange disc) was hidden. First: all icons in place. Second: the leaving disc is dim and covers the right half of the blue disc while the capsule narrows. It stays clear of the chevron. Third: the disc is gone and the chevron has moved right
- **Hiding, after:** [hide-after.png](./2026-11-26-cel-i-012-collapsible-tray/hide-after.png): the capsule holds the chevron (with its dot), the first orange disc, the blue disc and the power glyph. `shell.json` read `"tray_hidden": ["fake-missing", "fake-long"]`, the new id appended at the end
- **Showing one of two:** [show-one-before.png](./2026-11-26-cel-i-012-collapsible-tray/show-one-before.png), [show-one-1.png](./2026-11-26-cel-i-012-collapsible-tray/show-one-1.png), [show-one-after.png](./2026-11-26-cel-i-012-collapsible-tray/show-one-after.png): "Missing icon" leaves the window, which shrinks to one row, and the grey window glyph enters the bar (dim in the middle shot, with a fading remnant of the dot). The window stays open and the chevron stays; its dot is gone, as the remaining hidden app is not asking for attention. `shell.json` read `["fake-themepath"]`
- **Showing the last one:** [show-last-menu.png](./2026-11-26-cel-i-012-collapsible-tray/show-last-menu.png), [show-last-1.png](./2026-11-26-cel-i-012-collapsible-tray/show-last-1.png), [show-last-2.png](./2026-11-26-cel-i-012-collapsible-tray/show-last-2.png), [show-last-after.png](./2026-11-26-cel-i-012-collapsible-tray/show-last-after.png): the row's card says "Mostrar en la barra"; after the choice (the first shot is taken just before the change lands) the window shrinks and fades towards the chevron while the orange disc enters the bar; afterwards the bar holds the orange disc, the grey glyph, the blue disc and the power glyph, with no chevron and no window. `shell.json` read `"tray_hidden": []`

## Tests added

- **Hiding list:** `hiding_appends_an_id_once_and_showing_removes_it`, `set_tray_hidden_writes_the_key_and_keeps_others`, `own_write_is_not_reverted_by_a_stale_read`, `hide_entry_moves_the_id_into_tray_hidden`
- **Menu entry:** `menu_ends_with_hide_entry_for_a_bar_app`, `menu_ends_with_show_entry_for_a_hidden_app`, `app_without_menu_gets_a_card_with_only_the_entry`
- **Slot spring:** `hiding_a_tray_app_slides_the_icons_on_the_open_spring_and_settles`, `a_zone_change_still_jumps_when_the_hidden_list_changes_with_it`, `the_chevron_leaving_and_entering_slides_the_bar_on_its_spring`
- **Chevron and dot:** `chevron_shows_only_with_a_hidden_app`, `attention_dot_on_chevron_for_a_hidden_app`, `chevron_opens_the_overflow_window` (piece and Island)
- **Window:** `overflow_window_lists_hidden_apps_only`, `overflow_window_aligns_to_the_capsule_right_edge`, `overflow_row_left_click_activates`, `overflow_row_right_click_opens_the_app_menu`, `overflow_row_middle_click_secondary_activates`, `overflow_row_wheel_scrolls_the_app`
- **Review cases:** `last_hidden_app_gone_closes_the_window`, `a_hand_edit_of_the_hidden_list_updates_the_open_window`, `showing_the_last_hidden_app_closes_the_window_and_the_chevron_goes`, `escape_closes_the_apps_menu_first_then_the_window`, `the_window_lives_on_the_output_whose_chevron_was_pressed`
- **Toolkit rotation:** four tests in `crates/celestina-ui/tests/rotation.rs` (the glyph turn, the reduced-motion flip, no frame at rest)
- **Fix `de722c34`:** the tests in `crates/celestina-ui/tests/leaving.rs` (a leaving node fades where it was), `hiding_an_app_keeps_a_hand_edit_the_follower_has_not_delivered`
- **Fix `513ba5fa`:** `a_long_title_ends_in_an_ellipsis_and_the_attention_word_stays_inside_the_row`, `a_short_title_stays_whole` and the other tests it added to `sheet/tray_overflow.rs`

- **Final review fixes (`20a95bb2`, `fa5a1a34`):** `a_long_menu_cuts_the_apps_entries_and_never_the_shell_entry`, `a_cut_menu_draws_one_line_before_the_shell_entry`, `a_due_request_stays_for_its_card_until_the_message_takes_it`, `a_card_hung_from_a_window_row_goes_with_the_window`, `a_card_goes_when_its_app_is_shown_while_the_window_stays` and the `can_hide` test in `celestina-shell`

## Review

A model other than the implementers (Opus, high reasoning) ran `/code-review` over `e2a8ae81..9ac53371`: no Critical, three Important, six Minor. `20a95bb2` and `fa5a1a34` fix these:

- a long menu cut the shell's entry first; the app's entries are cut now;
- an app whose dbusmenu root stayed empty got no card; it gets the one-entry card after the 500 ms wait;
- "Ocultar de la barra" on a full list or an id the list cannot take did nothing; the entry is offered only when the list can take the id, and a write that changes nothing is logged;
- a card hung from a window row outlived the window, or its app shown from elsewhere; it closes with them;
- a cut menu could draw two lines in a row before the shell's entry.

A second round re-read the fixes and found no new Critical or Important.

## Limits

- Only fake items ran. Steam, Discord, nm-applet, blueman and solaar are `VAL-CEL-I-012`.
- No app with a working dbusmenu ran, so no shot shows an app's own entries followed by the separator and the hide entry. The menu tests cover it.
- The kit's virtual pointer sends no keys. The focus ring, Enter on the chevron and the Escape order (menu first, then window) were not seen live; `escape_closes_the_apps_menu_first_then_the_window` covers the order.
- Two outputs did not run live. The tests cover the window on the pressed output and the other bar following the list.
- One frame shows the glyph half turned; the spring curve was not measured on screen. Reduced motion did not run in the nest.
- The shots of the window's birth show a rounded shape fading in, then the final window. They cannot tell whether its size grows on the `CEL-D-009` spring.
- Not seen live: a hand edit of `shell.json` while the window is open, an app unregistering while it is open, the AT-SPI names and a window with enough rows to scroll.
- Beside the attention label a title gets about 80 physical pixels, so "Missing icon" shows as "Mis…on". The 240 px width and the label size come from the approved preview.
- A leaving icon fades in its place for 150 ms while its neighbour slides in, so for about one frame the two overlap. An icon that leaves inside its closing slot would need a slot for each tray icon in the bar tree.
- The window lists hidden apps in the tray's order, so an app asking for attention can sit below one that does not. The spec does not order them.
- `celestina-ui` paints a leaving node at its own last rect, without the slide of its ancestors (`de722c34`). The change applies to every leaving node in the shell.
- The author decides one case (review I3): an app with no dbusmenu gets the shell's card and the host's `ContextMenu` call at once. An app that answers `ContextMenu` with its own popup takes the grab, the compositor dismisses the card (and the window, when the card hangs from it), and the person cannot reach "Ocultar de la barra" from that app's menu. The fake items map no popup, so no shot shows it.
- The first hide or show writes the whole list, the defaults (`nm-applet`, `blueman`, `blueman-tray`) included, into `shell.json`. A later change of the defaults no longer reaches that file.
- A leaving node whose slide was still running jumps to its last laid-out rect before it fades (the toolkit change of `de722c34`).
- The give-up path sends the host `CloseMenu` before it opens the one-entry card; a `Menu` that still arrives after that opens a card the host no longer follows. It needs a late answer after 500 ms.

## Follow-up

- The author decides I3: (a) the card alone for such an app, with a way to ask for the app's own menu; (b) `ContextMenu` sent only when the card closes without a choice; or (c) keep both and write down that such apps are hidden from `shell.json` or the Island's Tray page.
- The author decides whether the title needs more room in a window row (a shorter label or a wider window).
- The author decides whether the one-frame overlap of a leaving icon needs a slot for each tray icon.

## Landing

- **Base revision:** `4e8cac212e373307bc229a402dea242d4cd954d2`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:10ffce897bf77c873242e8908fe4f1a12dec13c2b8eb5b3096c2f8d96a183ff3, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:21a776aa499e438ca7fd2731cf1f06c3cc23b4980343035e53f6c9fdff106768, verification_fingerprint sha256:bdf1f01d055fc0616141444c2e06baaf7d79efae197885f3f16168729c9be91b
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
