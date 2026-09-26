#!/usr/bin/env python3
"""Pure landing rules used by scripts/land-unit.py.

Every function here maps texts, registry tables or one Git tree to a result, so
the orchestration in land-unit.py can be tested on literal inputs. The ledger
parsing reuses documentation_contract.py and the scopes reuse
project_registry.py; nothing here restates their rules.
"""

from __future__ import annotations

from collections.abc import Callable, Iterable, Mapping
from dataclasses import asdict, dataclass, fields
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import posixpath
import subprocess
import tomllib
from typing import Any

from documentation_contract import (
    LEDGER_COLUMNS,
    extract_inline_links,
    is_separator_row,
    markdown_headings,
    normalized_heading,
    normalized_status,
    split_table_row,
)
from production_artifact import (
    ContractError,
    ERROR_PREFIX,
    VERIFICATION_ERRORS,
    expand_patterns,
    production_input_patterns,
    verification_input_patterns,
)
from project_registry import build_commit_scopes, path_allowed
from version_contract import VersionContractError, parse_registry, read_source_version
from version_tool import replace_source_version


LEDGER_HEADING = "change and commit ledger"
INVENTORY_HEADER = "added\tdeleted\tcontent\tpath"
OPEN_ROW_RULE = "`active`, or `done` without an inventory link"
VERSIONED_KINDS = ("bug", "milestone", "release")
# The diffstat cell does not change the plan's line counts, so a plan closed
# with this placeholder already has the plan's final numstat.
PLACEHOLDER_DIFFSTAT = "0 files, +0/-0"


REQUIRED_STATE = ("step", "branch", "kind", "summary", "project_id", "unit", "attempts")


class LandingError(RuntimeError):
    """The landing cannot proceed; the message says why."""


class LandingStop(LandingError):
    """A clean stop at one landing step, optionally naming the file at fault.

    `remedy` replaces the generic advice to resolve and continue, for a stop
    that `--continue` cannot clear.
    """

    def __init__(
        self, step: str, message: str, path: str | None = None, *, remedy: str | None = None
    ) -> None:
        super().__init__(message)
        self.step = step
        self.path = path
        self.remedy = remedy


@dataclass(frozen=True)
class UnitRef:
    project_id: str
    prefix: str
    plan_path: str
    unit: str
    summary: str
    evidence_path: str | None


@dataclass(frozen=True)
class InventoryRow:
    added: str
    deleted: str
    content: str
    path: str


@dataclass
class LandState:
    step: str
    branch: str
    kind: str
    summary: str
    project_id: str
    unit: str
    attempts: int
    # Recorded by later steps so that --continue reuses their outcome.
    base: str = ""
    tip: str = ""
    check: str = ""
    build: str = ""
    inventory: str = ""
    # main before the fast-forward, and the sealed commit it moved to.
    previous: str = ""
    sealed: str = ""

    def dump(self) -> str:
        lines = []
        for key, value in asdict(self).items():
            rendered = str(value) if isinstance(value, int) else toml_string(value)
            lines.append(f"{key} = {rendered}")
        return "\n".join(lines) + "\n"

    @classmethod
    def load(cls, text: str) -> "LandState":
        try:
            data = tomllib.loads(text)
        except tomllib.TOMLDecodeError as error:
            raise LandingError(f"invalid landing state: {error}") from error
        values: dict[str, Any] = {}
        for field in fields(cls):
            value = data.get(field.name)
            expected = int if field.name == "attempts" else str
            if value is None and field.name not in REQUIRED_STATE:
                value = ""
            if not isinstance(value, expected) or isinstance(value, bool):
                raise LandingError(f"invalid landing state: `{field.name}` is missing or invalid")
            values[field.name] = value
        return cls(**values)


def toml_string(value: str) -> str:
    """A TOML basic string for `value`.

    JSON's escapes are TOML's, except that JSON writes a character outside the
    BMP as a surrogate pair, which TOML rejects, so non-ASCII text stays
    literal; DEL is the one control character JSON leaves raw and TOML forbids.
    """
    return json.dumps(value, ensure_ascii=False).replace("\x7f", "\\u007f")


# Ledger tables.


@dataclass(frozen=True)
class LedgerTable:
    lines: list[str]
    header: list[str]
    separator: int
    rows: list[int]

    def cells(self, line_index: int) -> dict[str, str]:
        cells = split_table_row(self.lines[line_index])
        return dict(zip(self.header, cells))

    def unit_rows(self) -> dict[str, int]:
        return {self.cells(index).get("unit", "").strip("` "): index for index in self.rows}


def ledger_table(text: str, label: str, step: str) -> LedgerTable:
    lines = text.splitlines(keepends=True)
    headings = [
        line_number
        for _level, heading, line_number in markdown_headings(text)
        if normalized_heading(heading) == LEDGER_HEADING
    ]
    if len(headings) != 1:
        raise LandingStop(step, f"{label} has no single `Change and commit ledger` table")
    cursor = headings[0]
    while cursor < len(lines) and not lines[cursor].lstrip().startswith("|"):
        if lines[cursor].lstrip().startswith("#"):
            break
        cursor += 1
    if cursor + 1 >= len(lines) or not lines[cursor].lstrip().startswith("|"):
        raise LandingStop(step, f"{label} has no `Change and commit ledger` table")
    header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
    if not is_separator_row(split_table_row(lines[cursor + 1])) or not set(
        LEDGER_COLUMNS
    ).issubset(header):
        raise LandingStop(step, f"{label} has a malformed ledger table")
    rows = []
    index = cursor + 2
    while index < len(lines) and lines[index].lstrip().startswith("|"):
        rows.append(index)
        index += 1
    return LedgerTable(lines, header, cursor + 1, rows)


def has_inventory_link(cell: str) -> bool:
    return any(target.endswith(".numstat.tsv") for _line, target in extract_inline_links(cell))


def owner_tables(registry: Mapping[str, object]) -> list[Mapping[str, object]]:
    owners: list[Mapping[str, object]] = []
    suite = registry.get("suite")
    if isinstance(suite, Mapping):
        owners.append(suite)
    projects = registry.get("projects", [])
    if isinstance(projects, list):
        owners.extend(project for project in projects if isinstance(project, Mapping))
    return owners


def is_plan_file(path: str, directory: str) -> bool:
    return (
        posixpath.dirname(path) == directory
        and path.endswith(".md")
        and posixpath.basename(path).casefold() != "readme.md"
    )


def discover_unit(
    registry: Mapping[str, object],
    changed_paths: set[str],
    read_plan: Callable[[str], str | None],
) -> UnitRef:
    """Find the one open ledger row of the one active plan a branch changes.

    Plan texts come from `read_plan`, so the caller decides which revision is
    read.
    """
    candidates: list[tuple[str, Mapping[str, object]]] = []
    for owner in owner_tables(registry):
        directory = owner.get("active_plans")
        if not isinstance(directory, str):
            continue
        candidates.extend(
            (path, owner) for path in sorted(changed_paths) if is_plan_file(path, directory)
        )
    if len(candidates) != 1:
        listed = ", ".join(path for path, _owner in candidates) or "none"
        raise LandingStop(
            "preflight",
            f"the branch must change exactly one active plan; found {len(candidates)}: {listed}",
        )
    plan_path, owner = candidates[0]
    text = read_plan(plan_path)
    if text is None:
        raise LandingStop("preflight", f"the branch deletes its plan {plan_path}", plan_path)
    table = ledger_table(text, plan_path, "preflight")
    open_rows = []
    for index in table.rows:
        cells = table.cells(index)
        status = normalized_status(cells.get("status", ""))
        if status == "active" or (
            status == "done" and not has_inventory_link(cells.get("files / areas", ""))
        ):
            open_rows.append(cells)
    if len(open_rows) != 1:
        units = ", ".join(cells.get("unit", "").strip("` ") for cells in open_rows)
        raise LandingStop(
            "preflight",
            f"{plan_path} must have exactly one ledger row that is {OPEN_ROW_RULE}; "
            f"found {len(open_rows)}" + (f": {units}" if units else ""),
            plan_path,
        )
    cells = open_rows[0]
    unit = cells["unit"].strip("` ")
    raw_prefix = cells["commit prefix"].strip().strip("`").strip()
    prefix = raw_prefix[:-1] if raw_prefix.endswith(":") else ""
    if prefix not in build_commit_scopes(dict(registry)):
        raise LandingStop(
            "preflight",
            f"{plan_path} row {unit}: `{raw_prefix}` is not a registered prefix",
            plan_path,
        )
    if prefix != owner.get("commit_prefix"):
        raise LandingStop(
            "preflight",
            f"{plan_path} row {unit}: `{prefix}:` is not the plan owner's "
            f"`{owner.get('commit_prefix')}:`",
            plan_path,
        )
    evidence_path = None
    for _line, target in extract_inline_links(cells.get("automated evidence", "")):
        if target.endswith(".md"):
            evidence_path = posixpath.normpath(
                posixpath.join(posixpath.dirname(plan_path), target)
            )
            break
    return UnitRef(
        project_id=str(owner.get("id")),
        prefix=prefix,
        plan_path=plan_path,
        unit=unit,
        summary=cells.get("intended change", "").strip(),
        evidence_path=evidence_path,
    )


def owner_docs_root(owner: Mapping[str, object]) -> str:
    """The owner's `docs/` directory: the parent of its plans directory."""
    active = owner.get("active_plans")
    if not isinstance(active, str):
        raise LandingError(f"{owner.get('id')} registers no active_plans")
    return posixpath.dirname(posixpath.dirname(active))


def inventory_path(owner: Mapping[str, object], unit: UnitRef) -> str:
    """`<owner docs>/inventories/<plan-slug>/<unit>.numstat.tsv`."""
    plan_slug = posixpath.splitext(posixpath.basename(unit.plan_path))[0]
    return posixpath.join(
        owner_docs_root(owner), "inventories", plan_slug, f"{unit.unit}.numstat.tsv"
    )


def subject(prefix: str, kind: str, summary: str) -> str:
    return f"{prefix}-{kind}: {summary}"


def kind_refusal(registry: Mapping[str, object], project_id: str, kind: str) -> str | None:
    """Why `kind` cannot land for `project_id`, or None when it can.

    A versioned kind bumps the owner's version, so the owner must be a
    versioned project; the version contract decides which ones are.
    """
    if kind not in VERSIONED_KINDS:
        return None
    if project_id == "suite":
        return f"suite-{kind} bumps products, which the author records by hand"
    try:
        model = parse_registry(registry, "docs/projects.toml")
    except VersionContractError as error:
        raise LandingError(f"cannot read the version registry: {error}") from error
    owner = model.owner_map().get(project_id)
    if owner is None or not owner.versioned:
        return (
            f"{project_id} is not versioned, so a {kind} has no version to bump; "
            "land it with --kind maintenance"
        )
    return None


def scope_violations(
    prefix: str, registry: Mapping[str, object], changed_paths: set[str]
) -> list[str]:
    scope = build_commit_scopes(dict(registry)).get(prefix)
    if scope is None:
        return sorted(changed_paths)
    return sorted(path for path in changed_paths if not path_allowed(path, scope))


def matches_pattern(path: str, pattern: str) -> bool:
    """Whether `path`, or a directory above it, matches one registered input pattern."""
    parts = path.split("/")
    return any(
        fnmatch.fnmatchcase("/".join(parts[:end]), pattern) for end in range(1, len(parts) + 1)
    )


def affected_projects(
    root: Path, registry: Mapping[str, object], changed_paths: set[str], owner_id: str
) -> list[dict]:
    """The owner, then every other registered project whose inputs hold a changed path.

    The owner comes first when it is a registered project (a `suite` unit has
    none); the others follow in registry order. A project is affected when a
    changed path is one of its production inputs, which production_artifact.py
    fingerprints, expanded on the tree at `root`, or matches one of the raw
    production or verification input patterns, so that a deleted file still
    counts; a pattern also matches every path below a directory it names. The
    verification inputs are the whole set production_artifact.py fingerprints,
    so a shared script such as scripts/production-common.sh affects every
    project. `registry` must be the one of the tree at `root`: an input it
    names that does not exist there stops the landing.
    """
    owner: list[dict] = []
    affected = []
    projects = registry.get("projects", [])
    for project in projects if isinstance(projects, list) else []:
        if not isinstance(project, dict):
            continue
        if project.get("id") == owner_id:
            owner.append(project)
            continue
        patterns = production_input_patterns(dict(registry), project)
        inputs: list[str] = []
        for pattern in patterns:
            try:
                inputs.extend(logical for _disk, logical in expand_patterns(root, [pattern]))
            except ContractError as error:
                raise LandingStop(
                    "build_if_stale",
                    f"docs/projects.toml: the production input `{pattern}` of "
                    f"{project.get('id')} cannot be expanded on the rebased tip: {error}",
                    "docs/projects.toml",
                ) from error
        try:
            patterns.extend(verification_input_patterns(root, project))
        except ContractError as error:
            raise LandingStop(
                "build_if_stale",
                f"docs/projects.toml: the verification inputs of {project.get('id')} "
                f"cannot be read on the rebased tip: {error}",
                "docs/projects.toml",
            ) from error
        if any(
            path == item or path.startswith(f"{item}/") for path in changed_paths for item in inputs
        ) or any(matches_pattern(path, pattern) for path in changed_paths for pattern in patterns):
            affected.append(project)
    return owner + affected


def verification_only(check_output: str) -> bool:
    """Whether a failed `check --require-verified` reports only what a verification clears.

    production_artifact.py prints its errors on one line joined with "; ",
    and the verification errors contain "; " themselves, so each is removed
    whole before the rest is read.
    """
    lines = check_output.strip().splitlines()
    if not lines or not lines[0].startswith(ERROR_PREFIX):
        return False
    rest = lines[0][len(ERROR_PREFIX) :]
    found = False
    for error in VERIFICATION_ERRORS:
        if error in rest:
            found = True
            rest = rest.replace(error, "")
    return found and not any(part.strip() for part in rest.split(";"))


def masked_lines(table: LedgerTable, unit: str) -> list[tuple[int, str]]:
    """Every line of the table's text but `unit`'s ledger row, with its 1-based number."""
    row = table.unit_rows().get(unit)
    return [(index + 1, line) for index, line in enumerate(table.lines) if index != row]


def first_difference(
    left: list[tuple[int, str]], right: list[tuple[int, str]]
) -> tuple[int, int] | None:
    """The line numbers at which two masked texts first differ, or None when they match.

    A text that ends first differs one line past its last line.
    """
    for index in range(max(len(left), len(right))):
        mine = left[index] if index < len(left) else None
        theirs = right[index] if index < len(right) else None
        if mine is None or theirs is None or mine[1] != theirs[1]:
            return (
                mine[0] if mine else (left[-1][0] + 1 if left else 1),
                theirs[0] if theirs else (right[-1][0] + 1 if right else 1),
            )
    return None


def merge_plan(base_text: str, main_text: str, branch_text: str, unit: str, path: str) -> str:
    """Main's plan with the branch's row for `unit`; every other line is main's.

    That is only sound when the branch changed nothing else in the plan, so
    the merge stops unless the branch's text equals the base's with the unit's
    row masked on both sides.
    """
    base = ledger_table(base_text, "the base's plan", "rebase")
    main = ledger_table(main_text, "main's plan", "rebase")
    branch = ledger_table(branch_text, "the branch's plan", "rebase")
    branch_rows = branch.unit_rows()
    if unit not in branch_rows:
        raise LandingStop("rebase", f"the branch's plan has no ledger row {unit}")
    difference = first_difference(masked_lines(branch, unit), masked_lines(base, unit))
    if difference is not None:
        raise LandingStop(
            "rebase",
            f"{path}: the branch changed the plan beyond its own row {unit}: the plan differs "
            f"outside that row, first at line {difference[0]} on the branch and line "
            f"{difference[1]} at the fork point, so the landing cannot merge it: resolve "
            f"{path} in the landing worktree, `git add` it, run `git rebase --continue` "
            "there, then land-unit.py --continue",
            path,
        )
    branch_index = branch_rows[unit]
    replacement = branch.lines[branch_index]
    if not replacement.endswith("\n"):
        replacement += "\n"
    lines = list(main.lines)
    main_rows = main.unit_rows()
    if unit in main_rows:
        lines[main_rows[unit]] = replacement
        return "".join(lines)
    anchor = main.separator
    position = branch.rows.index(branch_index)
    if position > 0:
        previous_unit = branch.cells(branch.rows[position - 1]).get("unit", "").strip("` ")
        anchor = main_rows.get(previous_unit, main.separator)
    lines.insert(anchor + 1, replacement)
    return "".join(lines)


# Debt ratchets.


def ratchet_rows(text: str, label: str) -> dict[tuple[str, ...], tuple[int, int]]:
    """Map each row key to (value, index of the integer cell)."""
    rows: dict[tuple[str, ...], tuple[int, int]] = {}
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        cells = line.split("\t")
        numbers = [index for index, cell in enumerate(cells) if cell.isdecimal()]
        if len(numbers) != 1:
            raise LandingStop(
                "rebase", f"{label}: a ratchet row needs exactly one integer cell: {line!r}"
            )
        value_index = numbers[0]
        key = tuple(cell for index, cell in enumerate(cells) if index != value_index)
        rows[key] = (int(cells[value_index]), value_index)
    return rows


def is_ratchet_row(line: str) -> bool:
    return bool(line.strip()) and not line.lstrip().startswith("#")


def is_comment(line: str) -> bool:
    return line.lstrip().startswith("#")


def ratchet_comments(text: str) -> dict[tuple[str, ...], str | None]:
    """Each row key's comment: the comment line immediately above the row, if any."""
    lines = text.splitlines(keepends=True)
    comments: dict[tuple[str, ...], str | None] = {}
    for index, line in enumerate(lines):
        if is_ratchet_row(line):
            above = lines[index - 1] if index else ""
            key = ratchet_key(line.rstrip("\r\n").split("\t"))
            comments[key] = above if is_comment(above) else None
    return comments


def with_newline(line: str) -> str:
    return line if line.endswith("\n") else line + "\n"


def merge_ratchet(base_text: str, main_text: str, branch_text: str) -> str:
    """Merge a debt ratchet by row key; a row's comment is the comment line just above it.

    A key on both sides takes the lower value and keeps its comment, unless
    only the branch changed that comment; a key one side removed goes, with its
    comment; a key only the branch added is appended with its comment. Every
    other line, including the file's head comments, and the order follow main.
    """
    base = ratchet_rows(base_text, "base ratchet")
    main = ratchet_rows(main_text, "main's ratchet")
    branch = ratchet_rows(branch_text, "the branch's ratchet")
    base_comments = ratchet_comments(base_text)
    main_comments = ratchet_comments(main_text)
    branch_comments = ratchet_comments(branch_text)
    lines = main_text.splitlines(keepends=True)
    attached = {
        index - 1
        for index, line in enumerate(lines)
        if index and is_ratchet_row(line) and is_comment(lines[index - 1])
    }
    output: list[str] = []
    for index, line in enumerate(lines):
        if index in attached:
            continue
        if not is_ratchet_row(line):
            output.append(line)
            continue
        cells = line.rstrip("\r\n").split("\t")
        key = ratchet_key(cells)
        value, value_index = main[key]
        comment = main_comments[key]
        if key in branch:
            value = min(value, branch[key][0])
            if key in base and comment == base_comments[key]:
                comment = branch_comments[key]
        elif key in base:
            continue
        if comment is not None:
            output.append(with_newline(comment))
        cells[value_index] = str(value)
        ending = line[len(line.rstrip("\r\n")) :]
        output.append("\t".join(cells) + ending)
    if output and not output[-1].endswith("\n"):
        output[-1] += "\n"
    for line in branch_text.splitlines(keepends=True):
        if not is_ratchet_row(line):
            continue
        key = ratchet_key(line.rstrip("\r\n").split("\t"))
        if key not in main and key not in base:
            comment = branch_comments[key]
            if comment is not None:
                output.append(with_newline(comment))
            output.append(with_newline(line))
    return "".join(output)


def ratchet_key(cells: list[str]) -> tuple[str, ...]:
    return tuple(cell for cell in cells if not cell.isdecimal())


# Lockfiles.


def lockfile_blockers(lockfile: str, unresolved: Iterable[str]) -> list[str]:
    """The unresolved `Cargo.toml` files of `lockfile`'s workspace: its directory and below.

    Cargo reads those manifests to rewrite the lockfile, so while one holds
    conflict markers the lockfile cannot be merged.
    """
    directory = posixpath.dirname(lockfile)
    prefix = f"{directory}/" if directory else ""
    return sorted(
        path
        for path in unresolved
        if posixpath.basename(path) == "Cargo.toml" and path.startswith(prefix)
    )


def lock_packages(text: str, label: str) -> dict[str, set[tuple[str, str]]]:
    try:
        packages = tomllib.loads(text).get("package", [])
    except tomllib.TOMLDecodeError as error:
        raise LandingStop("rebase", f"{label} is not valid TOML: {error}") from error
    found: dict[str, set[tuple[str, str]]] = {}
    for package in packages if isinstance(packages, list) else []:
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            found.setdefault(package["name"], set()).add(
                (str(package.get("version", "")), str(package.get("checksum", "")))
            )
    return found


def lockfile_upgrades(main_lock: str, merged_lock: str) -> list[str]:
    """Packages in both lockfiles whose version or checksum moved.

    A package that only gained or only lost an entry is an added or removed
    dependency of the branch; one that lost an entry and gained another moved.
    """
    main = lock_packages(main_lock, "main's lockfile")
    merged = lock_packages(merged_lock, "the merged lockfile")
    return sorted(
        name
        for name in main.keys() & merged.keys()
        if main[name] - merged[name] and merged[name] - main[name]
    )


# Version sources.


def unbumped_files(
    registry: Mapping[str, object],
    read_base: Callable[[str], bytes | None],
    read_branch: Callable[[str], bytes | None],
) -> dict[str, bytes]:
    """Branch files with every registered version moved back to the base's.

    Only the version assignments and the history file return to the base; a
    dependency the branch added to the same manifest stays.
    """
    try:
        model = parse_registry(registry, "docs/projects.toml")
    except VersionContractError as error:
        raise LandingError(f"cannot read the version registry: {error}") from error
    if model.policy is None:
        return {}
    rewrites: dict[str, bytes] = {}
    history = model.policy.history_file
    base_history = read_base(history)
    branch_history = read_branch(history)
    if base_history is not None and branch_history != base_history:
        rewrites[history] = base_history
    current: dict[str, bytes] = {}
    for owner in model.owners:
        for spec in owner.sources:
            base_raw = read_base(spec.path)
            branch_raw = current.get(spec.path, read_branch(spec.path))
            if base_raw is None or branch_raw is None:
                continue
            try:
                base_version = read_source_version(spec, base_raw, f"base:{spec.path}")
                branch_version = read_source_version(spec, branch_raw, f"branch:{spec.path}")
                if base_version != branch_version:
                    current[spec.path] = replace_source_version(
                        spec, branch_raw, branch_version, base_version, spec.path
                    )
            except VersionContractError as error:
                raise LandingError(f"cannot read the version of {spec.path}: {error}") from error
    for path, raw in current.items():
        if raw != read_branch(path):
            rewrites[path] = raw
    return rewrites


# Inventories and ledger closure.


def git_program() -> str:
    return os.environ.get("LAND_UNIT_GIT", "git")


def git_run(root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run one Git command in `root`; every Git call of this module goes through here.

    It never reads the terminal, and it keeps stdout and stderr.
    """
    command = [git_program(), "--literal-pathspecs", "-C", str(root), *args]
    try:
        return subprocess.run(
            command,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as error:
        raise LandingError(f"cannot run {' '.join(command)}: {error}") from error


def git_output(root: Path, *args: str, allowed: tuple[int, ...] = (0,)) -> str:
    result = git_run(root, *args)
    if result.returncode not in allowed:
        raise LandingError(f"{' '.join(result.args)} failed: {result.stderr.strip()}")
    return result.stdout


def numstat_values(output: str, path: str) -> tuple[str, str]:
    lines = [line for line in output.splitlines() if line.strip()]
    if not lines:
        return "0", "0"
    if len(lines) != 1:
        raise LandingError(f"Git returned more than one numstat row for {path}")
    cells = lines[0].split("\t", 2)
    if len(cells) != 3:
        raise LandingError(f"Git returned an invalid numstat row for {path}: {lines[0]}")
    return cells[0], cells[1]


def final_digest(path: Path) -> str:
    try:
        if path.is_symlink():
            payload = os.fsencode(os.readlink(path))
        else:
            payload = path.read_bytes()
    except OSError as error:
        raise LandingError(f"cannot read {path}: {error}") from error
    return hashlib.sha256(payload).hexdigest()


def numstat_rows(root: Path, base: str, paths: Iterable[str]) -> list[InventoryRow]:
    rows = []
    for path in sorted(set(paths)):
        tracked = git_run(root, "cat-file", "-e", f"{base}:{path}").returncode == 0
        if tracked:
            output = git_output(root, "diff", "--numstat", "--no-renames", base, "--", path)
        else:
            output = git_output(
                root, "diff", "--no-index", "--numstat", os.devnull, path, allowed=(0, 1)
            )
        added, deleted = numstat_values(output, path)
        disk = root / path
        content = final_digest(disk) if os.path.lexists(disk) else "deleted"
        rows.append(InventoryRow(added, deleted, content, path))
    return rows


def render_inventory(
    unit: str, base: str, rows: list[InventoryRow], inventory_path: str
) -> str:
    rows = [row for row in rows if row.path != inventory_path]
    pathspecs = sorted({row.path for row in rows} | {inventory_path})
    head = [
        f"# {unit} exact change inventory",
        "",
        f"Base revision\t{base}",
        *(f"Pathspec\t{path}" for path in pathspecs),
        "Calculation\ttracked paths use git diff --numstat --no-renames; new paths use /dev/null",
        "Hashes\tSHA-256 of final bytes",
        "",
        INVENTORY_HEADER,
    ]
    total = len(head) + len(rows) + 1
    table = sorted(
        [*rows, InventoryRow(str(total), "0", "self", inventory_path)],
        key=lambda row: row.path,
    )
    body = [f"{row.added}\t{row.deleted}\t{row.content}\t{row.path}" for row in table]
    return "\n".join([*head, *body]) + "\n"


def diffstat(rows: list[InventoryRow]) -> str:
    added = sum(int(row.added) for row in rows if row.added.isdecimal())
    deleted = sum(int(row.deleted) for row in rows if row.deleted.isdecimal())
    return f"{len(rows)} files, +{added}/-{deleted}"


def sealed_diffstat(unit: str, base: str, rows: list[InventoryRow], inventory: str) -> str:
    """The unit's diffstat including the inventory's own row.

    `rows` are the unit's other paths, computed with the plan closed with
    PLACEHOLDER_DIFFSTAT; the final diffstat has the same line counts, so it
    renders an inventory of the same length and the value is a fixed point.
    """
    draft = render_inventory(unit, base, rows, inventory)
    self_row = InventoryRow(str(len(draft.splitlines())), "0", "self", inventory)
    return diffstat([*[row for row in rows if row.path != inventory], self_row])


def render_row(cells: list[str]) -> str:
    return "| " + " | ".join(cell.replace("|", r"\|") for cell in cells) + " |\n"


def close_ledger_row(
    plan_text: str, unit: str, inventory_link: str, diffstat: str, evidence_link: str
) -> str:
    """Close `unit`'s row; the links are already relative to the plan's directory."""
    table = ledger_table(plan_text, "the plan", "seal")
    rows = table.unit_rows()
    if unit not in rows:
        raise LandingStop("seal", f"the plan has no ledger row {unit}")
    index = rows[unit]
    cells = split_table_row(table.lines[index])
    if len(cells) != len(table.header):
        raise LandingStop("seal", f"the ledger row {unit} has the wrong number of cells")
    values = {
        "status": "done",
        "files / areas": f"[inventory]({inventory_link})",
        "diffstat": diffstat,
        "automated evidence": f"[evidence]({evidence_link})",
    }
    for column, value in values.items():
        cells[table.header.index(column)] = value
    lines = list(table.lines)
    lines[index] = render_row(cells)
    return "".join(lines)


def landing_section(base: str, check_output: str, build: str | None) -> str:
    return (
        "## Landing\n\n"
        f"- **Base revision:** `{base}`\n"
        f"- **Check:** {check_output}\n"
        f"- **Build:** {build if build is not None else 'artifact current; no build'}\n"
    )
