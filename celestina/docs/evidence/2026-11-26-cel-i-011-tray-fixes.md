# Evidence: tray icons that never resolved and tray menus that never opened (CEL-I-011)

- **Date:** 2026-11-26
- **Scope:** `CEL-I-011`: `celestina/src/tray`, `celestina/src/providers/tray.rs`, `celestina/src/pieces/tray.rs`, `celestina/src/island`, `celestina/src/app.rs`, the icon lookup in `celestina/crates/celestina-paint`, their tests, the ledger row and `VAL-CEL-I-011`
- **Environment:** worktree `celestina-CEL-I-011` from `main`; the nested look ran on the nest's own bus
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** not needed: no new look. The fallback glyph is the bundled `app-window` icon in `textMuted`, as the approved design (4.2) says, and the menus are the existing ones

## Procedure

```sh
cd celestina
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked          # 1862 lib tests, three runs in a row
```

The nested check ran through the guarded kit: `env -i`, `--own-bus`, a fake
`wpctl` and `pactl`, nested niri at scale 1.5 on a 2560 by 1440 logical output,
a `shell.json` with the tray and session pieces at the end of the bar and
`bar.tray_hidden` empty, and the nest lock. A probe inside the nest (it aborts
when its display, niri socket or bus equal the host's) started a small
`StatusNotifierItem` server on the nest's bus and registered three items with
the shell's watcher:

- **Theme path:** `IconName` `fakeorange`, `IconThemePath` a temporary
  directory holding `fakeorange.png`, `Menu` `/NO_DBUSMENU`, `ItemIsMenu` false;
- **Missing icon:** an `IconName` found nowhere, no pixmap, no theme path,
  `Menu` `/NO_DBUSMENU`;
- **Item is menu:** `IconName` `fakeblue` in the same directory, `Menu`
  `/NO_DBUSMENU`, `ItemIsMenu` true.

Every item logs the calls it receives to a file. A virtual pointer
(`zwlr_virtual_pointer_manager_v1`) clicked each icon in the bar and `grim`
shot the nest's display.

## Result

- **Icons:** [tray-icons.png](./2026-11-26-cel-i-011-tray-fixes/tray-icons.png): the orange disc is the icon found in the item's own `IconThemePath` (before this unit it drew nothing), the grey window glyph is the fallback for the item whose icon exists nowhere (before: an empty slot), the blue disc is the third item's icon
- **Context menu calls:** [context-menu-calls.log](./2026-11-26-cel-i-011-tray-fixes/context-menu-calls.log), in click order, with the pointer's logical position (the surface is anchored to the output's whole top edge, so these are output coordinates; the icons sit at x 2409, 2443 and 2478, y 27):
  - right click on the theme path item (`/NO_DBUSMENU`): `ContextMenu(2409,27)`;
  - right click on the missing icon item: `ContextMenu(2443,27)`;
  - left click on the item that is only a menu: `ContextMenu(2478,27)` and no `Activate`;
  - right click on the same item: `ContextMenu(2478,27)`;
  - left click on the theme path item (not only a menu): `Activate(0,0)`
- **After the clicks:** [tray-after-clicks.png](./2026-11-26-cel-i-011-tray-fixes/tray-after-clicks.png): the bar is unchanged and no menu of ours opened, because these items serve none; the item would show its own
- **Tests added:**
  - `no_dbusmenu_path_is_no_menu`;
  - `right_click_without_menu_calls_context_menu` and `failed_layout_falls_back_to_context_menu` (over a private bus with a fake item);
  - `item_is_menu_opens_the_menu_on_left_click` (the piece and the Tray page);
  - `empty_root_waits_for_one_layout_update` and `empty_root_gives_up_after_500_ms`;
  - `opened_and_closed_events_are_sent`, `an_empty_root_is_followed_for_its_late_layout` and `a_failed_layout_is_not_followed_nor_opened`
- **Pointer position:** the bar's input handler keeps the last pointer position in the surface's logical coordinates. The Island's surface is anchored to the whole top edge with no margin, so these are the output's coordinates, rounded to whole pixels (a keyboard request uses the row's centre)
- **Late layout:** an empty root keeps the pending request for 500 ms from the first empty one, using the surface's own `redraw_after` timer and a check in the next frame, never a sleep; the host stops following the item when the request is dropped
- **Review:** not run by the implementer; the unit's final review is the controller's

## Limits

- The host resolves icon files after the item's read budget, without a bound of its own: a peer whose `IconThemePath` sits on a hung mount blocks the tray host thread (the whole tray stops; the shell's loop keeps running). `theme_dir`'s `is_dir` did the same before this unit.
- `broad_icon_dir` reads the home directory from `$HOME`, without a fallback to the password database.
- Icon files in an item's own directory are looked up for 48 px (the bar's 24 at scale 2), because the host does not know the output's scale; a scale 1 output scales them down.
- Only fake items were used. Steam, Discord, nm-applet, blueman, solaar and shelly were not run; they are `VAL-CEL-I-011`.
- No item with a working dbusmenu was driven in the nest, so the shell's own menu opening was not shot; the host's menu path is covered by the bus tests in `dbusmenu.rs`.
- The `ContextMenu` call was seen to arrive; the menu the app then draws belongs to the app and was not seen.
- A theme installed after the shell started is not seen until a restart (the `freedesktop-icons` cache): the design's stated limit.
- The nest ran twice (one run for the icons, one for the clicks), both with the binary built from the unit's code commit.
- `ContextMenu` gets the pointer in the shell's output-local logical coordinates; the StatusNotifierItem spec expects screen coordinates. The shell does not know an output's position in the global space (its output info has size and scale only), so no offset is added; on a second output an app may place its menu off by that output's origin.
- The icon file is checked, then drawn later: a peer can swap a symlink inside its own directory in between. The painter re-reads with its own size bound, so the risk is low and accepted.
- An SVG a peer names (at most 1 MiB) is decoded on the loop thread, as the user's theme icons already are; a crafted one can stall it. The broad-directory refusal narrows the new surface; a follow-up item moves the decoding off the thread.
- The icon files are looked up on the host's thread for a 24 px size with the freedesktop sizes around it; the output's scale is not known there.

## Follow-up

None.

## Landing

- **Base revision:** `7df47858d29896693a42990a7fd03e936ba6f7c2`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:6731ce13a3401656cb7b3aa3a46b5e56e44da7c0544a9ec95de16837f174e48d, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
