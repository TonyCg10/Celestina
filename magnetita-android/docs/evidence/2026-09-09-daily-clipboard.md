# Clipboard both ways: in front, tile, share target — AND-2-A

- **Date:** 2026-09-09
- **Scope:** `AND-2-A` of
  [`../plans/active/2026-09-09-daily-set.md`](../plans/active/2026-09-09-daily-set.md):
  `app/src/main/java/org/celestina/magnetita/clipboard/`, the clipboard
  additions in `link/` (`ClipboardPolicy`, the signals, the controller's
  outbound slot, the service), `MainActivity`, the device screen's row,
  strings, themes, the manifest, the tests; the roadmap and plan that open
  `AND-2`
- **Environment:** as `AND-1-A`; the phone was unplugged during this unit
- **Artifact:** `app/build/outputs/apk/release/app-release.apk`, built and
  verified, not yet installed

## Design

- **Inbound:** a `ClipboardText` signal writes the primary clip and tells
  the policy, so the same text is never sent back.
- **Outbound, three doors:** Android hands the clipboard only to the
  application in front. So the service listens to clip changes (they fire
  only in front) and reads the clip when the main window gains focus and
  when the desktop asks; the quick-settings tile opens a window that shows
  nothing, reads the clip once it has the focus, sends and closes; and the
  share target takes `ACTION_SEND` text from any application. The screen
  says so in one line.
- **Policy** (`ClipboardPolicy`, pure): text only, not blank, within the
  protocol's 256 KiB, no NUL, and never the value last exchanged in either
  direction.
- **Controller:** an outbound slot the session drains on its next poll,
  newest value wins, like the desktop's; signals now carry the decoded
  clipboard text the core provides.

## Procedure

```sh
cd magnetita-android
./gradlew --no-daemon -q assembleDebug testDebugUnitTest lintRelease
scripts/build-production.sh && scripts/verify-production.sh
```

## Result

- **Exit:** 0. 13 unit tests: the policy (echo, blank, bound, NUL), the
  signals (clipboard text decoded, request, undecodable stays other), and
  the controller (a handed clipboard goes out on the next poll; clipboard
  and request signals arrive decoded). `lintRelease`: 0 errors; the tile
  uses the `PendingIntent` form on 34 and the `Intent` form below it.

## Limits

- Not run on the S25U in this unit: the phone was unplugged. `VAL-MAG-12`
  covers the four doors against the daemon; the next plug-in installs the
  build and this record gains the observation.
- The clipboard listener in the service fires only with the application in
  front, by Android's rule; background changes reach the desktop through
  the tile or the share target, as the screen says.
