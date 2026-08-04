#!/usr/bin/env python3
"""Require the Git index to equal the selected delivery inventory batch."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import subprocess
import sys
import tomllib
import types
from urllib.parse import unquote, urlsplit


BASE_RE = re.compile(r"Base revision\t([0-9a-f]{40})")
HASH_RE = re.compile(r"[0-9a-f]{64}")
HEADER = "added\tdeleted\tcontent\tpath"


class StagedUnitError(RuntimeError):
    pass


HEAD_PROJECT_RULES = "scripts/project_registry.py"
HEAD_DOCUMENTATION_RULES = "scripts/documentation_contract.py"
DOCUMENTATION_RULE_NAMES = (
    "CHECKPOINT_RE",
    "HEADING_RE",
    "LEDGER_COLUMNS",
    "extract_inline_links",
    "is_separator_row",
    "markdown_headings",
    "normalized_heading",
    "normalized_status",
    "split_table_row",
)


def git(root: Path, *args: str, check: bool = True) -> bytes:
    process = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and process.returncode != 0:
        detail = process.stderr.decode("utf-8", "replace").strip()
        raise StagedUnitError(
            f"git {' '.join(args)} failed" + (f": {detail}" if detail else "")
        )
    return process.stdout


def committed_python_module(
    root: Path,
    path: str,
    module_name: str,
) -> types.ModuleType:
    raw = git(root, "show", f"HEAD:{path}", check=False)
    if not raw:
        raise StagedUnitError(f"committed interpretation rule is missing: {path}")
    try:
        source = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise StagedUnitError(
            f"committed interpretation rule is not UTF-8: {path}: {error}"
        ) from error

    module = types.ModuleType(module_name)
    module.__file__ = f"HEAD:{path}"
    module.__package__ = None
    sys.modules[module_name] = module
    try:
        exec(compile(source, module.__file__, "exec"), module.__dict__)
    except SystemExit as error:
        raise StagedUnitError(
            f"committed interpretation rule attempted to exit: {path}: {error.code!r}"
        ) from error
    except Exception as error:
        raise StagedUnitError(
            f"could not load committed interpretation rule: {path}: {error}"
        ) from error
    return module


def install_committed_documentation_rules(root: Path) -> None:
    """Install only the ledger parsers committed in HEAD.

    The staged-unit guard reads delivery data from INDEX and may inspect the
    worktree only to fail conservatively on omitted partial-plan changes. No
    unstaged or staged Python module defines what counts as an inventory.
    """

    previous_project_registry = sys.modules.get("project_registry")
    try:
        committed_python_module(root, HEAD_PROJECT_RULES, "project_registry")
        documentation = committed_python_module(
            root,
            HEAD_DOCUMENTATION_RULES,
            "_celestina_head_documentation_contract",
        )
    finally:
        if previous_project_registry is None:
            sys.modules.pop("project_registry", None)
        else:
            sys.modules["project_registry"] = previous_project_registry

    for name in DOCUMENTATION_RULE_NAMES:
        value = getattr(documentation, name, None)
        if value is None:
            raise StagedUnitError(
                f"{HEAD_DOCUMENTATION_RULES} does not expose required rule {name}"
            )
        globals()[name] = value


def decode_paths(raw: bytes) -> list[str]:
    return [os.fsdecode(part) for part in raw.split(b"\0") if part]


def normalized_repo_path(raw: str) -> str:
    path = PurePosixPath(raw)
    if (
        not raw
        or path.is_absolute()
        or str(path) != raw
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise StagedUnitError(f"non-normalized inventory path: {raw}")
    return raw


def index_bytes(root: Path, path: str) -> bytes | None:
    process = subprocess.run(
        ["git", "-C", str(root), "show", f":{path}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return process.stdout if process.returncode == 0 else None


def path_exists_at_head(root: Path, path: str) -> bool:
    process = subprocess.run(
        ["git", "-C", str(root), "cat-file", "-e", f"HEAD:{path}"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return process.returncode == 0


def worktree_bytes(root: Path, path: str) -> bytes | None:
    candidate = root / normalized_repo_path(path)
    try:
        resolved = candidate.resolve(strict=False)
        resolved.relative_to(root.resolve())
    except (OSError, ValueError) as error:
        raise StagedUnitError(f"plan path leaves the repository: {path}") from error
    if candidate.is_symlink():
        raise StagedUnitError(f"a canonical plan cannot be a symlink: {path}")
    try:
        return candidate.read_bytes()
    except FileNotFoundError:
        return None
    except OSError as error:
        raise StagedUnitError(f"could not read the worktree plan: {path}: {error}") from error


def inventory_references(text: str, plan_path: str) -> set[str]:
    references: set[str] = set()
    parent = posixpath.dirname(plan_path)
    for _line, raw_target in extract_inline_links(text):
        target = unquote(urlsplit(raw_target).path)
        if not target.endswith(".numstat.tsv"):
            continue
        normalized = posixpath.normpath(posixpath.join(parent, target))
        if normalized == ".." or normalized.startswith("../"):
            raise StagedUnitError(
                f"inventory reference leaves the repository: {plan_path} -> {target}"
            )
        references.add(normalized_repo_path(normalized))
    return references


def registry_delivery_layouts(
    root: Path,
    revision: str,
) -> dict[str, tuple[str, str, str]]:
    if revision == "INDEX":
        raw = index_bytes(root, "docs/projects.toml")
    elif revision == "HEAD":
        raw = git(root, "show", "HEAD:docs/projects.toml", check=False) or None
    else:
        raise StagedUnitError(f"unknown registry revision: {revision}")
    if raw is None:
        raise StagedUnitError(f"{revision}:docs/projects.toml: registry is missing")
    try:
        registry = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise StagedUnitError(
            f"{revision}:docs/projects.toml: invalid registry: {error}"
        ) from error

    suite = registry.get("suite")
    projects = registry.get("projects")
    if registry.get("schema_version") != 1:
        raise StagedUnitError(
            f"{revision}:docs/projects.toml requires schema_version = 1"
        )
    if not isinstance(suite, dict):
        raise StagedUnitError(
            f"{revision}:docs/projects.toml does not contain [suite]"
        )
    if not isinstance(projects, list):
        raise StagedUnitError(
            f"{revision}:docs/projects.toml does not contain [[projects]]"
        )

    owners: list[object] = [suite, *projects]
    layouts: dict[str, tuple[str, str, str]] = {}
    for index, owner in enumerate(owners):
        if not isinstance(owner, dict):
            raise StagedUnitError(
                f"{revision}:docs/projects.toml owner {index} is not a table"
            )
        owner_id = owner.get("id")
        prefix = owner.get("commit_prefix")
        active = owner.get("active_plans")
        if not all(isinstance(value, str) and value for value in (owner_id, prefix, active)):
            raise StagedUnitError(
                f"{revision}:docs/projects.toml owner {index} lacks id, prefix, or active plans"
            )
        active = normalized_repo_path(active)
        docs_root = posixpath.dirname(posixpath.dirname(active))
        inventory_root = posixpath.join(docs_root, "inventories")
        for plan_directory in (
            active,
            posixpath.join(posixpath.dirname(active), "archive"),
        ):
            if plan_directory in layouts:
                raise StagedUnitError(
                    f"plan directory registered by multiple owners: {plan_directory}"
                )
            layouts[plan_directory] = (owner_id, prefix, inventory_root)
    return layouts


def delivery_layouts(root: Path) -> dict[str, tuple[str, str, str]]:
    """Return the conservative union of committed and staged delivery roots."""

    committed = registry_delivery_layouts(root, "HEAD")
    staged = registry_delivery_layouts(root, "INDEX")
    layouts = dict(committed)
    for directory, layout in staged.items():
        previous = layouts.get(directory)
        if previous is not None and previous != layout:
            raise StagedUnitError(
                "HEAD and INDEX assign one plan directory to conflicting owners: "
                f"{directory}: {previous[0]}/{previous[1]} vs {layout[0]}/{layout[1]}"
            )
        layouts[directory] = layout
    return layouts


def is_plan(path: str, directories: set[str]) -> bool:
    return (
        path.endswith(".md")
        and posixpath.basename(path).casefold() != "readme.md"
        and posixpath.dirname(path) in directories
    )


def is_inventory(path: str, directories: set[str]) -> bool:
    if not path.endswith(".numstat.tsv"):
        return False
    return any(
        path.startswith(f"{directory}/")
        and "/" in path[len(directory) + 1 :]
        for directory in directories
    )


def ledger_inventory_records(
    text: str,
    plan_path: str,
    owner_id: str,
    owner_prefix: str,
    inventory_root: str,
) -> dict[str, list[tuple[str, str, str, str]]]:
    records: dict[str, list[tuple[str, str, str, str]]] = {}
    ledger_lines = [
        line_number
        for _level, heading, line_number in markdown_headings(text)
        if normalized_heading(heading) == "change and commit ledger"
    ]
    if len(ledger_lines) != 1:
        return records
    lines = text.splitlines()
    cursor = ledger_lines[0]
    while cursor < len(lines) and not lines[cursor].strip():
        cursor += 1
    while cursor < len(lines) and not lines[cursor].lstrip().startswith("|"):
        if HEADING_RE.match(lines[cursor]):
            return records
        cursor += 1
    if cursor + 1 >= len(lines):
        return records
    header_cells = split_table_row(lines[cursor])
    header = [" ".join(cell.casefold().split()) for cell in header_cells]
    if not is_separator_row(split_table_row(lines[cursor + 1])):
        return records
    required = set(LEDGER_COLUMNS)
    if not required.issubset(header):
        return records
    indexes = {column: header.index(column) for column in required}
    cursor += 2
    while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
        cells = split_table_row(lines[cursor])
        cursor += 1
        if len(cells) != len(header) or normalized_status(cells[indexes["status"]]) != "done":
            continue
        unit = cells[indexes["unit"]].strip("` ")
        if CHECKPOINT_RE.fullmatch(unit) is None:
            raise StagedUnitError(f"done row has an invalid unit in {plan_path}: {unit}")
        raw_prefix = cells[indexes["commit prefix"]].strip().strip("`").strip()
        commit_prefix = raw_prefix[:-1] if raw_prefix.endswith(":") else raw_prefix
        for inventory_path in inventory_references(
            cells[indexes["files / areas"]], plan_path
        ):
            records.setdefault(inventory_path, []).append(
                (plan_path, unit, commit_prefix, owner_id)
            )
            expected = posixpath.join(
                inventory_root,
                PurePosixPath(plan_path).stem,
                f"{unit}.numstat.tsv",
            )
            if inventory_path != expected:
                raise StagedUnitError(
                    f"done row {unit} must link its canonical path {expected}: "
                    f"{inventory_path}"
                )
            if commit_prefix != owner_prefix:
                raise StagedUnitError(
                    f"done row {unit} for {owner_id} requires prefix "
                    f"{owner_prefix}: {commit_prefix}"
                )
    return records


def staged_ledger_records(
    root: Path,
    staged_paths: set[str],
    layouts: dict[str, tuple[str, str, str]],
) -> dict[str, list[tuple[str, str, str, str]]]:
    records: dict[str, list[tuple[str, str, str, str]]] = {}
    for plan_path in sorted(
        path for path in staged_paths if posixpath.dirname(path) in layouts
    ):
        if not is_plan(plan_path, set(layouts)):
            continue
        staged = index_bytes(root, plan_path)
        if staged is None:
            continue
        try:
            text = staged.decode("utf-8")
        except UnicodeDecodeError as error:
            raise StagedUnitError(f"staged plan is not UTF-8: {plan_path}: {error}") from error
        owner_id, owner_prefix, inventory_root = layouts[posixpath.dirname(plan_path)]
        for inventory_path, candidates in ledger_inventory_records(
            text,
            plan_path,
            owner_id,
            owner_prefix,
            inventory_root,
        ).items():
            records.setdefault(inventory_path, []).extend(candidates)
    return records


def newly_referenced_inventories(
    root: Path, staged_paths: set[str], directories: set[str]
) -> set[str]:
    references: set[str] = set()
    for plan_path in sorted(path for path in staged_paths if is_plan(path, directories)):
        staged = index_bytes(root, plan_path)
        if staged is None:
            continue
        try:
            staged_text = staged.decode("utf-8")
        except UnicodeDecodeError as error:
            raise StagedUnitError(f"staged plan is not UTF-8: {plan_path}: {error}") from error
        base = git(root, "show", f"HEAD:{plan_path}", check=False)
        try:
            base_text = base.decode("utf-8") if base else ""
        except UnicodeDecodeError:
            base_text = ""
        references.update(
            inventory_references(staged_text, plan_path)
            - inventory_references(base_text, plan_path)
        )
    return references


def unstaged_new_inventory_references(
    root: Path, staged_paths: set[str], directories: set[str]
) -> dict[str, set[str]]:
    """Find new worktree inventory links omitted from a partially staged plan."""
    omitted: dict[str, set[str]] = {}
    for plan_path in sorted(path for path in staged_paths if is_plan(path, directories)):
        staged = index_bytes(root, plan_path)
        if staged is None:
            continue
        worktree = worktree_bytes(root, plan_path)
        if worktree is None:
            raise StagedUnitError(
                f"staged host plan does not exist in the worktree: {plan_path}"
            )
        try:
            staged_text = staged.decode("utf-8")
            worktree_text = worktree.decode("utf-8")
        except UnicodeDecodeError as error:
            raise StagedUnitError(
                f"staged/worktree plan is not UTF-8: {plan_path}: {error}"
            ) from error
        staged_references = inventory_references(staged_text, plan_path)
        missing = inventory_references(worktree_text, plan_path) - staged_references
        if missing:
            omitted[plan_path] = missing
    return omitted


def staged_inventory_hosts(
    root: Path, staged_paths: set[str], directories: set[str]
) -> dict[str, set[str]]:
    hosts: dict[str, set[str]] = {}
    for plan_path in sorted(path for path in staged_paths if is_plan(path, directories)):
        staged = index_bytes(root, plan_path)
        if staged is None:
            continue
        try:
            text = staged.decode("utf-8")
        except UnicodeDecodeError as error:
            raise StagedUnitError(f"staged plan is not UTF-8: {plan_path}: {error}") from error
        for inventory_path in inventory_references(text, plan_path):
            hosts.setdefault(inventory_path, set()).add(plan_path)
    return hosts


def parse_inventory(
    root: Path, inventory_path: str, head: str
) -> list[tuple[str, str, str, str, str]]:
    raw = index_bytes(root, inventory_path)
    if raw is None:
        raise StagedUnitError(f"inventory is not staged: {inventory_path}")
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise StagedUnitError(
            f"staged inventory is not UTF-8: {inventory_path}: {error}"
        ) from error

    bases = [match.group(1) for line in lines if (match := BASE_RE.fullmatch(line))]
    if len(bases) != 1:
        raise StagedUnitError(f"inventory requires one Base revision: {inventory_path}")
    if bases[0] != head:
        raise StagedUnitError(
            f"staged Base revision must be HEAD {head}: {inventory_path} declares {bases[0]}"
        )
    headers = [index for index, line in enumerate(lines) if line == HEADER]
    if len(headers) != 1:
        raise StagedUnitError(f"inventory requires one numstat header: {inventory_path}")

    rows: list[tuple[str, str, str, str, str]] = []
    seen: set[str] = set()
    self_rows = 0
    for line in lines[headers[0] + 1 :]:
        if not line:
            continue
        cells = line.split("\t")
        if len(cells) != 4:
            raise StagedUnitError(f"invalid staged row in {inventory_path}: {line}")
        added, deleted, content, path = cells
        normalized_repo_path(path)
        if path in seen:
            raise StagedUnitError(f"duplicate path in {inventory_path}: {path}")
        seen.add(path)
        values_are_lines = added.isdecimal() and deleted.isdecimal()
        values_are_binary = added == "-" and deleted == "-"
        if not values_are_lines and not values_are_binary:
            raise StagedUnitError(f"invalid numstat in {inventory_path}: {path}")
        if content == "self":
            self_rows += 1
            if path != inventory_path:
                raise StagedUnitError(f"self row does not belong to {inventory_path}: {path}")
        elif content != "deleted" and HASH_RE.fullmatch(content) is None:
            raise StagedUnitError(f"invalid content in {inventory_path}: {path}")
        rows.append((inventory_path, added, deleted, content, path))
    if self_rows != 1:
        raise StagedUnitError(f"inventory requires one self row: {inventory_path}")
    return rows


def actual_numstat(root: Path, path: str) -> tuple[str, str]:
    output = git(root, "diff", "--cached", "--numstat", "--no-renames", "HEAD", "--", path)
    lines = [line for line in output.splitlines() if line]
    if len(lines) != 1:
        raise StagedUnitError(f"Git did not return one staged numstat row: {path}")
    cells = lines[0].split(b"\t", maxsplit=2)
    if len(cells) != 3:
        raise StagedUnitError(f"Git returned an invalid staged numstat row: {path}")
    return cells[0].decode("ascii"), cells[1].decode("ascii")


def validate_batch(
    root: Path,
    inventories: list[str],
    quiet: bool,
    commit_prefix: str | None = None,
    forbid_delivery: bool = False,
) -> None:
    staged_paths = set(
        decode_paths(git(root, "diff", "--cached", "--name-only", "--no-renames", "-z"))
    )
    layouts = delivery_layouts(root)
    plan_dirs = set(layouts)
    inventory_dirs = {layout[2] for layout in layouts.values()}
    partially_staged = unstaged_new_inventory_references(root, staged_paths, plan_dirs)
    if partially_staged:
        rendered = "; ".join(
            f"{plan}: {', '.join(sorted(references))}"
            for plan, references in sorted(partially_staged.items())
        )
        raise StagedUnitError(
            "host plan leaves done inventories outside the index: " + rendered
        )

    new_references = newly_referenced_inventories(root, staged_paths, plan_dirs)
    hosts = staged_inventory_hosts(root, staged_paths, plan_dirs)
    ledger_records = staged_ledger_records(root, staged_paths, layouts)
    noncanonical_references = sorted(
        reference
        for reference in hosts
        if not is_inventory(reference, inventory_dirs)
    )
    if noncanonical_references:
        raise StagedUnitError(
            "plan links inventories outside the owner's stable root: "
            + ", ".join(noncanonical_references)
        )
    missing_references = sorted(
        reference
        for reference in new_references - staged_paths
        if not path_exists_at_head(root, reference)
    )
    if missing_references:
        raise StagedUnitError(
            "staged plan references new inventories that are not staged: "
            + ", ".join(missing_references)
        )

    all_staged_inventories = {
        path for path in staged_paths if is_inventory(path, inventory_dirs)
    }
    deleted_plans = sorted(
        path
        for path in staged_paths
        if is_plan(path, plan_dirs)
        and index_bytes(root, path) is None
        and path_exists_at_head(root, path)
    )
    immutable = sorted(
        path for path in all_staged_inventories if path_exists_at_head(root, path)
    )
    if immutable:
        raise StagedUnitError(
            "historical inventories are immutable; create a new unit and inventory: "
            + ", ".join(immutable)
        )
    staged_inventories = {
        path for path in all_staged_inventories if index_bytes(root, path) is not None
    }
    deleted_inventories = all_staged_inventories - staged_inventories
    if inventories:
        selected = {normalized_repo_path(path) for path in inventories}
        noncanonical = sorted(
            path for path in selected if not is_inventory(path, inventory_dirs)
        )
        if noncanonical:
            raise StagedUnitError(
                "selected inventories are outside canonical plans: "
                + ", ".join(noncanonical)
            )
        unavailable = sorted(path for path in selected if index_bytes(root, path) is None)
        if unavailable:
            raise StagedUnitError(
                "selected inventories do not exist in the index: "
                + ", ".join(unavailable)
            )
        omitted = sorted(staged_inventories - selected)
        if omitted:
            raise StagedUnitError(
                "staged inventories were not selected: " + ", ".join(omitted)
            )
    else:
        selected = staged_inventories
    if selected and forbid_delivery:
        raise StagedUnitError(
            "a merge cannot close delivery units; commit the inventoried batch "
            "separately"
        )
    if not selected:
        if deleted_plans:
            raise StagedUnitError(
                "deleting or archiving a plan requires an administrative unit "
                "with a new inventory: " + ", ".join(deleted_plans)
            )
        if deleted_inventories:
            raise StagedUnitError(
                "deleted inventories require a destination inventory that claims "
                "their path as deleted: " + ", ".join(sorted(deleted_inventories))
            )
        if not quiet:
            print("staged-unit: no staged unit closure")
        return

    head = git(root, "rev-parse", "HEAD").decode("ascii").strip()
    claims: dict[str, list[tuple[str, str, str, str]]] = {}
    inventory_paths: dict[str, set[str]] = {}
    for inventory_path in sorted(selected):
        rows = parse_inventory(
            root, inventory_path, head
        )
        inventory_paths[inventory_path] = {path for _s, _a, _d, _c, path in rows}
        for source, added, deleted, content, path in rows:
            claims.setdefault(path, []).append((source, added, deleted, content))

    inventory_host: dict[str, str] = {}
    required_prefixes: set[str] = set()
    for inventory_path in sorted(selected):
        candidates = ledger_records.get(inventory_path, [])
        if len(candidates) != 1:
            rendered = ", ".join(
                sorted(f"{plan}#{unit}" for plan, unit, _prefix, _owner in candidates)
            ) or "none"
            raise StagedUnitError(
                f"staged inventory requires exactly one host done row: {inventory_path} "
                f"({rendered})"
            )
        inventory_host[inventory_path] = candidates[0][0]
        required_prefixes.add(candidates[0][2])

    if len(required_prefixes) != 1:
        raise StagedUnitError(
            "an inventory batch requires exactly one commit prefix: "
            + ", ".join(sorted(required_prefixes))
        )
    required_prefix = next(iter(required_prefixes))
    if commit_prefix is not None and commit_prefix != required_prefix:
        raise StagedUnitError(
            f"the batch requires subject `{required_prefix}:`, not `{commit_prefix}:`"
        )

    ordered = sorted(selected)
    for index, left in enumerate(ordered):
        for right in ordered[index + 1 :]:
            shared = inventory_paths[left].intersection(inventory_paths[right])
            if inventory_host[left] == inventory_host[right]:
                shared.discard(inventory_host[left])
            if shared:
                raise StagedUnitError(
                    f"staged inventories overlap: {left}, {right}: "
                    + ", ".join(sorted(shared))
                )

    claimed_paths = set(claims)
    missing = sorted(staged_paths - claimed_paths)
    extra = sorted(claimed_paths - staged_paths)
    if missing:
        raise StagedUnitError(
            "staging contains paths outside the inventoried batch: " + ", ".join(missing)
        )
    if extra:
        raise StagedUnitError(
            "inventory contains paths that are not staged: " + ", ".join(extra)
        )

    for path, rows in sorted(claims.items()):
        actual_added, actual_deleted = actual_numstat(root, path)
        staged = index_bytes(root, path)
        for source, expected_added, expected_deleted, content in rows:
            if (expected_added, expected_deleted) != (actual_added, actual_deleted):
                raise StagedUnitError(
                    f"staged numstat mismatch for {path} in {source}: "
                    f"inventory {expected_added}/{expected_deleted}, "
                    f"Git {actual_added}/{actual_deleted}"
                )
            if content == "self":
                continue
            if content == "deleted":
                if staged is not None:
                    raise StagedUnitError(f"{path} declares deleted but exists in the index")
                continue
            if staged is None:
                raise StagedUnitError(f"{path} does not exist in the index")
            digest = hashlib.sha256(staged).hexdigest()
            if digest != content:
                raise StagedUnitError(
                    f"staged SHA-256 mismatch for {path} in {source}: "
                    f"inventory {content}, index {digest}"
                )
    if not quiet:
        print(
            f"staged-unit: OK ({len(selected)} inventory file(s), "
            f"{len(staged_paths)} path(s))"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inventories", nargs="*")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--commit-prefix")
    parser.add_argument("--forbid-delivery", action="store_true")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        install_committed_documentation_rules(root)
        validate_batch(
            root,
            args.inventories,
            args.quiet,
            args.commit_prefix,
            args.forbid_delivery,
        )
    except StagedUnitError as error:
        print(f"staged-unit: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
