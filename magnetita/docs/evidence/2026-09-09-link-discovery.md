# Discovery on `_magnetita._udp` through Avahi, both ways — MAG-P2-B

- **Date:** 2026-09-09
- **Scope:** `MAG-P2-B` of
  [`../plans/archive/2026-09-09-link.md`](../plans/archive/2026-09-09-link.md):
  `celestina-rs/crates/magnetita-link/src/discovery.rs` (the one parser),
  `celestina-rs/crates/magnetitad/src/link_wire/discovery.rs` (the
  daemon's advertise and browse), and `mirror_discovery.rs` now reading
  Avahi through the shared parser
- **Environment:** as in [the link record](2026-09-09-link-endpoint.md);
  the parser is tested on the output `avahi-browse -rpt` produced on this
  host during `MAG-R1`, and on a synthetic Magnetita listing
- **Artifact:** not applicable until `MAG-P2-C`'s deployment

## Procedure

```sh
cd celestina-rs
cargo test -p magnetita-link discovery
cargo test -p magnetitad mirror_discovery
cd .. && sh scripts/check-architecture-contract.sh
```

## Result

- **Exit:** 0
- **Observed:** `magnetita-link` gained `discovery`: the service type
  `_magnetita._udp`, the port `1760` (inside the range the author's `ufw`
  already admits, unused by KDE Connect), one `parse_resolved` for an
  `avahi-browse -rpt` line, one `reachability_rank` (loopback, link-local
  and unspecified addresses refused, IPv4 before IPv6 — the exact rule the
  mirror discovery had), and `parse_peers`, which keeps only names that
  are plausible device ids (alphanumeric, at most 64 bytes), non-zero
  ports and reachable addresses, ranked and de-duplicated. The mirror's
  discovery now delegates its line parsing and its ranking to that module
  and its own test of the observed `MAG-R1` output still passes byte for
  byte. The daemon advertises itself with one `avahi-publish` child owned
  by pid and process group for the daemon's lifetime, with the human name
  as a TXT record, and browses with the same bounded `avahi-browse` recipe
  the mirror uses; the dialer only dials advertised ids that are pinned and
  not already connected.
- **One owner:** two parsers of the same Avahi output would have been the
  second appearance of a recipe; there is one, in the crate both the daemon
  and the peer link.

## Limits

- Not observed against a real advertisement on the LAN: no phone speaks
  the wire yet. `MAG-P3` is the first advertisement from the other side.
- `avahi-publish` must exist on the host; when it does not, the daemon
  logs and listens without advertising, and the QR still names the
  address.

## Follow-up

None for `MAG-P2-B`.
