# `magnetita-proto`: commands, input and the mirror session — MAG-P1-D

- **Date:** 2026-09-09
- **Scope:** `MAG-P1-D` of
  [`../plans/active/2026-09-09-protocol-core.md`](../plans/active/2026-09-09-protocol-core.md): `src/control/` and `src/mirror.rs`
- **Environment:** as in
  [the envelope record](2026-09-09-protocol-envelope-and-hello.md); no new
  dependency
- **Artifact:** not applicable

## Procedure

```sh
cd celestina-rs
cargo fmt -p magnetita-proto -- --check
cargo clippy -p magnetita-proto --all-targets -- -D warnings
cargo test -p magnetita-proto
cargo check --workspace
cd .. && scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0 for every command, run once for the three units closed together
- **Observed:** `test result: ok. 63 passed; 0 failed` for the crate; Clippy
  clean; workspace check clean; the three contracts `OK`.
  - **Commands** carry an id and a name and nothing else: there is no field
    for a command line, and a name longer than an identifier is refused, as
    is a duplicate id.
  - **Input** is five typed events — motion, button, scroll, evdev key,
    text — with motion also allowed as a datagram; motion wider than `i16`
    and an unknown button are refused.
  - **The mirror** opens with the desktop's request (longest edge, fps,
    bit rate, codec, audio) and the phone's answer (the size and codec it
    actually encodes); touches, Android key codes and the three global
    gestures go back. A mirror at 0 fps, 0×N pixels, an unknown codec or an
    unknown gesture is refused.

## Limits

- Vectors pin this implementation; there is deliberately no second one.
- The video and audio streams themselves are raw elementary streams on
  their own QUIC streams, framed by the link, not by this crate.

## Follow-up

None for `MAG-P1-D`.
