# Evidence: the prune keeps the release cache

- **Date:** 2026-10-06
- **Scope:** AUD-1-K — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable

## Request

AUD-1-J pruned every build tree down to its registered artifacts. The author
then asked to avoid a long build for a change as small as a colour. A QML file
is compiled into its application (`QmlModule` in `build.rs`, loaded from
`qrc:`), so such a change always rebuilds; after a full prune that build
started from nothing. The author chose, on 2026-10-06, to keep the release
cache and remove only the debug builds.

## Design

- `scripts/build_trees.py` removes each Cargo `debug` profile directory
  directly under a build root or one level below it (a target triple, or a
  nested target such as `celestina-rs/target/workspace`), and keeps the rest:
  release caches, CMake and Gradle trees, and every registered artifact and
  manifest, also one inside a debug directory. It never follows a symbolic
  link, including a symlinked entry beside the profile.
- The release QML module that `qmllint-cxxqt.sh` reads stays, so the
  `<artifact_manifest>.pruned` mark and the landing's "build before verifying
  a pruned project" rule of AUD-1-J are no longer needed and are removed in
  this unit, leaving one path.
- `land-unit.py`'s `prune` step prints
  `land-unit: pruned the debug builds from the build trees; freed <N> GiB`.

## Procedure

```sh
python3 scripts/test-build-trees.py
bash scripts/test-land-unit.sh
bash scripts/test-worktree.sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
bash scripts/test-documentation-contract.sh
bash scripts/test-commit-scope.sh
```

## Result

- `test-build-trees.py`: 14 tests pass, covering debug profiles at the root,
  under a target triple and under a nested target; the release cache, CMake
  tree and unrelated directories kept; a registered artifact inside a debug
  profile kept; a halted project untouched; dry run; a missing root; a second
  prune that frees nothing; a path outside the checkout refused; and the
  command. `test_a_symlink_is_never_followed` first failed on this design: a
  symlinked entry beside a debug profile let the prune remove a directory
  outside the tree; `debug_directories` now requires the entry itself to be a
  real directory.
- `test-land-unit.sh`: 85 tests pass; `test_a_landing_prunes_the_debug_builds`
  checks that the debug profile goes, a release cache file stays and
  `status-production.sh` still passes.
- Every other command above exits 0.

## Limits

The canonical checkout still holds the `.pruned` marks AUD-1-J's manual prune
wrote; they are ignored build files that nothing reads after this unit, and
are removed by hand after the landing. The release caches AUD-1-J removed come
back with each application's next production build.

## Landing

- **Base revision:** `5264566d1c727577f771a802d36491818cd9cc3d`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
