# Evidence: prune the build trees after a landing deploys

- **Date:** 2026-10-06
- **Scope:** AUD-1-J — suite
- **Environment:** CachyOS, Python 3.14
- **Artifact:** not applicable

## Request

The author asked on 2026-10-06 that the build trees stop occupying the disk
once the final version is installed, since every build regenerates them, and
that this happen automatically at the end of every landing, for every project.
On that date the canonical checkout held about 177 GB, almost all of it Cargo
`target/debug` trees (`celestina-rs` 45 GB, `siderita` 34 GB, `celestina`
28 GB, `magnetita` 16 GB) that no installed binary uses.

The request amends the rule of root `AGENTS.md`, the production artifact
contract and ADR 0003 that no entry runs `clean`: the reuse of caches between a
build and its deploy is unchanged; only what remains after the deploy is
removed.

## Design

- `scripts/build_trees.py` owns the prune. It derives the build roots from the
  registry (the first `target` or `build` directory of each artifact path and
  manifest of a project that is not halted), keeps every project's registered
  artifacts and manifest, halted ones included, and removes everything else
  under those roots. `check`, `deploy-production.sh` and
  `status-production.sh` therefore judge the same bytes after a prune, and a
  current artifact is not rebuilt.
- A pruned project gets the mark `<artifact_manifest>.pruned`. A
  verification-only landing would lint a release QML module the prune removed,
  so `land-unit.py`'s `check_and_build` builds a marked project instead of
  verifying it alone, and clears the mark after the build.
- `land-unit.py` gains a last step, `prune`, after `deploy`: it prunes with the
  sealed commit's registry, prints what it freed, warns instead of stopping on
  a failure (the unit has landed), then removes the landing worktree.
- `scripts/worktree.sh close` removes the shared session Cargo target when it
  closes the last session worktree; the landing worktree does not count.
- The halted shell's own trees (`celestina/build`, `celestina/target`) are
  never touched (AGENTS.md "Halted projects").

## Procedure

```sh
python3 scripts/test-build-trees.py
bash scripts/test-worktree.sh
bash scripts/test-land-unit.sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-architecture-scanners.sh
python3 scripts/test-version-contract.py
bash scripts/test-documentation-contract.sh
bash scripts/test-commit-scope.sh
python3 scripts/build_trees.py --root <canonical checkout> prune --dry-run
```

## Result

- `test-build-trees.py`: 15 tests pass. They cover the roots derived from the
  registry, a shared Cargo target that keeps another project's artifact, a
  halted project's tree left whole, the marks, `--dry-run`, a missing root, a
  symlink removed without being followed, a second prune that frees nothing, a
  path outside the checkout refused, and the command refusing a session
  worktree.
- `test-land-unit.sh`: 86 tests pass, including two new fixture landings:
  `test_a_landing_prunes_the_build_trees_to_their_artifacts` (the caches go,
  the artifact, manifest and mark stay, and `status-production.sh` still
  passes) and `test_a_pruned_project_builds_before_it_verifies`. With the
  pruned-project rule removed from `check_and_build`, the second test fails.
- `test-worktree.sh`: passes, including the new case that closing the last
  session removes `.cargo-target` while an open session or the landing
  worktree keeps it.
- Every guard above exits 0.
- The dry run over the canonical checkout lists 223 entries, all under
  `target/` or `build/` trees of projects that are not halted, and keeps every
  registered artifact and manifest.

## Limits

The first real landing runs the prune over the canonical checkout; its
`Landing` section and output record the space it freed.

## Landing

- **Base revision:** `52805376be2f375fa0f6e327d7f7eaf9393ef2af`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
