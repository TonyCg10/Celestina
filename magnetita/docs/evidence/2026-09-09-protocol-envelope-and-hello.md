# `magnetita-proto`: envelope, hello, negotiation and the bound rule — MAG-P1-A

- **Date:** 2026-09-09
- **Scope:** `MAG-P1-A` of
  [`../plans/archive/2026-09-09-protocol-core.md`](../plans/archive/2026-09-09-protocol-core.md):
  `celestina-rs/crates/magnetita-proto` (`Cargo.toml`, `src/lib.rs`,
  `src/bound.rs`, `src/envelope.rs`, `src/error.rs`, `src/hello.rs`), the
  workspace manifest and lock, and the crate's registration in
  `docs/projects.toml`
- **Environment:** `celestina-rs` on the pinned `1.97.1` toolchain,
  `minicbor 2.3.0` (`std`, no derive), the workspace lints
  (`unsafe_code = "forbid"`, `clippy::all`)
- **Artifact:** not applicable; nothing consumes the crate until `MAG-P2`

## Procedure

```sh
cd celestina-rs
cargo fmt -p magnetita-proto -- --check
cargo clippy -p magnetita-proto --all-targets -- -D warnings
cargo test -p magnetita-proto
cd .. && scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0 for every command
- **Observed:** `test result: ok. 20 passed; 0 failed`; Clippy clean;
  `Documentation contract: OK`, `Language contract: OK`,
  `Architecture contract: OK`. What the tests pin:
  - the envelope's committed vector
    `a500010101021903e8031a00bc614e0443010203` and the hello's, both
    round-tripping byte for byte; changing either is a protocol change;
  - an unknown key is skipped, a missing key is refused by name, a future
    version is refused before any other field is read;
  - a body over 1 MiB is refused at its header before a byte is copied, a
    body at exactly the bound is accepted, an input over the message limit
    is refused before decoding starts, a length that lies about the buffer
    is refused, indefinite-length containers are refused;
  - a name over 128 bytes, more than 256 capabilities, a duplicate
    capability and an unknown device kind are each refused with their own
    typed reason; a hello with no capabilities is valid;
  - negotiation keeps the intersection at the lower version, in local
    order, and an empty remote yields an empty set.
- **Owner and boundary:** the crate depends on `minicbor` only; every
  peer-chosen length passes through `bound`, and the message modules never
  call the decoder's string or bytes readers directly. The registry lists
  the crate under Magnetita with its own component prefix
  `magnetita-proto`.

## Limits

- No consumer yet: the crate is proven by its tests, not by a link. `MAG-P2`
  is the first link.
- The vectors were produced by this implementation and then committed;
  they pin the wire from now on, they do not prove agreement with a
  second implementation, of which there is deliberately none.
- The error type collapses every CBOR-level failure to `Malformed("cbor")`
  by design; a developer who needs the offset runs the decoder directly.

## Follow-up

`MAG-P1-B`, pairing state machines, is next in the plan's build order.
