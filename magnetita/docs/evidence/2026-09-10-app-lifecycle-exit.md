# The production exit — MAG-M1-C

- **Date:** 2026-09-10
- **Scope:** `MAG-M1-C` of
  [`../plans/archive/2026-09-10-app-lifecycle.md`](../plans/archive/2026-09-10-app-lifecycle.md):
  `STATUS.md`, this record
- **Environment:** the author's session
- **Artifact:** `magnetita` and `magnetitad`, built, verified and installed
  by `scripts/complete-production.sh`

## Procedure

```sh
magnetita/scripts/complete-production.sh
```

## Result

- **Exit:** 0: `artifact: magnetita current and verified`, both binaries
  installed under `~/.local/bin`. `org.celestina.Devices1` and `Mirror1`
  are untouched, so the shell, Siderita and the daemon need no change;
  `magnetita-core` did not change, so the shell bundle's exit is not owed.

## Limits

- The running app instance, if any, carries the old bytes until the
  author restarts it.
