#!/usr/bin/env python3
"""Pure landing rules used by scripts/land-unit.py.

Every function here maps texts, registry tables or one Git tree to a result, so
the orchestration in land-unit.py can be tested on literal inputs. The ledger
parsing reuses documentation_contract.py and the scopes reuse
project_registry.py; nothing here restates their rules.
"""

from __future__ import annotations

from collections.abc import Callable, Collection, Iterable, Mapping
from dataclasses import asdict, dataclass, fields
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
    inputs_changed,
)
from project_registry import build_commit_scopes, halted_projects, path_allowed
from repo_git import GitError
import repo_git
from version_contract import VersionContractError, parse_registry, read_source_version
from version_tool import replace_source_version


LEDGER_HEADING = "change and commit ledger"
INVENTORY_HEADER = "added\tdeleted\tcontent\tpath"
OPEN_ROW_RULE = "`active`, or `done` without an inventory link"
# The cells close_ledger_row writes when the seal closes a row; every other
# cell of a row is the session's.
SEAL_COLUMNS = ("status", "files / areas", "diffstat", "automated evidence")
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
    # Other active plans the branch changes that discover_unit set aside: the
    # plans of the units a stacked branch depends on, which hold only rows
    # origin/main settles; a plan main lacks with no open row; and a plan that
    # only follows links into a renamed halted owner's tree.
    settled_plans: tuple[str, ...] = ()


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
    # The deployable projects that build_if_stale built or verified, which
    # the deploy step deploys once the push landed them; space-separated ids,
    # each removed once its deploy and status entries passed (TOOL-8).
    deploy: str = ""
    # "yes" once origin/main holds the sealed commit: from then on nothing
    # moves main back, and --abort only reports what is not deployed.
    pushed: str = ""
    # The failed `contracts` run the author landed over with --accept-red-ci.
    red_ci: str = ""

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
        return {row_unit(self.cells(index)): index for index in self.rows}


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


def is_open_row(cells: Mapping[str, str]) -> bool:
    """`active`, or `done` without an inventory link: a unit that has not landed."""
    status = normalized_status(cells.get("status", ""))
    return status == "active" or (
        status == "done" and not has_inventory_link(cells.get("files / areas", ""))
    )


def is_closed_row(cells: Mapping[str, str]) -> bool:
    """`done` with an inventory link: a unit the seal closed."""
    return normalized_status(cells.get("status", "")) == "done" and has_inventory_link(
        cells.get("files / areas", "")
    )


def row_unit(cells: Mapping[str, str]) -> str:
    return cells.get("unit", "").strip("` ")


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


def is_active_plan(registry: Mapping[str, object], path: str) -> bool:
    """Whether `path` is an active plan of a registered owner."""
    return any(
        isinstance(owner.get("active_plans"), str)
        and is_plan_file(path, str(owner.get("active_plans")))
        for owner in owner_tables(registry)
    )


def renamed_owner_roots(
    main_registry: Mapping[str, object], branch_registry: Mapping[str, object]
) -> dict[str, str]:
    """{former owner's `active_plans`: renamed owner's} for each halted owner the branch renames.

    A branch owner that declares `history_owner` and `halted` carries the
    history of the origin/main owner of that id with its tree
    (check-staged-units.py `renamed_owner_moves`). The pairing holds only in
    the commit that renames the owner: the branch owner is absent on main, the
    main owner declares no `history_owner` itself, and the two plan
    directories differ. Empty for a branch that renames nothing.
    """
    main_owners = {
        str(owner.get("id")): owner for owner in owner_tables(main_registry) if "id" in owner
    }
    roots: dict[str, str] = {}
    for owner in owner_tables(branch_registry):
        former = owner.get("history_owner")
        if not isinstance(former, str) or "halted" not in owner or owner.get("id") in main_owners:
            continue
        main_owner = main_owners.get(former)
        if main_owner is None or main_owner.get("history_owner") is not None:
            continue
        old_root, new_root = main_owner.get("active_plans"), owner.get("active_plans")
        if isinstance(old_root, str) and isinstance(new_root, str) and old_root != new_root:
            roots[old_root] = new_root
    return roots


def moved_plan_paths(
    main_registry: Mapping[str, object],
    branch_registry: Mapping[str, object],
    changed_paths: Collection[str],
    read_branch: Callable[[str], str | None],
    read_main: Callable[[str], str | None],
) -> set[str]:
    """The old-path plans a renamed halted owner moved unchanged: deletions that are moves.

    For each root pair of `renamed_owner_roots`, a changed plan file under the
    former `active_plans` directory that the branch lacks, whose text on main
    is the text the branch holds at the same place under the new directory,
    where main has nothing, moved with the owner's tree: it is not a plan the
    branch changes. A plan whose copy differs, or whose copy main already
    tracks, pairs with nothing.
    """
    moved: set[str] = set()
    for old_root, new_root in renamed_owner_roots(main_registry, branch_registry).items():
        for path in sorted(changed_paths):
            if not is_plan_file(path, old_root) or read_branch(path) is not None:
                continue
            new_path = new_root + path[len(old_root):]
            if new_path not in changed_paths or read_main(new_path) is not None:
                continue
            original = read_main(path)
            if original is not None and read_branch(new_path) == original:
                moved.add(path)
    return moved


def follow_renamed_links(text: str, plan_path: str, renamed_roots: Mapping[str, str]) -> str:
    """`text` with each inline link into a renamed halted owner's `docs/` at its new path.

    Each pair of `renamed_roots` names the owner's old and new `active_plans`
    directories, whose `docs/` roots `owner_docs_root` derives the same way. A
    link target that resolves under the old root is read under the new one;
    one already under the new root is written in the same form. Other links
    and all other text are untouched, so two texts that differ only in where
    such links point compare equal.
    """
    directory = posixpath.dirname(plan_path)
    roots = [
        (posixpath.dirname(posixpath.dirname(old)), posixpath.dirname(posixpath.dirname(new)))
        for old, new in renamed_roots.items()
    ]
    rewritten: dict[int, dict[str, str]] = {}
    for line_number, target in extract_inline_links(text):
        resolved = posixpath.normpath(posixpath.join(directory, target))
        for old_docs, new_docs in roots:
            if not old_docs or not new_docs:
                continue
            if resolved == old_docs or resolved.startswith(old_docs + "/"):
                rewritten.setdefault(line_number, {})[target] = new_docs + resolved[len(old_docs):]
            elif resolved == new_docs or resolved.startswith(new_docs + "/"):
                rewritten.setdefault(line_number, {})[target] = resolved
    if not rewritten:
        return text
    lines = text.splitlines(keepends=True)
    for line_number, targets in rewritten.items():
        line = lines[line_number - 1]
        for target, new_target in targets.items():
            # Anchored on the closing delimiter, so a target that is a prefix
            # of another link's target rewrites only its own link.
            line = line.replace(f"](<{target}>)", f"](<{new_target}>)").replace(
                f"]({target})", f"]({new_target})"
            )
        lines[line_number - 1] = line
    return "".join(lines)


def only_follows_renamed_links(
    main_text: str | None, branch_text: str, plan_path: str, renamed_roots: Mapping[str, str]
) -> bool:
    """Whether the branch's plan differs from main's only in links into a renamed owner's tree.

    Such a plan (another owner's, linking a record the rename moved) changes no
    row of its own, and is set aside while another changed plan remains. False
    without a renamed owner, for a plan main lacks, and for an unchanged plan.
    """
    if not renamed_roots or main_text is None or main_text == branch_text:
        return False
    return follow_renamed_links(main_text, plan_path, renamed_roots) == follow_renamed_links(
        branch_text, plan_path, renamed_roots
    )


def has_open_row(text: str, label: str) -> bool:
    """Whether the plan has an open row on the branch; a plan without a ledger counts as open."""
    try:
        return bool(open_units(text, label))
    except LandingStop:
        return True


def discover_unit(
    registry: Mapping[str, object],
    changed_paths: set[str],
    read_plan: Callable[[str], str | None],
    read_main_plan: Callable[[str], str | None] | None = None,
    branch_unit: str | None = None,
    *,
    read_base_plan: Callable[[str], str | None] | None = None,
    stacked_on: Callable[[str], bool] | None = None,
    moved_plans: Collection[str] = (),
    renamed_roots: Mapping[str, str] | None = None,
) -> UnitRef:
    """Find the one open ledger row of the one active plan a branch changes.

    Plan texts come from `read_plan` (the branch), `read_main_plan`
    (origin/main) and `read_base_plan` (the fork point), so the caller decides
    which revisions are read. `moved_plans` are the old-path plans a renamed
    halted owner moved unchanged (`moved_plan_paths`): the branch deletes them
    only as paths, so they are not its plans and are left out before the
    deletion stop. A changed plan that `plan_settled` finds holds
    nothing main lacks belongs to units that already landed, such as the
    dependencies of a stacked branch, and is set aside while another plan
    remains; so are, only while `renamed_roots` (from `renamed_owner_roots`)
    names a renamed halted owner, a plan that main and the fork point lack and
    whose branch text has no open row, which the rename delivers as
    documentation (the new project's plan with only `planned` rows), and a
    plan that differs from main's only in links that follow the renamed
    owner's tree (`only_follows_renamed_links`). In the unit's plan, a row
    whose line equals main's is not the branch's (main's own open rows, such as the author's in-flight work), and
    a row open on the branch that main closed is set aside the same way, except the row of `branch_unit`, the unit the
    branch is named after: when main closed it, in any plan, it is the unit,
    which preflight then refuses as landed, and the unit found must be it.
    A stop that lists open rows main has not
    closed names them; `stacked_on(unit)` says whether the branch is stacked on
    that unit's branch, which words the stop as a landing order.
    """
    read_main = read_main_plan or (lambda _path: None)
    read_base = read_base_plan or (lambda _path: None)
    moved = set(moved_plans)
    renamed = dict(renamed_roots or {})
    plans: list[tuple[str, Mapping[str, object]]] = []
    for owner in owner_tables(registry):
        directory = owner.get("active_plans")
        if not isinstance(directory, str):
            continue
        plans.extend(
            (path, owner)
            for path in sorted(changed_paths)
            if is_plan_file(path, directory) and path not in moved
        )
    texts: dict[str, str] = {}
    for path, _owner in plans:
        text = read_plan(path)
        if text is None:
            raise LandingStop("preflight", f"the branch deletes its plan {path}", path)
        texts[path] = text
    own_plans = [
        (path, owner)
        for path, owner in plans
        if branch_unit is not None
        and branch_unit in open_units(texts[path], path, read_main(path))
    ]
    # The unit's own row closed on main: preflight refuses it as landed, even
    # beside another plan that is open.
    landed_own = [
        (path, owner)
        for path, owner in own_plans
        if branch_unit in closed_units(read_main(path), path)
    ]
    # Set aside while another plan remains: the plans of landed units and,
    # only while a halted owner is renamed, a new plan with no open row and a
    # plan that only follows the renamed owner.
    documentation = {
        path
        for path, _owner in plans
        if renamed
        and read_main(path) is None
        and read_base(path) is None
        and not has_open_row(texts[path], path)
    }
    retargeted = {
        path
        for path, _owner in plans
        if only_follows_renamed_links(read_main(path), texts[path], path, renamed)
    }
    unsettled = [
        (path, owner)
        for path, owner in plans
        if path not in documentation
        and path not in retargeted
        and not plan_settled(read_base(path), read_main(path), texts[path])
    ]
    chosen = landed_own or unsettled or own_plans or plans
    if len(chosen) != 1:
        listed = ", ".join(path for path, _owner in chosen) or "none"
        # A plan main lacks while the fork point had it was archived on main.
        archived = [
            path
            for path, _owner in chosen
            if read_main(path) is None and read_base(path) is not None
        ]
        pending = [
            unit
            for path, _owner in chosen
            if path not in archived
            for unit in open_units(texts[path], path, read_main(path))
            - closed_units(read_main(path), path)
            if unit != branch_unit
        ]
        raise LandingStop(
            "preflight",
            f"the branch must change exactly one active plan; found {len(chosen)}: {listed}"
            + "".join(f"; {path} is no longer under active/ on origin/main" for path in archived)
            + dependency_hint(sorted(pending), stacked_on),
        )
    plan_path, owner = chosen[0]
    table = ledger_table(texts[plan_path], plan_path, "preflight")
    main_text = read_main(plan_path)
    open_rows = changed_open_rows(table, main_text, plan_path)
    landed = closed_units(main_text, plan_path)
    own = [cells for cells in open_rows if row_unit(cells) == branch_unit]
    if own and branch_unit in landed:
        candidates = own
    else:
        candidates = [cells for cells in open_rows if row_unit(cells) not in landed] or open_rows
    if len(candidates) != 1:
        units = [row_unit(cells) for cells in candidates]
        # A row open on main as well is another unit's in-flight row, which
        # the branch changed; it is no dependency waiting to land.
        main_open = set() if main_text is None else open_units(main_text, plan_path)
        edited = [unit for unit in units if unit != branch_unit and unit in main_open]
        pending = [
            unit
            for unit in units
            if unit != branch_unit and unit not in landed and unit not in edited
        ]
        raise LandingStop(
            "preflight",
            f"{plan_path} must have exactly one ledger row that is {OPEN_ROW_RULE}; "
            f"found {len(units)}"
            + (f": {', '.join(units)}" if units else "")
            + "".join(
                f"; the branch changed {unit}, which is open on origin/main as well and "
                "belongs to its own unit"
                for unit in edited
            )
            + dependency_hint(pending, stacked_on),
            plan_path,
        )
    cells = candidates[0]
    unit = row_unit(cells)
    if branch_unit is not None and unit != branch_unit:
        raise LandingStop(
            "preflight",
            f"{plan_path}: the branch is named for {branch_unit}, but its open row is {unit}",
            plan_path,
        )
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
        settled_plans=tuple(path for path, _owner in plans if path != plan_path),
    )


def changed_open_rows(table: LedgerTable, main_text: str | None, label: str) -> list[dict[str, str]]:
    """The open rows of `table` that the branch changed or added.

    A row whose line equals origin/main's (`main_text`) is open there as well,
    such as the author's in-flight work, and is not the branch's.
    """
    main_lines: set[str] = set()
    if main_text is not None:
        main = ledger_table(main_text, f"origin/main's {label}", "preflight")
        main_lines = {main.lines[index].rstrip("\r\n") for index in main.rows}
    return [
        table.cells(index)
        for index in table.rows
        if is_open_row(table.cells(index))
        and table.lines[index].rstrip("\r\n") not in main_lines
    ]


def open_units(text: str, label: str, main_text: str | None = None) -> set[str]:
    """The units whose ledger row in `text` is open, less those equal to `main_text`'s."""
    table = ledger_table(text, label, "preflight")
    return {row_unit(cells) for cells in changed_open_rows(table, main_text, label)}


def closed_units(text: str | None, label: str) -> set[str]:
    """The units whose ledger row in `text`, origin/main's plan, the seal closed."""
    if text is None:
        return set()
    table = ledger_table(text, f"origin/main's {label}", "preflight")
    return {row_unit(cells) for cells in map(table.cells, table.rows) if is_closed_row(cells)}


def dependency_hint(pending: list[str], stacked_on: Callable[[str], bool] | None) -> str:
    """The tail of a preflight stop naming open rows that origin/main has not closed.

    Only the units whose branch the branch is stacked on are named as units to
    land first; other open rows are only named.
    """
    if not pending:
        return ""
    stacked = [unit for unit in pending if stacked_on is not None and stacked_on(unit)]
    hint = f"; origin/main has not closed {', '.join(pending)}"
    if stacked:
        hint += (
            ": a stacked branch lands after the unit it is stacked on, so land "
            f"{', '.join(stacked)} first"
        )
    return hint


def other_unit_record(
    registry: Mapping[str, object], path: str, own: set[str], added: bool
) -> str | None:
    """Which kind of another unit's record `path` is, or None: main's copy settles its conflict.

    When both sides `added` the path, an evidence record directly in an
    owner's `docs/evidence/`, other than its index, is `evidence record`, and
    an inventory at `<owner docs>/inventories/<plan-slug>/` is `inventory`. A
    stacked branch carries its dependency's evidence as the session left it,
    while main's copy has the landing section; an inventory is immutable once
    main tracks it. The unit's `own` records, a record the fork point already
    had (a modify/modify conflict, whose branch edit main's copy would drop)
    and every other path are None.
    """
    if path in own or not added:
        return None
    directory = posixpath.dirname(path)
    for owner in owner_tables(registry):
        if not isinstance(owner.get("active_plans"), str):
            continue
        docs = owner_docs_root(owner)
        if (
            directory == posixpath.join(docs, "evidence")
            and path.endswith(".md")
            and posixpath.basename(path).casefold() != "readme.md"
        ):
            return "evidence record"
        if (
            path.endswith(".numstat.tsv")
            and posixpath.dirname(directory) == posixpath.join(docs, "inventories")
        ):
            return "inventory"
    return None


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


def affected_projects(
    root: Path,
    registry: Mapping[str, object],
    changed_paths: set[str],
    owner_id: str,
    read_base: Callable[[str], bytes | None],
) -> list[dict]:
    """The owner, then every other registered project whose inputs hold a changed path.

    The owner comes first when it is a registered project (a `suite` unit has
    none); the others follow in registry order. Whether a project's inputs
    hold a changed path is production_artifact.py's `inputs_changed`: an
    expanded production input, or a raw production or verification pattern so
    that a deleted file still counts. A shared verification input that each
    project reads only in part, such as `docs/projects.toml` or a debt
    ratchet, counts only when the project's slice of it changed; `read_base`
    returns a path's bytes on the base. `registry` must be the one of the tree
    at `root`: an input it names that does not exist there stops the landing.

    A halted project is never affected, not even as the owner: no landing
    builds, verifies or deploys it (AGENTS.md "Halted projects").
    """
    try:
        halted = {project.id for project in halted_projects(dict(registry))}
    except ValueError as error:
        raise LandingStop(
            "build_if_stale", f"docs/projects.toml: {error}", "docs/projects.toml"
        ) from error
    owner: list[dict] = []
    affected = []
    projects = registry.get("projects", [])
    for project in projects if isinstance(projects, list) else []:
        if not isinstance(project, dict) or project.get("id") in halted:
            continue
        if project.get("id") == owner_id:
            owner.append(project)
            continue
        try:
            changed = inputs_changed(root, dict(registry), project, changed_paths, read_base)
        except ContractError as error:
            raise LandingStop(
                "build_if_stale",
                f"docs/projects.toml: the inputs of {project.get('id')} cannot be "
                f"read on the rebased tip: {error}",
                "docs/projects.toml",
            ) from error
        if changed:
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


def masked_lines(table: LedgerTable, units: set[str]) -> list[tuple[int, str]]:
    """Every line of the table's text but the ledger rows of `units`, with its 1-based number."""
    rows = {index for unit, index in table.unit_rows().items() if unit in units}
    return [(index + 1, line) for index, line in enumerate(table.lines) if index not in rows]


def settled_rows(
    branch: LedgerTable, main: LedgerTable, unit: str | None, base: LedgerTable | None = None
) -> set[str]:
    """The units other than `unit` whose branch row main's plan already holds or supersedes.

    A row equal to main's is main's already. A row that is open on the branch
    and that main closed, with as many cells as the header on both sides and
    every cell the seal does not write equal, is the row of a unit that landed
    as its session left it: a branch stacked on that unit carries it. A row
    the fork point had already closed never qualifies that way, since a branch
    that reopens a landed row changed it. Taking main's row loses nothing in
    either case.
    """
    main_rows = main.unit_rows()
    base_rows = base.unit_rows() if base is not None else {}
    settled = set()
    for other, index in branch.unit_rows().items():
        if other == unit or other not in main_rows:
            continue
        line = branch.lines[index]
        main_line = main.lines[main_rows[other]]
        if line.rstrip("\r\n") == main_line.rstrip("\r\n"):
            settled.add(other)
            continue
        mine, theirs = branch.cells(index), main.cells(main_rows[other])
        if (
            branch.header == main.header
            and len(split_table_row(line)) == len(branch.header)
            and len(split_table_row(main_line)) == len(main.header)
            and not (
                base is not None
                and other in base_rows
                and is_closed_row(base.cells(base_rows[other]))
            )
            and is_open_row(mine)
            and is_closed_row(theirs)
            and all(
                mine.get(column, "").strip() == theirs.get(column, "").strip()
                for column in branch.header
                if column not in SEAL_COLUMNS
            )
        ):
            settled.add(other)
    return settled


def masked_texts(table: LedgerTable, units: set[str]) -> list[str]:
    return [line for _number, line in masked_lines(table, units)]


def plan_settled(base_text: str | None, main_text: str | None, branch_text: str) -> bool:
    """Whether a plan the branch changes holds nothing main lacks.

    That is so when, once the rows `settled_rows` accepts are masked on every
    side, the branch's plan equals main's or the fork point's: every line the
    branch changed is a row main already holds or closed. A plan main lacks,
    or one without a ledger on the branch or on main, is not settled.
    """
    if main_text is None:
        return False
    try:
        branch = ledger_table(branch_text, "the branch's plan", "rebase")
        main = ledger_table(main_text, "main's plan", "rebase")
    except LandingStop:
        return False
    base = None
    if base_text is not None:
        try:
            base = ledger_table(base_text, "the base's plan", "rebase")
        except LandingStop:
            base = None
    settled = settled_rows(branch, main, None, base)
    mine = masked_texts(branch, settled)
    return mine == masked_texts(main, settled) or (
        base is not None and mine == masked_texts(base, settled)
    )


def merge_other_evidence(main_text: str, branch_text: str, path: str) -> str:
    """Main's copy of another unit's evidence record, when the branch's adds nothing to it.

    The seal writes a record as its text without trailing newlines, a blank
    line, then the `## Landing` section, so the branch's copy adds nothing
    when it equals main's without that section. Any other difference is an
    edit of another unit's record, which the landing does not drop.
    """
    marker = "\n\n## Landing\n"
    cut = main_text.rfind(marker)
    if cut >= 0 and branch_text.rstrip("\n") == main_text[:cut]:
        return main_text
    raise LandingStop(
        "rebase",
        f"{path}: the branch edited another unit's evidence record beyond what origin/main "
        f"has, so the landing cannot merge it: drop the edit on the branch, or resolve {path} "
        "in the landing worktree, `git add` it, run `git rebase --continue` there, then "
        "land-unit.py --continue",
        path,
    )


def merge_settled_plan(base_text: str | None, main_text: str, branch_text: str, path: str) -> str:
    """Main's text of another active plan the branch changes, when `plan_settled` holds."""
    if plan_settled(base_text, main_text, branch_text):
        return main_text
    raise LandingStop(
        "rebase",
        f"{path}: the branch changed another unit's plan beyond the rows origin/main "
        f"closed, so the landing cannot merge it: resolve {path} in the landing worktree, "
        "`git add` it, run `git rebase --continue` there, then land-unit.py --continue",
        path,
    )


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

    That is only sound when the branch changed nothing else in the plan that
    main does not already hold, so the merge stops unless the branch's text
    equals the base's once the unit's row and the rows `settled_rows` accepts
    are masked on both sides.
    """
    base = ledger_table(base_text, "the base's plan", "rebase")
    main = ledger_table(main_text, "main's plan", "rebase")
    branch = ledger_table(branch_text, "the branch's plan", "rebase")
    branch_rows = branch.unit_rows()
    if unit not in branch_rows:
        raise LandingStop("rebase", f"the branch's plan has no ledger row {unit}")
    masked = {unit} | settled_rows(branch, main, unit, base)
    difference = first_difference(masked_lines(branch, masked), masked_lines(base, masked))
    if difference is not None:
        row = {index + 1: other for other, index in branch_rows.items()}.get(difference[0])
        raise LandingStop(
            "rebase",
            f"{path}: the branch changed the plan beyond its own row {unit}: the plan differs "
            f"outside that row, first at line {difference[0]} on the branch and line "
            f"{difference[1]} at the fork point"
            + (f" (ledger row {row})" if row is not None else "")
            + ", so the landing cannot merge it: resolve "
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
        previous_unit = row_unit(branch.cells(branch.rows[position - 1]))
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

    It never reads the terminal and keeps stdout and stderr, decoded so that a
    path that is not UTF-8 reaches the caller instead of a traceback (TOOL-19).
    """
    try:
        result = repo_git.run(root, "--literal-pathspecs", *args, program=git_program())
    except GitError as error:
        raise LandingError(str(error)) from error
    return subprocess.CompletedProcess(
        result.args,
        result.returncode,
        repo_git.decode(result.stdout),
        repo_git.decode(result.stderr),
    )


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
        disk = root / path
        if tracked:
            output = git_output(root, "diff", "--numstat", "--no-renames", base, "--", path)
            added, deleted = numstat_values(output, path)
        elif os.path.islink(disk):
            # A new symbolic link is one line to Git (its target). `diff
            # --no-index` would follow a link to a directory and compare the
            # directory instead, reporting nothing, so the row is written
            # without asking Git.
            added, deleted = "1", "0"
        else:
            output = git_output(
                root, "diff", "--no-index", "--numstat", os.devnull, path, allowed=(0, 1)
            )
            added, deleted = numstat_values(output, path)
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
    values = dict(
        zip(
            SEAL_COLUMNS,
            ("done", f"[inventory]({inventory_link})", diffstat, f"[evidence]({evidence_link})"),
            strict=True,
        )
    )
    for column, value in values.items():
        cells[table.header.index(column)] = value
    lines = list(table.lines)
    lines[index] = render_row(cells)
    return "".join(lines)


def landing_section(
    base: str,
    check_output: str,
    build: str | None,
    *,
    deploy: str | None = None,
    red_ci: str | None = None,
) -> str:
    """The evidence's Landing section; `deploy` and `red_ci` add a line each."""
    lines = [
        "## Landing\n\n",
        f"- **Base revision:** `{base}`\n",
        f"- **Check:** {check_output}\n",
        f"- **Build:** {build if build is not None else 'artifact current; no build'}\n",
    ]
    if deploy is not None:
        lines.append(f"- **Deploy:** after the push: {deploy}\n")
    if red_ci is not None:
        lines.append(
            f"- **CI:** GitHub `contracts` had failed on main ({red_ci}); "
            "landed with `--accept-red-ci`\n"
        )
    return "".join(lines)
