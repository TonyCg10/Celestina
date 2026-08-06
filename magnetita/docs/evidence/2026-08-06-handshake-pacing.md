# Evidence: 2026-08-06 the handshake waits instead of spinning

- **Date:** 2026-08-06
- **Scope:** `MAG-S1-C`; plan
  [network-input-hardening](../plans/active/2026-08-05-network-input-hardening.md);
  a low finding of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source correction with compilation, lint and unit tests. No
  production build, no deployment, no daemon started, no phone paired
- **Artifact:** none; no production build ran

## What was wrong

`complete_tls` loops until the handshake finishes or the deadline passes. Two of
its arms retry. The retryable-timeout arm is paced by the socket read timeout,
which is what makes the loop cheap. The other arm — still handshaking, and
`complete_io` returned without blocking — retried immediately with nothing to
slow it down, so it would spin a link pump thread at full CPU until the deadline
expired.

It is a latent shape rather than an observed failure: no state was found that
reliably produces it, and the absolute deadline added by `MAG-A1` bounds how
long it could last. Closing it costs one line, which is cheaper than continuing
to reason about whether it is reachable.

## What changed

- `magnetita-net/src/link.rs`, `complete_tls` — the non-blocking still-handshaking
  arm sleeps for `HANDSHAKE_POLL_INTERVAL`, the same interval the socket timeout
  uses, because it is waiting for the same thing: more bytes from the peer.

## Procedure

```sh
cargo test -p magnetita-net                              # in celestina-rs/
cargo fmt --all --check
cargo clippy -p magnetita-net --all-targets --locked -- -D warnings
```

## Result

| Command | Result |
|---|---|
| `cargo test -p magnetita-net` | 45 pass, 0 fail |
| `cargo fmt --all --check` | clean |
| `cargo clippy -p magnetita-net --all-targets` | passes, no diagnostics |

No test was added. The arm is reached only when a TLS implementation returns
`Ok` without blocking while still handshaking, which the existing fixtures do
not produce and which a fake would assert about the fake rather than about
rustls. What the change guarantees is a bound on the loop, and the existing
deadline tests already cover the loop terminating.

## Limits

This is pacing, not a fix for an observed hang: nobody has seen the spin. The
claim is that the arm cannot spin any more, not that it ever did.
