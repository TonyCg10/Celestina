# MAG-R2 — The mirror without discovery

- **Opened:** 2026-08-19
- **Closed:** 2026-08-19
- **Plan ID:** mirror-without-discovery
- **Status:** done
- **Authorization:** the author asked whether the mirror could stop depending on
  enabling Wireless debugging every time Android turns it off, and after the
  investigation asked for the result to be implemented
- **Scope:** magnetita
- **Implementation checkpoint:** MAG-R2
- **Author-validation checkpoint:** `VAL-MAG-09` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** none

## Hypothesis

`MAG-R1` delivered one press, but only while Android was advertising. The
advertisement is a property of wireless debugging, which Android turns off on
every reboot and at its own discretion — so the author was back to enabling it
by hand.

The claim this checkpoint tests is that the two things are separable: that
`adb tcpip` opens a *different* listener which survives wireless debugging
being turned off, and that pinning the phone to it once removes the discovery
dependency from every mirror after the first.

## Tangible outcome

The Mirror control reaches the phone with nothing advertised at all. Enabling
Wireless debugging becomes a once-per-phone-reboot gesture rather than a
constant one.

## Scope

- Pin the device to `adb tcpip` port 5555 once a discovered endpoint is up, and
  reconnect there.
- Remember that endpoint across daemon restarts, validated on load.
- Prefer a live advertisement; fall back to the remembered port on failure.

## Exclusions

- **Surviving a reboot of the phone.** Attempted and refused: `setprop
  persist.adb.tcp.port 5555` returned `Failed to set property`, and the phone
  has no `su`. This is a root-only capability and the checkpoint does not
  pretend otherwise.
- Any change to pairing. `MAG-R1`'s six-digit path is untouched.
- The QR pairing decision, still open and independent of this.

## Measured facts this plan is built on

Taken on the author's S25U, not reasoned about:

- With wireless debugging on, `settings get global adb_wifi_enabled` is `1` and
  the phone advertises. Setting it to `0` over adb stopped the advertisement
  (`avahi-browse` fell to zero records) while port 5555 **stayed open** and the
  device **stayed `device`**.
- The setting is writable from `adb shell`, so a connected phone can be told to
  re-enable its own wireless debugging — which also re-randomises the port, as
  the new advertisement on `33721` then `42293` showed.
- `persist.adb.tcp.port` cannot be set without root.

## Build order

One unit. The pin, the memory and the fallback are one behaviour: any two of
them without the third leaves the mirror unable to reach a phone that is not
advertising, which is the whole point.

## Implementation exit

```sh
cd celestina-rs
cargo fmt --all --check
cargo clippy -p magnetita-core -p magnetitad --all-targets --locked -- -D warnings
cargo test -p magnetita-core -p magnetitad --locked
```

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-R2-A | `magnetita:` | done | [exact inventory](../../inventories/2026-08-19-mirror-without-discovery/MAG-R2-A.numstat.tsv) | 12 files, +752/-81 | Pin the device to the fixed `adb tcpip` port once a discovered endpoint is up and reconnect there; remember that endpoint across daemon restarts, validated on load and refused unless it is the fixed port; prefer a live advertisement and fall back to the remembered port when the connection fails, once, so a dead pair cannot loop | `cargo test -p magnetita-core -p magnetitad` (132 + 84 pass), `clippy -D warnings`, `cargo fmt --all --check`; live on the S25U — recorded in [mirror without discovery evidence](../../evidence/2026-08-19-mirror-without-discovery.md) | `VAL-MAG-09` |

## What the live test changed

The fallback is not a precaution; it is a defect this plan's own first
deployment exposed. With wireless debugging freshly turned off, Avahi kept
serving the **cached** advertisement for about a minute. The daemon dialled
that dead port, failed, and gave up — reporting `connect-failed` while the
fixed port was answering the whole time. Preferring the advertisement is right;
treating its failure as final was not.
