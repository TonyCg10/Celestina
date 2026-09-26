# Evidence: the session worktree entry and the production refusal

- **Date:** 2026-09-25
- **Scope:** `LND-1-A` of the
  [seal-at-landing plan](../plans/archive/2026-09-25-seal-at-landing.md):
  `scripts/worktree.sh`, `scripts/test-worktree.sh`, the session-worktree
  refusal in `scripts/production_artifact.py` (`WORKTREE_MARKER`,
  `session_worktree_marker`, `main`) and its tests in
  `scripts/test-production-artifacts.py`
- **Environment:** repository checkout on Linux; Git 2.43.0, Python 3 standard
  library, `/bin/sh` is dash; fixture repositories and fake production entries
  only
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
bash scripts/test-worktree.sh
bash scripts/test-production-artifacts.sh
```

Test-first order: the two `session_worktree` tests ran against the unchanged
tool before the refusal existed, and `scripts/test-worktree.sh` ran before
`scripts/worktree.sh` existed.

## Result

- **Exit:** every command above exited 0; the architecture guard printed
  `Architecture contract: OK`, the language guard `Language contract: OK`, and
  the documentation guard `Documentation contract: OK` after its known
  `SID-G7-D` errata lines.
- **Before the change:** `python3 scripts/test-production-artifacts.py -k
  session_worktree -v` failed: `run-build`, `run-verification` and `status`
  exited 0 instead of 1 with the marker present, and the fake build entry ran.
  `sh scripts/test-worktree.sh` failed at its first case because
  `scripts/worktree.sh` did not exist.
- **After the change:** `scripts/test-worktree.sh` printed six `ok` lines:
  `open` creates the branch `unit/<project>/<unit>`, the worktree at
  `<parent>/<basename>.worktrees/<project>-<unit>/` on `origin/main`, the
  marker with `project`, `unit` and `canonical`, and a Cargo configuration
  naming the shared `.cargo-target`; a second `open` is refused with `already
  exists`; an unregistered project is refused; `close` is refused with `not on
  origin/main` while the branch carries unpublished work and removes the
  worktree and branch once it is published; `suite` is a valid project; the
  two written files are listed once each in the shared `info/exclude`, so a
  new worktree's `git status --porcelain --untracked-files=all` is empty and a
  later `open` adds no duplicate pattern.
- `scripts/test-production-artifacts.sh` ran 31 artifact tests, including the
  two new ones, and the production-common fixtures, all passing: with
  `.celestina-worktree` at the repository root, `run-build`,
  `run-verification` and `status` exit 1 with `production-artifact: this is a
  session worktree; production runs happen at landing (scripts/land-unit.py)`
  and no entry runs, while `check` still answers.

## Limits

- No real Cargo build ran. The shared target directory is only written into
  the fixture's configuration; the author's first real worktree exercises it.
- `scripts/production_artifact.py` and `scripts/test-production-artifacts.py`
  are verification inputs of every registered project, so existing manifests
  need re-verification before their next deployment. No production entry ran
  and nothing was deployed; this unit changes no deployable app.

## Follow-up

`LND-1-B` adds the landing tool the refusal message names.
