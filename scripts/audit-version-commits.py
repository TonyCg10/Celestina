#!/usr/bin/env python3
"""Audit published commits against the typed product-version contract.

The local commit hook validates the index before a commit is created. This
history audit applies the same transition rule to committed parent/child trees
so a commit made with hooks disabled cannot bypass the contract in CI.
"""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
from pathlib import Path
import subprocess
import sys
import tomllib

from project_registry import parse_subject_change
from version_contract import validate_staged_transition


ROOT = Path(__file__).resolve().parent.parent
REGISTRY_PATH = "docs/projects.toml"
HISTORY_PATH = "docs/version-history.tsv"
PUBLISHED_WRAPPERS = frozenset({"fixup", "squash", "amend"})


class AuditError(ValueError):
    """The Git history cannot satisfy or be evaluated against the contract."""


def git(root: Path, *arguments: str) -> bytes:
    process = subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        detail = process.stderr.decode("utf-8", "replace").strip()
        command = " ".join(arguments)
        suffix = f": {detail}" if detail else ""
        raise AuditError(f"git {command} failed{suffix}")
    return process.stdout


def decode_utf8(raw: bytes, label: str) -> str:
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AuditError(f"{label} is not UTF-8: {error}") from error


def history_exists_at_head(root: Path) -> bool:
    raw = git(root, "ls-tree", "-r", "--name-only", "-z", "HEAD", "--", HISTORY_PATH)
    paths = [path for path in raw.split(b"\0") if path]
    return HISTORY_PATH.encode("utf-8") in paths


def adoption_commit(root: Path) -> str:
    raw = git(
        root,
        "log",
        "--format=%H",
        "--diff-filter=A",
        "--reverse",
        "HEAD",
        "--",
        HISTORY_PATH,
    )
    commits = [line for line in raw.decode("ascii").splitlines() if line]
    if not commits:
        raise AuditError(
            f"{HISTORY_PATH} exists in HEAD but no reachable addition commit was found"
        )
    return commits[0]


def commits_after_adoption(root: Path, adoption: str) -> tuple[str, ...]:
    raw = git(
        root,
        "rev-list",
        "--reverse",
        "--topo-order",
        "--no-merges",
        f"{adoption}..HEAD",
    )
    return tuple(line for line in raw.decode("ascii").splitlines() if line)


def first_parent(root: Path, commit: str) -> str:
    raw = git(root, "rev-list", "--parents", "-n", "1", commit)
    fields = raw.decode("ascii").split()
    if not fields or fields[0] != commit:
        raise AuditError("Git returned an invalid parent record")
    if len(fields) != 2:
        raise AuditError(
            f"expected one parent for a non-merge commit, found {len(fields) - 1}"
        )
    return fields[1]


def required_blob(root: Path, revision: str, path: str) -> bytes:
    process = subprocess.run(
        ["git", "-C", str(root), "show", f"{revision}:{path}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        detail = process.stderr.decode("utf-8", "replace").strip()
        suffix = f": {detail}" if detail else ""
        raise AuditError(f"{revision}:{path} is missing or unreadable{suffix}")
    return process.stdout


def optional_blob(root: Path, revision: str, path: str) -> bytes | None:
    process = subprocess.run(
        ["git", "-C", str(root), "show", f"{revision}:{path}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return process.stdout if process.returncode == 0 else None


def registry_at(root: Path, revision: str) -> dict[str, object]:
    raw = required_blob(root, revision, REGISTRY_PATH)
    try:
        registry = tomllib.loads(decode_utf8(raw, f"{revision}:{REGISTRY_PATH}"))
    except tomllib.TOMLDecodeError as error:
        raise AuditError(f"{revision}:{REGISTRY_PATH} is invalid TOML: {error}") from error
    if not isinstance(registry, dict):
        raise AuditError(f"{revision}:{REGISTRY_PATH} must contain a TOML table")
    return registry


def subject_at(root: Path, commit: str) -> str:
    raw = git(root, "show", "-s", "--format=%s", commit)
    return decode_utf8(raw, f"{commit} subject").removesuffix("\n")


def changed_paths(root: Path, parent: str, commit: str) -> tuple[str, ...]:
    raw = git(
        root,
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "--no-renames",
        "-r",
        "-z",
        parent,
        commit,
    )
    return tuple(
        path.decode("utf-8", "surrogateescape")
        for path in raw.split(b"\0")
        if path
    )


def audit_commit(root: Path, commit: str) -> None:
    parent = first_parent(root, commit)
    parent_registry = registry_at(root, parent)
    commit_registry = registry_at(root, commit)
    parent_policy = parent_registry.get("version_policy")
    if parent_policy is not None and not isinstance(parent_policy, Mapping):
        raise AuditError(f"{parent}:{REGISTRY_PATH} version_policy must be a table")

    subject = subject_at(root, commit)
    prefix, kind, action, wrapper = parse_subject_change(
        subject,
        allow_legacy=parent_policy is None,
    )
    if wrapper in PUBLISHED_WRAPPERS:
        raise AuditError(
            f'published {wrapper}! commit "{subject}" must be squashed before delivery'
        )

    paths = changed_paths(root, parent, commit)

    def read_transition_blob(revision: str, path: str) -> bytes | None:
        if revision == "HEAD":
            source = parent
        elif revision == "INDEX":
            source = commit
        else:
            raise AuditError(f'version contract requested unknown revision "{revision}"')
        return optional_blob(root, source, path)

    validate_staged_transition(
        parent_registry,
        commit_registry,
        prefix,
        kind,
        action,
        wrapper,
        read_transition_blob,
        paths,
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    root = args.root.resolve()

    try:
        git(root, "rev-parse", "--verify", "HEAD^{commit}")
        if not history_exists_at_head(root):
            print(
                f"version-commit-audit: OK (pre-adoption; {HISTORY_PATH} is not in HEAD)"
            )
            return 0

        adoption = adoption_commit(root)
        commits = commits_after_adoption(root, adoption)
        for commit in commits:
            try:
                audit_commit(root, commit)
            except (AuditError, ValueError) as error:
                print(
                    f"version-commit-audit: {commit[:12]}: "
                    f"{type(error).__name__}: {error}",
                    file=sys.stderr,
                )
                return 1

        print(
            f"version-commit-audit: OK ({len(commits)} non-merge commits after "
            f"adoption {adoption[:12]})"
        )
        return 0
    except AuditError as error:
        print(f"version-commit-audit: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
