# Celestina: clearing notifications, a tray that works, and a collapsible tray

- **Date:** 2026-11-26
- **Entries:** `CEL-I-010` (notifications), `CEL-I-011` (tray fixes), `CEL-I-012` (collapsible tray), on the shell's standing plan `CEL-TRACKS`
- **Approved preview:** the author approved on 2026-11-26 the notifications and tray preview (variant A for notifications, Windows style with list G2 for the tray, and the calls listed in §5), published as a private artifact; its source is kept at `/home/toni/CODIGO/previews/2026-11-26-cel-i-010-012.html`
- **Status:** approved; implementation by the plans in `docs/superpowers/plans/2026-11-26-celestina-cel-i-010-plan.md`, `…-cel-i-011-plan.md` and `…-cel-i-012-plan.md`

## 1. Intent

After using celestina 1.26.0 the author asked for three things:

1. The notifications window cannot delete anything, and the bell's unread dot stays after the window is opened.
2. Many tray icons do not show, and some apps' right-click menus do not open.
3. A collapsible tray in the Windows style: a chevron button that opens a small window with the hidden apps, and an option on each app to hide it.

Success: the dot goes out when the window is seen, every row can be removed, the history can be emptied in one press, every app that registers a tray item shows an icon and opens its menu, and the bar holds only the apps the author wants, with the rest one press away.

## 2. Goals, non-goals, anti-goals

- **Goals:** the three requests above, on the menu windows of `CEL-D-009` (the window grows out of its piece, variant B) and the slot spring of `CEL-D-008`.
- **Non-goals:**
  - drag and drop to reorder or hide tray icons;
  - grouping notifications by app;
  - an undo for clearing the history;
  - a tooltip on tray icons (`DESIGN.md` §6 keeps tooltips out of the suite).
- **Anti-goals:**
  - a hidden app that asks for attention and leaves no trace in the bar;
  - the bar changing width when the tray window opens (the macOS-style inline expansion was rejected for this reason);
  - a tray change that blocks the loop on a slow or hostile D-Bus peer.

## 3. Today (1.26.0), from the code

### Notifications

- The domain lives in `celestina-rs/crates/celestina-shell/src/notifications.rs`:
  - `live` holds at most 5 notifications;
  - `history` holds at most 50, and the page shows 20.
- `MarkRead` marks only `live`, while `unread()` counts `live` and `history`. A notification that expires unseen, or that leaves `live` because a sixth one arrived, enters `history` unread. The bell's dot reads the page rows, so it stays lit after the window is opened.
- The rows can only be removed in part:
  - only live rows have a close button, and it moves the notification to `history`;
  - `NOTIFICATIONS_DISMISS_ALL` ("dismiss all") (`copy::NOTIFICATIONS_DISMISS_ALL`) closes the live ones only;
  - `NotificationsCommand::ClearHistory` exists, but no UI or verb sends it;
  - there is no command that removes a single history entry.

### Tray

- `src/tray/item.rs` never reads `IconThemePath`, `ItemIsMenu`, `AttentionIconName` or `AttentionIconPixmap`.
- `IconName` values are handled badly in two ways:
  - an `IconName` that holds an absolute path is rejected;
  - an app's private icon directory is never searched.
- The fallback icon is the theme name `app-window`, which the author's themes (Adwaita, hicolor) do not have, so the slot draws nothing.
- `pieces/tray.rs` and `tray/menu.rs` handle the right click badly:
  - it opens only a `com.canonical.dbusmenu` menu, and the `ContextMenu` method is never called;
  - `"/NO_DBUSMENU"` counts as a menu;
  - an app whose menu root is empty at first and fills on `LayoutUpdated` never gets its menu shown.
- An item with `ItemIsMenu = true` receives `Activate` on the left click instead of opening its menu.
- `bar.tray_hidden` in `shell.json` hides apps by exact `Id`. It can only be edited by hand, and hidden apps show only on the Island's tray page.

## 4. Design

### 4.1 CEL-I-010: notifications (variant A)

- **Seen means read.**
  - `mark_read()` marks every notification read, live and history alike.
  - The worker publishes a snapshot whenever any notification was unread.
  - The dot goes out while the window is born; with reduced motion it disappears without a fade.
- **Remove one.**
  - Every row, live or finished, has its close button, a 24 px button with radius 8 and a 16 px `x` glyph in `textMuted`.
  - The states are the toolkit's own: hover `controlHover`, pressed `controlPressed` at scale 0.96, keyboard focus with the 2 px `focusRing`.
  - The button removes the notification from the list. If it is still live, it is also closed (`CloseReason::Dismissed`), so its producer hears the close.
  - It is a new domain operation, `remove(id)`, behind a new command `NotificationsCommand::Remove(u32)`. The banner's close button keeps `Close`.
- **Clear all.**
  - The `NOTIFICATIONS_DISMISS_ALL` ("dismiss all") bar becomes a `NOTIFICATIONS_CLEAR_ALL` ("clear all") bar, a new copy constant `NOTIFICATIONS_CLEAR_ALL`, in the same place and style.
  - It shows whenever the list is non-empty. It closes the live notifications (`Dismissed`) and empties `history`.
  - It asks for no confirmation (the author's call). `copy::NOTIFICATIONS_DISMISS_ALL` and its bar are removed.
- **Motion.**
  - A removed row folds in 200 ms (`motion.normal`, One UI curve) while its opacity falls in 100 ms, and the rows below rise with it.
  - `NOTIFICATIONS_CLEAR_ALL` ("clear all") folds the rows with a 30 ms stagger, then shows the empty label `copy::NOTIFICATIONS_EMPTY`, and the window narrows to its minimum width (240) on the 350 ms spring.
  - Reduced motion: the rows go at once.
- **The IPC verb.** `celestina notifications clear` is added beside `dismiss-all`, for scripts.

### 4.2 CEL-I-011: tray fixes (no new look)

- **Icons**, in this order:
  1. `IconName` looked up in the app's `IconThemePath` first;
  2. then the user's theme;
  3. then `IconPixmap` (the size nearest to the output's scaled 24 px, no longer the smallest one ≥ 22);
  4. then the fallback.
- An absolute `IconName` path is accepted only when it points at a regular file under the item's `IconThemePath`, `$XDG_DATA_DIRS` or the user's data dir, and has a `.png` or `.svg` extension. Anything else is refused, so a peer cannot make the shell read arbitrary files.
- The fallback is the bundled Lucide `app-window` glyph from `celestina-style/icons` in `textMuted`, like the notifications' fallback bell, instead of a theme name.
- **Attention.** When `Status` is `NeedsAttention`, `AttentionIconName` or `AttentionIconPixmap` replaces the icon.
- **Menus.**
  - `"/NO_DBUSMENU"` and `"/"` mean no dbusmenu.
  - A right click on an item with no dbusmenu, or one whose layout fails, calls `ContextMenu(x, y)` with the pointer's output coordinates, so the app shows its own menu.
  - `ItemIsMenu = true` makes the left click open the menu too.
  - Opening sends `Event(0, "opened")`, and closing sends `Event(0, "closed")`.
  - An empty root keeps the request pending for one `LayoutUpdated`, up to 500 ms, before it gives up.
  - Every call stays within the existing 1 s timeout, off the loop thread.
- **Fresh themes.** The icon lookup is re-run when the tray item changes (`NewIcon`, `NewIconThemePath`). A theme installed after start is out of scope (the `freedesktop-icons` cache); that limit goes in `VALIDATION.md`.

### 4.3 CEL-I-012: collapsible tray (Windows style, list G2)

- **The chevron.**
  - The tray shows only visible apps, plus a chevron-up button at its left end.
  - It uses the box and states of a tray icon (32 px, 4 px pad, radius 12) with an 18 px Lucide `chevron-up` glyph.
  - Its accessible name is a new copy constant: `TRAY_HIDDEN` ("hidden icons"), and `TRAY_HIDDEN_ATTENTION` ("hidden icons, one needs attention") when a hidden app needs attention.
  - The chevron shows only while at least one registered app is hidden (the author's call). With no hidden app the tray is today's tray.
- **The window.**
  - A press on the chevron opens a menu window of `CEL-D-009`, grown out of the chevron (variant B).
  - Its right edge lines up with the right capsule's right edge (`CEL-D-009`'s end-zone rule, the author's call over the preview's centred placement), 8 px under the bar.
  - The window is 240 px wide and lists the hidden apps as rows like the Island's tray page: a 40 px row with a 24 px icon and the title, and `copy::TRAY_NEEDS_ATTENTION` in accent when the app needs attention.
  - While it is open the chevron stays lit and its glyph turns 180° on the 350 ms spring.
  - A row responds like a tray icon: left click `Activate` (or the menu when `ItemIsMenu`), right click the app's menu, middle click `SecondaryActivate`, the wheel `Scroll`.
- **Hide and show.**
  - Every app's right-click menu ends with a separator and one shell entry: `TRAY_HIDE` ("hide from the bar") (with a Lucide `eye-off` glyph) for an app in the bar, and `TRAY_SHOW` ("show in the bar") (`eye`) for an app in the window. Both are new copy constants.
  - The entry writes `bar.tray_hidden` in `shell.json` through `ShellSettings`, the way `set_do_not_disturb` writes its key.
  - The icon leaves or enters its bar slot on `CEL-D-008`'s slot spring.
  - Apps without a menu get the same entry in a menu of their own, holding only that entry.
- **Attention.** A hidden app that needs attention puts the 7 px accent dot on the chevron, in the bell's corner, and marks its row.
- **The Island page.** The tray page keeps listing every app, hidden or not.

## 5. Calls made after the preview, approved with it

- Each notifications row's close button removes the row and also closes a live notification.
- `NOTIFICATIONS_CLEAR_ALL` ("clear all") asks for no confirmation.
- The tray window lines up with the right capsule's edge, not centred under the chevron.
- The chevron disappears when no app is hidden.
- List G2 rather than icon grid G1, because the suite has no tooltips.

## 6. Constraints

- English in the repository; product copy only in `celestina/src/copy.rs`.
- No `unwrap`, `expect` or `panic!` in production; no `unsafe` outside `egl.rs` and `pam.rs`.
- D-Bus input is hostile:
  - icon paths are bounded as in §4.2;
  - pixmaps keep today's size limits;
  - no call blocks the loop.
- At rest the bar's frame count does not grow.
- Tests in the same unit for each domain change. The look and motion are checked on screenshots from the guarded nest (fake `wpctl`/`pactl`, `--own-bus`, `CELESTINA_DDC=0`, `CELESTINA_BACKLIGHT=0`), with a fake tray item on the nest's bus.

## 7. Order

`CEL-I-010` is independent. `CEL-I-011` comes before `CEL-I-012`, because both touch `src/tray` and `src/pieces/tray.rs`, and the collapsible tray builds on the fixed menus.
