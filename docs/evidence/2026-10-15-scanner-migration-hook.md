# Evidence: a language scanner migration commits through the pre-commit hook

- **Date:** 2026-10-15
- **Scope:** CEL-1-A0 — suite (`scripts/git_hooks.py`, `scripts/test-staged-units.sh`,
  `AGENTS.md`), branch `unit/suite/CEL-1-A0` on `da4647fc`
- **Environment:** CachyOS, Python 3.14, Git; the scratch clones under the
  session's scratchpad, outside the repository, with `core.hooksPath .githooks`
- **Artifact:** not applicable (hook runner only)

## Procedure

### The runner

`scripts/git_hooks.py` (`committed_language_rows`): while the index changes
`scripts/check-language-contract.py`, `pre-commit` restores HEAD's
`scripts/language-baseline.tsv` into the index copy before the language
contract runs, and puts the staged rows back before the documentation
contract. HEAD's scanner thus measures the index against the rows it was
built for: the rows a staged scanner retires do not stop the commit, new or
grown debt still does, and whether the retired rows may go stays the
`commit-msg` guard's verdict (the declared evidence field). The runner is
dormant on today's tree: no commit of this unit stages the scanner, so every
commit is measured exactly as before. `CEL-1-A1`, stacked on this branch, is
the first commit that stages the scanner.

`scripts/test-staged-units.sh` stages a migration through the real hooks in
the hook fixture: a file with one legacy line and its row, then the scanner
wrapped so that the file is history, the row removed and the field
``- **Resolved language debt:** `scripts/check-language-contract.py` ``
appended to the fixture evidence. The wrapper forwards whatever arguments the
scanner's `is_history` takes, so the case runs with today's one-argument
scanner and with a scanner that takes halted roots.

| Command | Exit | Output |
|---|---|---|
| `sh scripts/test-staged-units.sh` with HEAD's runner at `da4647fc` (the new case copied into a scratch clone at `main`) | 1 | `FAIL: the hooks refused a declared language scanner migration:` then `language-contract: ERROR: app/src/legacy.rs: new non-English repository text (1 suspicious line(s))` |
| `sh scripts/test-staged-units.sh` | 0 | Staged inventories: OK |
| `bash scripts/test-commit-scope.sh` | 0 | Commit scope: OK |
| `python3 scripts/test-version-contract.py` | 0 | OK |
| `python3 scripts/test-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (139 legacy file(s) ratcheted) |
| `bash scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `python3 scripts/version_tool.py check` | 0 | version-contract: OK (11 owners) |
| `python3 scripts/test-land-unit.py` | 0 | OK |

### The refusal this unit answers

The landing seals a unit as one commit on `main`, judged by `main`'s hooks:
HEAD's runner and HEAD's scanner over the index. The rules of the rename
(`CEL-1-A1`: the `history_owner` alias, the renamed-owner moves, halted
trees as history with the 8 `celestina/` rows retired) squashed together
with this runner onto `da4647fc` are refused, because the old runner measures
the index against the staged rows, which lack the 8 rows that HEAD's scanner
still counts. Split into two landings, the runner lands first and dormant
(change policy, "a semantics-changing rule migration first lands compatible
dormant behavior, then activates it"), and the rules land as one commit
judged by it. The script rebuilds the three squashed commits in a fresh
clone; `WHOLE` is the single-unit branch before the split
(`backup/cel-1-a0-before-split`, `048e28f7`), `A0` and `A1` the two
branches.

```bash
#!/bin/bash
# Squash each branch's diff onto its base in a fresh clone and commit through
# the hooks. SOURCE holds the refs; DEST must not exist.
set -uo pipefail
SOURCE=${SOURCE:?} DEST=${DEST:?}
BASE=da4647fc02f9c9f8645282772ae69b4304172725
WHOLE=${WHOLE:-backup/cel-1-a0-before-split}
A0=${A0:-unit/suite/CEL-1-A0} A1=${A1:-unit/suite/CEL-1-A1}
git clone -q "$SOURCE" "$DEST" && cd "$DEST" || exit 2
git config core.hooksPath .githooks
whole=$(git -C "$SOURCE" rev-parse "$WHOLE")
a0=$(git -C "$SOURCE" rev-parse "$A0") a1=$(git -C "$SOURCE" rev-parse "$A1")
for commit in "$whole" "$a0" "$a1"; do # the clone brings every branch
    git cat-file -e "$commit^{commit}" || exit 2
done
squash() { # squash FROM TO SUBJECT: stage the diff FROM..TO and commit
    git diff --binary "$1" "$2" | git apply --index || exit 2
    git commit -q -m "$3"
    printf 'exit %s: squash of %s..%s\n' "$?" "$1" "$2" >&2
}
git checkout -q "$BASE"
squash "$BASE" "$whole" 'suite-maintenance: Add the history_owner alias to the version contract, inventory moves under a renamed halted owner, halted trees as history for the language scanner, and rehearse the shell rename'
git reset -q --hard "$BASE"
squash "$BASE" "$a0" 'suite-maintenance: Allow a language scanner migration through the pre-commit hook'
squash "$a0" "$a1" 'suite-maintenance: Add the history_owner alias to the version contract, inventory moves under a renamed halted owner, halted trees as history for the language scanner, and rehearse the shell rename'
```

Run on 2026-10-15 with `SOURCE` a clone holding the three branches, `WHOLE`
at `048e28f7`, `A0` this branch and `A1` the branch stacked on it, whose diff
against this branch is 19 files, +1337/−89, patch id `89a0e27f` (the diff
does not contain this record, so the id survives its edits); each squash's
exit as the script prints it:

| Step | Exit | Output |
|---|---|---|
| squash of `da4647fc..048e28f7` (the unit before the split) | 1 | `language-contract: ERROR: celestina/qml/OutputChooser.qml: new non-English repository text (29 suspicious line(s))` and the same for `celestina/scripts/activate-production.sh`, `celestina/scripts/qmllint-production.sh`, `celestina/scripts/run.sh`, `celestina/scripts/smoke-production.sh`, `celestina/src/niriclient.cpp`, `celestina/tests/traymenu_test.cpp`, `celestina/tests/tst_outputchooser.qml` (the 8 retired rows); nothing committed |
| squash of this branch's diff against `da4647fc` (this unit) | 0 | `Language contract: OK (139 legacy file(s) ratcheted)`, `staged-unit: no staged unit closure`; committed |
| squash of `CEL-1-A1`'s diff against this branch, on the previous commit | 0 | `Language contract: OK (139 legacy file(s) ratcheted)` (HEAD's scanner against HEAD's rows), `staged-unit: no staged unit closure`; committed |

## Result

- **Exit:** 0 for every check of the branch; the reproduction's refusal is the
  expected exit 1 and both split squashes exit 0
- **Observed:** the runner changes no verdict on today's tree (every guard
  and test green, 139 legacy files ratcheted as on `main`); the single-unit
  squash is refused by `main`'s hooks with the 8 retired rows as new debt; the
  same change split into this unit and `CEL-1-A1` commits through them in
  order

## Limits

- The reproduction squashes the branches' diffs; the landing's sealed commit
  also carries the unit's inventory, which the hooks judge as a closure. That
  part of the landing is unchanged by this unit.
- While a migration is staged, a row it lowers for a file it does not stage is
  checked by no hook of that commit; only the new scanner measures it
  afterwards (CI, or the next commit's `pre-commit`). Recorded in `AGENTS.md`.
- The test proves the runner in the hook fixture with HEAD's scanner; the
  real migration, `CEL-1-A1`'s scanner commit, is proven by its own evidence
  (`docs/evidence/2026-10-14-rename-rules.md`, on that branch) and by the
  third step above.

## Follow-up

`CEL-1-A1` on `unit/suite/CEL-1-A1`, stacked on this branch: the rules of the
rename and the scanner migration this runner lets through.

## Landing

- **Base revision:** `da4647fc02f9c9f8645282772ae69b4304172725`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
