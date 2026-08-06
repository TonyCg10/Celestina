# Evidence: 2026-08-05 late provider insertion

- **Date:** 2026-08-05
- **Status:** implementation active
- **Scope:** celestina — the boundary at which an already-playing MPRIS source
  fails to become visible in the panel after a full shell start
- **Trigger:** failed `VAL-R1-01` rerun against deployed Celestina 0.6.2
- **Environment:** live session with Firefox playing before Celestina started
  and remaining visible to `playerctl`; the host-owned aggregate helper plus a
  separately started helper
- **Artifact:** deployed Celestina 0.6.2, the build the failed `VAL-R1-01`
  rerun was exercised against. No new artifact was produced for this record
- **Live rollback:** Celestina stopped, Noctalia restored, zero Celestina
  inhibitors remained

## Procedure

The failed `VAL-R1-01` rerun was narrowed by starting a second aggregate helper
beside the host-owned one and comparing what each published, then by replacing
only the host-owned helper and observing the panel again. The session was rolled
back to Noctalia afterwards.

## Result

### Observed boundary

Firefox was playing before Celestina started and remained visible to
`playerctl`. The first aggregate helper stayed alive with no media diagnostic,
yet the panel showed no media. A separately started helper published the valid
Firefox payload repeatedly. Replacing only the host-owned helper then made the
same payload visible. This excludes MPRIS discovery and leaves late insertion
into the host/panel provider map as the corrective boundary.

## Limits

The record establishes the boundary only. It does not identify which binding
step inside the host/panel provider map drops a key inserted after the first
binding, and it rests on one live session with one player (Firefox). No
automated evidence was recorded here; the corrective unit carries its own.
