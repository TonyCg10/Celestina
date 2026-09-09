# Gestures from the desktop through the phone's accessibility service — MAG-P0-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P0-C` of
  [`../plans/archive/2026-09-07-own-protocol-spikes.md`](../plans/archive/2026-09-07-own-protocol-spikes.md);
  verifies the input half of the
  [mirror discussion](../discussions/2026-09-04-mirror-capture-path.md)
- **Environment:** the phone, Wi-Fi and toolchain of
  [the QUIC phone record](2026-09-08-quic-on-the-phone.md). Phone: a
  throwaway `AccessibilityService` (`canPerformGestures`) holding one QUIC
  connection to the desktop through the `magspike` core (keep-alive 1 s),
  turning text commands into `dispatchGesture` strokes — a 20 ms tap, a
  120 ms swipe — and a throwaway `TouchActivity` reporting the desktop-synced
  time of every `ACTION_DOWN` it receives. Desktop: scratch `inputspike`
  server on UDP 1759 sending 40 taps and 20 swipes 400 ms apart, matching
  each to the service's `onCompleted` and to the screen's `ACTION_DOWN`.
  Clock offset 186 ms at a 5 ms round-trip, synced as in the mirror record
- **Artifact:** not applicable

## Procedure

```sh
./inputspike                                   # desktop: waits on UDP 1759, then drives 60 gestures
adb shell am start -n com.example.magnetita/.TouchActivity
# the accessibility service is enabled on the phone; the touch screen stays in front
```

Two timestamps per gesture, both against the desktop's clock: *landed* is
the moment the target activity received the touch — what a user would call
the tap happening — and *dispatched* is the service's completion callback,
which fires only after the stroke's own duration.

## Result

- **Exit:** 0; 60 of 60 gestures reported both timestamps, none incomplete
- **Observed:**

```
FINAL tap   dispatched  n=40 p50=  68 p90= 117 max= 167 min=  28 ms
FINAL tap   landed      n=40 p50=  46 p90=  88 max= 145 min=   7 ms
FINAL swipe dispatched  n=20 p50= 198 p90= 207 max= 213 min= 127 ms
FINAL swipe landed      n=20 p50=  74 p90=  86 max=  89 min=   6 ms
```

  - A tap from the desktop lands on the phone's screen at the median 46 ms
    after it was sent, best 7 ms, p90 88 ms. The service's own share is
    small: the command reached the phone 16–75 ms after sending (the same
    Wi-Fi jitter every record shows) and the gesture landed 10–20 ms
    after that; the swipe's `dispatched` figure is the 120 ms stroke
    itself plus that.
  - The best cases (7 ms, 6 ms) are inside the clock-sync error, which is
    about half the 5 ms round-trip; they say the floor is the network,
    not the service.
  - Nothing was refused or cancelled: `dispatchGesture` accepted every
    stroke while the test activity was in front.

### What it took, for `MAG-P5` and `MAG-P6`

- Android 13+ marks a side-loaded app's accessibility service as a
  *restricted setting*: the author's toggle was rejected twice
  (`ACCESS_RESTRICTED_SETTINGS` app-op `rejectTime`), and the
  "Allow restricted settings" menu the documentation promises did not
  appear on One UI. `adb shell cmd appops set <pkg> ACCESS_RESTRICTED_SETTINGS allow`
  lifted it; after that the service enabled and bound. Every reinstall
  re-arms the restriction. The product app must document this once, or
  be installed in a way Android treats as a store.
- Writing `enabled_accessibility_services` with a value that carried a
  stray newline silently dropped another app's service (Link to Windows);
  it was restored the same minute. Never compose that setting from a file.
- The phone-side control link must read commands and write reports under
  separate locks; with one lock every report waited for the next command.

## Limits

- Taps and one swipe on a test activity that welcomes them; a secure
  surface or an app that refuses accessibility gestures was not tried.
- Text entry and key events through the service were not measured; they
  are a different API (`performAction` with `ACTION_SET_TEXT`) and go on
  `MAG-P6`'s list.
- One evening, one phone, the same Wi-Fi as the other records.

## Follow-up

None for `MAG-P0-C`; the mirror discussion cites this record for input.
`MAG-P0`'s only unmeasured item is the nest half of `MAG-P0-D`.
