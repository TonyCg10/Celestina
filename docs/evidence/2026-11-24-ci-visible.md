# Evidence: CI-VISIBLE — a red workflow stops the landing, and the `celestina` workflow runs the pinned toolchain

- **Date:** 2026-11-24
- **Scope:** `.github/workflows/celestina.yml` (the toolchain), the CI read
  of `scripts/land-unit.py` with the state comment and the evidence line of
  `scripts/landing.py`, their tests in `scripts/test-land-unit.py`,
  `docs/contracts/landing.md` ("A red main" and the places that named
  `contracts`), `celestina/AGENTS.md` ("Local verification") and this unit's
  ledger row.
- **Environment:** CachyOS, worktree `suite-CI-VISIBLE` cut from `main` at
  `44b8cc2a`. The landing tests run in throwaway repositories with fixture
  files for the workflow status; the one live read below was a `GET` of
  GitHub's public API. Nothing was landed or pushed by this session.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The `celestina` workflow was red on `main` while the landings of 2026-11-23
and 2026-11-24 went over it, and had failed there on 2026-10-24 already, for
two reasons
([the Clippy 1.100 record](../../celestina/docs/evidence/2026-11-24-clippy-1-100.md)):

1. The workflow installed `stable` and set `RUSTUP_TOOLCHAIN: stable`, while
   `celestina/rust-toolchain.toml` pins 1.97.1 and every local check runs
   that. The runner's stable moved to 1.99 in October and to 1.100.0 on
   2026-11-09; each brought lints the pinned Clippy does not have, so `main`
   turned red with no commit to blame and no local check could see it.
2. The landing's preflight read the latest run of `contracts.yml` only. A red
   `celestina` or `celestina-rs` run stopped nothing and was printed nowhere.

The author chose both remedies: pin the workflow, and make the landing read
every workflow.

## Procedure

```sh
python3 scripts/test-land-unit.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/celestina.yml'))"
# one read-only request per workflow file of origin/main, as the tool sends it:
curl -s -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/<owner>/<repository>/actions/workflows/<file>/runs?branch=main&event=push&per_page=1"
```

## Result

**The workflow** has no `RUSTUP_TOOLCHAIN` and installs with
`rustup toolchain install`, which reads `celestina/rust-toolchain.toml` from
the job's working directory, as `celestina-rs.yml` has done since it was
written; the toolchain file is part of the cache key. The three checks are
unchanged.

**The landing** lists the workflow files tracked on `origin/main`
(`git ls-tree -z`, names ending in `.yml` or `.yaml`) and asks each for its
own newest push run on `main` (`per_page=1`). A run is red when it completed
with `failure`, `timed_out` or `startup_failure`. Any red workflow stops the
preflight, which names each one and its run; `--accept-red-ci` lands over
them and the evidence's `CI` line names the same runs. What the read kept
from before: `LAND_UNIT_CI_STATUS=off` skips it; any other value is the URL,
now with `{workflow}` replaced by each file's percent-quoted name; a status
that cannot be read, a malformed one and a workflow list that cannot be read
are warnings and never a stop; a remote that is not on GitHub is not read.
The whole read shares one deadline of 20 s, so an offline landing waits as
long as it did with one workflow.

**One request per workflow, not one for the repository.** The first version
read the newest hundred runs of the repository in one request. The review
found that a path-filtered workflow's red run leaves that window after some
forty pushes that do not touch its paths (the window spanned five and a half
weeks on this repository), after which it would be skipped in silence: the
failure this unit exists to end. A workflow whose file is no longer on
`origin/main` is never asked, so the retired `celestina-qt.yml`, whose last
run is red for good, stops nothing.

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-land-unit.py` | 0 | 110 tests, OK (96 on `main`, 101 before the two fix rounds) |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| the workflow file parsed as YAML | 0 | parses |
| the live read, one request per workflow file of `origin/main` | 0 | `celestina-rs.yml` completed, success (`b99a22b4`); `celestina.yml` completed, success (`44b8cc2a`); `contracts.yml` completed, success (`44b8cc2a`) |

The tests of the read, each seen failing before the code or against a
mutation of it: a red `celestina` run with a green `contracts` stops the
preflight; two red workflows are both named; each workflow is read by itself
(a red workflow is found whatever the others' runs); an older red run under
a newer green one, a newest run in progress and a red run of a workflow
whose file is not on `origin/main` do not stop; one unreadable workflow
warns and a red one read after it still stops; a workflow with no run is a
notice; a `status` or `conclusion` that is not a string warns and does not
raise; a failing `git ls-tree` warns; a file under `.github/workflows/` that
is not a workflow is not asked; a file name with a space is quoted; and once
the shared deadline is spent the remaining workflows are named in one
warning, with the clock injected and no waiting.

## Reviews

A model other than the implementer, high reasoning, twice (2026-11-24). The
first review passed spec and quality with one Important finding, the window
described above, and five Minor ones (a failing `ls-tree` turned the read
off in silence, a wrong size in a comment, reliance on an undocumented order,
a traceback on a malformed `conclusion`, a note in `celestina/AGENTS.md` that
was too narrow); all fixed or made moot by the per-workflow read. The second
found each fixed and three Minor ones (two behaviours no test could catch,
file names neither read raw nor quoted, a worst case of 20 s per workflow),
fixed in turn; that last round was checked by the coordinating session
against the reviewer's findings, not by a third review.

## Limits

- The workflow's change is not seen on a runner until the unit is on `main`:
  that `rustup toolchain install` with no argument installs the file's
  toolchain with its components is read from `celestina-rs.yml`, which has
  the same step and file and is green, and from rustup's changelog.
- Pinning removes the surprise, not the debt: Clippy of a newer Rust may
  still find what 1.97.1 does not. Moving a pin is now a unit of its own
  (`docs/contracts/landing.md`); until then `cargo +<version> clippy` shows
  what it will cost.
- The read needs the repository to be public and GitHub reachable; otherwise
  it warns and the landing goes on, as before. Branch protection on `main`
  stays the author's own gate.
- The landing tool lands a unit with the script that is running, so this
  unit's own landing still reads `contracts` only.

## Landing

- **Base revision:** `840562fc528b1c05fa3e99f7aa145e37f65e4c92`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
