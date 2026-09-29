# Evidence: the landing skips a halted project

- **Date:** 2026-09-28
- **Scope:** `HALT-SHELL-0` of the
  [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md):
  `halted_projects` and `halted_owner` in `scripts/project_registry.py`, and
  `affected_projects` in `scripts/landing.py`, which leaves out every project
  whose registry entry carries `halted = "YYYY-MM-DD"`, even as the owner
- **Environment:** session worktree `unit/suite/HALT-SHELL-0` on the author's
  machine (Linux 7.2.8); Python 3.14.7, Git 2.55.0. The fixtures build their
  own trees in temporary directories; no production entry ran
- **Artifact:** not applicable; the unit changes no registered production or
  verification input, so its landing builds nothing

## Why this unit exists

The author halted the Celestina shell on 2026-09-27, and `HALT-SHELL` records
that halt. Its first landing on 2026-09-28 stopped at `build_if_stale`:
`main`'s landing does not know the `halted` key, so the unit's change to
`docs/projects.toml` marked the shell as affected, and the shell's
`verify-production.sh` failed with exit 8 on its own tests
(`celestina-indicator-menu`: `aSoftMenuKeepsOneOuterGlassCard`, elevation 2
where 0 was expected; `celestina-output-chooser`: three `TrayItemsMenu` focus
and keyboard cases). `HALT-SHELL` changes no QML or C++, so those failures
predate it, and the halted shell is not repaired. The landing was aborted.

The root contract's rule for a semantic rule change applies: add compatible
dormant behaviour first, then activate it. This unit is the dormant half. It
carries the same bytes of the three files as `HALT-SHELL`, and no registry
entry has the `halted` key yet, so nothing changes until `HALT-SHELL` lands
and adds it; that landing then runs with this code and leaves the shell out.

## Procedure

```sh
python3 scripts/test-land-unit.py LandingFunctions
python3 scripts/test-land-unit.py
bash scripts/test-land-unit.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

The new test `test_affected_projects_never_include_a_halted_project` was
first run against `origin/main`'s `scripts/` (RED), then against this unit's
(GREEN). Its affected-project set was also simulated for this unit's changed
paths with `origin/main`'s `affected_projects`: it is empty.

## Result

- **RED:** `LandingFunctions` ran 28 tests with 1 failure, the new test.
- **GREEN:** every command above exited 0 on the final tree;
  `LandingFunctions` ran 28 tests, `OK`.

## Limits

- The shell's failing tests are recorded, not investigated: the shell is
  halted.

## Landing

- **Base revision:** `d36c71dbd884bca7a0d5e5165f5bfb30ccc5d2a0`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
