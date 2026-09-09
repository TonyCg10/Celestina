# `magnetita-proto`: the daily-set catalog, the shared codec and the wire document — MAG-P1-C

- **Date:** 2026-09-09
- **Scope:** `MAG-P1-C` of
  [`../plans/active/2026-09-09-protocol-core.md`](../plans/active/2026-09-09-protocol-core.md): `src/codec.rs`, the new bounds in `src/bound.rs`, `src/lib.rs`, `src/daily/`, [the wire document](../protocol.md) and its README link
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
  - **One codec.** `codec.rs` is the only place a map is written or read: a
    `Map` builder with a known field count, and a `read` that hands each
    key to the message's own matcher and skips the rest. No message module
    calls the decoder's string or byte readers directly, so the bound rule
    holds by construction; four bounds were added for the clipboard, icons,
    file names and vCards.
  - **The daily set,** six capabilities, eighteen messages, each with a
    committed vector or a byte-exact assertion. Pinned refusals: a battery
    level over 100; a clipboard over 256 KiB; a notification icon over
    64 KiB, or an action without a label; a `share` name that is empty, `.`,
    `..`, or contains `/` or NUL; a media command that commands nothing, or
    a volume over 100; a request tolerates unknown fields.
  - **The wire document** lists every capability, kind, key, bound and both
    pairing constructions, and states the versioning rule.

## Limits

- Vectors pin this implementation; there is deliberately no second one.
- Messages, not behaviour: what a peer does with them is `MAG-P2` onwards.

## Follow-up

None for `MAG-P1-C`.
