# Evidence: 2026-08-06 a byte-exact way to send a file to the phone

- **Date:** 2026-08-06
- **Scope:** `MAG-S1-B`; plan
  [network-input-hardening](../plans/active/2026-08-05-network-input-hardening.md);
  the `send_to_phone` item of stage 3 in the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md),
  applying [ADR 0008](../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md)
  to `org.celestina.Devices1`
- **Environment:** source correction with formatting, lint and unit tests for
  the `celestina-rs` workspace excluding `celestina-shell-core`, which the
  author's hardware-safety hold puts out of bounds. No production build, no
  deployment, no restart or inspection of the live `magnetitad`, no phone paired
- **Artifact:** none; no production build ran

## What was wrong

`SendFile(device_id, path)` takes the path as a `String`, and a Linux filename
is a byte string that is not required to be UTF-8. A caller holding such a name
has no way to put it in that argument: converting it lossily replaces each
invalid byte with U+FFFD, and what arrives names a file that does not exist.
Siderita's send-to-phone menu item did exactly that, and it was the last verb in the
suite that let a lossy path leave a process.

The argument's type was also the one input to this interface that was simply
trusted to be well formed, which is the discipline the rest of this plan
applies everywhere else.

## What changed

- `crates/magnetitad/src/devices.rs::send_file_uri` — a new `SendFileUri`
  method taking the percent-encoded `file://` URI. It is the spelling the suite
  already speaks wherever a path leaves a process — the document portal, the
  clipboard, a drag payload — so this adds no encoding to the contract, only a
  second door onto the same one.
- `crates/magnetitad/src/devices.rs::path_for_file_uri` and `FileUriError` — the
  decode, over `celestina_core::percent::decode_strict`, the codec the suite
  already owns. A URI that is not `file://`, one carrying an authority that is
  not this host, and one whose escapes are malformed, empty, absolute-less or
  NUL-bearing are each refused with their own reason rather than salvaged: a
  malformed URI means the sender is not speaking this contract, and guessing at
  its intent is how a transfer reaches the wrong file. `%23` and `%3F` decode to
  ordinary name characters here, because the encoder that produced them escaped
  them precisely so a name would not be read as ending early.
- `crates/magnetitad/src/devices.rs::Command::SendFile` — carries a `PathBuf`
  instead of a `String`. This is daemon-private, and leaving it a `String` would
  have discarded the bytes one line after decoding them. `crates/magnetitad/src/
  link_commands.rs` drops the `PathBuf::from` that used to rebuild it.
- `crates/magnetitad/src/devices.rs::send_file` — unchanged in meaning and in
  wire shape, and now documented as the compatibility path: it is a published
  interface, and altering what its argument means would break any other caller.

The consumer side is Siderita's `SID-G7-G`, recorded in
[its evidence](../../../siderita/docs/evidence/2026-08-06-path-key-correctness-debt.md).

## Procedure

```sh
cd celestina-rs
cargo fmt --all --check
cargo clippy --workspace --exclude celestina-shell-core --all-targets --locked -- -D warnings
cargo test --workspace --exclude celestina-shell-core --all-targets --locked
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --exclude celestina-shell-core --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --workspace --exclude celestina-shell-core --all-targets --locked` | every crate green; `magnetitad` 67 passed, 0 failed |

Three tests were added to `devices.rs`. The round trip pins the URI for a name
that is not valid UTF-8 — `file:///home/u/na%FFme` against the bytes
`b"/home/u/na\xffme"` — beside a name with a space, a name with a `#`, and the
`localhost` authority. The refusal pins each typed reason: a non-`file` scheme,
a bare path, a remote authority, a truncated escape, a non-hexadecimal escape,
an empty URI and an embedded NUL. The third asserts that the command built from
a decoded URI still holds those bytes, and that the same name put through
`to_string_lossy` does not — which is the defect, stated as an assertion rather
than as prose.

## Limits

`celestina-shell-core` was excluded from every command. The author's hold on
Celestina forbids building or running it after two GPU losses, and this unit
touches nothing it depends on.

Nothing was exercised over a real bus. `SendFileUri` was not called from
`busctl`, no `magnetitad` was started, and no file was transferred to a phone:
what is proven is that the decode answers the right bytes and the right
refusals. That a file whose name is not valid UTF-8 actually arrives on the
phone belongs to `VAL-MAG-HARDENING` in
[`../../VALIDATION.md`](../../VALIDATION.md).

The name the phone is shown for such a file still falls back to `archivo`,
because `link_commands.rs` needs a `&str` for the peer's metadata and that name
has no UTF-8 spelling. The bytes served are the right file's; only its label is
generic. That is a display limit, not a delivery one, and it is unchanged by
this unit.
