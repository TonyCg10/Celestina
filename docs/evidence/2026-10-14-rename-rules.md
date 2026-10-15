# Evidence: halted trees are history, and the shell rename rehearsed

- **Date:** 2026-10-14
- **Scope:** CEL-1-A1 (Task 3 of the plan; the unit split off from CEL-1-A0 after the final review) — suite (`scripts/`, `scripts/language-baseline.tsv`)
- **Environment:** CachyOS, Python 3.14, Git, Rust 1.97.1; the rehearsal in a
  scratch clone of this branch, outside the repository
- **Artifact:** not applicable (guards only); the rehearsal built, verified
  and deployed the `celestina` stub into a scratch prefix

## Procedure

### The language rule (Steps 1–2)

`scripts/check-language-contract.py`: `is_history(path, halted_roots=None)`
is true for the history directories and for every path under a halted
project's tree (its registered `path` with a trailing slash). `scan(root)`
reads the halted roots from `docs/projects.toml` through
`project_registry.halted_projects` and passes them; `main`'s historical
comparison is unchanged. Called with one argument, `is_history` reads the
staged registry once per process. `scripts/commit_scope.py` passes the halted
roots of the staged registry, read with HEAD's `halted_projects`, when HEAD's
scanner accepts them, so the hook never imports a staged or unstaged registry
module. The 8 rows under `celestina/` leave `scripts/language-baseline.tsv`.

| Command | Exit | Output |
|---|---|---|
| `python3 scripts/test-language-contract.py` before the rule | 1 | `test_a_halted_tree_is_history`: `['halted/README.md: non-English canonical text at line(s) 2'] != []` |
| `python3 scripts/test-language-contract.py` | 0 | Ran 19 tests, OK |
| `python3 scripts/check-language-contract.py` | 0 | OK (131 legacy files), no "debt is gone", no new entry |
| `LANGUAGE_COMPARE_REF=HEAD python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/test-commit-scope.sh` before `commit_scope.py` passed the roots | 1 | "the hook executed staged or unstaged registry code" (a scanner that imports `project_registry` loads the worktree's copy) and "legacy prose in the tree the staged registry halts was measured" |
| `bash scripts/test-commit-scope.sh` | 0 | Commit scope: OK |

### The rehearsal (Step 3)

A scratch clone of this branch with `core.hooksPath .githooks`; this
branch's three commits replayed in it through the hooks; then the CEL-1-A
edit list below applied and staged with `git add -A`. The script below
rebuilds that staged state from a fresh clone. The mechanical edits of items
1–5 and 7 are written out in the order they were applied; the new files of
items 6–8 (the `celestina/` skeleton, the two `celestina-rs` crates and the
rename evidence) are taken from the rehearsal commit `3313d261`, the tip of
`rehearsal/cel-1-a`, a local, unpushed branch of this repository (fetched into
the canonical checkout from the session's rehearsal clone; the author may
delete it once CEL-1-A has landed). Its last step proves the staged change
equal, by patch id, to that commit's.

```bash
#!/bin/bash
# Rebuild the staged CEL-1-A rename in a fresh clone. SOURCE holds the A0 and
# A1 commits (the A1 worktree, or the canonical checkout once A1 lands);
# AUTHORED is the canonical checkout, /home/toni/CODIGO/CELESTINA, whose local
# branch `rehearsal/cel-1-a` (3313d261) holds the rehearsal commit, the source
# of the new files of items 6-8; DEST must not exist.
set -euo pipefail
SOURCE=${SOURCE:?} AUTHORED=${AUTHORED:?} DEST=${DEST:?}
step() { "$@"; printf 'exit %s: %s\n' "$?" "$*" >&2; }

step git clone -q "$SOURCE" "$DEST"          # the branch's commits arrive with the clone
cd "$DEST"
step git config core.hooksPath .githooks
step git fetch -q "$AUTHORED" rehearsal/cel-1-a
test "$(git rev-parse FETCH_HEAD)" = 3313d2619c456affd067ba37685b949d39b5f25a

# 1. The move, bytes unchanged.
step git mv celestina celestina-qt

# 2. Registry: the halted table renamed, the new table after it, the
#    celestina-tokens component scope.
step python3 - <<'PY'
import re
from pathlib import Path
p = Path("docs/projects.toml"); s = p.read_text()
start = s.index('[[projects]]\nid = "celestina"\n')
end = s.index("\n# `toolchains` names what each build runs")
head_old = ('[[projects]]\nid = "celestina"\nname = "Celestina shell"\npath = "celestina"\n'
            'kind = "qt-shell"\ncommit_prefix = "celestina"\n')
old = s[start:end]; assert old.startswith(head_old)
rest = re.sub(r'(?<![A-Za-z0-9_-])celestina/', 'celestina-qt/', old[len(head_old):])
head_new = '''[[projects]]
id = "celestina-qt"
name = "Celestina shell (Qt, halted)"
path = "celestina-qt"
kind = "qt-shell"
commit_prefix = "celestina-qt"
# Registered as `celestina` until 2026-10-14; the history rows that carry that
# name up to the Rust shell's baseline are this project's
# (docs/contracts/versioning.md, "Renaming a project").
history_owner = "celestina"
'''
new_table = '''
[[projects]]
id = "celestina"
name = "Celestina shell"
path = "celestina"
kind = "rust-shell"
commit_prefix = "celestina"
version_source = { kind = "cargo-package", path = "celestina/Cargo.toml", package = "celestina" }
version_mirrors = [
  { kind = "cargo-lock", path = "celestina/Cargo.lock", package = "celestina" },
]
agents = "celestina/AGENTS.md"
readme = "celestina/README.md"
status = "celestina/STATUS.md"
roadmap = "celestina/ROADMAP.md"
validation = "celestina/VALIDATION.md"
active_plans = "celestina/docs/plans/active"
context_documents = [
  "docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md",
  "celestina-style/DESIGN.md",
  "celestina/docs/decisions/README.md",
]
source_roots = ["celestina/src", "celestina/crates", "celestina-rs/crates/celestina-shell"]
commit_roots = ["celestina/", "celestina-rs/crates/celestina-shell/"]
include_workspace_manifests = true
production_role = "activatable-shell"
deployable = true
toolchains = ["rust"]
build_script = "celestina/scripts/build-production.sh"
complete_script = "celestina/scripts/complete-production.sh"
verify_script = "celestina/scripts/verify-production.sh"
deploy_script = "celestina/scripts/deploy-production.sh"
activate_script = "celestina/scripts/activate-production.sh"
status_script = "celestina/scripts/status-production.sh"
artifact_manifest = "celestina/target/production-artifact.toml"
artifact_paths = ["celestina/target/release/celestina"]
cargo_manifests = ["celestina/Cargo.toml"]
production_inputs = ["celestina/Cargo.toml", "celestina/Cargo.lock", "celestina/rust-toolchain.toml", "celestina/src", "celestina/crates", "celestina/celestina.desktop", "celestina-style/fonts", "celestina-style/icons", "celestina-rs/crates/celestina-core", "celestina-rs/crates/celestina-settings", "celestina-rs/crates/celestina-shell", "celestina-rs/crates/celestina-tokens"]
verification_inputs = ["celestina/scripts/smoke.sh", "celestina/scripts/dev-session.kdl", "celestina-rs/rust-toolchain.toml"]
component_commit_scopes = [
  { prefix = "celestina-shell", roots = ["celestina-rs/crates/celestina-shell/"], include_workspace_manifests = true },
]
'''
s = s[:start] + head_new + rest.rstrip("\n") + "\n" + new_table + s[end:]
a = '  { prefix = "dotfiles-core", roots = ["celestina-rs/crates/dotfiles-core/"], include_workspace_manifests = true },\n]'
assert s.count(a) == 1
s = s.replace(a, a[:-1] + '  { prefix = "celestina-tokens", roots = ["celestina-rs/crates/celestina-tokens/"], include_workspace_manifests = true },\n]')
p.write_text(s)
PY

# 3. The baseline row of the new project.
step sh -c 'printf "celestina\t0.1.0\tbaseline\tCEL-1-A\tAdopt the Rust shell skeleton as its verified baseline\n" >> docs/version-history.tsv'

# 4. AGENTS.md "Halted projects" (the agent's draft; the author rewrites it).
step python3 - <<'PY'
from pathlib import Path
p = Path("AGENTS.md"); s = p.read_text()
old = '''The Celestina shell is halted by the author's decision of 2026-09-27. The halt
covers the `celestina/` project, the crate registered only for it
(`celestina-rs/crates/celestina-shell-core`), their commit prefixes
`celestina:` and `celestina-shell-core:`, the shell's scripts, and its
registered production entries.
'''
new = '''The Qt/QML Celestina shell is halted by the author's decision of 2026-09-27.
The halt covers the `celestina-qt/` project (moved from `celestina/` on
2026-10-14; id and prefix `celestina-qt`), the crate registered only for it
(`celestina-rs/crates/celestina-shell-core`), their commit prefixes
`celestina-qt:` and `celestina-shell-core:`, the shell's scripts, and its
registered production entries.
'''
tail = '''The halt is lifted only by the author editing this section. The commit guard
accepts a change to a halted project only in a commit that also changes this
section.
'''
salvage = '''**Salvage by copy.** The Rust shell at `celestina/` may copy, never edit,
these files of the halted tree and adapt them in its own crates, recording in
each copy's head comment the source path and the commit it came from:
`celestina-rs/crates/celestina-shell-core/src/{notifications,launcher,coalesce,workspace_groups,workspace_map,settings,calendar}.rs`
and the `bounded` helper of that crate's `src/lib.rs` into
`celestina-rs/crates/celestina-shell`; `celestina-qt/src/provider_adapter/{worker,notifications,launcher}.rs`
and `celestina-qt/src/niri_adapter.rs` as reference for
`celestina/src/providers`; `celestina-qt/scripts/dev-session.kdl` and
`dev-session.sh` for `celestina/scripts`. Later deliveries extend this list by
the same edit.

'''
assert s.count(old) == 1 and s.count(tail) == 1
p.write_text(s.replace(old, new).replace(tail, salvage + tail))
PY

# 5. References outside the tree.
step python3 - <<'PY'
import re
from pathlib import Path
def edit(path, old, new):
    p = Path(path); s = p.read_text(); assert s.count(old) == 1, (path, old)
    p.write_text(s.replace(old, new))
edit("README.md",
 "| [celestina](celestina/) | Niri shell, panel, overlays and session command service; **halted** since 2026-09-27, see [Halted projects](AGENTS.md#halted-projects) | Rust · C++20 · Qt/QML |\n",
 "| [celestina](celestina/) | Niri shell in pure Rust: Island, launcher, notifications, tray; CEL-1 in progress | Rust |\n"
 "| [celestina-qt](celestina-qt/) | The Qt/QML shell, **halted** since 2026-09-27, see [Halted projects](AGENTS.md#halted-projects) | Rust · C++20 · Qt/QML |\n")
edit("ROADMAP.md",
 "| Celestina shell (halted since 2026-09-27; see [Halted projects](AGENTS.md#halted-projects)) | [celestina/ROADMAP.md](celestina/ROADMAP.md) |\n",
 "| Celestina shell | [celestina/ROADMAP.md](celestina/ROADMAP.md) |\n"
 "| Celestina Qt shell (halted since 2026-09-27; see [Halted projects](AGENTS.md#halted-projects)) | [celestina-qt/ROADMAP.md](celestina-qt/ROADMAP.md) |\n")
edit("HOST-HYGIENE.md", "(`celestina/scripts/build-patched-xdpw.sh`)", "(`celestina-qt/scripts/build-patched-xdpw.sh`)")
edit("magnetita/ROADMAP.md",
 "that request arrives. `magnetita-core` changed, so closing will also require\n`celestina/scripts/complete-production.sh`.\n",
 "that request arrives. `magnetita-core` changed; the shell that consumed it is\nhalted, so closing does not run `celestina-qt/scripts/complete-production.sh`.\n")
edit("magnetita/AGENTS.md",
 "service. When `magnetita-core` changes, also run\n`celestina/scripts/complete-production.sh` because the shell consumes its phone\nprojection; this updates the on-disk shell bundle but does not activate it. Do\n",
 "service. The shell that consumed `magnetita-core`'s phone projection is halted\n(`celestina-qt/`); its `celestina-qt/scripts/complete-production.sh` is not run\nwhen `magnetita-core` changes, as AGENTS.md \"Halted projects\" rules. Do\n")
edit("siderita/STATUS.md", "`celestina/qml/MenuSection.qml`.", "`celestina-qt/qml/MenuSection.qml`.")
for path in ("cuprita/AGENTS.md", "hematita/AGENTS.md", "calcita/AGENTS.md", "selenita/AGENTS.md"):
    edit(path, "reference `celestina/` or `celestina-shell-core`", "reference `celestina-qt/` or `celestina-shell-core`")
link = re.compile(r"(\]\((?:\.\./)+)celestina/")
for path, expected in [
    ("docs/history/suite-roadmap-through-2026-08-03.md", 6),
    ("docs/plans/archive/2026-08-05-desktop-entry-registration.md", 1),
    ("docs/evidence/2026-09-28-halt-shell.md", 1),
    ("docs/evidence/2026-08-05-static-suite-audit.md", 1),
    ("celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md", 1),
]:
    p = Path(path); new, n = link.subn(r"\1celestina-qt/", p.read_text())
    assert n == expected, (path, n); p.write_text(new)
edit("Celestina.code-workspace", '      "path": "celestina"\n    },\n',
     '      "path": "celestina"\n    },\n    {\n      "path": "celestina-qt"\n    },\n')
edit("scripts/check-architecture-contract.sh",
 '        celestina/CMakeLists.txt celestina/qml celestina; then\n        fail "celestina/CMakeLists.txt: incomplete or invalid QML registration"',
 '        celestina-qt/CMakeLists.txt celestina-qt/qml celestina; then\n        fail "celestina-qt/CMakeLists.txt: incomplete or invalid QML registration"')
edit("scripts/radius-baseline.tsv", "9\tcelestina\n", "9\tcelestina-qt\n")
PY
step git mv .github/workflows/celestina.yml .github/workflows/celestina-qt.yml
step python3 - <<'PY'
from pathlib import Path
p = Path(".github/workflows/celestina-qt.yml"); s = p.read_text()
for a, b, n in [
    ("name: celestina\n", "name: celestina-qt\n", 1),
    ("      - 'celestina/**'\n", "      - 'celestina-qt/**'\n", 2),
    ("      - '.github/workflows/celestina.yml'\n", "      - '.github/workflows/celestina-qt.yml'\n", 2),
    ("        working-directory: celestina\n", "        working-directory: celestina-qt\n", 1),
    ("            celestina/target\n", "            celestina-qt/target\n", 1),
    ("key: cargo-celestina-${{ runner.os }}-${{ hashFiles('celestina/Cargo.lock', 'celestina/Cargo.toml') }}",
     "key: cargo-celestina-qt-${{ runner.os }}-${{ hashFiles('celestina-qt/Cargo.lock', 'celestina-qt/Cargo.toml') }}", 1),
]:
    assert s.count(a) == n, a; s = s.replace(a, b)
p.write_text(s)
PY

# 6-8. The new files: the celestina/ skeleton (item 6), the two workspace
#      crates (item 7) and the rename evidence (item 8), as authored in the
#      rehearsal commit; then the workspace members and the lock.
step git checkout FETCH_HEAD -- celestina celestina-rs/crates/celestina-shell \
    celestina-rs/crates/celestina-tokens docs/evidence/2026-10-14-shell-rename.md
step python3 - <<'PY'
from pathlib import Path
p = Path("celestina-rs/Cargo.toml"); s = p.read_text()
a = '    "crates/celestina-settings",\n    "crates/celestina-shell-core",\n'
assert s.count(a) == 1
p.write_text(s.replace(a, '    "crates/celestina-settings",\n    "crates/celestina-shell",\n'
                          '    "crates/celestina-shell-core",\n    "crates/celestina-tokens",\n'))
PY
step sh -c 'cd celestina-rs && cargo check -q -p celestina-shell -p celestina-tokens'
step sh -c 'cd celestina-rs && cargo check -q -p celestina-shell -p celestina-tokens --locked'

step git add -A
# The staged rename is the rehearsal commit's change, byte for byte.
step sh -c 'test "$(git diff --cached --no-renames --binary HEAD | git patch-id --stable)" = \
    "$(git diff --no-renames --binary FETCH_HEAD^ FETCH_HEAD | git patch-id --stable)"'
```

Run on 2026-10-15 with `SOURCE` this branch at its scanner-migration commit
(`Ignore language debt in halted trees and rehearse the shell rename`),
`AUTHORED` the canonical checkout `/home/toni/CODIGO/CELESTINA` and `DEST` a
new scratch directory, each step's exit as the script prints it:

| Step | Exit |
|---|---|
| `git clone -q "$SOURCE" "$DEST"` | 0 |
| `git config core.hooksPath .githooks` | 0 |
| `git fetch -q "$AUTHORED" rehearsal/cel-1-a` (and the SHA check) | 0 |
| `git mv celestina celestina-qt` | 0 |
| item 2, the registry (`python3 -`) | 0 |
| item 3, the history row | 0 |
| item 4, `AGENTS.md` (`python3 -`) | 0 |
| item 5, the references (`python3 -`) | 0 |
| `git mv .github/workflows/celestina.yml .github/workflows/celestina-qt.yml` | 0 |
| item 5, the workflow (`python3 -`) | 0 |
| `git checkout FETCH_HEAD -- celestina celestina-rs/crates/celestina-shell celestina-rs/crates/celestina-tokens docs/evidence/2026-10-14-shell-rename.md` | 0 |
| item 7, the workspace members (`python3 -`) | 0 |
| `cargo check -q -p celestina-shell -p celestina-tokens` (in `celestina-rs`) | 0 |
| the same with `--locked` | 0 |
| `git add -A` | 0 |
| the patch-id comparison | 0 |

The resulting index tree equals the rehearsal commit's tree
(`git write-tree` = `git rev-parse FETCH_HEAD^{tree}`), and the comparison
fails when one staged byte differs.

The first replay surfaced a defect in the hook chain itself: the pre-commit
hook runs HEAD's language scanner over the index against the staged rows, so
a declared scanner migration (scanner and retired rows in one commit, the
only way `commit_scope.py` lets a row fall without its source) could never
commit: `language-contract: ERROR: celestina/qml/OutputChooser.qml: new
non-English repository text (29 suspicious line(s))` and seven more. Rule
change: when the index changes the scanner, `scripts/git_hooks.py` measures
the index against HEAD's rows; new or grown debt is still refused, and the
retired rows are the commit-msg guard's verdict. Test:
`scripts/test-staged-units.sh` commits a declared migration through the real
hooks (RED: "the hooks refused a declared language scanner migration") and
refuses one that also brings new debt.

Guards over the staged rename, from the clone's root, HEAD holding this
branch's rules. Round 1 is the first run; the last column is the final run
after the rule changes below.

| Command | Round 1 | Final |
|---|---|---|
| `python3 scripts/check-staged-units.py --root .` | 0 | 0 |
| `printf '%s\n' $(git diff --cached --name-only --no-renames) \| python3 scripts/commit_scope.py --check 'suite-maintenance: Move the halted shell to celestina-qt and register the Rust shell'` | 0 | 0 |
| `python3 scripts/commit_scope.py --root . --check-index '<the same subject>'` | — | 0 |
| `python3 scripts/version_tool.py check` | 0 | 0 (12 owners) |
| `bash scripts/check-architecture-contract.sh` | 1 | 0 |
| `python3 scripts/check-language-contract.py` | 0 | 0 |
| `bash scripts/check-documentation-contract.sh` | 1 (381 errors) | 0 |
| `bash scripts/test-commit-scope.sh` | 1 | 0 |
| `sh scripts/test-documentation-contract.sh` | 0 | 0 |
| `python3 scripts/test-land-unit.py` | 1 | 0 |
| `bash scripts/test-architecture-scanners.sh` | 0 | 0 |
| `sh scripts/test-staged-units.sh` | 0 | 0 |
| `python3 scripts/test-version-contract.py` | 1 | 0 |
| `cd celestina && cargo build --release --locked && ./target/release/celestina --check` | 0 | 0 |

Then the rename was committed in the clone through the hooks, with no
`--no-verify`: `pre-commit` (language and documentation contracts over the
index, the staged-units guard: "63 inventory file(s) and 21 plan(s) moved
with their renamed owner") and `commit-msg` both exit 0. On the committed
tree, every command of `.github/workflows/contracts.yml` with
`LANGUAGE_COMPARE_REF` and `ARCHITECTURE_COMPARE_REF` set to the parent:

| Command | First | Final |
|---|---|---|
| `bash scripts/check-documentation-contract.sh` (the committed-tree probe) | 0 | 0 |
| `bash scripts/test-commit-scope.sh` | 1 | 0 |
| `sh scripts/test-production-artifacts.sh` | 1 | 0 |
| `python3 scripts/audit-version-commits.py --since <parent>` | 0 | 0 |
| the other 15 commands of the workflow | 0 | 0 |

`celestina/scripts/build-production.sh`, `verify-production.sh`,
`deploy-production.sh --prefix <scratch>` and `status-production.sh --prefix
<scratch>` exit 0 on the committed tree; `activate-production.sh` exits 1
with `activate-production: the shell is not deliverable before CEL-1-B`.

### Rule changes the rehearsal forced

Each with its test; no halted byte was edited.

1. **Architecture contract, round 1.** `could not find
   qt_add_qml_module(celestina-qt ...)`: the third argument of the
   `cmake-qml-registration` call is the CMake target, which stays
   `celestina`. A correction of the edit list (item 5), not a rule change.
2. **Documentation contract, round 1.** 381 errors, all under
   `celestina-qt/docs/{plans,inventories}`: Scope does not match the owner
   (21), ledger prefix `celestina:` outside owner `celestina-qt` (68),
   Pathspecs outside the scope of `celestina:` (40), `self` rows naming the
   old path (63), and the moved inventories read as pending ones (63 × 3).
   The records keep, in frozen bytes, the identity they were written under.
   Rule change in `scripts/documentation_contract.py`: a halted owner that
   declares `history_owner` keeps the Scope and ledger checks of its plans,
   and so its inventories, out of the contract; they were validated when they
   landed and `check-staged-units.py` proves the move byte for byte. Test:
   `scripts/test-documentation-contract.sh` with the fixture
   `scripts/fixtures/documentation/renamed-halted-owner/` (RED: Scope, ledger
   prefix, `self` marker, host plan and evidence errors; GREEN; still refused
   without `history_owner` or without `halted`). The committed-tree probe the
   brief named (`check_historical_plan_host` resolving a moved inventory's
   host plan at the rename commit) passes with no further change: the
   renamed owner's ledgers are no longer resolved at all.
3. **`test-land-unit.py`, round 1.** It runs the documentation contract;
   green with change 2.
4. **`test-version-contract.py`, round 1.** The static test compared the
   history rows' owner names with the registered ids; a renamed project's
   rows carry its `history_owner`. The test now compares the names the rows
   carry; `validate_snapshot` already proves every owner its own baseline.
5. **`test-commit-scope.sh`, round 1.** `expected fail, got pass ->
   "suite-maintenance: align the shell with a project"`: the scope cases are
   judged by HEAD's registry and named the shell literally. They now read
   the halted shell's tree and prefix from HEAD's registry, and the halt
   fixture from the worktree's registry it copies, so the rename needs no
   edit of this test.
6. **`test-commit-scope.sh`, committed tree.** `history does not pass its
   own guard: 7175df0 celestina-milestone: ...`: the history replay judges old
   commits by HEAD's registry, where `celestina:` now names the Rust shell
   and no longer covers `celestina-shell-core`. Rule change in
   `scripts/commit_scope.py` (`--history-scope-only` only): a renamed
   project's `history_owner` prefix also covers that project's scope in a
   replay. Test: a fixture renames the shell, registers its old name again,
   and replays an old change into the renamed scope (accepted), into the new
   owner's scope (accepted), into another project (refused), and without
   `history_owner` (refused).
7. **`test-production-artifacts.sh`, committed tree.** The delegation test
   requires each entry to name its project's id; the halted entries name
   `celestina` in frozen bytes. The test skips a renamed halted project's
   entries, which the halt keeps every tool from running.

### The CEL-1-A edit list, as corrected

1. `git mv celestina celestina-qt` (bytes unchanged).
2. `docs/projects.toml`: the `celestina` table becomes `celestina-qt` with
   `history_owner = "celestina"` and every `celestina/` path renamed; the new
   `celestina` table after it; `celestina-tokens` in `celestina-rs`'s
   component scopes. As the Task 5 brief gives them.
3. `docs/version-history.tsv`: the one baseline row, appended.
4. `AGENTS.md` "Halted projects": as drafted in the brief, except that the
   salvage list names `celestina-shell-core/src/{notifications,launcher,coalesce,workspace_groups,workspace_map,settings,calendar}.rs`
   "and the `bounded` helper of that crate's `src/lib.rs`": `bounded` is a
   function of `lib.rs`, there is no `bounded.rs`.
5. References: as listed, with three corrections. The architecture contract
   becomes `celestina-qt/CMakeLists.txt celestina-qt/qml celestina` (the
   target keeps its name). `scripts/test-commit-scope.sh` needs no edit
   (change 5). One more historical link:
   `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md`
   (STYLE-G7-K's row, target only), so 10 link targets outside the README
   and ROADMAP rows.
6. The new `celestina/` skeleton, as listed. The plan indexes write the
   inventory root as text, not a link, because
   `celestina/docs/inventories/` does not exist until the first landing.
7. The `celestina-rs` stubs and members; `Cargo.lock` gains the two entries.
8. `docs/evidence/2026-10-14-shell-rename.md`.

## Result

- **Exit:** every command above exits 0 in its final run, on this branch, on
  the staged rename and on the committed rename.
- **Resolved language debt:** `scripts/check-language-contract.py`
- **Observed:** the 8 rows under `celestina/` (43 lines) leave the language
  baseline; the language guard reports 131 ratcheted files. The rehearsal
  moved all 484 files, 63 inventories and 21 plans among them, byte for byte.
  The history file holds 68 rows named `celestina` before the new baseline,
  not the 20 the design's §4 states.

## Limits

- The rehearsal ran in a scratch clone; CEL-1-A itself is the author's
  commit and its landing. The landing tool has no dry run; its tests ran.
- The documentation contract no longer re-derives a renamed halted owner's
  ledgers and inventories; their integrity across the move rests on
  `check-staged-units.py`'s byte-identity rule.

## Follow-up

CEL-1-A, with the corrected list above.

## Landing

- **Base revision:** `bcad4c27f5b50838d180b713e0bacf95cb5575dc`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 0: artifact: celestina-style current; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 0: artifact: siderita current; `production_artifact.py check magnetita --require-verified` exit 0: artifact: magnetita current; `production_artifact.py check magnetita-android --require-verified` exit 0: artifact: magnetita-android current; `production_artifact.py check grafita --require-verified` exit 0: artifact: grafita current; `production_artifact.py check fluorita --require-verified` exit 0: artifact: fluorita current; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check calcita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** hematita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:228d7ffc22ed8ddabcb3ae3c14e7915f6bc0d24da3b99d8b43bd306d2b89ca84, verification_fingerprint sha256:37be3d4989d0d0c8c06472f4394d920b78836e45eb110ff183255ddb3e507383; cuprita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:48d0a5aff8376952f2af729758d3394f4b9520ff6e6740bc749ff0bfb74638dd, verification_fingerprint sha256:56a6ae47eb504823a71b26cfcc7316677a88d34e7e50f6af9189b5c479bf5d21; calcita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:398dcfa1a5bcec5b7d692add5316e25d53e081e34ce0b19c225721584830c73c, verification_fingerprint sha256:1c11547039c2ddf3790e0172b8ce873df58a44f5314c192ea97d40a1102e3b60; selenita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:40a621ac8c1d49e1dfb76919c130680af814bdb91a261ac5d2c95f30809d9a33, verification_fingerprint sha256:981cbce1af96ae1e73bcf8c8cc2674576e59d0bd8651ce00db627a19ecdd860c
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
