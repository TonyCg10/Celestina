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
import sys

from project_registry import parse_registry, parse_subject_change
from repo_git import BlobReader, GitError
import repo_git
from version_contract import validate_staged_transition


ROOT = Path(__file__).resolve().parent.parent
REGISTRY_PATH = "docs/projects.toml"
HISTORY_PATH = "docs/version-history.tsv"
PUBLISHED_WRAPPERS = frozenset({"fixup", "squash", "amend"})


class AuditError(ValueError):
    """The Git history cannot satisfy or be evaluated against the contract."""


def git(root: Path, *arguments: str) -> bytes:
    try:
        return repo_git.output(root, *arguments)
    except GitError as error:
        raise AuditError(str(error)) from error


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


def audited_commits(root: Path, adoption: str, since: str | None) -> tuple[str, ...]:
    """The non-merge commits after the adoption, or only those after `since`.

    CI passes the revision before the push, so a push re-audits what it
    published and not the whole history (TOOL-20); the schedule replays all.
    """
    ranges = [f"{adoption}..HEAD"]
    if since is not None:
        ranges.append(f"^{since}")
    raw = git(root, "rev-list", "--reverse", "--topo-order", "--no-merges", *ranges)
    return tuple(line for line in raw.decode("ascii").splitlines() if line)


class History:
    """Commits, registries and blobs read through one `git cat-file --batch`."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.reader = BlobReader(root)
        self.registries: dict[str, dict[str, object]] = {}
        self.changes: dict[str, tuple[str, ...]] = {}

    def close(self) -> None:
        self.reader.close()

    def read(self, name: str) -> bytes | None:
        try:
            return self.reader.read(name)
        except GitError as error:
            raise AuditError(str(error)) from error

    def commit(self, commit: str) -> tuple[tuple[str, ...], str]:
        """A commit's parents and its subject as `%s` renders it."""
        raw = self.read(f"{commit}^{{commit}}")
        if raw is None:
            raise AuditError(f"{commit} is not a commit")
        head, _, message = raw.partition(b"\n\n")
        parents = tuple(
            line.split(b" ", 1)[1].decode("ascii")
            for line in head.split(b"\n")
            if line.startswith(b"parent ")
        )
        paragraph = message.lstrip(b"\n").split(b"\n\n", 1)[0]
        subject = " ".join(
            line.strip()
            for line in decode_utf8(paragraph, f"{commit} subject").splitlines()
        )
        return parents, subject

    def first_parent(self, commit: str) -> str:
        parents, _subject = self.commit(commit)
        if len(parents) != 1:
            raise AuditError(
                f"expected one parent for a non-merge commit, found {len(parents)}"
            )
        return parents[0]

    def required_blob(self, revision: str, path: str) -> bytes:
        raw = self.read(f"{revision}:{path}")
        if raw is None:
            raise AuditError(f"{revision}:{path} is missing or unreadable")
        return raw

    def optional_blob(self, revision: str, path: str) -> bytes | None:
        return self.read(f"{revision}:{path}")

    def registry_at(self, revision: str) -> dict[str, object]:
        if revision not in self.registries:
            raw = self.required_blob(revision, REGISTRY_PATH)
            try:
                self.registries[revision] = parse_registry(
                    raw, f"{revision}:{REGISTRY_PATH}"
                )
            except ValueError as error:
                raise AuditError(str(error)) from error
        return self.registries[revision]

    def prefetch_changes(self, commits: Sequence[str]) -> None:
        """Every audited commit's changed paths, in one `git log`."""
        if not commits:
            return
        raw = git(
            self.root,
            "log",
            "--no-walk=unsorted",
            "--format=%x01%H",
            "--name-only",
            "-z",
            "--no-renames",
            *commits,
        )
        for chunk in raw.split(b"\x01"):
            if not chunk:
                continue
            commit, _, rest = chunk.partition(b"\0")
            self.changes[commit.decode("ascii").strip()] = tuple(
                path.decode("utf-8", "surrogateescape")
                for path in rest.lstrip(b"\n").split(b"\0")
                if path
            )

    def changed_paths(self, parent: str, commit: str) -> tuple[str, ...]:
        if commit in self.changes:
            return self.changes[commit]
        raw = git(
            self.root,
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
            path.decode("utf-8", "surrogateescape") for path in raw.split(b"\0") if path
        )


def audit_commit(history: History, commit: str) -> None:
    parent = history.first_parent(commit)
    parent_registry = history.registry_at(parent)
    commit_registry = history.registry_at(commit)
    parent_policy = parent_registry.get("version_policy")
    if parent_policy is not None and not isinstance(parent_policy, Mapping):
        raise AuditError(f"{parent}:{REGISTRY_PATH} version_policy must be a table")

    _parents, subject = history.commit(commit)
    prefix, kind, action, wrapper = parse_subject_change(
        subject,
        allow_legacy=parent_policy is None,
    )
    if wrapper in PUBLISHED_WRAPPERS:
        raise AuditError(
            f'published {wrapper}! commit "{subject}" must be squashed before delivery'
        )

    paths = history.changed_paths(parent, commit)

    def read_transition_blob(revision: str, path: str) -> bytes | None:
        if revision == "HEAD":
            source = parent
        elif revision == "INDEX":
            source = commit
        else:
            raise AuditError(f'version contract requested unknown revision "{revision}"')
        return history.optional_blob(source, path)

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
    parser.add_argument(
        "--since",
        metavar="REV",
        help="audit only the commits after REV; an unknown or all-zero REV audits everything",
    )
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
        since = args.since or None
        if since is not None and (
            set(since) == {"0"}
            or repo_git.run(root, "rev-parse", "--verify", "--quiet", f"{since}^{{commit}}").returncode
            != 0
        ):
            # A new branch reports an all-zero "before"; a force push may name a
            # commit this clone lacks. Either way the whole history is audited.
            print(f"version-commit-audit: {since} is not a known commit; auditing all history")
            since = None
        commits = audited_commits(root, adoption, since)
        history = History(root)
        try:
            history.prefetch_changes(commits)
            for commit in commits:
                try:
                    audit_commit(history, commit)
                except (AuditError, ValueError) as error:
                    print(
                        f"version-commit-audit: {commit[:12]}: "
                        f"{type(error).__name__}: {error}",
                        file=sys.stderr,
                    )
                    return 1
        finally:
            history.close()

        scope = f"adoption {adoption[:12]}" if since is None else since[:12]
        print(f"version-commit-audit: OK ({len(commits)} non-merge commits after {scope})")
        return 0
    except AuditError as error:
        print(f"version-commit-audit: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
