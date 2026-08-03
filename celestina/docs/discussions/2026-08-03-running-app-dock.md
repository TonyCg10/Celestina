# Running-app dock retention

- **Opened:** 2026-08-03
- **Status:** open
- **Question:** Does the lived shell still need a running-application dock after Noctalia is removed?

## Context

The dock is visible product scope with window tracking, launch/focus semantics
and output behavior. It should not be rebuilt merely because the old shell had
one.

## Strongest case

It provides a persistent, discoverable way to launch and focus frequent
applications without opening the launcher.

## Counter-case

It duplicates launcher/workspace behavior and adds continuous state, layout and
hotplug cost to the final removal phase.

## Alternatives

Drop it, keep a minimal favorites-only surface, or retain full running-app
state after lived use proves the need.

## Falsifiers and evidence needed

The author's experience using the completed shell without the dock and a clear
workflow that the launcher/workspace controls do not cover.

## Conclusion

Pending. R8 must record the author decision before adding a dock slice.
