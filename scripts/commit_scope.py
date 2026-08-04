#!/usr/bin/env python3
"""Validate a commit subject and its paths against docs/projects.toml."""

from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
import tomllib
import types
from collections.abc import Callable
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
ARCHITECTURE_RATCHET = "scripts/architecture-baseline.tsv"
LANGUAGE_RATCHET = "scripts/language-baseline.tsv"
ARCHITECTURE_RESOLUTION_FIELD = "- **Resolved architecture debt:** `{}`"
# The language ratchet's counterpart, and a narrower one. Architecture debt is
# resolved per source; language debt can also fall because the *measuring rule*
# changed — an exemption the scanner did not make before — and then no source
# earned the reduction and none can be staged with it. That is the only case
# this field covers, which is why it names the scanner rather than a file.
LANGUAGE_MIGRATION_FIELD = "- **Resolved language debt:** `{}`"
PROJECT_REGISTRY = "scripts/project_registry.py"
ARCHITECTURE_SCANNER = "scripts/architecture_scanners.py"
LANGUAGE_SCANNER = "scripts/check-language-contract.py"
VERSION_CONTRACT = "scripts/version_contract.py"
REGISTRY = "docs/projects.toml"

# The hook intentionally uses a finite, broad vocabulary instead of pretending
# to solve English grammar. The first action word must be an unambiguous command
# verb; history replay has an explicit scope-only mode for inherited subjects.
IMPERATIVE_VERBS = frozenset(
    {
        "absorb",
        "add",
        "adopt",
        "align",
        "allow",
        "apply",
        "archive",
        "audit",
        "avoid",
        "build",
        "bump",
        "change",
        "clarify",
        "clean",
        "close",
        "complete",
        "configure",
        "consolidate",
        "convert",
        "correct",
        "cover",
        "create",
        "delegate",
        "deploy",
        "document",
        "drop",
        "enable",
        "enforce",
        "establish",
        "expand",
        "expose",
        "extend",
        "extract",
        "fix",
        "guard",
        "handle",
        "harden",
        "honor",
        "implement",
        "improve",
        "ignore",
        "integrate",
        "introduce",
        "inspect",
        "isolate",
        "keep",
        "limit",
        "lower",
        "make",
        "mark",
        "merge",
        "migrate",
        "move",
        "normalize",
        "prevent",
        "prepare",
        "preserve",
        "protect",
        "publish",
        "read",
        "rebuild",
        "record",
        "reduce",
        "refactor",
        "refresh",
        "register",
        "reject",
        "remove",
        "rename",
        "replace",
        "republish",
        "require",
        "resolve",
        "restore",
        "retire",
        "review",
        "reuse",
        "rework",
        "route",
        "run",
        "set",
        "simplify",
        "split",
        "standardize",
        "support",
        "synchronize",
        "test",
        "trace",
        "track",
        "translate",
        "unify",
        "update",
        "use",
        "validate",
        "verify",
        "wire",
    }
)
NON_ENGLISH_TERMS = (
    "actualiza",
    "agente",
    "al",
    "archivo",
    "archivos",
    "arregla",
    "cambio",
    "cambios",
    "con",
    "de",
    "del",
    "desplegar",
    "el",
    "en",
    "estado",
    "la",
    "las",
    "los",
    "para",
    "por",
    "proyecto",
    "prueba",
    "pruebas",
    "que",
    "regla",
    "ruta",
    "sin",
    "un",
    "una",
    "unos",
    "unas",
    "verificar",
    "vista",
    "vistas",
)
NON_ENGLISH_ACTION = re.compile(
    r"[\u00e1\u00e9\u00ed\u00f3\u00fa\u00fc\u00f1"
    r"\u00c1\u00c9\u00cd\u00d3\u00da\u00dc\u00d1\u00bf\u00a1]"
    + r"|\b(?:"
    + "|".join(NON_ENGLISH_TERMS)
    + r")\b",
    re.IGNORECASE,
)


def fail(message: str) -> "None":
    print(f"commit-msg: {message}", file=sys.stderr)
    raise SystemExit(1)


def load_scopes(
    root: Path,
    *,
    registry_source: str,
    rule_namespace: dict[str, object],
) -> tuple[dict[str, Any], dict[str, object], dict[str, Any]]:
    if registry_source not in {"HEAD", "INDEX"}:
        fail(f"unknown registry source: {registry_source}")
    raw = git_blob(root, registry_source, REGISTRY)
    if raw is None:
        fail(f"{registry_source}:{REGISTRY}: registry is missing")
    try:
        data = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        fail(f"{registry_source}:{REGISTRY}: invalid registry: {error}")
    namespace = rule_namespace

    build_scopes = namespace.get("build_commit_scopes")
    if not callable(build_scopes):
        fail(f"{PROJECT_REGISTRY} does not expose build_commit_scopes")
    try:
        scopes = call_dynamic_rule(
            f"HEAD:{PROJECT_REGISTRY} build_commit_scopes over {registry_source}:{REGISTRY}",
            build_scopes,
            data,
        )
    except (KeyError, TypeError, ValueError) as error:
        fail(str(error))
    if not isinstance(scopes, dict):
        fail(f"{PROJECT_REGISTRY} returned invalid commit scopes")
    return scopes, namespace, data


def git_output(root: Path, *args: str, check: bool = True) -> bytes:
    process = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if check and process.returncode != 0:
        fail(f"could not run git {' '.join(args)}")
    return process.stdout


def is_merge(root: Path) -> bool:
    return subprocess.run(
        ["git", "-C", str(root), "rev-parse", "-q", "--verify", "MERGE_HEAD"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0


def staged_paths(root: Path) -> list[str]:
    raw = git_output(root, "diff", "--cached", "--name-only", "--no-renames", "-z")
    return [part.decode("utf-8", "surrogateescape") for part in raw.split(b"\0") if part]


def stdin_paths() -> list[str]:
    raw = sys.stdin.buffer.read()
    if b"\0" in raw:
        parts = raw.split(b"\0")
    else:
        parts = raw.splitlines()
    return [part.decode("utf-8", "surrogateescape") for part in parts if part]


def git_blob(root: Path, revision: str, path: str) -> bytes | None:
    spec = f":{path}" if revision == "INDEX" else f"{revision}:{path}"
    process = subprocess.run(
        ["git", "-C", str(root), "show", spec],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return process.stdout if process.returncode == 0 else None


def python_namespace(raw: bytes, source: str, path: str) -> dict[str, object]:
    try:
        text_source = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        fail(f"{source}: interpretation rule is not UTF-8: {error}")

    digest = hashlib.sha256(raw).hexdigest()[:16]
    module_name = f"_celestina_index_{Path(path).stem}_{digest}"
    module = types.ModuleType(module_name)
    module.__file__ = source
    module.__package__ = None
    sys.modules[module_name] = module
    try:
        exec(compile(text_source, source, "exec"), module.__dict__)
    except SystemExit as error:
        fail(f"{source}: interpretation module attempted to exit with {error.code!r}")
    except Exception as error:
        fail(f"{source}: could not load interpretation rule: {error}")
    return module.__dict__


def call_dynamic_rule(label: str, rule: Callable[..., Any], *args: object) -> Any:
    try:
        return rule(*args)
    except SystemExit as error:
        fail(f"{label} attempted to exit with {error.code!r}")


def revision_python_namespace(
    root: Path, revision: str, path: str
) -> dict[str, object]:
    raw = git_blob(root, revision, path)
    if raw is None:
        fail(f"{revision}:{path}: interpretation rule is missing")
    return python_namespace(raw, f"{revision}:{path}", path)


def head_python_namespace(root: Path, path: str) -> dict[str, object]:
    return revision_python_namespace(root, "HEAD", path)


def parse_architecture_ratchet(raw: bytes, source: str) -> dict[tuple[str, str], int]:
    result: dict[tuple[str, str], int] = {}
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        fail(f"{source}: architecture ratchet is not UTF-8: {error}")
    for number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if (
            len(fields) != 3
            or fields[0] not in {"lines", "control"}
            or not fields[2].isdigit()
            or int(fields[2]) <= 0
        ):
            fail(f"{source}:{number}: invalid architecture ratchet row")
        key = (fields[0], fields[1])
        if key in result:
            fail(f"{source}:{number}: duplicate architecture ratchet row")
        result[key] = int(fields[2])
    return result


def parse_language_ratchet(raw: bytes, source: str) -> dict[str, int]:
    result: dict[str, int] = {}
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        fail(f"{source}: language ratchet is not UTF-8: {error}")
    for number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 2 or not fields[0].isdigit() or int(fields[0]) <= 0:
            fail(f"{source}:{number}: invalid language ratchet row")
        if fields[1] in result:
            fail(f"{source}:{number}: duplicate language ratchet row")
        result[fields[1]] = int(fields[0])
    return result


def index_mode(root: Path, path: str) -> str | None:
    process = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--stage", "--", path],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    if process.returncode != 0 or not process.stdout.strip():
        return None
    return process.stdout.split(maxsplit=1)[0]


def changed_keys(before: dict[object, int], after: dict[object, int]) -> set[object]:
    return {key for key in set(before) | set(after) if before.get(key) != after.get(key)}


def has_staged_architecture_resolution(
    root: Path,
    source: str,
    prefix: str,
    staged: set[str],
    registry: dict[str, Any],
    architecture_namespace: dict[str, object],
) -> bool:
    evidence_root_for_prefix = architecture_namespace.get(
        "canonical_evidence_root_for_prefix"
    )
    is_evidence_path = architecture_namespace.get("is_canonical_evidence_path")
    if not callable(evidence_root_for_prefix) or not callable(is_evidence_path):
        fail(f"{ARCHITECTURE_SCANNER} does not expose canonical evidence rules")
    try:
        evidence_root = call_dynamic_rule(
            f"HEAD:{ARCHITECTURE_SCANNER} canonical_evidence_root_for_prefix",
            evidence_root_for_prefix,
            registry,
            prefix,
        )
    except (TypeError, ValueError, RuntimeError) as error:
        fail(f"{REGISTRY}: could not resolve evidence ownership: {error}")
    if evidence_root is None:
        return False

    marker = ARCHITECTURE_RESOLUTION_FIELD.format(source)
    for path in sorted(staged):
        if not call_dynamic_rule(
            f"HEAD:{ARCHITECTURE_SCANNER} is_canonical_evidence_path",
            is_evidence_path,
            path,
            (evidence_root,),
        ):
            continue
        if index_mode(root, path) not in {"100644", "100755"}:
            continue
        raw = git_blob(root, "INDEX", path)
        if raw is None:
            continue
        try:
            lines = raw.decode("utf-8").splitlines()
        except UnicodeDecodeError:
            continue
        if marker in (line.strip() for line in lines):
            return True
    return False


def has_staged_language_migration(
    root: Path,
    prefix: str,
    staged: set[str],
    registry: dict[str, Any],
    architecture_namespace: dict[str, object],
) -> bool:
    """Whether this commit is an accepted language-scanner migration.

    Two things must be true together, and neither alone is enough. The scanner
    itself has to change in this commit, because a measurement can only move
    without a source when the rule doing the measuring moved. And the unit's
    evidence has to say so in the exact declared field, so the reduction is
    something somebody wrote down rather than something that merely happened.
    """
    if LANGUAGE_SCANNER not in staged:
        return False

    evidence_root_for_prefix = architecture_namespace.get(
        "canonical_evidence_root_for_prefix"
    )
    is_evidence_path = architecture_namespace.get("is_canonical_evidence_path")
    if not callable(evidence_root_for_prefix) or not callable(is_evidence_path):
        fail(f"{ARCHITECTURE_SCANNER} does not expose canonical evidence rules")
    try:
        evidence_root = call_dynamic_rule(
            f"HEAD:{ARCHITECTURE_SCANNER} canonical_evidence_root_for_prefix",
            evidence_root_for_prefix,
            registry,
            prefix,
        )
    except (TypeError, ValueError, RuntimeError) as error:
        fail(f"{REGISTRY}: could not resolve evidence ownership: {error}")
    if evidence_root is None:
        return False

    marker = LANGUAGE_MIGRATION_FIELD.format(LANGUAGE_SCANNER)
    for path in sorted(staged):
        if not call_dynamic_rule(
            f"HEAD:{ARCHITECTURE_SCANNER} is_canonical_evidence_path",
            is_evidence_path,
            path,
            (evidence_root,),
        ):
            continue
        if index_mode(root, path) not in {"100644", "100755"}:
            continue
        raw = git_blob(root, "INDEX", path)
        if raw is None:
            continue
        try:
            lines = raw.decode("utf-8").splitlines()
        except UnicodeDecodeError:
            continue
        if marker in (line.strip() for line in lines):
            return True
    return False


def architecture_value(
    raw: bytes,
    kind: str,
    key: str,
    source: str,
    architecture_namespace: dict[str, object],
) -> int:
    if kind == "lines":
        return len(raw.splitlines())
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        fail(f"{source}: guarded QML is not UTF-8: {error}")
    control = architecture_namespace.get("CONTROL")
    strip_comments = architecture_namespace.get("strip_qml_comments")
    if not hasattr(control, "finditer") or not callable(strip_comments):
        fail(f"{ARCHITECTURE_SCANNER} does not expose CONTROL and strip_qml_comments")
    stripped = call_dynamic_rule(
        f"HEAD:{ARCHITECTURE_SCANNER} strip_qml_comments",
        strip_comments,
        text,
    )
    matches = call_dynamic_rule(
        f"HEAD:{ARCHITECTURE_SCANNER} CONTROL.finditer",
        lambda value: list(control.finditer(value)),
        stripped,
    )
    return sum(
        1
        for match in matches
        if match.group(1) == key.rsplit(":", 1)[1]
    )


def language_value(
    raw: bytes, source: str, language_namespace: dict[str, object]
) -> int:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        return 0
    scanner = language_namespace.get("suspicious_lines")
    if not callable(scanner):
        fail("language contract does not expose suspicious_lines")
    return call_dynamic_rule(
        f"HEAD:{LANGUAGE_SCANNER} suspicious_lines",
        lambda value: len(scanner(value)),
        text,
    )


def validate_architecture_index(
    root: Path,
    prefix: str,
    before: dict[tuple[str, str], int],
    after: dict[tuple[str, str], int],
    staged: set[str],
    registry: dict[str, Any],
    architecture_namespace: dict[str, object],
) -> None:
    for key in sorted(set(before) | set(after)):
        kind, name = key
        source = name.rsplit(":", 1)[0] if kind == "control" else name
        if source not in staged and key not in changed_keys(before, after):
            continue
        raw = git_blob(root, "INDEX", source)
        expected = after.get(key)
        if expected is None and key in before and kind == "lines":
            if not has_staged_architecture_resolution(
                root,
                source,
                prefix,
                staged,
                registry,
                architecture_namespace,
            ):
                marker = ARCHITECTURE_RESOLUTION_FIELD.format(source)
                fail(
                    f"{ARCHITECTURE_RATCHET}: removing {key} requires staged "
                    f"evidence in the prefix's canonical evidence root with "
                    f"exactly: {marker}"
                )
        if raw is None:
            if expected is not None:
                fail(
                    f"{ARCHITECTURE_RATCHET}: {key} remains after its source was deleted"
                )
            continue
        actual = architecture_value(
            raw, kind, name, source, architecture_namespace
        )
        if expected is None:
            if kind == "control" and actual != 0:
                fail(
                    f"{ARCHITECTURE_RATCHET}: removed {key}, but the staged source "
                    f"still contains {actual} instance(s)"
                )
        elif actual != expected:
            fail(
                f"{ARCHITECTURE_RATCHET}: {key} records {expected}, but the staged "
                f"source has {actual}"
            )


def validate_language_index(
    root: Path,
    after: dict[str, int],
    staged: set[str],
    language_namespace: dict[str, object],
) -> None:
    text_suffixes = language_namespace.get("TEXT_SUFFIXES")
    is_localization = language_namespace.get("is_localization")
    is_history = language_namespace.get("is_history")
    is_canonical = language_namespace.get("is_canonical")
    if not isinstance(text_suffixes, set):
        fail("language contract does not expose TEXT_SUFFIXES")
    if not all(callable(rule) for rule in (is_localization, is_history, is_canonical)):
        fail("language contract does not expose path classification rules")

    candidates = (set(after) & staged) | {
        path
        for path in staged
        if Path(path).suffix.lower() in text_suffixes
        and path not in {ARCHITECTURE_RATCHET, LANGUAGE_RATCHET}
    }
    for source in sorted(candidates):
        raw = git_blob(root, "INDEX", source)
        expected = after.get(source)
        if raw is None:
            if expected is not None:
                fail(f"{LANGUAGE_RATCHET}: row remains after source deletion: {source}")
            continue
        if index_mode(root, source) == "120000":
            actual = 0
        elif call_dynamic_rule(
            f"HEAD:{LANGUAGE_SCANNER} is_localization",
            is_localization,
            source,
        ) or call_dynamic_rule(
            f"HEAD:{LANGUAGE_SCANNER} is_history",
            is_history,
            source,
        ):
            actual = 0
        else:
            actual = language_value(raw, source, language_namespace)
        if call_dynamic_rule(
            f"HEAD:{LANGUAGE_SCANNER} is_canonical",
            is_canonical,
            source,
        ) and actual:
            fail(f"{source}: staged canonical text contains non-English development prose")
        recorded = expected or 0
        if actual != recorded:
            if expected is None:
                fail(
                    f"{source}: staged language debt is {actual}, but no baseline row exists"
                )
            fail(
                f"{LANGUAGE_RATCHET}: {source} records {expected}, but the staged "
                f"source has {actual}"
            )


def validate_ratchet_updates(
    root: Path,
    prefix: str,
    paths: list[str],
    registry: dict[str, Any],
    authorities: tuple[tuple[str, Any, dict[str, object]], ...],
) -> None:
    staged = set(paths)
    # Only committed code interprets staged data. A staged scanner change takes
    # effect after it lands; the current commit is measured by the HEAD rules.
    architecture_namespace = head_python_namespace(root, ARCHITECTURE_SCANNER)
    language_namespace = head_python_namespace(root, LANGUAGE_SCANNER)

    def path_authorized(source: str) -> bool:
        for label, scope, namespace in authorities:
            path_allowed_rule = namespace.get("path_allowed")
            if not callable(path_allowed_rule):
                fail(f"{label}:{PROJECT_REGISTRY} does not expose path_allowed")
            if not call_dynamic_rule(
                f"{label}:{PROJECT_REGISTRY} path_allowed",
                path_allowed_rule,
                source,
                scope,
            ):
                return False
        return True

    allow_all = all(bool(scope.allow_all) for _label, scope, _namespace in authorities)

    def validate_rows(
        ratchet_path: str,
        before: dict[object, int],
        after: dict[object, int],
        source_path: Callable[[object], str],
        sourceless: Callable[[], bool] = lambda: False,
    ) -> None:
        changed = changed_keys(before, after)
        if not changed and not allow_all:
            fail(
                f'{ratchet_path}: prefix "{prefix}:" may only change a ratchet row '
                "together with the source that earned it"
            )
        for key in sorted(changed, key=str):
            old = before.get(key)
            new = after.get(key)
            source = source_path(key)
            if old is None:
                fail(f"{ratchet_path}: new debt row is forbidden: {key}")
            if new is not None and old is not None and new >= old:
                fail(f"{ratchet_path}: a changed row must strictly decrease: {key}")
            if source not in staged and not sourceless():
                fail(
                    f"{ratchet_path}: changed row requires its source in the same commit: "
                    f"{source}"
                )
            if not path_authorized(source):
                fail(
                    f'{ratchet_path}: source {source} is outside the effective '
                    f'HEAD/INDEX scope for prefix "{prefix}:"'
                )
    old_raw = git_blob(root, "HEAD", ARCHITECTURE_RATCHET)
    new_raw = git_blob(root, "INDEX", ARCHITECTURE_RATCHET)
    if old_raw is None and new_raw is None and ARCHITECTURE_RATCHET not in staged:
        architecture_before: dict[tuple[str, str], int] = {}
        architecture_after: dict[tuple[str, str], int] = {}
    elif old_raw is None or new_raw is None:
        fail(f"{ARCHITECTURE_RATCHET}: shared ratchet file cannot be added or deleted")
    else:
        architecture_before = parse_architecture_ratchet(
            old_raw, f"HEAD:{ARCHITECTURE_RATCHET}"
        )
        architecture_after = parse_architecture_ratchet(
            new_raw, f"INDEX:{ARCHITECTURE_RATCHET}"
        )
    if ARCHITECTURE_RATCHET in staged:
        validate_rows(
            ARCHITECTURE_RATCHET,
            architecture_before,
            architecture_after,
            lambda key: key[1].rsplit(":", 1)[0] if key[0] == "control" else key[1],
        )
    validate_architecture_index(
        root,
        prefix,
        architecture_before,
        architecture_after,
        staged,
        registry,
        architecture_namespace,
    )

    old_raw = git_blob(root, "HEAD", LANGUAGE_RATCHET)
    new_raw = git_blob(root, "INDEX", LANGUAGE_RATCHET)
    if old_raw is None and new_raw is None and LANGUAGE_RATCHET not in staged:
        language_before: dict[str, int] = {}
        language_after: dict[str, int] = {}
    elif old_raw is None or new_raw is None:
        fail(f"{LANGUAGE_RATCHET}: shared ratchet file cannot be added or deleted")
    else:
        language_before = parse_language_ratchet(old_raw, f"HEAD:{LANGUAGE_RATCHET}")
        language_after = parse_language_ratchet(new_raw, f"INDEX:{LANGUAGE_RATCHET}")
    if LANGUAGE_RATCHET in staged:
        validate_rows(
            LANGUAGE_RATCHET,
            language_before,
            language_after,
            lambda key: str(key),
            # A declared scanner migration is the one way a row may fall
            # without the file that holds it: the rule changed, not the file.
            lambda: has_staged_language_migration(
                root, prefix, staged, registry, architecture_namespace
            ),
        )
    validate_language_index(root, language_after, staged, language_namespace)


def read_subject(message_file: str) -> str:
    try:
        return Path(message_file).read_text(encoding="utf-8").splitlines()[0]
    except (OSError, IndexError, UnicodeError) as error:
        fail(f"could not read the commit subject: {error}")


def parse_subject(
    subject: str,
    scopes: dict[str, Any],
    registry_namespace: dict[str, object],
    *,
    require_imperative: bool,
    require_kind: bool,
) -> tuple[str, str | None, str, str | None, Any]:
    parse_change = registry_namespace.get("parse_subject_change")
    parse_prefix = registry_namespace.get("parse_subject_prefix")
    try:
        if callable(parse_change):
            normalized, kind, action, wrapper = call_dynamic_rule(
                f"{PROJECT_REGISTRY} parse_subject_change",
                lambda: parse_change(
                    subject,
                    allow_legacy=not require_kind,
                ),
            )
        elif callable(parse_prefix):
            normalized, action = call_dynamic_rule(
                f"{PROJECT_REGISTRY} parse_subject_prefix",
                parse_prefix,
                subject,
            )
            kind = None
            wrapper = None
        else:
            fail(
                f"{PROJECT_REGISTRY} does not expose parse_subject_change or "
                "parse_subject_prefix"
            )
    except ValueError as error:
        expected = next(iter(scopes))
        fail(
            f"{error}; for example "
            f"'{expected}-maintenance: Update repository contracts'"
        )

    if require_imperative:
        if NON_ENGLISH_ACTION.search(action):
            fail(
                "the action appears to contain non-English prose; commit subjects "
                "must be written in English"
            )
        first_word = re.match(r"[A-Za-z]+", action)
        verb = first_word.group(0).casefold() if first_word is not None else ""
        if verb not in IMPERATIVE_VERBS:
            fail(
                "the action must start with a recognized English imperative "
                "such as add, fix, remove, update, or verify"
            )

    scope = scopes.get(normalized)
    if scope is None:
        known = ", ".join(f"{name}:" for name in sorted(scopes))
        fail(f'unknown prefix "{normalized}:"; registered prefixes: {known}')
    return normalized, kind, action, wrapper, scope


def validate(
    subject: str,
    paths: list[str],
    scopes: dict[str, Any],
    registry_namespace: dict[str, object],
    *,
    require_imperative: bool = True,
    require_kind: bool = True,
) -> tuple[str, str | None, str, str | None]:
    prefix, kind, action, wrapper, scope = parse_subject(
        subject,
        scopes,
        registry_namespace,
        require_imperative=require_imperative,
        require_kind=require_kind,
    )
    if scope.allow_all:
        return prefix, kind, action, wrapper

    path_allowed_rule = registry_namespace.get("path_allowed")
    if not callable(path_allowed_rule):
        fail(f"{PROJECT_REGISTRY} does not expose path_allowed")
    outside = [
        path
        for path in paths
        if not call_dynamic_rule(
            f"{PROJECT_REGISTRY} path_allowed",
            path_allowed_rule,
            path,
            scope,
        )
    ]
    if not outside:
        return prefix, kind, action, wrapper

    print(
        f'commit-msg: prefix "{prefix}:" does not cover this commit.\n',
        file=sys.stderr,
    )
    print("Outside scope:", file=sys.stderr)
    for path in outside:
        print(f"  {path}", file=sys.stderr)
    print(f'\nScope of "{prefix}:":', file=sys.stderr)
    for root in scope.roots:
        print(f"  {root}", file=sys.stderr)
    for path in scope.files:
        print(f"  {path}", file=sys.stderr)
    print('\nSplit the commit or use "suite:" for a genuinely cross-suite unit.', file=sys.stderr)
    raise SystemExit(1)


def validate_version_update(
    root: Path,
    prefix: str,
    kind: str | None,
    action: str,
    wrapper: str | None,
    paths: list[str],
    head_registry: dict[str, Any],
    index_registry: dict[str, Any],
) -> None:
    # The typed subject and version policy activate only after their
    # implementation and registry have landed. This preserves the same
    # HEAD-interprets-INDEX migration boundary as the other commit guards.
    if head_registry.get("version_policy") is None:
        return
    if kind is None:
        fail("the committed version policy requires a typed commit subject")

    namespace = head_python_namespace(root, VERSION_CONTRACT)
    rule = namespace.get("validate_staged_transition")
    if not callable(rule):
        fail(f"{VERSION_CONTRACT} does not expose validate_staged_transition")

    try:
        call_dynamic_rule(
            f"HEAD:{VERSION_CONTRACT} validate_staged_transition",
            lambda: rule(
                head_registry,
                index_registry,
                prefix,
                kind,
                action,
                wrapper,
                lambda revision, path: git_blob(root, revision, path),
                tuple(paths),
            ),
        )
    except ValueError as error:
        fail(f"version contract: {error}")


def validate_staged_unit_prefix(root: Path, prefix: str) -> None:
    checker = Path(__file__).resolve().parent / "check-staged-units.py"
    process = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--root",
            str(root),
            "--quiet",
            "--commit-prefix",
            prefix,
        ],
        check=False,
    )
    if process.returncode != 0:
        fail("the subject does not match the staged inventory batch")


def reject_merge_delivery(root: Path) -> None:
    checker = Path(__file__).resolve().parent / "check-staged-units.py"
    process = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--root",
            str(root),
            "--quiet",
            "--forbid-delivery",
        ],
        check=False,
    )
    if process.returncode != 0:
        fail("a merge cannot include an inventoried delivery batch")


def load_normal_authorities(
    root: Path,
) -> tuple[
    tuple[dict[str, Any], dict[str, object], dict[str, Any]],
    tuple[dict[str, Any], dict[str, object], dict[str, Any]],
]:
    committed_registry_rules = head_python_namespace(root, PROJECT_REGISTRY)
    return (
        load_scopes(
            root,
            registry_source="HEAD",
            rule_namespace=committed_registry_rules,
        ),
        load_scopes(
            root,
            registry_source="INDEX",
            rule_namespace=committed_registry_rules,
        ),
    )


def validate_normal_commit(
    subject: str,
    paths: list[str],
    head_rules: tuple[dict[str, Any], dict[str, object], dict[str, Any]],
    index_rules: tuple[dict[str, Any], dict[str, object], dict[str, Any]],
) -> tuple[
    str,
    str | None,
    str,
    str | None,
    dict[str, object],
    dict[str, Any],
    dict[str, Any],
    tuple[tuple[str, Any, dict[str, object]], ...],
]:
    head_scopes, head_namespace, head_registry = head_rules
    index_scopes, index_namespace, index_registry = index_rules
    head_subject = validate(
        subject,
        paths,
        head_scopes,
        head_namespace,
        require_kind=head_registry.get("version_policy") is not None,
    )
    index_subject = validate(
        subject,
        paths,
        index_scopes,
        index_namespace,
        require_kind=index_registry.get("version_policy") is not None,
    )
    if head_subject != index_subject:
        fail(
            "HEAD and INDEX interpret the commit subject differently; land the "
            "governance change under the existing suite authority first"
        )
    index_prefix, kind, action, wrapper = index_subject
    return (
        index_prefix,
        kind,
        action,
        wrapper,
        index_namespace,
        head_registry,
        index_registry,
        (
            ("HEAD", head_scopes[index_prefix], head_namespace),
            ("INDEX", index_scopes[index_prefix], index_namespace),
        ),
    )


def validate_merge(root: Path, paths: list[str]) -> None:
    reject_merge_delivery(root)
    staged = set(paths)
    changed_ratchets = sorted(
        staged & {ARCHITECTURE_RATCHET, LANGUAGE_RATCHET}
    )
    if changed_ratchets:
        fail(
            "merge commits cannot change debt ratchets; deliver those changes "
            "in an ordinary prefixed commit: " + ", ".join(changed_ratchets)
        )

    committed_registry_rules = head_python_namespace(root, PROJECT_REGISTRY)
    _scopes, _registry_namespace, registry = load_scopes(
        root,
        registry_source="INDEX",
        rule_namespace=committed_registry_rules,
    )
    architecture_namespace = head_python_namespace(root, ARCHITECTURE_SCANNER)
    language_namespace = head_python_namespace(root, LANGUAGE_SCANNER)

    architecture_raw = git_blob(root, "INDEX", ARCHITECTURE_RATCHET)
    language_raw = git_blob(root, "INDEX", LANGUAGE_RATCHET)
    if architecture_raw is None or language_raw is None:
        fail("the merge index is missing a shared debt ratchet")
    architecture = parse_architecture_ratchet(
        architecture_raw, f"INDEX:{ARCHITECTURE_RATCHET}"
    )
    language = parse_language_ratchet(language_raw, f"INDEX:{LANGUAGE_RATCHET}")

    suite = registry.get("suite")
    if not isinstance(suite, dict) or not isinstance(suite.get("commit_prefix"), str):
        fail(f"INDEX:{REGISTRY}: suite.commit_prefix is missing or invalid")
    validate_architecture_index(
        root,
        suite["commit_prefix"],
        architecture,
        architecture,
        staged,
        registry,
        architecture_namespace,
    )
    validate_language_index(root, language, staged, language_namespace)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("message_file", nargs="?")
    parser.add_argument("--check", metavar="SUBJECT")
    parser.add_argument("--history-scope-only", metavar="SUBJECT")
    parser.add_argument("--check-index", metavar="SUBJECT")
    parser.add_argument("--check-ratchets", metavar="PREFIX")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    root = args.root.resolve()

    if args.check is not None:
        committed_registry_rules = head_python_namespace(root, PROJECT_REGISTRY)
        scopes, registry_namespace, registry = load_scopes(
            root,
            registry_source="HEAD",
            rule_namespace=committed_registry_rules,
        )
        validate(
            args.check,
            stdin_paths(),
            scopes,
            registry_namespace,
            require_kind=registry.get("version_policy") is not None,
        )
        return

    if args.history_scope_only is not None:
        committed_registry_rules = head_python_namespace(root, PROJECT_REGISTRY)
        scopes, registry_namespace, _registry = load_scopes(
            root,
            registry_source="HEAD",
            rule_namespace=committed_registry_rules,
        )
        validate(
            args.history_scope_only,
            stdin_paths(),
            scopes,
            registry_namespace,
            require_imperative=False,
            require_kind=False,
        )
        return

    if args.check_index is not None:
        paths = staged_paths(root)
        head_rules, index_rules = load_normal_authorities(root)
        (
            prefix,
            kind,
            action,
            wrapper,
            _index_namespace,
            head_registry,
            registry,
            authorities,
        ) = validate_normal_commit(
            args.check_index,
            paths,
            head_rules,
            index_rules,
        )
        validate_ratchet_updates(root, prefix, paths, registry, authorities)
        validate_version_update(
            root,
            prefix,
            kind,
            action,
            wrapper,
            paths,
            head_registry,
            registry,
        )
        validate_staged_unit_prefix(root, prefix)
        return

    if args.check_ratchets is not None:
        head_rules, index_rules = load_normal_authorities(root)
        head_scopes, head_namespace, _head_registry = head_rules
        index_scopes, index_namespace, registry = index_rules
        head_scope = head_scopes.get(args.check_ratchets)
        index_scope = index_scopes.get(args.check_ratchets)
        if head_scope is None or index_scope is None:
            fail(f'unknown prefix "{args.check_ratchets}:"')
        validate_ratchet_updates(
            root,
            args.check_ratchets,
            staged_paths(root),
            registry,
            (
                ("HEAD", head_scope, head_namespace),
                ("INDEX", index_scope, index_namespace),
            ),
        )
        return

    if not args.message_file:
        fail("missing commit-message file")
    if is_merge(root):
        validate_merge(root, staged_paths(root))
        return
    paths = staged_paths(root)
    head_rules, index_rules = load_normal_authorities(root)
    (
        prefix,
        kind,
        action,
        wrapper,
        _index_namespace,
        head_registry,
        registry,
        authorities,
    ) = validate_normal_commit(
        read_subject(args.message_file),
        paths,
        head_rules,
        index_rules,
    )
    validate_ratchet_updates(root, prefix, paths, registry, authorities)
    validate_version_update(
        root,
        prefix,
        kind,
        action,
        wrapper,
        paths,
        head_registry,
        registry,
    )
    validate_staged_unit_prefix(root, prefix)


if __name__ == "__main__":
    main()
