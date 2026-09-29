#!/usr/bin/env python3
"""Run the repository's commit hooks with the rules committed in HEAD.

    git_hooks.py --root ROOT --rules DIR pre-commit
    git_hooks.py --root ROOT --rules DIR pre-merge-commit
    git_hooks.py --root ROOT --rules DIR commit-msg MESSAGE_FILE

`.githooks/<hook>` extracts `HEAD:scripts` into a temporary directory and passes
it as `--rules`; the guards run from there, so neither a staged nor an unstaged
edit of a guard decides the verdict on the commit that carries it (TOOL-6).
A guard HEAD does not have yet runs from the worktree, which happens once: in
the commit that adds it.

The guards judge the index, not the worktree (TOOL-5). The language and
documentation contracts walk files, so they run over a copy of the index
checked out into a temporary directory, with Git pointed at that copy; the
staged-unit guard and the commit-scope guard read the index themselves.

`pre-merge-commit` runs the checks of `pre-commit` that a merge can pass: a
merge never closes a delivery unit, and `commit-msg` refuses one that tries.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


HOOKS = ("pre-commit", "pre-merge-commit", "commit-msg")


class HookError(RuntimeError):
    """The hook cannot run its guards; the message says why."""


def say(message: str) -> None:
    print(f"hook: {message}", file=sys.stderr)


def guard(root: Path, rules: Path, name: str) -> Path:
    """The committed copy of `scripts/<name>`, or the worktree's when HEAD lacks it."""
    committed = rules / name
    if committed.is_file():
        return committed
    worktree = root / "scripts" / name
    if not worktree.is_file():
        raise HookError(f"scripts/{name} exists neither in HEAD nor in the worktree")
    say(f"HEAD has no scripts/{name}; the worktree copy runs until it is committed")
    return worktree


def git(root: Path, *args: str, env: dict[str, str] | None = None) -> str:
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=root,
            env=env,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise HookError(f"cannot run git {' '.join(args)}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise HookError(f"git {' '.join(args)} failed: {detail}")
    return result.stdout.decode("utf-8", "surrogateescape").strip()


def index_file(root: Path) -> Path:
    """The index Git is committing: `GIT_INDEX_FILE` for a hook, else the default."""
    configured = os.environ.get("GIT_INDEX_FILE")
    if configured:
        path = Path(configured)
        return path if path.is_absolute() else root / path
    return Path(git(root, "rev-parse", "--path-format=absolute", "--git-path", "index"))


def index_snapshot(root: Path, scratch: Path) -> dict[str, str]:
    """Check the index out under `scratch` and return the environment that reads it.

    Git runs against the real repository (`GIT_DIR`) with the copy as its work
    tree and a private copy of the index, so `git status` there sees no change,
    `git ls-files` lists the index, and a refresh never writes the real index.
    """
    tree = scratch / "tree"
    tree.mkdir()
    index = scratch / "index"
    shutil.copyfile(index_file(root), index)
    git_dir = git(root, "rev-parse", "--absolute-git-dir")
    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in {"GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_PREFIX"}
    }
    environment.update(
        {
            "GIT_DIR": git_dir,
            "GIT_WORK_TREE": str(tree),
            "GIT_INDEX_FILE": str(index),
        }
    )
    git(tree, "checkout-index", "--all", "--force", env=environment)
    return environment


def run_guard(argv: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    try:
        result = subprocess.run(argv, cwd=cwd, env=env, check=False, stdin=subprocess.DEVNULL)
    except OSError as error:
        raise HookError(f"cannot run {' '.join(argv)}: {error}") from error
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def contracts_over_index(root: Path, rules: Path) -> None:
    """The language and documentation contracts, over the index."""
    python = sys.executable
    with tempfile.TemporaryDirectory(prefix="celestina-index-") as temporary:
        scratch = Path(temporary).resolve()
        environment = index_snapshot(root, scratch)
        tree = scratch / "tree"
        run_guard(
            [python, str(guard(root, rules, "check-language-contract.py")), "--root", str(tree)],
            tree,
            environment,
        )
        # The documentation contract used to run only in CI, so a red result
        # did not stop anything: five commits reached published `main` while it
        # was failing on ten committed files.
        run_guard(
            [
                python,
                str(guard(root, rules, "documentation_contract.py")),
                "--root",
                str(tree),
                "--quiet",
            ],
            tree,
            environment,
        )


def pre_commit(root: Path, rules: Path) -> None:
    contracts_over_index(root, rules)
    run_guard([sys.executable, str(guard(root, rules, "check-staged-units.py")), "--root", str(root)], root)


def pre_merge_commit(root: Path, rules: Path) -> None:
    # A merge does not close a delivery unit; commit-msg refuses one that does,
    # so the staged-unit batch has nothing to judge here.
    contracts_over_index(root, rules)


def commit_msg(root: Path, rules: Path, message_file: str) -> None:
    run_guard(
        [
            sys.executable,
            str(guard(root, rules, "commit_scope.py")),
            "--root",
            str(root),
            message_file,
        ],
        root,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--rules", type=Path, required=True)
    parser.add_argument("hook", choices=HOOKS)
    parser.add_argument("arguments", nargs="*")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    rules = args.rules.resolve()
    try:
        if args.hook == "commit-msg":
            if len(args.arguments) != 1:
                parser.error("commit-msg takes the message file")
            commit_msg(root, rules, args.arguments[0])
        elif args.arguments:
            parser.error(f"{args.hook} takes no arguments")
        elif args.hook == "pre-commit":
            pre_commit(root, rules)
        else:
            pre_merge_commit(root, rules)
    except HookError as error:
        say(str(error))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
