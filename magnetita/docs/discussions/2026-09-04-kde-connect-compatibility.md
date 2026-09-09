# The KDE Connect wire after the own application reaches parity

- **Opened:** 2026-09-04
- **Status:** applied
- **Question:** once the own Android application delivers the author's daily
  set, is the KDE Connect wire removed from `magnetitad`, or kept alongside?

## Context

`magnetita-net` and the daemon's session layer implement the KDE Connect
handshake, pairing v8, payload transport and `sshfs` mount, hardened by
`MAG-S1`. During the program both wires must coexist so the author never
loses the phone link. Afterwards, keeping both means two trust stores, two
pairing flows, two discovery mechanisms and two sets of hostile-input
boundaries to maintain for one phone. This blocks `MAG-P7` and decides
whether Siderita's mount stays on `sshfs`.

## Strongest case

Remove it. The suite's rule is one owner per invariant and no second active
path once the replacement is proven; the KDE Connect wire would be a second
path forever. Its measured limits — manual phone-to-desktop clipboard, the
`sshfs` command surface, one blocking stream — are the reasons the own
protocol exists. Removing it also lets the daemon drop the KDE Connect UDP and
TCP ports and the mount subprocess, shrinking the attack surface `MAG-S1`
audited.

## Counter-case

The stock KDE Connect client pairs with any KDE Connect desktop, so the
author's phone keeps working with other machines; the own app only works
with Magnetita. Keeping the wire costs nothing until it breaks, and it is the
only tested path today. Removing it before the own `storage` capability
exists also removes Siderita's phone browsing.

## Alternatives

- Remove the wire in `MAG-P7`, after `storage` over the own protocol replaces
  the mount for Siderita. Leading option.
- Keep the wire, frozen, behind a setting that defaults to off, and delete it
  in a later maintenance checkpoint once the own app has a year of daily use.
- Keep both indefinitely. Rejected by the one-owner rule unless the author
  overrides it.

## Falsifiers and evidence needed

The author's daily use of the own app through `MAG-P4`–`MAG-P6` without
reaching for the stock client, recorded as `VAL-MAG` results, and the
`storage` capability browsing the phone from Siderita. Any daily task that
still needs the stock client keeps the wire until that task is covered.

## Conclusion

**Remove the KDE Connect wire in `MAG-P7`**, concluded by the author on
2026-09-04, after `storage` over the own protocol replaces the `sshfs` mount
for Siderita. One owner per invariant: one trust store, one discovery, one
wire. The counter-case is honoured by ordering — nothing is removed until
`VAL-MAG-12` through `VAL-MAG-14` record daily use of the own app — and by
the ADR's *Revisit when*. Applied in
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md) §8 and
`MAG-P7-C`.
