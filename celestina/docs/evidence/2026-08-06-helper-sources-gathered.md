# Evidence: 2026-08-06 the helper sources are gathered, not listed

- **Date:** 2026-08-06
- **Scope:** `LVR-3-E`; plan
  [late-provider-insertion](../plans/active/2026-08-05-late-provider-insertion.md);
  a low finding of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source and text only. The GPU safety hold stands, so nothing
  was configured, compiled, tested, built, deployed or run
- **Artifact:** none, and none may be produced during the hold

## What was wrong

`celestina-helpers` listed its `DEPENDS` by hand: ten files, where `src/` holds
nineteen, and none of `celestina-shell-core`, which the helpers also compile.

Nothing breaks today. The target is `ALL`, so CMake always considers it out of
date and runs cargo, which does its own change tracking. But a `DEPENDS` list is
read as a statement about what a target is built from, and this one had drifted
into saying something untrue. It would become a real staleness bug the moment
anyone converted it to `add_custom_command`, which is exactly the kind of change
someone makes while trusting the list.

## What changed

- `CMakeLists.txt` — the sources are gathered with `file(GLOB_RECURSE …
  CONFIGURE_DEPENDS)` over `src/*.rs` and the shell core's `src/*.rs`, with the
  two manifests appended. `CONFIGURE_DEPENDS` makes the build system re-run the
  glob when the directory contents change, so the list stays true without anyone
  remembering to update it.

## Procedure

None. No command was run against this project.

```text
The GPU safety hold forbids running any Celestina executable, provider,
build, test, deployment or activation — configuring the build included.
```

## Result

Not verified by execution. Reviewed by reading: `GLOB_RECURSE` with
`CONFIGURE_DEPENDS` is the documented CMake spelling for a dependency set that
should follow the directory, the paths resolve relative to
`CMAKE_CURRENT_SOURCE_DIR`, and the manifests are appended rather than globbed
because they are two known files.

## Limits

This has not been configured, so the glob has not been observed expanding to
anything. The claim is that the list can no longer drift, not that it has been
seen to work; the first configure after the hold ends will settle it, and if the
glob were wrong the failure would be a target that rebuilds too often rather
than one that rebuilds too rarely.

`CONFIGURE_DEPENDS` costs a directory check per build and is not portable to
every generator. Both are acceptable here: this is one target in one project
built by one generator on one machine.
