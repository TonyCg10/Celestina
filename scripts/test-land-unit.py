#!/usr/bin/env python3
"""Unit tests for scripts/landing.py and fixture landings of scripts/land-unit.py."""

from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import posixpath
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import types
import unittest


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

import landing  # noqa: E402  (the scripts directory is not a package)


def setUpModule() -> None:
    # Fixture repositories must not read the author's Git configuration:
    # signing, push negotiation or hooks there would change what they record.
    os.environ["GIT_CONFIG_GLOBAL"] = os.devnull
    os.environ["GIT_CONFIG_NOSYSTEM"] = "1"


REGISTRY_TOML = """schema_version = 1

[suite]
id = "suite"
commit_prefix = "suite"
active_plans = "docs/plans/active"
allow_all_commit_paths = true

[commit_policy]
workspace_manifests = ["ws/Cargo.lock"]
shared_ratchet_files = ["scripts/architecture-baseline.tsv"]

[version_policy]
history_file = "docs/version-history.tsv"
tag_format = "<project>-v<version>"
bug_increment = "patch"
milestone_increment = "minor"
release_increment = "major"
maintenance_increment = "none"

[[projects]]
id = "app"
commit_prefix = "app"
version_source = { kind = "cargo-package", path = "app/Cargo.toml", package = "app" }
version_mirrors = [{ kind = "cargo-lock", path = "app/Cargo.lock", package = "app" }]
active_plans = "app/docs/plans/active"
commit_roots = ["app/"]
include_workspace_manifests = true
deployable = true
build_script = "app/scripts/build-production.sh"
verify_script = "app/scripts/verify-production.sh"
deploy_script = "app/scripts/deploy-production.sh"
status_script = "app/scripts/status-production.sh"
complete_script = "app/scripts/complete-production.sh"
artifact_manifest = "app/target/production-artifact.toml"
artifact_paths = ["app/target/release/app"]
production_inputs = ["app/Cargo.toml", "app/src"]
verification_inputs = ["app/tests"]
"""
REGISTRY = tomllib.loads(REGISTRY_TOML)
PLAN = "app/docs/plans/active/2026-09-25-fixture.md"
LEDGER_HEADER = (
    "| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change "
    "| Automated evidence | Author validation |\n"
    "|---|---|---|---|---|---|---|---|\n"
)


def ledger_row(
    unit: str,
    status: str,
    *,
    prefix: str = "`app:`",
    files: str = "`app/src/main.rs`",
    diffstat: str = "—",
    evidence: str = "`cargo test`",
) -> str:
    return (
        f"| {unit} | {prefix} | {status} | {files} | {diffstat} | Change {unit} "
        f"| {evidence} | None |\n"
    )


def plan_text(*rows: str, prose: str = "The fixture plan.") -> str:
    return (
        "# FX — fixture\n\n- **Plan ID:** fixture\n\n"
        f"{prose}\n\n## Change and commit ledger\n\n{LEDGER_HEADER}"
        + "".join(rows)
        + "\n## Notes\n\nTrailing prose.\n"
    )


def cargo_toml(version: str, dependencies: str = "") -> str:
    return (
        f'[package]\nname = "app"\nversion = "{version}"\nedition = "2021"\n\n'
        f"[dependencies]\n{dependencies}"
    )


def cargo_lock(version: str, *extra: str) -> str:
    blocks = [f'[[package]]\nname = "app"\nversion = "{version}"\n', *extra]
    return "version = 4\n\n" + "\n".join(blocks)


def lock_package(name: str, version: str, checksum: str) -> str:
    return (
        f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
        'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
        f'checksum = "{checksum}"\n'
    )


def git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout


class LandingFunctions(unittest.TestCase):
    def test_discover_unit_requires_exactly_one_open_row(self) -> None:
        done_with_link = ledger_row(
            "FX-A",
            "done",
            files="[inventory](../../inventories/2026-09-25-fixture/FX-A.numstat.tsv)",
            diffstat="2 files, +3/-0",
            evidence="[evidence](../../evidence/2026-09-25-fx-a.md)",
        )
        open_row = ledger_row(
            "FX-B", "active", evidence="[evidence](../../evidence/2026-09-25-fx-b.md)"
        )
        changed = {PLAN, "app/src/main.rs", "app/docs/plans/active/README.md"}

        def discover(text: str, paths: set[str] = changed) -> landing.UnitRef:
            return landing.discover_unit(Path("."), REGISTRY, paths, {PLAN: text}.get)

        unit = discover(plan_text(done_with_link, open_row))
        self.assertEqual(
            unit,
            landing.UnitRef(
                project_id="app",
                prefix="app",
                plan_path=PLAN,
                unit="FX-B",
                summary="Change FX-B",
                evidence_path="app/docs/evidence/2026-09-25-fx-b.md",
            ),
        )
        # `done` without an inventory link is a unit the session closed by
        # hand without sealing it; it is still open for the landing.
        unsealed = discover(plan_text(ledger_row("FX-C", "done", diffstat="—")))
        self.assertEqual((unsealed.unit, unsealed.evidence_path), ("FX-C", None))

        cases = {
            "two open rows": (
                plan_text(open_row, ledger_row("FX-C", "done")),
                "found 2",
            ),
            "zero open rows": (
                plan_text(done_with_link, ledger_row("FX-B", "planned")),
                "found 0",
            ),
            "done row with a link": (plan_text(done_with_link), "found 0"),
            "unregistered prefix": (
                plan_text(ledger_row("FX-B", "active", prefix="`other:`")),
                "`other:` is not a registered prefix",
            ),
        }
        for case, (text, message) in cases.items():
            with self.subTest(case=case):
                with self.assertRaises(landing.LandingStop) as raised:
                    discover(text)
                self.assertEqual(raised.exception.step, "preflight")
                self.assertIn(message, str(raised.exception))
                if case != "unregistered prefix":
                    self.assertIn("`active`, or `done` without", str(raised.exception))
        with self.assertRaises(landing.LandingStop) as raised:
            discover(plan_text(open_row), {"app/src/main.rs"})
        self.assertIn("exactly one active plan; found 0", str(raised.exception))

    def test_merge_plan_replaces_own_row_and_keeps_main(self) -> None:
        done_a = ledger_row("X-A", "done", diffstat="1 files, +1/-0")
        planned_b = ledger_row("X-B", "planned")
        done_b = ledger_row("X-B", "done", diffstat="3 files, +9/-1")
        main_text = plan_text(done_a, planned_b, prose="Main prose moved on.")
        branch_text = plan_text(done_a, done_b, prose="The session edited prose.")

        merged = landing.merge_plan(main_text, branch_text, "X-B")
        self.assertEqual(merged, plan_text(done_a, done_b, prose="Main prose moved on."))

        other = ledger_row("X-Z", "done", diffstat="1 files, +2/-0")
        merged = landing.merge_plan(plan_text(done_a, other), branch_text, "X-B")
        self.assertEqual(merged, plan_text(done_a, done_b, other))
        merged = landing.merge_plan(plan_text(other), branch_text, "X-B")
        self.assertEqual(merged, plan_text(done_b, other))

        with self.assertRaises(landing.LandingStop):
            landing.merge_plan("# No ledger\n", branch_text, "X-B")
        with self.assertRaises(landing.LandingStop):
            landing.merge_plan(main_text, "# No ledger\n", "X-B")

    def test_merge_ratchet_takes_lower_and_drops_removed(self) -> None:
        layouts = {
            "class, key, value": lambda key, value: f"lines\t{key}\t{value}\n",
            "value, key": lambda key, value: f"{value}\t{key}\n",
        }
        for layout, row in layouts.items():
            with self.subTest(layout=layout):
                base = "# base comment\n" + row("a", 10) + row("b", 20) + row("c", 30)
                main = "# main comment\n" + row("a", 8) + "\n# b stays\n" + row("b", 20)
                branch = (
                    "# branch comment\n" + row("a", 9) + row("b", 15) + row("c", 30)
                    + row("d", 5)
                )
                self.assertEqual(
                    landing.merge_ratchet(base, main, branch),
                    "# main comment\n" + row("a", 8) + "\n# b stays\n" + row("b", 15)
                    + row("d", 5),
                )
        for broken in ("lines\tpath\n", "lines\t12\t10\n"):
            with self.subTest(broken=broken):
                with self.assertRaises(landing.LandingStop):
                    landing.merge_ratchet("", broken, "")

    def test_lockfile_upgrades_names_moved_packages(self) -> None:
        main_lock = cargo_lock(
            "0.1.0",
            lock_package("serde", "1.0.1", "a" * 64),
            lock_package("syn", "2.0.0", "b" * 64),
        )
        merged_lock = cargo_lock(
            "0.1.0",
            lock_package("serde", "1.0.2", "c" * 64),
            lock_package("syn", "2.0.0", "b" * 64),
            lock_package("newdep", "0.3.0", "d" * 64),
        )
        self.assertEqual(landing.lockfile_upgrades(main_lock, merged_lock), ["serde"])
        self.assertEqual(landing.lockfile_upgrades(main_lock, main_lock), [])
        rechecked = main_lock.replace("b" * 64, "e" * 64)
        self.assertEqual(landing.lockfile_upgrades(main_lock, rechecked), ["syn"])

    def test_numstat_rows_and_render_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git(root, "init", "-q")
            git(root, "config", "user.name", "Landing Fixture")
            git(root, "config", "user.email", "fixture@example.invalid")
            (root / "mod.txt").write_text("one\ntwo\nthree\n", encoding="utf-8")
            (root / "gone.txt").write_text("a\nb\n", encoding="utf-8")
            git(root, "add", ".")
            git(root, "commit", "-qm", "base")
            base = git(root, "rev-parse", "HEAD").strip()

            (root / "mod.txt").write_text("one\nTWO\nthree\nfour\nfive\n", encoding="utf-8")
            (root / "new.txt").write_text("fresh\nlines\n", encoding="utf-8")
            (root / "gone.txt").unlink()
            (root / "blob.bin").write_bytes(b"\x00\x01binary\x00")
            paths = ["new.txt", "mod.txt", "gone.txt", "blob.bin"]
            rows = landing.numstat_rows(root, base, paths)
        self.assertEqual(
            rows,
            [
                landing.InventoryRow(
                    "-", "-", hashlib.sha256(b"\x00\x01binary\x00").hexdigest(), "blob.bin"
                ),
                landing.InventoryRow("0", "2", "deleted", "gone.txt"),
                landing.InventoryRow(
                    "3",
                    "1",
                    hashlib.sha256(b"one\nTWO\nthree\nfour\nfive\n").hexdigest(),
                    "mod.txt",
                ),
                landing.InventoryRow(
                    "2", "0", hashlib.sha256(b"fresh\nlines\n").hexdigest(), "new.txt"
                ),
            ],
        )

        inventory = "docs/inventories/2026-09-25-fixture/FX-A.numstat.tsv"
        text = landing.render_inventory("FX-A", base, rows, inventory)
        lines = text.splitlines()
        self.assertEqual(lines[0], "# FX-A exact change inventory")
        self.assertIn(f"Base revision\t{base}", lines)
        header = lines.index("added\tdeleted\tcontent\tpath")
        table = [line.split("\t") for line in lines[header + 1 :]]
        self.assertEqual([cells[3] for cells in table], sorted(cells[3] for cells in table))
        self_rows = [cells for cells in table if cells[2] == "self"]
        self.assertEqual(self_rows, [[str(len(lines)), "0", "self", inventory]])
        pathspecs = {line.split("\t")[1] for line in lines if line.startswith("Pathspec\t")}
        self.assertEqual(pathspecs, {row.path for row in rows} | {inventory})
        self.assertTrue(text.endswith("\n"))

    def test_diffstat_ignores_binary_lines(self) -> None:
        rows = [
            landing.InventoryRow("3", "1", "0" * 64, "a"),
            landing.InventoryRow("-", "-", "1" * 64, "b"),
            landing.InventoryRow("0", "0", "2" * 64, "c"),
        ]
        self.assertEqual(landing.diffstat(rows), "3 files, +3/-1")

    def test_close_ledger_row_writes_relative_links(self) -> None:
        inventory = "app/docs/inventories/2026-09-25-fixture/FX-B.numstat.tsv"
        evidence = "app/docs/evidence/2026-09-25-fx-b.md"
        plan_directory = posixpath.dirname(PLAN)
        done_a = ledger_row("FX-A", "done", diffstat="1 files, +1/-0")
        text = plan_text(done_a, ledger_row("FX-B", "active"))

        closed = landing.close_ledger_row(
            text,
            "FX-B",
            posixpath.relpath(inventory, plan_directory),
            "4 files, +10/-2",
            posixpath.relpath(evidence, plan_directory),
        )
        self.assertEqual(
            closed,
            plan_text(
                done_a,
                "| FX-B | `app:` | done "
                "| [inventory](../../inventories/2026-09-25-fixture/FX-B.numstat.tsv) "
                "| 4 files, +10/-2 | Change FX-B "
                "| [evidence](../../evidence/2026-09-25-fx-b.md) | None |\n",
            ),
        )
        with self.assertRaises(landing.LandingStop):
            landing.close_ledger_row(text, "FX-Q", "a", "1 files, +1/-0", "b")

    def test_landing_section_two_shapes(self) -> None:
        base = "a" * 40
        built = landing.landing_section(
            base,
            "production-artifact: production inputs changed; run build-production.sh",
            f"complete-production.sh exit 0, manifest git_revision {'b' * 40}",
        )
        self.assertTrue(built.startswith("## Landing\n\n"))
        self.assertIn(f"- **Base revision:** `{base}`\n", built)
        self.assertIn(
            "- **Check:** production-artifact: production inputs changed; "
            "run build-production.sh\n",
            built,
        )
        self.assertIn(
            f"- **Build:** complete-production.sh exit 0, manifest git_revision {'b' * 40}\n",
            built,
        )
        current = landing.landing_section(base, "artifact: app current", None)
        self.assertIn("- **Check:** artifact: app current\n", current)
        self.assertIn("- **Build:** artifact current; no build\n", current)

    def test_land_state_round_trip(self) -> None:
        state = landing.LandState(
            step="rebase",
            branch="unit/app/FX-A",
            kind="bug",
            summary='Fix the "quoted" path \\ and tab\tsafely',
            project_id="app",
            unit="FX-A",
            attempts=2,
        )
        self.assertEqual(landing.LandState.load(state.dump()), state)
        with self.assertRaises(landing.LandingError):
            landing.LandState.load('step = "rebase"\n')

    def test_unbumped_files_restore_only_versions(self) -> None:
        history = "# history\napp\t0.1.0\tbaseline\tFX-0\tAdopt the fixture baseline\n"
        base = {
            "app/Cargo.toml": cargo_toml("0.1.0", 'serde = "1"\n'),
            "app/Cargo.lock": cargo_lock("0.1.0"),
            "docs/version-history.tsv": history,
        }
        branch = {
            "app/Cargo.toml": cargo_toml("0.1.1", 'serde = "1"\nsyn = "2"\n'),
            "app/Cargo.lock": cargo_lock("0.1.1", lock_package("syn", "2.0.0", "b" * 64)),
            "docs/version-history.tsv": history + "app\t0.1.1\tbug\tFX-A\tFix it\n",
        }

        def reader(files: dict[str, str]):
            return lambda path: files[path].encode("utf-8") if path in files else None

        rewrites = landing.unbumped_files(REGISTRY, reader(base), reader(branch))
        self.assertEqual(
            rewrites,
            {
                "app/Cargo.toml": cargo_toml("0.1.0", 'serde = "1"\nsyn = "2"\n').encode(),
                "app/Cargo.lock": cargo_lock(
                    "0.1.0", lock_package("syn", "2.0.0", "b" * 64)
                ).encode(),
                "docs/version-history.tsv": history.encode(),
            },
        )
        # Dependency-only changes to a version source are the session's work.
        dependency_only = dict(branch)
        dependency_only["app/Cargo.toml"] = cargo_toml("0.1.0", 'serde = "1"\nsyn = "2"\n')
        dependency_only["app/Cargo.lock"] = base["app/Cargo.lock"]
        dependency_only["docs/version-history.tsv"] = history
        self.assertEqual(
            landing.unbumped_files(REGISTRY, reader(base), reader(dependency_only)), {}
        )


COPIED_SCRIPTS = (
    "project_registry.py",
    "documentation_contract.py",
    "version_contract.py",
    "version_tool.py",
    "production_artifact.py",
    "complete-production.py",
    "landing.py",
    "land-unit.py",
)
PYTHON_GUARDS = ("check-staged-units.py", "commit_scope.py", "check-language-contract.py")
SHELL_GUARDS = ("check-architecture-contract.sh", "check-documentation-contract.sh")
GUARD_ORDER = [
    "check-staged-units.py",
    "commit_scope.py",
    "check-architecture-contract.sh",
    "check-language-contract.py",
    "check-documentation-contract.sh",
]
HISTORY = (
    "# owner\tversion\tkind\tunit\tsummary\n"
    "app\t0.1.0\tbaseline\tFX-0\tAdopt the fixture baseline\n"
)
RATCHET = "# Fixture debt ratchet.\nlines\tapp/src/main.rs\t10\n"
INVENTORY = "app/docs/inventories/2026-09-25-fixture/FX-A.numstat.tsv"
EVIDENCE = "app/docs/evidence/2026-09-25-fx-a.md"
STATUS_LINE = "- **Focus:** fixture baseline\n"


def ratchet(value: int) -> str:
    return RATCHET.replace("\t10\n", f"\t{value}\n")


def described(result: subprocess.CompletedProcess[str]) -> str:
    return f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"


def load_production_fixture() -> types.ModuleType:
    """Import test-production-artifacts.py to reuse its fixture entry scripts."""
    path = SCRIPTS / "test-production-artifacts.py"
    spec = importlib.util.spec_from_file_location("production_artifact_fixture", path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PRODUCTION_FIXTURE = load_production_fixture()


def python_guard(name: str) -> str:
    return f"""#!/usr/bin/env python3
\"\"\"Fixture double of {name}: record the call and pass unless told to fail.\"\"\"
import os
import sys

with open(os.path.join(os.environ["LAND_FIXTURE_RECORDS"], ".guards-ran"), "a") as log:
    log.write("{name}\\n")
if not sys.stdin.isatty():
    sys.stdin.read()
if os.environ.get("LAND_FIXTURE_FAIL_GUARD") == "{name}":
    print("fixture guard {name} failed", file=sys.stderr)
    sys.exit(1)
"""


def shell_guard(name: str) -> str:
    return f"""#!/bin/sh
set -eu
# Fixture double of {name}: record the call and pass unless told to fail.
printf '%s\\n' '{name}' >> "$LAND_FIXTURE_RECORDS/.guards-ran"
if [ "${{LAND_FIXTURE_FAIL_GUARD:-}}" = '{name}' ]; then
    printf '%s\\n' 'fixture guard {name} failed' >&2
    exit 1
fi
"""


COMPLETE_ENTRY = """#!/bin/sh
set -eu
# Fixture double of complete-production.sh: build and verify through the real
# artifact runner with the fixture entries, and record that it ran.
printf '%s\\n' "$(git rev-parse HEAD)" >> "$LAND_FIXTURE_RECORDS/.builds-ran"
if [ "${LAND_FIXTURE_BUILD_BEHAVIOR:-success}" = hang ]; then
    sleep 60
fi
python3 scripts/production_artifact.py run-build app > /dev/null
python3 scripts/production_artifact.py run-verification app > /dev/null
"""

FIXTURE_GIT = """#!/bin/sh
# Fixture wrapper of git, selected through LAND_UNIT_GIT:
# - LAND_FIXTURE_STOP_AFTER=seal-write turns the seal's `git add --all` into a
#   Ctrl-C of the landing, after the inventory exists and before it is staged;
# - LAND_FIXTURE_PUSH=offline moves origin away before the push, as if the
#   network went down;
# - LAND_FIXTURE_PUSH=reports-failure pushes, then reports a failure.
for argument in "$@"; do
    case "$argument:${LAND_FIXTURE_STOP_AFTER:-}:${LAND_FIXTURE_PUSH:-}" in
        --all:seal-write:*)
            kill -INT "$PPID"
            exit 130
            ;;
        push::offline)
            mv "$LAND_FIXTURE_ORIGIN" "$LAND_FIXTURE_ORIGIN.away"
            ;;
        push::reports-failure)
            git "$@" || exit
            printf '%s\\n' 'fatal: the remote end hung up unexpectedly' >&2
            exit 128
            ;;
    esac
done
exec git "$@"
"""


def repository_hook(name: str) -> str:
    return f"""#!/bin/sh
set -eu
# Fixture repository hook {name}: record the run and pass unless told to fail.
printf '%s\\n' '{name}' >> "$LAND_FIXTURE_RECORDS/.hooks-ran"
if [ "${{LAND_FIXTURE_FAIL_HOOK:-}}" = '{name}' ]; then
    printf '%s\\n' 'fixture hook {name} failed' >&2
    exit 1
fi
"""


FAKE_CARGO = """#!/bin/sh
set -eu
# Fixture double of cargo: record the call; fail like an offline resolver when
# LAND_FIXTURE_CARGO_OFFLINE_FAILS=1.
printf '%s\\n' "$*" >> "$LAND_FIXTURE_RECORDS/cargo-calls"
if [ "${LAND_FIXTURE_CARGO_OFFLINE_FAILS:-0}" = 1 ]; then
    printf '%s\\n' 'error: no matching package found; the network is needed' >&2
    exit 101
fi
printf '%s\\n' '{}'
"""


def pre_receive_hook(counter: Path, accept_after: int | None) -> str:
    """A hook that counts pushes, and moves main before each rejection.

    A rejected landing push means another landing won the race, so the double
    advances main by one empty commit whenever it rejects.
    """
    accept = "false" if accept_after is None else f'[ "$count" -gt {accept_after} ]'
    return f"""#!/bin/sh
set -eu
count=$(( $(cat '{counter}' 2>/dev/null || printf 0) + 1 ))
printf '%s\\n' "$count" > '{counter}'
if {accept}; then
    exit 0
fi
unset GIT_QUARANTINE_PATH GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
export GIT_AUTHOR_NAME=Racer GIT_AUTHOR_EMAIL=racer@example.invalid
export GIT_COMMITTER_NAME=Racer GIT_COMMITTER_EMAIL=racer@example.invalid
tree=$(git rev-parse 'refs/heads/main^{{tree}}')
racer=$(git commit-tree "$tree" -p refs/heads/main -m "Race $count")
git update-ref refs/heads/main "$racer"
printf '%s\\n' 'fixture: main moved; push rejected' >&2
exit 1
"""


class LandUnitFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.top = Path(self.temporary.name)
        self.origin = self.top / "origin.git"
        self.repo = self.top / "repo"
        self.worktrees = self.top / "repo.worktrees"
        self.landing_dir = self.worktrees / ".landing"
        self.records = self.top / "records"
        self.bin = self.top / "bin"
        for directory in (self.records, self.bin):
            directory.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text(FAKE_CARGO, encoding="utf-8")
        cargo.chmod(0o755)
        self.fixture_git = self.bin / "fixture-git"
        self.fixture_git.write_text(FIXTURE_GIT, encoding="utf-8")
        self.fixture_git.chmod(0o755)
        hooks = self.top / "hooks"
        hooks.mkdir()
        for name in ("pre-commit", "commit-msg"):
            (hooks / name).write_text(repository_hook(name), encoding="utf-8")
            (hooks / name).chmod(0o755)
        self.environment = dict(os.environ)
        self.environment.update(
            {
                "LAND_FIXTURE_RECORDS": str(self.records),
                "LAND_FIXTURE_ORIGIN": str(self.origin),
                "LAND_UNIT_CARGO": str(cargo),
                "PATH": f"{self.bin}{os.pathsep}{os.environ.get('PATH', '')}",
                "PYTHONDONTWRITEBYTECODE": "1",
                "GIT_EDITOR": "true",
            }
        )

        self.git(self.top, "init", "--quiet", "--bare", "--initial-branch=main", str(self.origin))
        self.git(self.top, "clone", "--quiet", str(self.origin), str(self.repo))
        self.configure(self.repo)
        # Hooks are repository configuration, so the session worktrees and the
        # landing worktree share them, as in the real clone.
        self.git(self.repo, "config", "core.hooksPath", str(hooks))
        self.git(self.repo, "checkout", "--quiet", "-b", "main")
        self.write_tree(self.repo)
        self.git(self.repo, "add", "--all")
        self.git(self.repo, "commit", "--quiet", "-m", "Establish the fixture")
        self.git(self.repo, "push", "--quiet", "--set-upstream", "origin", "main")
        (self.repo / "app/target/release").mkdir(parents=True)
        (self.repo / "app/target/release/app").write_bytes(b"fixture release\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    # Repository construction.

    def git(self, cwd: Path, *args: str, check: bool = True) -> str:
        result = subprocess.run(
            ["git", *args],
            cwd=cwd,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=self.environment,
        )
        if check and result.returncode != 0:
            self.fail(f"git {' '.join(args)} failed:\n{result.stderr}")
        return result.stdout

    def configure(self, clone: Path) -> None:
        self.git(clone, "config", "user.name", "Landing Fixture")
        self.git(clone, "config", "user.email", "fixture@example.invalid")

    def write(self, root: Path, relative: str, text: str, mode: int | None = None) -> None:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        if mode is not None:
            path.chmod(mode)

    def write_tree(self, root: Path) -> None:
        self.write(root, ".gitignore", "target/\n/.fixture-*\n__pycache__/\n")
        self.write(root, "docs/projects.toml", REGISTRY_TOML)
        self.write(root, "docs/version-history.tsv", HISTORY)
        self.write(root, "docs/notes.md", "# Notes\n\nSuite notes.\n")
        self.write(root, "ws/Cargo.lock", "version = 4\n")
        (root / "scripts").mkdir()
        for name in COPIED_SCRIPTS:
            shutil.copy2(SCRIPTS / name, root / "scripts" / name)
        for name in PYTHON_GUARDS:
            self.write(root, f"scripts/{name}", python_guard(name), 0o755)
        for name in SHELL_GUARDS:
            self.write(root, f"scripts/{name}", shell_guard(name), 0o755)
        self.write(root, "scripts/architecture-baseline.tsv", RATCHET)
        self.write(root, "app/Cargo.toml", cargo_toml("0.1.0"))
        self.write(root, "app/Cargo.lock", cargo_lock("0.1.0"))
        self.write(root, "app/src/main.rs", "fn main() {}\n")
        self.write(root, "app/tests/case.txt", "case\n")
        self.write(root, "app/STATUS.md", f"# App status\n\n{STATUS_LINE}")
        self.write(root, "app/docs/evidence/README.md", "# Evidence\n")
        self.write(root, PLAN, plan_text(ledger_row("FX-A", "planned")))
        entries = types.SimpleNamespace(root=root)
        (root / "app/scripts").mkdir(parents=True)
        for phase in ("build", "verify"):
            PRODUCTION_FIXTURE.ProductionArtifactFixture.write_entry_script(
                entries, "app", phase, "v1"
            )
        self.write(root, "app/scripts/complete-production.sh", COMPLETE_ENTRY, 0o755)
        for phase in ("deploy", "status"):
            self.write(root, f"app/scripts/{phase}-production.sh", f"#!/bin/sh\n# {phase}\n", 0o755)

    # Sessions and other landings.

    def open_branch(self, unit: str, project: str = "app") -> Path:
        worktree = self.worktrees / f"{project}-{unit}"
        self.git(self.repo, "fetch", "--quiet", "origin", "main")
        self.git(
            self.repo,
            "worktree",
            "add",
            "--quiet",
            "-b",
            f"unit/{project}/{unit}",
            str(worktree),
            "origin/main",
        )
        return worktree

    def commit_all(self, root: Path, message: str) -> None:
        self.git(root, "add", "--all")
        self.git(root, "commit", "--quiet", "-m", message)

    def set_ratchet(self, root: Path, value: int) -> None:
        self.write(root, "scripts/architecture-baseline.tsv", ratchet(value))

    def write_unit(
        self,
        worktree: Path,
        unit: str,
        *,
        bump: bool = False,
        touch_ratchet: int | None = None,
        edit_status: bool = False,
        touch_source: bool = True,
    ) -> None:
        evidence = f"app/docs/evidence/2026-09-25-{unit.lower()}.md"
        if touch_source:
            with (worktree / "app/src/main.rs").open("a", encoding="utf-8") as source:
                source.write(f"// {unit}\n")
        plan = (worktree / PLAN).read_text(encoding="utf-8")
        self.write(
            worktree,
            PLAN,
            plan.replace(
                ledger_row(unit, "planned"),
                ledger_row(
                    unit, "active", evidence=f"[evidence](../../evidence/{Path(evidence).name})"
                ),
            ),
        )
        self.write(
            worktree,
            evidence,
            f"# Evidence: {unit}\n\n- **Date:** 2026-09-25\n\n## Result\n\nThe unit works.\n",
        )
        if touch_ratchet is not None:
            self.set_ratchet(worktree, touch_ratchet)
        if edit_status:
            status = (worktree / "app/STATUS.md").read_text(encoding="utf-8")
            focus = status.replace(STATUS_LINE, f"- **Focus:** {unit}\n")
            self.write(worktree, "app/STATUS.md", focus)
        if bump:
            self.write(worktree, "app/Cargo.toml", cargo_toml("0.1.1"))
            self.write(worktree, "app/Cargo.lock", cargo_lock("0.1.1"))
            with (worktree / "docs/version-history.tsv").open("a", encoding="utf-8") as history:
                history.write(f"app\t0.1.1\tbug\t{unit}\tChange {unit}\n")
        self.commit_all(worktree, f"Work on {unit}")

    def advance_main(self, change, message: str) -> str:
        """Land `change(clone)` on origin/main from another clone; return the new main."""
        other = self.top / "other"
        if not other.exists():
            self.git(self.top, "clone", "--quiet", str(self.origin), str(other))
            self.configure(other)
        self.git(other, "pull", "--quiet", "--ff-only", "origin", "main")
        change(other)
        self.commit_all(other, message)
        self.git(other, "push", "--quiet", "origin", "main")
        return self.git(other, "rev-parse", "HEAD").strip()

    def build_main(self) -> None:
        """Build and verify the artifact of repo/'s current tree, then forget it ran."""
        result = subprocess.run(
            ["sh", "app/scripts/complete-production.sh"],
            cwd=self.repo,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=self.environment,
        )
        self.assertEqual(result.returncode, 0, msg=result.stderr)
        (self.records / ".builds-ran").unlink()

    def land(self, *arguments: str, **environment: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(self.repo / "scripts/land-unit.py"), *arguments],
            cwd=self.repo,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**self.environment, **environment},
        )

    # Observations.

    def forget_hooks(self) -> None:
        """Drop the hook runs of the session's own commits."""
        (self.records / ".hooks-ran").unlink(missing_ok=True)

    def records_of(self, name: str) -> list[str]:
        path = self.records / name
        return path.read_text(encoding="utf-8").splitlines() if path.exists() else []

    def origin_main(self) -> str:
        return self.git(self.origin, "rev-parse", "refs/heads/main").strip()

    def show_main(self, path: str) -> str:
        return self.git(self.origin, "show", f"refs/heads/main:{path}")

    def assert_landed(self, result: subprocess.CompletedProcess[str], before: str) -> str:
        self.assertEqual(result.returncode, 0, msg=described(result))
        head = self.origin_main()
        self.assertEqual(self.git(self.origin, "rev-parse", f"{head}^").strip(), before)
        self.assertEqual(self.git(self.repo, "rev-parse", "HEAD").strip(), head)
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")
        self.assertFalse(self.landing_dir.exists())
        return head

    def assert_not_landed(self, result: subprocess.CompletedProcess[str], before: str) -> None:
        self.assertEqual(result.returncode, 1, msg=described(result))
        self.assertEqual(self.origin_main(), before)

    def test_fast_forward_landing(self) -> None:
        before = self.origin_main()
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        self.forget_hooks()

        result = self.land("unit/app/FX-A", "--kind", "bug")
        head = self.assert_landed(result, before)

        # One sealed commit through the hooks; the landing worktree's temporary
        # commits and the rebase run none.
        self.assertEqual(self.records_of(".hooks-ran"), ["pre-commit", "commit-msg"])
        subject = self.git(self.origin, "log", "-1", "--format=%s", head).strip()
        self.assertEqual(subject, "app-bug: Change FX-A")
        inventory = self.show_main(INVENTORY)
        self.assertIn(f"Base revision\t{before}\n", inventory)
        self.assertEqual(self.git(self.origin, "rev-parse", f"{head}^").strip(), before)
        numstat = [
            line.split("\t")
            for line in self.git(self.origin, "show", "--numstat", "--format=", head).splitlines()
            if line
        ]
        expected = (
            f"{len(numstat)} files, +{sum(int(cells[0]) for cells in numstat)}"
            f"/-{sum(int(cells[1]) for cells in numstat)}"
        )
        plan = self.show_main(PLAN)
        self.assertIn(
            "| FX-A | `app:` | done "
            "| [inventory](../../inventories/2026-09-25-fixture/FX-A.numstat.tsv) "
            f"| {expected} | Change FX-A "
            "| [evidence](../../evidence/2026-09-25-fx-a.md) | None |\n",
            plan,
        )
        table = inventory.split("added\tdeleted\tcontent\tpath\n", 1)[1]
        inventory_paths = {line.split("\t")[3] for line in table.splitlines()}
        self.assertEqual(inventory_paths, {cells[2] for cells in numstat})
        evidence = self.show_main(EVIDENCE)
        landing_section = evidence[evidence.index("\n## Landing\n") :]
        self.assertIn(f"- **Base revision:** `{before}`\n", landing_section)
        self.assertRegex(
            landing_section,
            r"- \*\*Build:\*\* complete-production\.sh exit 0, "
            r"manifest git_revision [0-9a-f]{40}\n$",
        )
        self.assertIn('version = "0.1.1"', self.show_main("app/Cargo.toml"))
        self.assertEqual(
            self.show_main("docs/version-history.tsv"),
            HISTORY + "app\t0.1.1\tbug\tFX-A\tChange FX-A\n",
        )
        self.assertEqual(self.records_of(".guards-ran"), GUARD_ORDER)
        self.assertEqual(len(self.records_of(".builds-ran")), 1)


    def test_other_product_advanced_needs_no_build(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A", touch_source=False)
        self.build_main()
        before = self.advance_main(
            lambda clone: self.write(clone, "docs/notes.md", "# Notes\n\nAnother unit.\n"),
            "suite-maintenance: Change the suite notes",
        )

        result = self.land("unit/app/FX-A", "--kind", "maintenance")
        head = self.assert_landed(result, before)

        self.assertEqual(
            self.git(self.origin, "log", "-1", "--format=%s", head).strip(),
            "app-maintenance: Change FX-A",
        )
        self.assertEqual(self.records_of(".builds-ran"), [])
        self.assertEqual(self.show_main("app/Cargo.toml"), cargo_toml("0.1.0"))
        evidence = self.show_main(EVIDENCE)
        self.assertIn(
            "- **Check:** `production_artifact.py check app --require-verified` exit 0", evidence
        )
        self.assertTrue(evidence.endswith("- **Build:** artifact current; no build\n"))

    def test_same_product_advanced_rebumps_and_merges(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A", touch_ratchet=8)
        fx_z = ledger_row(
            "FX-Z",
            "done",
            files="[inventory](../../inventories/2026-09-25-fixture/FX-Z.numstat.tsv)",
            diffstat="5 files, +9/-3",
            evidence="[evidence](../../evidence/2026-09-25-fx-z.md)",
        )
        fx_z_history = "app\t0.1.1\tbug\tFX-Z\tChange FX-Z\n"

        def land_fx_z(clone: Path) -> None:
            self.write(clone, "app/Cargo.toml", cargo_toml("0.1.1"))
            self.write(clone, "app/Cargo.lock", cargo_lock("0.1.1"))
            self.write(clone, "docs/version-history.tsv", HISTORY + fx_z_history)
            self.set_ratchet(clone, 9)
            plan = (clone / PLAN).read_text(encoding="utf-8")
            planned = ledger_row("FX-A", "planned")
            self.write(clone, PLAN, plan.replace(planned, planned + fx_z))

        before = self.advance_main(land_fx_z, "app-bug: Change FX-Z")
        result = self.land("unit/app/FX-A", "--kind", "bug")
        self.assert_landed(result, before)

        self.assertEqual(self.show_main("app/Cargo.toml"), cargo_toml("0.1.2"))
        self.assertEqual(
            self.show_main("docs/version-history.tsv"),
            HISTORY + fx_z_history + "app\t0.1.2\tbug\tFX-A\tChange FX-A\n",
        )
        self.assertEqual(self.show_main("scripts/architecture-baseline.tsv"), ratchet(8))
        plan = self.show_main(PLAN)
        self.assertIn(fx_z, plan)
        self.assertIn("| FX-A | `app:` | done | [inventory](", plan)
        self.assertLess(plan.index("| FX-A |"), plan.index("| FX-Z |"))
        self.assertEqual(len(self.records_of(".builds-ran")), 1)

    def test_branch_bump_is_stripped_and_warned(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A", bump=True)
        before = self.origin_main()

        result = self.land("unit/app/FX-A", "--kind", "bug")
        self.assert_landed(result, before)

        self.assertIn("the session bumped", result.stderr)
        self.assertEqual(self.show_main("app/Cargo.toml"), cargo_toml("0.1.1"))
        self.assertEqual(self.show_main("app/Cargo.lock"), cargo_lock("0.1.1"))
        self.assertEqual(
            self.show_main("docs/version-history.tsv"),
            HISTORY + "app\t0.1.1\tbug\tFX-A\tChange FX-A\n",
        )
        self.assertEqual(len(self.records_of(".builds-ran")), 1)

    def test_plan_archived_on_main_stops(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        archived = PLAN.replace("/active/", "/archive/")

        def archive(clone: Path) -> None:
            (clone / archived).parent.mkdir(parents=True)
            (clone / PLAN).rename(clone / archived)

        before = self.advance_main(archive, "app-maintenance: Archive the fixture plan")
        result = self.land("unit/app/FX-A", "--kind", "bug")

        self.assert_not_landed(result, before)
        self.assertIn(f"stopped at rebase: {PLAN} is no longer an active plan", result.stderr)
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())
        self.assertEqual(self.records_of(".builds-ran"), [])

    def test_prose_conflict_stops_and_continue_lands(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A", edit_status=True)
        before = self.advance_main(
            lambda clone: self.write(
                clone, "app/STATUS.md", f"# App status\n\n- **Focus:** another unit\n"
            ),
            "app-maintenance: Record another focus",
        )

        stopped = self.land("unit/app/FX-A", "--kind", "bug")
        self.assert_not_landed(stopped, before)
        self.assertIn("stopped at rebase: conflict in app/STATUS.md", stopped.stderr)
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")

        resolved = "# App status\n\n- **Focus:** FX-A after another unit\n"
        self.write(self.landing_dir, "app/STATUS.md", resolved)
        self.git(self.landing_dir, "add", "app/STATUS.md")
        self.git(self.landing_dir, "rebase", "--continue")
        result = self.land("--continue")

        head = self.assert_landed(result, before)
        self.assertEqual(self.git(self.origin, "rev-parse", f"{head}^").strip(), before)
        self.assertEqual(self.show_main("app/STATUS.md"), resolved)
        self.assertIn(f"Base revision\t{before}\n", self.show_main(INVENTORY))
        self.assertEqual(len(self.records_of(".builds-ran")), 1)

    def test_lockfile_needing_network_stops(self) -> None:
        worktree = self.open_branch("FX-A")
        branch_lock = cargo_lock("0.1.0", lock_package("newdep", "0.3.0", "d" * 64))
        main_lock = cargo_lock("0.1.0", lock_package("otherdep", "1.0.0", "e" * 64))
        self.write(worktree, "app/Cargo.lock", branch_lock)
        self.write_unit(worktree, "FX-A")
        before = self.advance_main(
            lambda clone: self.write(clone, "app/Cargo.lock", main_lock),
            "app-maintenance: Lock another dependency",
        )

        result = self.land(
            "unit/app/FX-A", "--kind", "maintenance", LAND_FIXTURE_CARGO_OFFLINE_FAILS="1"
        )

        self.assert_not_landed(result, before)
        self.assertIn(
            "stopped at rebase: app/Cargo.lock: `cargo metadata --offline` failed", result.stderr
        )
        self.assertEqual(self.records_of("cargo-calls"), ["metadata --offline --format-version 1"])
        self.assertEqual(self.records_of(".builds-ran"), [])

    def install_pre_receive(self, accept_after: int | None) -> Path:
        counter = self.top / "pushes"
        self.write(self.origin, "hooks/pre-receive", pre_receive_hook(counter, accept_after), 0o755)
        return counter

    def test_push_rejected_retries_then_stops(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()
        counter = self.install_pre_receive(accept_after=1)

        result = self.land("unit/app/FX-A", "--kind", "bug")

        racer = self.git(self.origin, "rev-parse", "refs/heads/main^").strip()
        head = self.assert_landed(result, racer)
        self.assertEqual(counter.read_text(encoding="utf-8"), "2\n")
        self.assertEqual(self.git(self.origin, "log", "-1", "--format=%s", racer).strip(), "Race 1")
        self.assertEqual(self.git(self.origin, "rev-parse", f"{racer}^").strip(), before)
        self.assertIn(f"Base revision\t{racer}\n", self.show_main(INVENTORY))
        self.assertEqual(self.show_main("app/Cargo.toml"), cargo_toml("0.1.1"))
        # The racer did not move the product's inputs, so the retry reuses the
        # artifact the first attempt built.
        self.assertEqual(len(self.records_of(".builds-ran")), 1)

        # A fresh fixture where every push loses the race.
        self.tearDown()
        self.setUp()
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        counter = self.install_pre_receive(accept_after=None)

        result = self.land("unit/app/FX-A", "--kind", "bug")

        self.assertEqual(result.returncode, 1, msg=result.stderr)
        self.assertIn("stopped at push: the push was rejected", result.stderr)
        self.assertEqual(counter.read_text(encoding="utf-8"), "4\n")
        self.assertEqual(len(self.records_of(".builds-ran")), 1)
        self.assertEqual(
            self.git(self.repo, "rev-parse", "HEAD").strip(), self.origin_main()
        )
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())

    def test_push_offline_keeps_main_and_resumes(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()
        away = self.top / "origin.git.away"
        offline = {"LAND_UNIT_GIT": str(self.fixture_git), "LAND_FIXTURE_PUSH": "offline"}

        def assert_stopped_with_main_kept(result: subprocess.CompletedProcess[str]) -> None:
            self.assertEqual(result.returncode, 1, msg=described(result))
            self.assertIn(
                "stopped at push: the push failed and origin cannot be fetched", result.stderr
            )
            self.assertEqual(self.git(self.repo, "rev-parse", "refs/heads/main").strip(), before)
            self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")
            self.assertEqual(self.git(self.repo, "status", "--porcelain"), "")
            state = (self.landing_dir / ".land-state.toml").read_text(encoding="utf-8")
            self.assertIn('step = "commit_and_push"', state)

        assert_stopped_with_main_kept(self.land("unit/app/FX-A", "--kind", "bug", **offline))
        aborted = self.land("--abort")
        self.assertEqual(aborted.returncode, 0, msg=described(aborted))
        self.assertEqual(self.git(self.repo, "rev-parse", "refs/heads/main").strip(), before)
        self.assertEqual(self.git(self.repo, "status", "--porcelain", "--untracked-files=all"), "")
        self.assertFalse(self.landing_dir.exists())

        away.rename(self.origin)
        self.forget_hooks()
        assert_stopped_with_main_kept(self.land("unit/app/FX-A", "--kind", "bug", **offline))
        away.rename(self.origin)
        result = self.land("--continue")

        head = self.assert_landed(result, before)
        self.assertEqual(
            self.git(self.origin, "log", "-1", "--format=%s", head).strip(), "app-bug: Change FX-A"
        )
        # The resumed run pushed the commit sealed before the outage.
        self.assertEqual(self.records_of(".hooks-ran"), ["pre-commit", "commit-msg"])

    def test_push_reported_failed_but_landed(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()
        self.forget_hooks()

        result = self.land(
            "unit/app/FX-A",
            "--kind",
            "bug",
            LAND_UNIT_GIT=str(self.fixture_git),
            LAND_FIXTURE_PUSH="reports-failure",
        )

        head = self.assert_landed(result, before)
        self.assertIn(f"land-unit: landed {head} app-bug: Change FX-A", result.stdout)
        self.assertEqual(self.records_of(".hooks-ran"), ["pre-commit", "commit-msg"])
        self.assertEqual(len(self.records_of(".builds-ran")), 1)

    def test_review_focus_preflight_stops(self) -> None:
        inventory_on_main = "app/docs/inventories/2026-09-25-fixture/FX-0.numstat.tsv"

        def row_already_sealed(worktree: Path) -> None:
            plan = (worktree / PLAN).read_text(encoding="utf-8")
            active = ledger_row(
                "FX-A", "active", evidence="[evidence](../../evidence/2026-09-25-fx-a.md)"
            )
            sealed = ledger_row(
                "FX-A",
                "done",
                files="[inventory](../../inventories/2026-09-25-fixture/FX-A.numstat.tsv)",
                diffstat="3 files, +5/-0",
                evidence="[evidence](../../evidence/2026-09-25-fx-a.md)",
            )
            self.write(worktree, PLAN, plan.replace(active, sealed))

        def edits_inventory(worktree: Path) -> None:
            self.write(worktree, inventory_on_main, "# FX-0 exact change inventory, edited\n")

        def evidence_unlinked(worktree: Path) -> None:
            plan = (worktree / PLAN).read_text(encoding="utf-8")
            self.write(
                worktree,
                PLAN,
                plan.replace("[evidence](../../evidence/2026-09-25-fx-a.md)", "`cargo test`"),
            )

        def evidence_elsewhere(worktree: Path) -> None:
            plan = (worktree / PLAN).read_text(encoding="utf-8")
            self.write(worktree, "app/notes/fx-a.md", "# Evidence kept elsewhere\n")
            self.write(
                worktree,
                PLAN,
                plan.replace("../../evidence/2026-09-25-fx-a.md", "../../../notes/fx-a.md"),
            )

        def dirty_checkout() -> None:
            self.write(self.repo, "scratch.txt", "left behind\n")

        def other_branch() -> None:
            self.git(self.repo, "checkout", "--quiet", "-b", "elsewhere")

        cases = {
            "row already sealed": (
                row_already_sealed,
                None,
                "`active`, or `done` without an inventory link; found 0",
            ),
            "tracked inventory edited": (
                edits_inventory,
                None,
                f"inventories, which are immutable: {inventory_on_main}",
            ),
            "evidence link missing": (evidence_unlinked, None, "must link its evidence record"),
            "evidence outside the owner": (
                evidence_elsewhere,
                None,
                "app/notes/fx-a.md is not under app/docs/evidence/",
            ),
            "dirty canonical checkout": (
                None,
                dirty_checkout,
                "must be a clean main; it is on main with changes:\n?? scratch.txt",
            ),
            "canonical checkout elsewhere": (
                None,
                other_branch,
                "must be a clean main; it is on elsewhere",
            ),
        }
        for index, (case, (edit_branch, edit_canonical, message)) in enumerate(cases.items()):
            with self.subTest(case=case):
                if index:
                    self.tearDown()
                    self.setUp()
                self.advance_main(
                    lambda clone: self.write(
                        clone, inventory_on_main, "# FX-0 exact change inventory\n"
                    ),
                    "app-maintenance: Record an earlier inventory",
                )
                worktree = self.open_branch("FX-A")
                self.write_unit(worktree, "FX-A")
                if edit_branch is not None:
                    edit_branch(worktree)
                    self.commit_all(worktree, f"Break the unit: {case}")
                if edit_canonical is not None:
                    edit_canonical()
                before = self.origin_main()

                result = self.land("unit/app/FX-A", "--kind", "bug")

                self.assert_not_landed(result, before)
                self.assertIn("stopped at preflight: ", result.stderr)
                self.assertIn(message, result.stderr)
                self.assertEqual(self.records_of(".builds-ran"), [])
                self.assertFalse(self.landing_dir.exists())

    def test_interrupted_build_abort_restores_main(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()
        process = subprocess.Popen(
            [sys.executable, str(self.repo / "scripts/land-unit.py"), "unit/app/FX-A"]
            + ["--kind", "bug"],
            cwd=self.repo,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            env={**self.environment, "LAND_FIXTURE_BUILD_BEHAVIOR": "hang"},
            start_new_session=True,
            # A test runner may start with SIGINT ignored, which children
            # inherit; Ctrl-C needs its default disposition back.
            preexec_fn=lambda: signal.signal(signal.SIGINT, signal.SIG_DFL),
        )
        try:
            deadline = time.monotonic() + 60
            while not (self.records / ".builds-ran").exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    self.fail("the fixture build never started")
                time.sleep(0.05)
            # Ctrl-C reaches the whole foreground process group.
            os.killpg(process.pid, signal.SIGINT)
            _stdout, stderr = process.communicate(timeout=60)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()

        self.assertEqual(process.returncode, 1, msg=stderr)
        self.assertIn("interrupted during build_if_stale", stderr)
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--quiet", "HEAD", check=False), "")
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())
        self.assertEqual(self.origin_main(), before)

        aborted = self.land("--abort")

        self.assertEqual(aborted.returncode, 0, msg=aborted.stderr)
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")
        self.assertEqual(self.git(self.repo, "status", "--porcelain"), "")
        self.assertFalse(self.landing_dir.exists())
        self.assertEqual(self.origin_main(), before)

    def test_suite_unit_lands_without_build(self) -> None:
        plan = "docs/plans/active/2026-09-25-suite.md"
        suite_row = ledger_row("SU-A", "planned", prefix="`suite:`")
        self.advance_main(
            lambda clone: self.write(clone, plan, plan_text(suite_row)),
            "suite-maintenance: Open the suite plan",
        )
        worktree = self.open_branch("SU-A", project="suite")
        active = ledger_row(
            "SU-A",
            "active",
            prefix="`suite:`",
            evidence="[evidence](../../evidence/2026-09-25-su-a.md)",
        )
        self.write(worktree, plan, plan_text(active))
        self.write(worktree, "docs/notes.md", "# Notes\n\nSuite work.\n")
        self.write(worktree, "docs/evidence/2026-09-25-su-a.md", "# Evidence: SU-A\n")
        self.commit_all(worktree, "Work on SU-A")
        before = self.origin_main()

        refused = self.land("unit/suite/SU-A", "--kind", "bug")
        self.assert_not_landed(refused, before)
        self.assertIn("suite-bug bumps products", refused.stderr)

        result = self.land("unit/suite/SU-A", "--kind", "maintenance")
        head = self.assert_landed(result, before)
        self.assertEqual(
            self.git(self.origin, "log", "-1", "--format=%s", head).strip(),
            "suite-maintenance: Change SU-A",
        )
        self.assertIn(
            f"Base revision\t{before}\n",
            self.show_main("docs/inventories/2026-09-25-suite/SU-A.numstat.tsv"),
        )
        self.assertIn(
            "| SU-A | `suite:` | done "
            "| [inventory](../../inventories/2026-09-25-suite/SU-A.numstat.tsv) |",
            self.show_main(plan),
        )
        self.assertTrue(
            self.show_main("docs/evidence/2026-09-25-su-a.md").endswith(
                "- **Build:** none; a suite unit has no production owner\n"
            )
        )
        self.assertEqual(self.records_of(".builds-ran"), [])

    def test_suite_unit_changing_product_inputs_builds_once(self) -> None:
        plan = "docs/plans/active/2026-09-25-suite.md"
        self.advance_main(
            lambda clone: self.write(
                clone, plan, plan_text(ledger_row("SU-B", "planned", prefix="`suite:`"))
            ),
            "suite-maintenance: Open the suite plan",
        )
        worktree = self.open_branch("SU-B", project="suite")
        active = ledger_row(
            "SU-B",
            "active",
            prefix="`suite:`",
            evidence="[evidence](../../evidence/2026-09-25-su-b.md)",
        )
        self.write(worktree, plan, plan_text(active))
        with (worktree / "app/src/main.rs").open("a", encoding="utf-8") as source:
            source.write("// shared change\n")
        self.write(worktree, "docs/evidence/2026-09-25-su-b.md", "# Evidence: SU-B\n")
        self.commit_all(worktree, "Work on SU-B")
        self.build_main()
        before = self.origin_main()

        result = self.land("unit/suite/SU-B", "--kind", "maintenance")

        self.assert_landed(result, before)
        self.assertEqual(len(self.records_of(".builds-ran")), 1)
        evidence = self.show_main("docs/evidence/2026-09-25-su-b.md")
        self.assertIn(
            "- **Check:** `production_artifact.py check app --require-verified` exit 1", evidence
        )
        self.assertRegex(
            evidence,
            r"- \*\*Build:\*\* complete-production\.sh exit 0, "
            r"manifest git_revision [0-9a-f]{40}\n$",
        )

    def test_interrupted_seal_abort_removes_inventory(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()

        result = self.land(
            "unit/app/FX-A",
            "--kind",
            "bug",
            LAND_UNIT_GIT=str(self.fixture_git),
            LAND_FIXTURE_STOP_AFTER="seal-write",
        )

        self.assert_not_landed(result, before)
        self.assertIn("interrupted during seal", result.stderr)
        status = self.git(self.repo, "status", "--porcelain", "--untracked-files=all")
        self.assertIn(f"?? {INVENTORY}\n", status)
        state = (self.landing_dir / ".land-state.toml").read_text(encoding="utf-8")
        self.assertIn(f'inventory = "{INVENTORY}"', state)

        aborted = self.land("--abort")

        self.assertEqual(aborted.returncode, 0, msg=described(aborted))
        self.assertEqual(self.git(self.repo, "symbolic-ref", "--short", "HEAD").strip(), "main")
        self.assertEqual(self.git(self.repo, "status", "--porcelain", "--untracked-files=all"), "")
        self.assertFalse((self.repo / INVENTORY).exists())
        self.assertFalse(self.landing_dir.exists())
        self.assertEqual(self.origin_main(), before)

    def test_hook_failure_stops_without_commit(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()
        branch_tip = self.git(self.repo, "rev-parse", "refs/heads/unit/app/FX-A").strip()
        self.forget_hooks()

        result = self.land("unit/app/FX-A", "--kind", "bug", LAND_FIXTURE_FAIL_HOOK="pre-commit")

        self.assert_not_landed(result, before)
        self.assertIn(
            "stopped at commit: the repository hooks rejected the sealed commit", result.stderr
        )
        self.assertIn("fixture hook pre-commit failed", result.stderr)
        self.assertEqual(self.records_of(".hooks-ran"), ["pre-commit"])
        self.assertEqual(self.git(self.repo, "rev-parse", "HEAD").strip(), before)
        self.assertEqual(self.git(self.repo, "rev-parse", "refs/heads/main").strip(), before)
        self.assertEqual(
            self.git(self.repo, "rev-parse", "refs/heads/unit/app/FX-A").strip(), branch_tip
        )
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())

    def test_guard_failure_stops_before_commit(self) -> None:
        worktree = self.open_branch("FX-A")
        self.write_unit(worktree, "FX-A")
        before = self.origin_main()

        result = self.land(
            "unit/app/FX-A", "--kind", "bug", LAND_FIXTURE_FAIL_GUARD="check-language-contract.py"
        )

        self.assert_not_landed(result, before)
        self.assertIn(
            "stopped at guard: check-language-contract.py failed with exit 1", result.stderr
        )
        self.assertEqual(self.records_of(".guards-ran"), GUARD_ORDER[:4])
        self.assertEqual(self.git(self.repo, "rev-parse", "HEAD").strip(), before)
        self.assertEqual(self.git(self.repo, "rev-parse", "refs/heads/main").strip(), before)
        self.assertIn(INVENTORY, self.git(self.repo, "diff", "--cached", "--name-only"))
        self.assertEqual(len(self.records_of(".builds-ran")), 1)
        self.assertTrue((self.landing_dir / ".land-state.toml").is_file())


if __name__ == "__main__":
    unittest.main()
