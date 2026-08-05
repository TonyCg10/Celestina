#!/usr/bin/env bash

# Architectural guard for the whole suite. Keep it usable both from CI and from
# a checkout: it inspects tracked files plus non-ignored, not-yet-added files.
set -uo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
readonly baseline_file="${ARCHITECTURE_BASELINE_FILE:-$script_dir/architecture-baseline.tsv}"
readonly architecture_scanner="$script_dir/architecture_scanners.py"

cd "$repo_root"

failures=0

fail() {
    printf 'architecture: ERROR: %s\n' "$*" >&2
    failures=1
}

is_generated_path() {
    case $1 in
        build/* | */build/* | target/* | */target/*) return 0 ;;
        *) return 1 ;;
    esac
}

is_guarded_source() {
    case $1 in
        *.rs | *.qml | *.cpp | *.cc | *.cxx | *.h | *.hh | *.hpp) return 0 ;;
        *) return 1 ;;
    esac
}

check_baseline_history() {
    local compare_ref=${ARCHITECTURE_COMPARE_REF:-}
    [[ -n $compare_ref ]] || return

    if ! git rev-parse --verify --quiet "$compare_ref^{commit}" >/dev/null; then
        fail "cannot resolve ARCHITECTURE_COMPARE_REF=$compare_ref; history is missing to protect the baseline"
        return
    fi

    if ! git cat-file -e "$compare_ref:scripts/architecture-baseline.tsv" 2>/dev/null; then
        echo "architecture: initial baseline; no history at $compare_ref"
        return
    fi

    if ! python3 "$architecture_scanner" baseline-history \
        "$compare_ref" "$baseline_file" "$repo_root/docs/projects.toml" \
        --root "$repo_root"
    then
        fail "the architecture baseline may only decrease against $compare_ref"
    fi
}

check_modularity_debt() {
    if [[ ! -f $baseline_file ]]; then
        fail "missing baseline $baseline_file"
        return
    fi

    declare -A baseline=()
    declare -A baseline_entries=()
    local raw kind path maximum extra entry line_number=0

    while IFS= read -r raw || [[ -n $raw ]]; do
        ((line_number += 1))
        [[ -z $raw || $raw == \#* ]] && continue

        IFS=$'\t' read -r kind path maximum extra <<< "$raw"
        if [[ -z ${kind:-} || -z ${path:-} || -z ${maximum:-} || -n ${extra:-} ]]; then
            fail "scripts/architecture-baseline.tsv:$line_number: expected exactly three TSV columns"
            continue
        fi
        if [[ $kind != lines && $kind != control ]]; then
            fail "scripts/architecture-baseline.tsv:$line_number: unknown class '$kind'"
            continue
        fi
        if [[ ! $maximum =~ ^[1-9][0-9]*$ ]]; then
            fail "scripts/architecture-baseline.tsv:$line_number: '$maximum' is not a positive maximum"
            continue
        fi
        entry="$kind:$path"
        if [[ ${baseline_entries[$entry]+present} ]]; then
            fail "scripts/architecture-baseline.tsv:$line_number: duplicate entry for '$kind $path'"
            continue
        fi
        baseline_entries["$entry"]=1

        [[ $kind == lines ]] || continue
        if is_generated_path "$path" || ! is_guarded_source "$path"; then
            fail "scripts/architecture-baseline.tsv:$line_number: '$path' is not in the guarded set"
            continue
        fi
        baseline["$path"]=$maximum
    done < "$baseline_file"

    local file
    local guarded_count=0
    while IFS= read -r -d '' file; do
        is_generated_path "$file" && continue
        is_guarded_source "$file" || continue
        [[ -f $file ]] || continue
        ((guarded_count += 1))

    done < <(git ls-files --cached --others --exclude-standard -z)

    if ((guarded_count == 0)); then
        fail "found no Rust/QML/C++ sources to measure"
    fi

    local lines expected
    for path in "${!baseline[@]}"; do
        if [[ ! -f $path ]]; then
            fail "scripts/architecture-baseline.tsv: stale debt or missing file: '$path'"
            continue
        fi
        # awk counts the final logical line even when a file omits its trailing
        # newline; removing that newline must not evade the debt ratchet.
        lines=$(awk 'END { print NR }' "$path")
        expected=${baseline[$path]}
        if ((lines > expected)); then
            fail "$path: grew from $expected to $lines lines; inventoried debt may not grow"
        elif ((lines < expected)); then
            fail "$path: fell from $expected to $lines lines; lower the baseline to $lines to lock the improvement in"
        fi
    done
}

build_qml_registry() {
    python3 - "$1" <<'PY'
import pathlib
import re
import sys

source = sys.argv[1]
text = pathlib.Path(source).read_text(encoding="utf-8")
text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
text = re.sub(r"//.*", "", text)

module_start = re.search(
    r"\blet\s+([A-Za-z_]\w*)\s*=\s*QmlModule::new\s*\(", text
)
if not module_start:
    print(f"{source}: QmlModule construction not found", file=sys.stderr)
    raise SystemExit(1)

# Read the complete QmlModule builder statement, not arbitrary path strings in
# rerun-if-changed, constants or dead variables.
statement_start = module_start.start()
depth = {"(": 0, "[": 0, "{": 0}
pairs = {")": "(", "]": "[", "}": "{"}
in_string = False
escaped = False
statement_end = None
for offset, char in enumerate(text[statement_start:], statement_start):
    if in_string:
        if escaped:
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == '"':
            in_string = False
        continue
    if char == '"':
        in_string = True
    elif char in depth:
        depth[char] += 1
    elif char in pairs:
        opening = pairs[char]
        depth[opening] -= 1
        if depth[opening] < 0:
            print(f"{source}: invalid delimiters in QmlModule", file=sys.stderr)
            raise SystemExit(1)
    elif char == ";" and all(value == 0 for value in depth.values()):
        statement_end = offset + 1
        break

if statement_end is None:
    print(f"{source}: incomplete QmlModule statement", file=sys.stderr)
    raise SystemExit(1)

module_var = module_start.group(1)
module_statement = text[statement_start:statement_end]
builder_pattern = rf"CxxQtBuilder::new_qml_module\s*\(\s*{re.escape(module_var)}\s*\)"
if not re.search(builder_pattern, text[statement_end:]):
    print(f"{source}: QmlModule '{module_var}' never reaches CxxQtBuilder", file=sys.stderr)
    raise SystemExit(1)

paths = re.findall(r'QmlFile::from\s*\(\s*"(qml/[^"\n]+\.qml)"\s*\)', module_statement)

if re.search(r"\.qml_files\s*\(\s*QML_FILES\s*\)", module_statement):
    qml_files = re.search(
        r"\bconst\s+QML_FILES\s*:\s*&\s*\[\s*&str\s*\]\s*=\s*&\[(.*?)\]\s*;",
        text,
        flags=re.S,
    )
    if not qml_files:
        print(f"{source}: QML_FILES is used but does not have the canonical format", file=sys.stderr)
        raise SystemExit(1)
    listed_strings = re.findall(r'"([^"\n]+)"', qml_files.group(1))
    invalid = [path for path in listed_strings if not re.fullmatch(r"qml/[^\n]+\.qml", path)]
    if invalid:
        print(f"{source}: QML_FILES contains a non-QML path: {invalid[0]}", file=sys.stderr)
        raise SystemExit(1)
    paths.extend(listed_strings)

if not paths:
    print(f"{source}: QmlModule registers no QML file", file=sys.stderr)
    raise SystemExit(1)
if len(paths) != len(set(paths)):
    print(f"{source}: some QML paths are registered more than once", file=sys.stderr)
    raise SystemExit(1)

for path in sorted(paths):
    print(path)
PY
}

check_qml_registration() {
    local app build_file file relative base canonical registered registry
    local resolved_file resolved_canonical

    for app in siderita magnetita grafita fluorita; do
        build_file="$app/build.rs"
        if [[ ! -f $build_file ]]; then
            fail "missing $build_file"
            continue
        fi
        if ! registry=$(build_qml_registry "$build_file"); then
            fail "$build_file: could not read the effective QML registry"
            continue
        fi

        while IFS= read -r -d '' file; do
            relative=${file#"$app/"}

            if [[ -L $file ]]; then
                base=${file##*/}
                canonical="celestina-style/$base"
                # Shared style links live at qml/<name>. They are exempt from
                # being regular app sources only when that registered name is
                # actually present in build.rs.
                if [[ -e $canonical || -L $canonical ]]; then
                    resolved_file=$(realpath -e -- "$file" 2>/dev/null || true)
                    resolved_canonical=$(realpath -e -- "$canonical" 2>/dev/null || true)
                    if [[ -z $resolved_file || $resolved_file != "$resolved_canonical" ]]; then
                        fail "$file: shared symlink points outside $canonical"
                    fi
                    if ! grep -Fxq -- "qml/$base" <<< "$registry"; then
                        fail "$file: shared symlink not registered in $build_file as qml/$base"
                    fi
                    continue
                fi
            fi

            if ! grep -Fxq -- "$relative" <<< "$registry"; then
                fail "$file: plain QML missing from $build_file (missing \"$relative\")"
            fi
        done < <(find "$app/qml" \( -type f -o -type l \) -name '*.qml' -print0)

        while IFS= read -r registered; do
            [[ -n $registered ]] || continue
            if [[ ! -e "$app/$registered" && ! -L "$app/$registered" ]]; then
                fail "$build_file: registers '$registered', but the file does not exist"
            fi
        done <<< "$registry"
    done

    if ! python3 "$architecture_scanner" cmake-qml-registration \
        celestina/CMakeLists.txt celestina/qml celestina; then
        fail "celestina/CMakeLists.txt: incomplete or invalid QML registration"
    fi
}

check_style_public_api() {
    if ! python3 - <<'PY'
import collections
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

style = pathlib.Path("celestina-style")
cmake_path = style / "CMakeLists.txt"
qmldir_path = style / "qmldir"
errors = []

if not cmake_path.is_file() or not qmldir_path.is_file():
    print("architecture: ERROR: celestina-style qmldir or CMakeLists.txt is missing", file=sys.stderr)
    raise SystemExit(1)

cmake = re.sub(r"#.*", "", cmake_path.read_text(encoding="utf-8"))

def cmake_list(keyword):
    match = re.search(rf"(?m)^\s*{keyword}\s*$", cmake)
    if not match:
        errors.append(f"{cmake_path}: missing the {keyword} section")
        return []
    result = []
    for line in cmake[match.end():].splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped == ")" or re.fullmatch(r"[A-Z][A-Z0-9_]*", stripped):
            break
        result.extend(stripped.split())
    return result

cmake_qml = cmake_list("QML_FILES")
cmake_resources = cmake_list("RESOURCES")

qmldir_entries = []
for number, raw in enumerate(qmldir_path.read_text(encoding="utf-8").splitlines(), 1):
    line = raw.split("#", 1)[0].strip()
    if not line or line.startswith("module "):
        continue
    match = re.fullmatch(
        r"(?:(singleton)\s+)?([A-Za-z_]\w*)\s+([0-9]+(?:\.[0-9]+)*)\s+([^\s]+\.qml)",
        line,
    )
    if not match:
        errors.append(f"{qmldir_path}:{number}: invalid public entry")
        continue
    qmldir_entries.append(
        {"singleton": bool(match.group(1)), "type": match.group(2), "file": match.group(4)}
    )

actual_qml = sorted(path.name for path in style.glob("*.qml") if path.is_file())
qmldir_qml = [entry["file"] for entry in qmldir_entries]

def report_duplicates(label, values):
    for value, count in collections.Counter(values).items():
        if count > 1:
            errors.append(f"{label}: duplicate entry '{value}'")

report_duplicates(f"{cmake_path} QML_FILES", cmake_qml)
report_duplicates(str(qmldir_path), qmldir_qml)

def compare_sets(left_label, left, right_label, right):
    left_set = set(left)
    right_set = set(right)
    for value in sorted(left_set - right_set):
        errors.append(f"{value}: present in {left_label}, missing from {right_label}")
    for value in sorted(right_set - left_set):
        errors.append(f"{value}: present in {right_label}, missing from {left_label}")

compare_sets("celestina-style", actual_qml, "CMake QML_FILES", cmake_qml)
compare_sets("celestina-style", actual_qml, "qmldir", qmldir_qml)

for entry in qmldir_entries:
    expected_type = pathlib.Path(entry["file"]).stem
    if entry["type"] != expected_type:
        errors.append(
            f"{qmldir_path}: type '{entry['type']}' does not match {entry['file']}"
        )

cmake_singletons = set()
for match in re.finditer(r"set_source_files_properties\s*\((.*?)\)", cmake, flags=re.S):
    body = match.group(1)
    if not re.search(r"\bQT_QML_SINGLETON_TYPE\s+TRUE\b", body):
        continue
    before_properties = body.split("PROPERTIES", 1)[0]
    cmake_singletons.update(re.findall(r"[A-Za-z0-9_.+/-]+\.qml", before_properties))
qmldir_singletons = {entry["file"] for entry in qmldir_entries if entry["singleton"]}
compare_sets("CMake singletons", cmake_singletons, "qmldir singletons", qmldir_singletons)

qrc_resources = []
qrc_paths = sorted(style.glob("*.qrc"))
if not qrc_paths:
    errors.append("celestina-style: no QRC manifest found")
for qrc_path in qrc_paths:
    try:
        root = ET.parse(qrc_path).getroot()
    except (ET.ParseError, OSError) as error:
        errors.append(f"{qrc_path}: invalid QRC: {error}")
        continue
    for element in root.findall(".//file"):
        if element.text and element.text.strip():
            qrc_resources.append(element.text.strip())

report_duplicates("QRC", qrc_resources)
report_duplicates(f"{cmake_path} RESOURCES", cmake_resources)
compare_sets("CMake RESOURCES", cmake_resources, "QRC", qrc_resources)
for resource in sorted(set(cmake_resources) | set(qrc_resources)):
    if not (style / resource).is_file():
        errors.append(f"celestina-style/{resource}: resource declared but missing")

if errors:
    for error in errors:
        print(f"architecture: ERROR: {error}", file=sys.stderr)
    raise SystemExit(1)
PY
    then
        failures=1
    fi
}

check_top_level_auto_bindings() {
    local hits
    # Keep this in lock-step with the existing CI/smoke rule. Object literals
    # such as append({key: key}) are inside parentheses; a real top-level QML
    # binding has parenthesis depth zero.
    if ! hits=$(python3 "$architecture_scanner" qml-auto-bindings \
        siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style); then
        fail "the QML auto-binding scanner could not complete its inspection"
        return
    fi

    if [[ -n $hits ]]; then
        printf '%s\n' "$hits"
        fail "'x: x' auto-binding at higher QML depth; rename the property or use an alias"
    fi
}

check_visual_contract() {
    if ! python3 scripts/check-sealed-colours.py; then
        failures=1
    fi

    if ! bash celestina-style/scripts/check-style-contract.sh; then
        fail "the celestina-style visual guard failed"
    fi
}

check_shared_style_links() {
    local app file base canonical resolved_file resolved_canonical

    if ! python3 "$architecture_scanner" shared-style-links \
        celestina-style siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml; then
        fail "shared QML symlinks do not respect the relative canonical target"
    fi

    for app in siderita magnetita grafita fluorita; do
        # Check only assets that the style explicitly exposes for source-tree
        # consumption. This covers QML plus the icon/font manifests and trees,
        # without confusing unrelated same-named application directories.
        for canonical in celestina-style/*.qml celestina-style/*.qrc \
            celestina-style/icons celestina-style/fonts; do
            [[ -e $canonical || -L $canonical ]] || continue
            base=${canonical##*/}
            file="$app/qml/$base"
            [[ -e $file || -L $file ]] || continue

            if [[ ! -L $file ]]; then
                fail "$file: copy of the shared style; it must be a symlink to $canonical"
                continue
            fi
            resolved_file=$(realpath -e -- "$file" 2>/dev/null || true)
            resolved_canonical=$(realpath -e -- "$canonical" 2>/dev/null || true)
            if [[ -z $resolved_file || $resolved_file != "$resolved_canonical" ]]; then
                fail "$file: style symlink points outside $canonical"
            fi
        done
    done
}

check_local_control_ratchet() {
    # Deliberate raw Qt Controls live in the same history-protected baseline as
    # oversized sources. They may disappear as shared controls land, but no new
    # file/type pair and no additional instance may be added silently.
    declare -A baseline=()
    declare -A actual=()
    local raw kind key maximum extra file control count control_rows

    while IFS= read -r raw || [[ -n $raw ]]; do
        [[ -z $raw || $raw == \#* ]] && continue
        IFS=$'\t' read -r kind key maximum extra <<< "$raw"
        [[ $kind == control ]] || continue
        [[ -n ${key:-} && $maximum =~ ^[1-9][0-9]*$ && -z ${extra:-} ]] || continue
        baseline["$key"]=$maximum
    done < "$baseline_file"

    if ! control_rows=$(python3 "$architecture_scanner" local-controls \
        --style-root celestina-style \
        siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml); then
        fail "the local Qt control scanner could not complete its inspection"
        return
    fi

    while IFS=$'\t' read -r file control; do
        [[ -n $file && -n $control ]] || continue
        key="$file:$control"
        count=${actual[$key]:-0}
        actual["$key"]=$((count + 1))
    done <<< "$control_rows"

    for key in "${!actual[@]}"; do
        if [[ ! ${baseline[$key]+present} ]]; then
            fail "$key: Qt control rebuilt outside the baseline; reuse or extend celestina-style"
            continue
        fi
        if ((actual[$key] > baseline[$key])); then
            fail "$key: grew from ${baseline[$key]} to ${actual[$key]} local Qt control instances"
        elif ((actual[$key] < baseline[$key])); then
            fail "$key: the local control shrank; lower its row in scripts/architecture-baseline.tsv"
        fi
    done

    for key in "${!baseline[@]}"; do
        if [[ ! ${actual[$key]+present} ]]; then
            fail "$key: stale local control exception; remove it from the guard"
        fi
    done
}

check_dependency_direction() {
    local hits metadata grep_status

    # The celestina-rs workspace is the Qt-free domain boundary. Dependency
    # declarations for UI/compositor stacks belong in an app adapter instead.
    # cargo metadata resolves renamed and workspace-inherited dependencies, so
    # aliases cannot evade this boundary.
    if ! metadata=$(cargo metadata --manifest-path celestina-rs/Cargo.toml \
        --format-version 1 --no-deps --locked 2>/dev/null); then
        fail "cargo metadata could not validate the celestina-rs dependencies"
    elif ! hits=$(python3 "$architecture_scanner" dependency-metadata \
        <<< "$metadata"); then
        fail "the dependency scanner could not parse cargo metadata"
    elif [[ -n $hits ]]; then
        printf '%s\n' "$hits"
        fail "a celestina-rs crate declares a UI/compositor dependency"
    fi

    if hits=$(grep -RInEH --include='*.qml' \
        '^[[:space:]]*import[[:space:]]+org\.celestina\.(siderita|magnetita|grafita|fluorita)([[:space:]]|$)' \
        celestina-style 2>/dev/null); then
        :
    else
        grep_status=$?
        if ((grep_status != 1)); then
            fail "grep could not inspect the celestina-style QML dependencies"
            return
        fi
    fi
    if [[ -n $hits ]]; then
        printf '%s\n' "$hits"
        fail "celestina-style imports an application module"
    fi
}

check_baseline_history
check_modularity_debt
check_qml_registration
check_style_public_api
check_top_level_auto_bindings
check_visual_contract
check_shared_style_links
check_local_control_ratchet
check_dependency_direction

if ((failures)); then
    exit 1
fi

echo "Architecture contract: OK"
