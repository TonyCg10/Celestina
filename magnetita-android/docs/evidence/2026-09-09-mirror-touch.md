# Touches and navigation from the desktop — AND-4-B

- **Date:** 2026-09-09
- **Scope:** `AND-4-B` of
  [`../plans/active/2026-09-09-link-mirror.md`](../plans/active/2026-09-09-link-mirror.md):
  `app/src/main/java/org/celestina/magnetita/mirror/MirrorInput.kt`,
  `app/src/main/res/xml/mirror_input.xml`, this record
- **Environment:** as `AND-1-A`
- **Artifact:** the same release build as `AND-4-A`

## Design

`MirrorInput` is an accessibility service with `canPerformGestures`. A
desktop touch arrives in the picture's pixels, `MirrorGeometry.toScreen`
maps it onto the screen, and the service plays a finger as one stroke
continued segment by segment: `down` opens it, each `move` extends it
from the last point over 16 ms, `up` closes it. Back, home and recents are
`performGlobalAction`. Key codes are received and ignored: the API cannot
inject them. The device screen's "Control de pantalla" row opens the
system's accessibility settings where the person enables it.

## Procedure

```sh
cd magnetita-android && ./gradlew --no-daemon -q testDebugUnitTest lintDebug
```

## Result

- **Exit:** 0; the geometry the service relies on is pinned by the JVM
  tests of `AND-4-A`.

## Limits

- Multi-finger gestures are separate strokes, not one gesture: pinches
  are not reproduced.
- Whether a continued stroke lands as a drag on the S25U is the author's
  observation (`VAL-MAG-14`).
