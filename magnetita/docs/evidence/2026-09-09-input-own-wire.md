# The virtual pointer and keyboard — MAG-P5-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P5-B` of
  [`../plans/archive/2026-09-09-remote-control.md`](../plans/archive/2026-09-09-remote-control.md):
  `celestina-rs/crates/magnetitad/src/link_wire/input.rs`, the session's
  input and datagram branches in `link_wire/mod.rs`, the `evdev`
  dependency, the core's input sends and command decoding, the peer's
  `--run` and `--type`, the plan and roadmap records that archive
  `MAG-P4` and open `MAG-P5`, this record
- **Environment:** the workspace's loopback tests; this host, where
  `/dev/uinput` carries the `uaccess` rule `HOST-HYGIENE.md` records
- **Artifact:** `magnetitad`, deployed

## Design

- `InputSink` is the boundary: the wire's motion, buttons, scroll, keys
  and text are decoded at the session (key codes outside 1..=248 are
  refused) and handed to the sink after a governor of four thousand
  events a second per session. The production sink opens one virtual
  device, "Magnetita phone", through the `evdev` crate's safe builder on
  the first event: relative axes, three buttons, the whole keyboard; it
  is destroyed with the daemon. The loopback tests record instead.
- Text is typed key by key from a US table with shift where needed;
  characters off the table are skipped. Scroll arrives in 1/120 wheel
  steps and goes out as high-resolution and, per whole notch, coarse
  wheel events.
- Motion may arrive as a datagram: the session reads datagrams next to
  the control stream and treats them the same.

## Procedure

```sh
cd celestina-rs && cargo test -p magnetitad
cargo test -p magnetitad the_virtual_device_opens -- --ignored
```

## Result

- **Exit:** 0. The table, the dispatch and the governor have their tests;
  the loopback test sends a key and a text on the stream and motion as a
  datagram and finds all three in the recorder. The ignored test opened
  and closed the real device on this host without one event.

## Limits

- No event was emitted on this host: the virtual device feeds the live
  compositor, and moving the author's pointer from here is the one thing
  these records never do. `VAL-MAG-13` is the author's hand on the phone.
- The table is a US layout; the desktop's own keymap is not consulted.
