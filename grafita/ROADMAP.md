# Grafita implementation roadmap

- **Status:** idle
- **Active implementation checkpoint:** none
- **Related author validation:** none; completed observations and deliberate
  exclusions are in [VALIDATION.md](VALIDATION.md)

## No active implementation checkpoint

Grafita 1.0 is implemented and its G0-G6 arc is closed. Do not keep an empty
milestone open, copy manual checks into this file or turn conditional ideas into
work without a demonstrated need and author approval.

## Conditions for opening the next checkpoint

A new checkpoint must name one falsifiable problem, the affected core/host
boundary and a tangible result. Current candidates are conditional:

- a real unsupported text document plus an explicit reversible choice may open
  a legacy-encoding checkpoint;
- an accepted design-system decision plus demonstrated common semantics may
  open extraction of the two editor presentations;
- a measured startup, memory or interaction regression may open highlighter or
  large-document work.

Each candidate excludes unrelated IDE features and must preserve the content-
based acceptance and loss-free-save contracts.

## Implementation exit

For any newly authorised checkpoint, close implementation when its code and
focused domain tests pass and every affected deployable host completes its
registered production flow. Grafita-only work uses
`grafita/scripts/complete-production.sh`; a shared `grafita-core` change also
uses `siderita/scripts/complete-production.sh`. Each command builds its release
once, verifies those exact bytes and deploys them to the author's normal test
destination. A perceptible/manual result goes to `VALIDATION.md` and never
keeps the implementation checkpoint open.

## Closed evidence

The completed G0-G6 implementation, measurements, fixes and real-session
observations are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
