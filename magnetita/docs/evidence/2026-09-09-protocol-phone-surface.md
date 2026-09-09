# `magnetita-proto`: sms, contacts and telephony — MAG-P1-E

- **Date:** 2026-09-09
- **Scope:** `MAG-P1-E` of
  [`../plans/archive/2026-09-09-protocol-core.md`](../plans/archive/2026-09-09-protocol-core.md): `src/phone/`
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
  - **SMS**: conversations, thread pages (limit 1–256, refused outside),
    a send that must carry a body, and received messages; attachments are
    named by `share` transfer id. Addresses are identifiers, bodies are
    text, both bounded.
  - **Contacts**: one-way, versioned per contact, paged with a `complete`
    flag; a vCard over 16 KiB is refused; a request without a version means
    everything.
  - **Telephony**: four call states and three actions, each refused when
    unknown; the resolved name is optional.

## Limits

- Vectors pin this implementation; there is deliberately no second one.
- Permissions, `TelecomManager` and the SMS provider are the phone's
  (`MAG-P4`), not the protocol's.

## Follow-up

None for `MAG-P1-E`; `MAG-P1` is complete and `MAG-P2`, the link, is next.
