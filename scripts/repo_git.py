#!/usr/bin/env python3
"""The one Git runner of the repository scripts (TOOL-18).

`run` starts one Git process that never reads the terminal and keeps both
output streams as bytes; `output` raises `GitError` with Git's own message on
a failure, and a timeout is a `GitError` as well, never a hang. `BlobReader`
serves many object reads through one `git cat-file --batch` process, which is
what makes the guards that replay history fast (TOOL-7, TOOL-20).

Callers keep their own error types: each turns a `GitError` into the failure
its tool reports.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping
import os
from pathlib import Path
import subprocess


class GitError(RuntimeError):
    """A Git command could not run, timed out, or failed."""


def run(
    root: Path,
    *args: str,
    program: str = "git",
    input: bytes | None = None,
    timeout: float | None = None,
    env: Mapping[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    """Run `git -C root ARGS`; the caller reads `returncode`."""
    command = [program, "-C", str(root), *args]
    try:
        return subprocess.run(
            command,
            input=input,
            stdin=None if input is not None else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=None if env is None else dict(env),
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise GitError(f"git {' '.join(args)} timed out after {timeout:g} s") from error
    except OSError as error:
        raise GitError(f"cannot run git {' '.join(args)}: {error}") from error


def output(root: Path, *args: str, allowed: Iterable[int] = (0,), **options: object) -> bytes:
    """The stdout of `git -C root ARGS`, or `GitError` naming Git's message."""
    result = run(root, *args, **options)  # type: ignore[arg-type]
    if result.returncode not in tuple(allowed):
        detail = decode(result.stderr).strip()
        raise GitError(f"git {' '.join(args)} failed" + (f": {detail}" if detail else ""))
    return result.stdout


def merge_heads(root: Path) -> tuple[str, ...]:
    """The commits a merge in progress brings in, or none outside a merge.

    Git writes `MERGE_HEAD` when a merge stops and before `commit-msg` runs.
    During an automatic merge's `pre-merge-commit` it has not written it yet,
    and passes each merged commit as a `GITHEAD_<object id>` variable instead.
    """
    located = run(root, "rev-parse", "--git-path", "MERGE_HEAD")
    candidates: list[str] = []
    if located.returncode == 0:
        path = Path(decode(located.stdout).strip())
        if not path.is_absolute():
            path = root / path
        try:
            candidates = path.read_text(encoding="ascii").split()
        except (OSError, UnicodeDecodeError):
            candidates = []
    if not candidates:
        candidates = [
            key[len("GITHEAD_") :]
            for key in sorted(os.environ)
            if key.startswith("GITHEAD_")
            and len(key) in (len("GITHEAD_") + 40, len("GITHEAD_") + 64)
            and all(character in "0123456789abcdef" for character in key[len("GITHEAD_") :])
        ]
    heads = []
    for candidate in candidates:
        verified = run(root, "rev-parse", "-q", "--verify", f"{candidate}^{{commit}}")
        if verified.returncode == 0:
            heads.append(decode(verified.stdout).strip())
    return tuple(heads)


def decode(raw: bytes) -> str:
    """Git's bytes as text; a path that is not UTF-8 survives the round trip."""
    return raw.decode("utf-8", "surrogateescape")


def paths(raw: bytes) -> list[str]:
    """The paths of NUL-separated (`-z`) output."""
    return [os.fsdecode(part) for part in raw.split(b"\0") if part]


class BlobReader:
    """Object reads through one `git cat-file --batch` process.

    `read("REV:PATH")` or `read("<oid>")` returns the object's bytes, or None
    when Git has no such object. A name that holds a newline cannot pass
    through the batch protocol, so it is read by a process of its own.
    """

    def __init__(self, root: Path, *, env: Mapping[str, str] | None = None) -> None:
        self.root = root
        self.env = None if env is None else dict(env)
        self.process: subprocess.Popen[bytes] | None = None

    def __enter__(self) -> "BlobReader":
        return self

    def __exit__(self, *_exception: object) -> None:
        self.close()

    def start(self) -> subprocess.Popen[bytes]:
        if self.process is None:
            try:
                self.process = subprocess.Popen(
                    ["git", "-C", str(self.root), "cat-file", "--batch"],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    env=self.env,
                )
            except OSError as error:
                raise GitError(f"cannot run git cat-file --batch: {error}") from error
        return self.process

    def read(self, name: str) -> bytes | None:
        if "\n" in name or "\r" in name:
            result = run(self.root, "cat-file", "-p", name, env=self.env)
            return result.stdout if result.returncode == 0 else None
        process = self.start()
        assert process.stdin is not None and process.stdout is not None
        try:
            process.stdin.write(os.fsencode(name) + b"\n")
            process.stdin.flush()
            header = process.stdout.readline()
        except OSError as error:
            raise GitError(f"git cat-file --batch failed reading {name}: {error}") from error
        if not header:
            raise GitError(f"git cat-file --batch ended while reading {name}")
        fields = header.split()
        if len(fields) != 3 or fields[1] in {b"missing", b"ambiguous"}:
            return None
        try:
            size = int(fields[2])
        except ValueError as error:
            raise GitError(f"git cat-file --batch returned {header!r} for {name}") from error
        payload = process.stdout.read(size)
        process.stdout.read(1)
        if len(payload) != size:
            raise GitError(f"git cat-file --batch ended inside {name}")
        return payload

    def exists(self, name: str) -> bool:
        return self.read(name) is not None

    def close(self) -> None:
        process, self.process = self.process, None
        if process is None:
            return
        if process.stdin is not None:
            try:
                process.stdin.close()
            except OSError:
                pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        if process.stdout is not None:
            process.stdout.close()
