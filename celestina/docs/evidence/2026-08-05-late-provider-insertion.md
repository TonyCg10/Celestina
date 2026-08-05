# Evidence: 2026-08-05 late provider insertion

- **Status:** implementation active
- **Trigger:** failed `VAL-R1-01` rerun against deployed Celestina 0.6.2
- **Live rollback:** Celestina stopped, Noctalia restored, zero Celestina
  inhibitors remained

## Observed boundary

Firefox was playing before Celestina started and remained visible to
`playerctl`. The first aggregate helper stayed alive with no media diagnostic,
yet the panel showed no media. A separately started helper published the valid
Firefox payload repeatedly. Replacing only the host-owned helper then made the
same payload visible. This excludes MPRIS discovery and leaves late insertion
into the host/panel provider map as the corrective boundary.
