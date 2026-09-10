# Call state, mute, answer, hang up on the own wire — MAG-P4-G

- **Date:** 2026-09-09
- **Scope:** `MAG-P4-G` of
  [`../plans/archive/2026-09-09-daily-set.md`](../plans/archive/2026-09-09-daily-set.md):
  the call lines and buttons in `link_wire/phone.rs`, `call_changed` in
  `link_wire/mod.rs`, the registry's `callState`, `callNumber` and
  `callName`, `Devices1.CallAction`, the `telephony` setting, the desktop
  app's call banner with its three glyphs, the core's `send_call_event`
  and command decoding, the peer's `--call`, this record
- **Environment:** as `MAG-P4-E`
- **Artifact:** `magnetitad` and the desktop app, deployed

## Design

- A `CallEvent` sets the registry entry's `callState` (ringing, answered,
  missed; empty once ended), `callNumber` and `callName` (the phone's
  name, else the book's, else the number) and emits `Changed`, so the
  desktop app and the shell's phone menu read it additively.
- The event also drives one notification under the key `call`: ringing
  offers Mute, Answer and Hang up; answered offers Hang up; missed shows
  without buttons; ended closes it. A button pressed on the server
  becomes `CallCommand` through the same bridge as the phone's own
  notifications; `Devices1.CallAction` takes the same three words.
- The desktop app's device page shows a banner while there is a call,
  with the three actions as glyphs.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
magnetita-peer connect 10.0.0.134:1760 --contact "Ana|+34600111222" --call "+34600111222" --hold 5
busctl --user call … CallAction ss <peer id> HangUp
```

## Result

- **Exit:** 0. The loopback test sends a ringing call for a number the
  book resolves, finds `ringing` and `Ana` on the registry entry, the
  three-button notification, feeds the third button through the bridge
  and receives `HangUp` on the phone side; an ended call clears the entry
  and closes the notification.
- **Live:** not exercised with a real call. The peer path is the one
  recorded; `VAL-MAG-12` covers the phone.

## Limits

- The shell's phone menu reads `Devices1` and gains the three fields; its
  display of the call is the shell project's to add.
- Mute lowers the phone's ring volume rather than silencing one call;
  answering uses the deprecated telecom call that sideloaded apps still
  reach with the grant.
