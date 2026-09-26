#!/usr/bin/env python3
"""Land one unit branch on main from the canonical checkout.

    land-unit.py BRANCH --kind bug|milestone|release|maintenance [--summary TEXT]
    land-unit.py --continue
    land-unit.py --abort

The steps follow docs/superpowers/specs/2026-09-25-parallel-unit-landing-design.md
section 5.1, as amended in section 10: preflight, unbump, rebase, bump_version,
checkout_canonical, pre_guards, build_if_stale, seal, run_guards,
commit_and_push. The rebase happens in a
landing worktree at <parent>/<basename>.worktrees/.landing/, which also holds
the resumable state in .land-state.toml. Commits made there are temporary: the
only commit that reaches main is the sealed one, created in the canonical
checkout through the repository hooks.

Exit 0 when the unit landed, 1 when the landing stopped or failed, 2 on usage
errors. LAND_UNIT_GIT, LAND_UNIT_CARGO and LAND_UNIT_PYTHON override the
programs it runs.
"""

from __future__ import annotations

import argparse
from collections.abc import Callable
from dataclasses import dataclass
import os
from pathlib import Path
import posixpath
import subprocess
import sys
import tomllib

from landing import (
    PLACEHOLDER_DIFFSTAT,
    VERSIONED_KINDS,
    LandingError,
    LandingStop,
    LandState,
    UnitRef,
    affected_projects,
    close_ledger_row,
    discover_unit,
    inventory_path,
    kind_refusal,
    landing_section,
    lockfile_blockers,
    lockfile_upgrades,
    is_active_plan,
    merge_plan,
    merge_other_evidence,
    merge_ratchet,
    merge_settled_plan,
    numstat_rows,
    other_unit_record,
    owner_docs_root,
    owner_tables,
    render_inventory,
    scope_violations,
    sealed_diffstat,
    subject,
    unbumped_files,
    verification_only,
)
from production_artifact import session_worktree_marker


KINDS = ("bug", "milestone", "release", "maintenance")
STATE_FILE = ".land-state.toml"
MAX_RETRIES = 3
EDITOR_ENVIRONMENT = {"GIT_EDITOR": "true"}
# After the seal, the staged unit is what the guards and hooks judge, and
# --continue judges the same bytes again.
SEALED_REMEDY = (
    "the unit is sealed, so --continue only repeats this step on the same bytes, which "
    "helps only when the cause lies outside the unit, such as a missing tool; otherwise "
    "run land-unit.py --abort, fix the branch in its session worktree, and land it again"
)


@dataclass
class LandContext:
    root: Path
    landing_dir: Path
    registry: dict
    state: LandState
    unit: UnitRef | None


# Processes.


def program(variable: str, default: str) -> str:
    return os.environ.get(variable) or default


def run_process(
    argv: list[str],
    cwd: Path,
    *,
    stdin: str | None = None,
    environment: dict[str, str] | None = None,
    check: bool = True,
    capture: bool = True,
    raw: bool = False,
) -> subprocess.CompletedProcess:
    """Run one program; with `raw`, stdout stays bytes."""
    command = " ".join(argv)
    try:
        result = subprocess.run(
            argv,
            cwd=cwd,
            input=stdin,
            # Captured commands never read the terminal; a build may.
            stdin=subprocess.DEVNULL if stdin is None and capture else None,
            env={**os.environ, **environment} if environment else None,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
            text=not raw,
            check=False,
        )
    except OSError as error:
        raise LandingError(f"cannot run `{command}`: {error}") from error
    if check and result.returncode != 0:
        stderr = result.stderr.decode("utf-8", "replace") if raw else result.stderr
        detail = ((stderr or "") + ("" if raw else result.stdout or "")).strip()
        raise LandingError(
            f"`{command}` failed with exit {result.returncode}" + (f": {detail}" if detail else "")
        )
    return result


def run(
    ctx: LandContext,
    *args: str,
    cwd: Path | None = None,
    check: bool = True,
    stdin: str | None = None,
    environment: dict[str, str] | None = None,
    raw: bool = False,
) -> subprocess.CompletedProcess:
    """Run one Git command; every Git call of the landing goes through here."""
    return run_process(
        [program("LAND_UNIT_GIT", "git"), "--literal-pathspecs", *args],
        cwd or ctx.root,
        stdin=stdin,
        environment=environment,
        check=check,
        raw=raw,
    )


def succeeds(ctx: LandContext, *args: str, cwd: Path | None = None) -> bool:
    return run(ctx, *args, cwd=cwd, check=False).returncode == 0


def revision(ctx: LandContext, name: str, cwd: Path | None = None) -> str:
    result = run(ctx, "rev-parse", "--verify", "--quiet", f"{name}^{{commit}}", cwd=cwd)
    return result.stdout.strip()


def blob(ctx: LandContext, spec: str, cwd: Path | None = None) -> bytes | None:
    """The bytes of `REV:PATH` or `:N:PATH`, or None when Git has no such blob."""
    result = run(ctx, "cat-file", "blob", spec, cwd=cwd, check=False, raw=True)
    return result.stdout if result.returncode == 0 else None


def text_of(raw: bytes, label: str) -> str:
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise LandingError(f"{label} is not UTF-8: {error}") from error


def paths_of(output: str) -> set[str]:
    return {path for path in output.split("\0") if path}


def say(message: str) -> None:
    print(f"land-unit: {message}", file=sys.stderr)


# Files.


def write_file(path: Path, data: bytes | str) -> None:
    try:
        path.write_bytes(data.encode("utf-8") if isinstance(data, str) else data)
    except OSError as error:
        raise LandingError(f"cannot write {path}: {error}") from error


def read_file(path: Path) -> bytes:
    try:
        return path.read_bytes()
    except OSError as error:
        raise LandingError(f"cannot read {path}: {error}") from error


def remove_file(path: Path) -> None:
    """Remove `path`; a missing file is already removed."""
    try:
        path.unlink(missing_ok=True)
    except OSError as error:
        raise LandingError(f"cannot remove {path}: {error}") from error


def make_directory(path: Path) -> None:
    try:
        path.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise LandingError(f"cannot create {path}: {error}") from error


# State and registry.


def unit_of(ctx: LandContext) -> UnitRef:
    if ctx.unit is None:
        raise LandingError("the unit has not been discovered yet")
    return ctx.unit


def save_state(ctx: LandContext) -> None:
    path = ctx.landing_dir / STATE_FILE
    temporary = path.with_name(f".{STATE_FILE}.tmp")
    try:
        temporary.write_text(ctx.state.dump(), encoding="utf-8")
        temporary.replace(path)
    except OSError as error:
        raise LandingError(f"cannot record the landing state in {path}: {error}") from error


def registry_at(ctx: LandContext, rev: str) -> dict:
    raw = blob(ctx, f"{rev}:docs/projects.toml")
    if raw is None:
        raise LandingError(f"{rev} has no docs/projects.toml")
    try:
        return tomllib.loads(text_of(raw, f"{rev}:docs/projects.toml"))
    except tomllib.TOMLDecodeError as error:
        raise LandingError(f"{rev}:docs/projects.toml is invalid: {error}") from error


def owner_table(ctx: LandContext) -> dict:
    project_id = unit_of(ctx).project_id
    for owner in owner_tables(ctx.registry):
        if owner.get("id") == project_id:
            return dict(owner)
    raise LandingError(f"the registry has no owner {project_id}")


def unit_inventory(ctx: LandContext) -> str:
    return inventory_path(owner_table(ctx), unit_of(ctx))


def unit_subject(ctx: LandContext) -> str:
    return subject(unit_of(ctx).prefix, ctx.state.kind, ctx.state.summary)


def refuse_landed(ctx: LandContext, rev: str, step: str, remedy: str | None = None) -> None:
    """Stop when `rev`, which is origin/main, already tracks the unit's inventory."""
    inventory = unit_inventory(ctx)
    if blob(ctx, f"{rev}:{inventory}") is not None:
        raise LandingStop(
            step,
            f"{unit_of(ctx).unit} already landed: origin/main tracks {inventory}",
            inventory,
            remedy=remedy,
        )


def changed_on_branch(ctx: LandContext) -> set[str]:
    return paths_of(
        run(
            ctx, "diff", "--name-only", "--no-renames", "-z", f"origin/main...{ctx.state.branch}"
        ).stdout
    )


def stacked_on(ctx: LandContext, unit: str) -> bool:
    """Whether a branch `unit/<project>/<unit>` exists and is an ancestor of the landing's branch."""
    own = f"refs/heads/{ctx.state.branch}"
    refs = run(ctx, "for-each-ref", "--format=%(refname)", f"refs/heads/unit/*/{unit}").stdout
    return any(
        ref != own and succeeds(ctx, "merge-base", "--is-ancestor", ref, own)
        for ref in refs.split()
    )


def resolve_unit(ctx: LandContext, changed: set[str], main: str) -> UnitRef:
    """The branch's unit; plans and rows that `main` (origin/main or the base) settles are set aside."""
    branch = ctx.state.branch
    fork = run(ctx, "merge-base", main, branch).stdout.strip()

    def reader(rev: str) -> Callable[[str], str | None]:
        def read_plan(path: str) -> str | None:
            raw = blob(ctx, f"{rev}:{path}")
            return None if raw is None else text_of(raw, f"{rev}:{path}")

        return read_plan

    # worktree.sh names a unit's branch unit/<project>/<unit>; such a branch
    # must carry that unit, which tells it from the dependencies it carries.
    parts = branch.split("/")
    branch_unit = parts[2] if len(parts) == 3 and parts[0] == "unit" else None
    return discover_unit(
        ctx.registry,
        changed,
        reader(branch),
        reader(main),
        branch_unit,
        read_base_plan=reader(fork),
        stacked_on=lambda unit: stacked_on(ctx, unit),
    )


def carried_paths(ctx: LandContext, changed: set[str]) -> set[str]:
    """The changed paths that only carry the work of units origin/main already holds.

    A stacked branch carries its dependencies' commits. A path whose bytes on
    the branch equal origin/main's, a plan `discover_unit` set aside, and
    another unit's record the rebase resolves to origin/main's copy land
    nothing of this unit, so the scope of its prefix does not judge them; the
    guards judge what the rebased tip really changes.
    """
    unit = unit_of(ctx)
    branch = ctx.state.branch
    differs = paths_of(
        run(ctx, "diff", "--name-only", "--no-renames", "-z", "origin/main", branch).stdout
    )
    fork = run(ctx, "merge-base", "origin/main", branch).stdout.strip()
    own = {unit.evidence_path or "", unit_inventory(ctx)}
    carried = set()
    for path in changed:
        if path not in differs or path in unit.settled_plans:
            carried.add(path)
            continue
        added = blob(ctx, f"{fork}:{path}") is None
        if (
            other_unit_record(ctx.registry, path, own, added) is not None
            and blob(ctx, f"origin/main:{path}") is not None
        ):
            carried.add(path)
    return carried


def rebase_in_progress(ctx: LandContext) -> bool:
    for name in ("rebase-merge", "rebase-apply"):
        raw = run(ctx, "rev-parse", "--git-path", name, cwd=ctx.landing_dir).stdout.strip()
        path = Path(raw)
        if (path if path.is_absolute() else ctx.landing_dir / path).exists():
            return True
    return False


# Steps.


def preflight(ctx: LandContext) -> None:
    state = ctx.state
    if ctx.landing_dir.exists():
        raise LandingStop(
            "preflight",
            f"a landing is already in progress in {ctx.landing_dir}; "
            "run land-unit.py --continue or --abort",
        )
    status = run(ctx, "status", "--porcelain").stdout
    head = run(ctx, "symbolic-ref", "--quiet", "--short", "HEAD", check=False).stdout.strip()
    if status.strip() or head != "main":
        where = f"on {head}" if head else "detached"
        raise LandingStop(
            "preflight",
            f"the canonical checkout must be a clean main; it is {where}"
            + (f" with changes:\n{status.rstrip()}" if status.strip() else ""),
        )
    run(ctx, "fetch", "--quiet", "origin", "main")
    if not succeeds(ctx, "merge-base", "--is-ancestor", "main", "origin/main"):
        raise LandingStop("preflight", "local main has commits that are not on origin/main")
    if not succeeds(ctx, "rev-parse", "--verify", "--quiet", f"refs/heads/{state.branch}"):
        raise LandingStop("preflight", f"no such branch: {state.branch}")
    ctx.registry = registry_at(ctx, "origin/main")
    changed = changed_on_branch(ctx)
    if not changed:
        raise LandingStop("preflight", f"{state.branch} has no changes against origin/main")
    unit = resolve_unit(ctx, changed, "origin/main")
    ctx.unit = unit
    refuse_landed(ctx, "origin/main", "preflight")
    violations = scope_violations(
        unit.prefix, ctx.registry, changed - carried_paths(ctx, changed)
    )
    if violations:
        raise LandingStop(
            "preflight",
            f"`{unit.prefix}:` does not own: {', '.join(violations)}",
            violations[0],
        )
    tracked_inventories = sorted(
        path
        for path in changed
        if path.endswith(".numstat.tsv") and blob(ctx, f"origin/main:{path}") is not None
    )
    if tracked_inventories:
        raise LandingStop(
            "preflight",
            "the branch edits tracked inventories, which are immutable: "
            + ", ".join(tracked_inventories),
            tracked_inventories[0],
        )
    evidence_root = posixpath.join(owner_docs_root(owner_table(ctx)), "evidence") + "/"
    if unit.evidence_path is None:
        raise LandingStop(
            "preflight",
            f"row {unit.unit} of {unit.plan_path} must link its evidence record "
            "in `Automated evidence`",
            unit.plan_path,
        )
    if not unit.evidence_path.startswith(evidence_root):
        raise LandingStop(
            "preflight",
            f"the evidence record {unit.evidence_path} is not under {evidence_root}",
            unit.evidence_path,
        )
    if blob(ctx, f"{state.branch}:{unit.evidence_path}") is None:
        raise LandingStop(
            "preflight",
            f"the evidence record {unit.evidence_path} does not exist on {state.branch}",
            unit.evidence_path,
        )
    refusal = kind_refusal(ctx.registry, unit.project_id, state.kind)
    if refusal is not None:
        raise LandingStop("preflight", refusal)
    state.summary = state.summary or unit.summary
    if not state.summary:
        raise LandingStop("preflight", f"row {unit.unit} has no Intended change")
    state.project_id = unit.project_id
    state.unit = unit.unit
    run(ctx, "worktree", "add", "--quiet", "--detach", str(ctx.landing_dir), state.branch)
    save_state(ctx)


def unbump(ctx: LandContext) -> None:
    """Start the landing tree from the branch as one commit without version changes."""
    state = ctx.state
    # A retry or a --continue lands on origin/main again; a local main that
    # holds other commits would reject the fast-forward only after a rebuild.
    if not succeeds(ctx, "merge-base", "--is-ancestor", "refs/heads/main", "origin/main"):
        raise LandingStop(
            "unbump",
            "local main has commits that are not on origin/main; keep them on another "
            "branch (`git branch <name> main`) and run `git reset --hard origin/main` "
            "on main before the landing rebuilds anything",
        )
    state.base = ""
    state.tip = ""
    state.sealed = ""
    state.previous = ""
    landing = ctx.landing_dir
    run(ctx, "checkout", "--quiet", "--force", "--detach", state.branch, cwd=landing)
    fork = run(ctx, "merge-base", "origin/main", state.branch).stdout.strip()
    rewrites = unbumped_files(
        ctx.registry,
        lambda path: blob(ctx, f"{fork}:{path}"),
        lambda path: blob(ctx, f"{state.branch}:{path}"),
    )
    for path, raw in rewrites.items():
        write_file(landing / path, raw)
    if rewrites:
        run(ctx, "add", "--", *sorted(rewrites), cwd=landing)
        say(
            "warning: the session bumped "
            + ", ".join(sorted(rewrites))
            + "; the landing drops that change and computes the version itself"
        )
    tree = run(ctx, "write-tree", cwd=landing).stdout.strip()
    squashed = run(
        ctx,
        "commit-tree",
        tree,
        "-p",
        fork,
        "-m",
        f"squash! {state.unit}: session work without version changes",
        cwd=landing,
    ).stdout.strip()
    run(ctx, "reset", "--quiet", "--soft", squashed, cwd=landing)


def merged_lockfile(ctx: LandContext, path: str, main_raw: bytes) -> bytes:
    target = ctx.landing_dir / path
    write_file(target, main_raw)
    remedy = (
        f"merge {path} by hand (Cargo with the network can), `git add` {path} in "
        f"{ctx.landing_dir}, then run land-unit.py --continue, or --abort to give up"
    )
    cargo = program("LAND_UNIT_CARGO", "cargo")
    result = run_process(
        [cargo, "metadata", "--offline", "--format-version", "1"], target.parent, check=False
    )
    if result.returncode != 0:
        raise LandingStop(
            "rebase",
            f"{path}: `cargo metadata --offline` failed, so merging the lockfile "
            f"needs the network: {result.stderr.strip()}",
            path,
            remedy=remedy,
        )
    merged = read_file(target)
    upgrades = lockfile_upgrades(text_of(main_raw, path), text_of(merged, path))
    if upgrades:
        raise LandingStop(
            "rebase",
            f"{path}: the merged lockfile moves packages main already locks: "
            + ", ".join(upgrades),
            path,
            remedy=remedy,
        )
    return merged


def hot_merge(ctx: LandContext, path: str) -> bytes | None:
    """The semantic merge of one conflicted hot file, or None for other files."""
    unit = unit_of(ctx)
    landing = ctx.landing_dir

    def stage(number: int) -> bytes | None:
        return blob(ctx, f":{number}:{path}", cwd=landing)

    base, main, branch = stage(1), stage(2), stage(3)
    own = {unit.evidence_path or "", unit_inventory(ctx)}
    record = other_unit_record(ctx.registry, path, own, base is None)
    if record == "evidence record" and main is not None and branch is not None:
        # A branch stacked on a landed unit carries that unit's record as its
        # session left it; main's copy adds only the landing section.
        merged = merge_other_evidence(text_of(main, path), text_of(branch, path), path)
        say(
            f"{path} is another unit's evidence record; the landing keeps origin/main's "
            "copy, which adds only its landing section"
        )
        return merged.encode()
    if record is not None and main is not None and branch is not None:
        say(f"warning: {path} is another unit's {record}; the landing keeps origin/main's copy")
        return main
    ratchets = ctx.registry.get("commit_policy", {}).get("shared_ratchet_files", [])
    another_plan = path != unit.plan_path and is_active_plan(ctx.registry, path)
    is_hot = (
        path == unit.plan_path
        or another_plan
        or path in ratchets
        or posixpath.basename(path) == "Cargo.lock"
    )
    if not is_hot:
        return None
    if main is None or branch is None:
        side = "main" if main is None else "the branch"
        raise LandingStop("rebase", f"{path} was deleted on {side}", path)
    if another_plan:
        # A plan of a unit this branch is stacked on, which landed meanwhile.
        merged_plan = merge_settled_plan(
            None if base is None else text_of(base, path),
            text_of(main, path),
            text_of(branch, path),
            path,
        )
        say(f"{path} holds only rows origin/main settles; the landing keeps origin/main's copy")
        return merged_plan.encode()
    if path == unit.plan_path:
        return merge_plan(
            text_of(base or b"", path), text_of(main, path), text_of(branch, path), unit.unit, path
        ).encode()
    if path in ratchets:
        return merge_ratchet(
            text_of(base or b"", path), text_of(main, path), text_of(branch, path)
        ).encode()
    return merged_lockfile(ctx, path, main)


def resolve_conflicts(ctx: LandContext) -> None:
    landing = ctx.landing_dir
    conflicted = sorted(
        paths_of(run(ctx, "diff", "--name-only", "--diff-filter=U", "-z", cwd=landing).stdout)
    )
    # Cargo rewrites a lockfile from its workspace's manifests, so every other
    # path is resolved first and each lockfile last.
    lockfiles = [path for path in conflicted if posixpath.basename(path) == "Cargo.lock"]
    unresolved = []
    for path in [path for path in conflicted if path not in lockfiles] + lockfiles:
        blockers = lockfile_blockers(path, unresolved) if path in lockfiles else []
        if blockers:
            raise LandingStop(
                "rebase",
                f"conflict in {', '.join(unresolved)}; {path} is merged once "
                f"{', '.join(blockers)} {'is' if len(blockers) == 1 else 'are'} resolved: "
                f"resolve each in {landing} and `git add` it, then run land-unit.py "
                f"--continue, which merges {path} and continues the rebase",
                blockers[0],
            )
        merged = hot_merge(ctx, path)
        if merged is None:
            unresolved.append(path)
            continue
        write_file(landing / path, merged)
        run(ctx, "add", "--", path, cwd=landing)
    if unresolved:
        raise LandingStop(
            "rebase",
            f"conflict in {', '.join(unresolved)}: resolve it in {landing}, `git add` it, "
            "run `git rebase --continue` there, then land-unit.py --continue",
            unresolved[0],
        )


def rebase(ctx: LandContext) -> None:
    state = ctx.state
    landing = ctx.landing_dir
    plan = unit_of(ctx).plan_path
    if not rebase_in_progress(ctx):
        finished = bool(state.base) and succeeds(
            ctx, "merge-base", "--is-ancestor", state.base, "HEAD", cwd=landing
        )
        if finished:
            return
        state.base = revision(ctx, "origin/main")
        save_state(ctx)
        # A retry rebases on a newer origin/main, which may carry this unit.
        refuse_landed(
            ctx, state.base, "rebase", "the unit is on main already: run land-unit.py --abort"
        )
        fork = run(ctx, "merge-base", state.base, state.branch).stdout.strip()
        if blob(ctx, f"{fork}:{plan}") is not None and blob(ctx, f"{state.base}:{plan}") is None:
            raise LandingStop(
                "rebase",
                f"{plan} is no longer an active plan on main; archiving a plan "
                "while a unit is open is the author's decision",
                plan,
            )
        started = run(
            ctx,
            "rebase",
            "--quiet",
            "--no-autosquash",
            state.base,
            cwd=landing,
            check=False,
            environment=EDITOR_ENVIRONMENT,
        )
        if started.returncode != 0 and not rebase_in_progress(ctx):
            raise LandingError(f"git rebase failed: {started.stderr.strip()}")
    while rebase_in_progress(ctx):
        resolve_conflicts(ctx)
        continued = run(
            ctx,
            "rebase",
            "--continue",
            cwd=landing,
            check=False,
            environment=EDITOR_ENVIRONMENT,
        )
        if continued.returncode != 0:
            pending = run(ctx, "diff", "--name-only", "--diff-filter=U", "-z", cwd=landing)
            if not rebase_in_progress(ctx) or not paths_of(pending.stdout):
                raise LandingError(f"git rebase --continue failed: {continued.stderr.strip()}")


def bump_version(ctx: LandContext) -> None:
    state = ctx.state
    if state.kind not in VERSIONED_KINDS:
        return
    unit = unit_of(ctx)
    landing = ctx.landing_dir
    history = ctx.registry.get("version_policy", {}).get("history_file", "")
    # Unbump left the tree without a version change, so a history difference
    # here is the bump commit of an interrupted run.
    if history and not succeeds(
        ctx, "diff", "--quiet", state.base, "HEAD", "--", history, cwd=landing
    ):
        return
    run(ctx, "reset", "--quiet", "--hard", cwd=landing)
    python = program("LAND_UNIT_PYTHON", sys.executable)
    run_process(
        [
            python,
            str(landing / "scripts/version_tool.py"),
            "--root",
            str(landing),
            "bump",
            unit.project_id,
            state.kind,
            "--unit",
            unit.unit,
            "--summary",
            state.summary,
        ],
        landing,
    )
    run(ctx, "add", "--update", cwd=landing)
    tree = run(ctx, "write-tree", cwd=landing).stdout.strip()
    bumped = run(ctx, "commit-tree", tree, "-p", "HEAD", "-m", "fixup! bump", cwd=landing)
    run(ctx, "reset", "--quiet", "--soft", bumped.stdout.strip(), cwd=landing)


def checkout_canonical(ctx: LandContext) -> None:
    ctx.state.tip = revision(ctx, "HEAD", cwd=ctx.landing_dir)
    save_state(ctx)
    run(ctx, "checkout", "--quiet", "--detach", ctx.state.tip)


def landed_paths(ctx: LandContext) -> set[str]:
    """The paths the rebased tip changes against the base."""
    state = ctx.state
    return paths_of(
        run(ctx, "diff", "--name-only", "--no-renames", "-z", state.base, state.tip).stdout
    )


def run_guard_chain(ctx: LandContext, guards: tuple, remedy: str | None = None) -> None:
    """Run each guard of `(name, interpreter, arguments, stdin)`; the first failure stops."""
    root = ctx.root
    for name, interpreter, arguments, stdin in guards:
        argv = [interpreter, str(root / "scripts" / name), *arguments]
        result = run_process(argv, root, stdin=stdin, check=False)
        if result.returncode != 0:
            output = (result.stdout + result.stderr).strip()
            raise LandingStop(
                "guard",
                f"{name} failed with exit {result.returncode}:\n{output}",
                name,
                remedy=remedy,
            )


def pre_guards(ctx: LandContext) -> None:
    """Before any build, the guards whose verdict does not depend on the seal."""
    python = program("LAND_UNIT_PYTHON", sys.executable)
    changed = "\0".join(sorted(landed_paths(ctx)))
    run_guard_chain(
        ctx,
        (
            ("commit_scope.py", python, ["--check", unit_subject(ctx)], changed),
            ("version_tool.py", python, ["check"], None),
            ("check-language-contract.py", python, [], None),
            ("check-architecture-contract.sh", "bash", [], None),
        ),
    )


def check_and_build(ctx: LandContext, project: dict) -> tuple[str, str | None]:
    """Ask the artifact tool whether `project` is current; build only when it must.

    When the check fails only because the artifact is not verified for the
    current tests and rules, the verification runs alone: the verify_script,
    then for a deployable project the deploy_script and status_script, the rest
    of what its complete_script runs. Otherwise a deployable project runs its
    complete_script, and one that is not runs its build_script, then its
    verify_script.
    """
    project_id = str(project.get("id"))
    python = program("LAND_UNIT_PYTHON", sys.executable)
    tool = str(ctx.root / "scripts/production_artifact.py")
    checked = run_process(
        [python, tool, "check", project_id, "--require-verified"], ctx.root, check=False
    )
    lines = [line for line in (checked.stdout + checked.stderr).splitlines() if line.strip()]
    check = (
        f"`production_artifact.py check {project_id} --require-verified` exit "
        f"{checked.returncode}: {lines[0] if lines else 'no output'}"
    )
    if checked.returncode == 0:
        return check, None
    deployable = bool(project.get("deployable"))
    if verification_only(checked.stderr):
        label = "verify"
        keys: tuple[str, ...] = (
            ("verify_script", "deploy_script", "status_script") if deployable else ("verify_script",)
        )
    else:
        label = "build"
        keys = ("complete_script",) if deployable else ("build_script", "verify_script")
    entries = []
    for key in keys:
        entry = project.get(key)
        if not isinstance(entry, str):
            raise LandingError(f"{project_id} registers no {key}")
        entries.append(entry)
    for entry in entries:
        built = run_process([str(ctx.root / entry)], ctx.root, check=False, capture=False)
        if built.returncode != 0:
            raise LandingStop(
                "build_if_stale", f"{entry} failed with exit {built.returncode}", entry
            )
    manifest_path = ctx.root / str(project.get("artifact_manifest", ""))
    raw = read_file(manifest_path)
    try:
        manifest = tomllib.loads(text_of(raw, str(manifest_path)))
    except tomllib.TOMLDecodeError as error:
        raise LandingError(f"cannot read the manifest {manifest_path}: {error}") from error
    ran = ", ".join(f"{posixpath.basename(entry)} exit 0" for entry in entries)
    # The manifest's git_revision names the temporary detached tip; the
    # fingerprints name the inputs and tests the artifact was made from.
    return check, (
        f"{project_id} {label}: {ran}, manifest source_fingerprint "
        f"{manifest.get('source_fingerprint', 'unknown')}, verification_fingerprint "
        f"{manifest.get('verification_fingerprint', 'unknown')}"
    )


def build_if_stale(ctx: LandContext) -> None:
    """Run the one production run the artifact contract needs for every affected project.

    The owner comes first, then every registered project whose production or
    verification inputs the unit changed, such as the deployable consumers of
    a library. The rebased tip's own registry names the inputs, since the unit
    may rename one and register the new name.
    """
    state = ctx.state
    registry = registry_at(ctx, state.tip)
    projects = affected_projects(ctx.root, registry, landed_paths(ctx), unit_of(ctx).project_id)
    if not projects:
        # Only a `suite` unit has no registered project as its owner.
        state.check = (
            "not applicable: the suite unit changes no registered production or verification input"
        )
        state.build = "none; no registered production input changed"
        return
    checks: list[str] = []
    builds: list[str] = []
    for project in projects:
        check, build = check_and_build(ctx, project)
        checks.append(check)
        if build is not None:
            builds.append(build)
    state.check = "; ".join(checks)
    state.build = "; ".join(builds)


def seal(ctx: LandContext) -> None:
    state = ctx.state
    unit = unit_of(ctx)
    root = ctx.root
    # Start from the landing tip each time so that a resumed seal writes once.
    run(ctx, "checkout", "--quiet", "--force", "--detach", state.tip)
    run(ctx, "reset", "--quiet", "--soft", state.base)
    inventory = unit_inventory(ctx)
    evidence = unit.evidence_path
    if evidence is None:
        raise LandingError(f"row {unit.unit} has no evidence record")
    paths = (landed_paths(ctx) | {unit.plan_path, evidence}) - {inventory}
    evidence_text = text_of(read_file(root / evidence), evidence)
    write_file(
        root / evidence,
        evidence_text.rstrip("\n")
        + "\n\n"
        + landing_section(state.base, state.check, state.build or None),
    )
    plan_text = text_of(read_file(root / unit.plan_path), unit.plan_path)
    plan_directory = posixpath.dirname(unit.plan_path)
    links = (
        posixpath.relpath(inventory, plan_directory),
        posixpath.relpath(evidence, plan_directory),
    )

    def write_plan(stat: str) -> None:
        write_file(
            root / unit.plan_path, close_ledger_row(plan_text, unit.unit, links[0], stat, links[1])
        )

    # Only the plan's hash moves between the placeholder and the final diffstat.
    write_plan(PLACEHOLDER_DIFFSTAT)
    rows = numstat_rows(root, state.base, paths)
    write_plan(sealed_diffstat(unit.unit, state.base, rows, inventory))
    rows = numstat_rows(root, state.base, paths)
    # Recorded before the file exists, so that --abort can remove it.
    state.inventory = inventory
    save_state(ctx)
    target = root / inventory
    make_directory(target.parent)
    write_file(target, render_inventory(unit.unit, state.base, rows, inventory))
    # The index already holds the tip, so a path the unit deletes is staged,
    # and `git add` refuses a pathspec that matches no file.
    present = sorted(path for path in paths | {inventory} if os.path.lexists(root / path))
    run(ctx, "add", "--all", "--", *present)


def run_guards(ctx: LandContext) -> None:
    python = program("LAND_UNIT_PYTHON", sys.executable)
    staged = paths_of(run(ctx, "diff", "--cached", "--name-only", "--no-renames", "-z").stdout)
    # The hooks' own commands, in the order of spec section 5.1 step 8.
    run_guard_chain(
        ctx,
        (
            ("check-staged-units.py", python, [unit_inventory(ctx)], None),
            ("commit_scope.py", python, ["--check", unit_subject(ctx)], "\0".join(sorted(staged))),
            ("version_tool.py", python, ["check"], None),
            ("check-architecture-contract.sh", "bash", [], None),
            ("check-language-contract.py", python, [], None),
            ("check-documentation-contract.sh", "sh", [], None),
        ),
        remedy=SEALED_REMEDY,
    )


def is_sealed(ctx: LandContext, rev: str) -> bool:
    """Whether `rev` is a sealed commit of this landing: on the base, with its subject."""
    if not rev:
        return False
    shown = run(ctx, "log", "-1", "--format=%P%x00%s", rev, "--", check=False)
    parents, _, line = shown.stdout.rstrip("\n").partition("\0")
    return shown.returncode == 0 and parents == ctx.state.base and line == unit_subject(ctx)


def seal_commit(ctx: LandContext) -> str:
    """The sealed commit, created through the hooks unless a resumed run made it."""
    for candidate in (ctx.state.sealed, "HEAD"):
        if is_sealed(ctx, candidate):
            return revision(ctx, candidate)
    committed = run(ctx, "commit", "--quiet", "-m", unit_subject(ctx), check=False)
    if committed.returncode != 0:
        output = (committed.stdout + committed.stderr).strip()
        raise LandingStop(
            "commit",
            f"the repository hooks rejected the sealed commit:\n{output}",
            remedy=SEALED_REMEDY,
        )
    return revision(ctx, "HEAD")


def restore_main(ctx: LandContext) -> None:
    """Put main back where it was before the fast-forward, if it still is the sealed commit."""
    state = ctx.state
    if not state.sealed or not state.previous:
        return
    if revision(ctx, "refs/heads/main") == state.sealed:
        # Detach first, so that moving main does not leave the tree behind it.
        run(ctx, "checkout", "--quiet", "--force", "--detach", state.sealed)
        run(ctx, "update-ref", "refs/heads/main", state.previous, state.sealed)
    run(ctx, "checkout", "--quiet", "--force", "main")


def finish(ctx: LandContext) -> None:
    print(f"land-unit: landed {ctx.state.sealed} {unit_subject(ctx)}")
    removed = run(ctx, "worktree", "remove", "--force", str(ctx.landing_dir), check=False)
    if removed.returncode != 0:
        say(f"warning: remove the landing worktree by hand: {removed.stderr.strip()}")


def commit_and_push(ctx: LandContext) -> str | None:
    state = ctx.state
    sealed = seal_commit(ctx)
    current = revision(ctx, "refs/heads/main")
    if current != sealed:
        if not succeeds(ctx, "merge-base", "--is-ancestor", current, sealed):
            raise LandingStop("push", "local main is not an ancestor of the sealed commit")
        state.previous = current
        state.sealed = sealed
        save_state(ctx)
    # From here main may be the unpushed seal, so any failure or Ctrl-C puts
    # it back before it propagates.
    try:
        if current != sealed:
            # A compare-and-swap fast-forward from the sealed tree already on
            # disk, so attaching main to it rewrites no file.
            run(ctx, "checkout", "--quiet", "--detach", sealed)
            run(ctx, "update-ref", "refs/heads/main", sealed, current)
        run(ctx, "checkout", "--quiet", "main")
        pushed = run(ctx, "push", "--quiet", "--set-upstream", "origin", "main", check=False)
    except (LandingError, KeyboardInterrupt):
        restore_main(ctx)
        raise
    if pushed.returncode == 0:
        finish(ctx)
        return None
    restore_main(ctx)
    fetched = run(ctx, "fetch", "--quiet", "origin", "main", check=False)
    if fetched.returncode != 0:
        raise LandingStop(
            "push",
            f"the push failed and origin cannot be fetched; main is restored, and "
            f"--continue pushes the same sealed commit: {pushed.stderr.strip()} "
            f"{fetched.stderr.strip()}",
        )
    origin = revision(ctx, "origin/main")
    if origin == sealed:
        # The push reached origin although Git reported a failure.
        run(ctx, "merge", "--quiet", "--ff-only", "origin/main")
        finish(ctx)
        return None
    # A fast-forward only: a commit made on local main from elsewhere stays.
    forwarded = run(ctx, "merge", "--quiet", "--ff-only", "origin/main", check=False)
    if forwarded.returncode != 0:
        state.step = "unbump"
        state.attempts = 0
        save_state(ctx)
        # restore_main leaves main alone once it is not the seal itself, so a
        # commit made on main during the push sits on the unpushed seal.
        on_seal = succeeds(ctx, "merge-base", "--is-ancestor", sealed, "refs/heads/main")
        raise LandingStop(
            "push",
            "the push was rejected and local main cannot fast-forward to origin/main, "
            "because it has commits origin/main lacks"
            + (f" on top of this landing's unpushed sealed commit {sealed}" if on_seal else "")
            + ": keep them on another branch (`git branch <name> main`), run "
            "`git reset --hard origin/main` on main"
            + (", which also drops the seal" if on_seal else "")
            + f", then --continue lands again from unbump: {forwarded.stderr.strip()}",
        )
    moved = origin != state.base
    state.attempts += 1
    if not moved or state.attempts > MAX_RETRIES:
        state.step = "unbump"
        state.attempts = 0
        save_state(ctx)
        reason = (
            f"main moved during {MAX_RETRIES + 1} pushes"
            if moved
            else "origin/main did not move, so it is not a race"
        )
        raise LandingStop(
            "push", f"the push was rejected and {reason}: {pushed.stderr.strip()}"
        )
    say(f"main moved; landing again on origin/main (retry {state.attempts} of {MAX_RETRIES})")
    return "unbump"


STEP_FUNCTIONS = (
    preflight,
    unbump,
    rebase,
    bump_version,
    checkout_canonical,
    pre_guards,
    build_if_stale,
    seal,
    run_guards,
    commit_and_push,
)
STEPS = tuple(function.__name__ for function in STEP_FUNCTIONS)


def drive(ctx: LandContext) -> None:
    index = STEPS.index(ctx.state.step)
    while index < len(STEPS):
        name = STEPS[index]
        ctx.state.step = name
        if name != "preflight":
            save_state(ctx)
        print(f"land-unit: {name}")
        restart = STEP_FUNCTIONS[index](ctx)
        index = STEPS.index(restart) if restart else index + 1


def resume(root: Path, landing_dir: Path) -> LandContext:
    path = landing_dir / STATE_FILE
    try:
        state = LandState.load(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise LandingError(f"no landing to continue: cannot read {path}: {error}") from error
    if state.step not in STEPS:
        raise LandingError(f"{path} names an unknown step: {state.step}")
    ctx = LandContext(root, landing_dir, {}, state, None)
    ctx.registry = registry_at(ctx, state.base or "origin/main")
    ctx.unit = resolve_unit(ctx, changed_on_branch(ctx), state.base or "origin/main")
    return ctx


def abort(ctx: LandContext) -> None:
    if not ctx.landing_dir.exists():
        raise LandingError(f"no landing in progress in {ctx.landing_dir}")
    try:
        text = (ctx.landing_dir / STATE_FILE).read_text(encoding="utf-8")
        ctx.state = LandState.load(text)
    except (OSError, LandingError) as error:
        say(f"warning: no readable landing state ({error}); removing the landing anyway")
    inventory = ctx.state.inventory
    # restore_main moves main back only while it still is the unpushed seal.
    restore_main(ctx)
    head = run(ctx, "symbolic-ref", "--quiet", "--short", "HEAD", check=False).stdout.strip()
    if head != "main":
        run(ctx, "checkout", "--quiet", "--force", "main")
    # The seal may have written the inventory before staging it; main never
    # tracks a unit's new inventory, so an untracked copy is the landing's.
    if inventory and os.path.lexists(ctx.root / inventory):
        if not succeeds(ctx, "ls-files", "--error-unmatch", "--", inventory):
            remove_file(ctx.root / inventory)
    run(ctx, "worktree", "remove", "--force", str(ctx.landing_dir))
    remove_file(ctx.landing_dir / STATE_FILE)
    print("land-unit: aborted; the canonical checkout is on main")


def parse_arguments(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("branch", nargs="?")
    parser.add_argument("--kind", choices=KINDS)
    parser.add_argument("--summary")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--continue", dest="resume", action="store_true")
    modes.add_argument("--abort", action="store_true")
    args = parser.parse_args(argv)
    if args.resume or args.abort:
        if args.branch or args.kind or args.summary:
            parser.error("--continue and --abort take no branch, --kind or --summary")
    elif not args.branch or not args.kind:
        parser.error("a landing needs BRANCH and --kind")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_arguments(argv)
    root = Path(__file__).resolve().parent.parent
    landing_dir = root.parent / f"{root.name}.worktrees" / ".landing"
    placeholder = LandState(
        "preflight", args.branch or "", args.kind or "", args.summary or "", "", "", 0
    )
    ctx = LandContext(root, landing_dir, {}, placeholder, None)
    try:
        if session_worktree_marker(root) is not None:
            raise LandingError("this is a session worktree; land from the canonical checkout")
        if args.abort:
            abort(ctx)
            return 0
        if args.resume:
            ctx = resume(root, landing_dir)
        drive(ctx)
    except LandingStop as stop:
        say(f"stopped at {stop.step}: {stop}")
        if stop.remedy is not None:
            say(stop.remedy)
        elif landing_dir.exists():
            say("resolve it, then run land-unit.py --continue, or --abort to give up")
        return 1
    except LandingError as error:
        say(str(error))
        return 1
    except KeyboardInterrupt:
        say(f"interrupted during {ctx.state.step}; run land-unit.py --continue or --abort")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
