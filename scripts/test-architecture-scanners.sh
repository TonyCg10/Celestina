#!/usr/bin/env bash

set -uo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
scanner="$script_dir/architecture_scanners.py"
fixtures="$script_dir/fixtures/architecture"
failures=0
fixture_tmp=''

fail() {
    printf 'architecture fixtures: ERROR: %s\n' "$*" >&2
    failures=1
}

if ! fixture_tmp=$(mktemp -d); then
    echo "architecture fixtures: ERROR: could not create the temporary directory" >&2
    exit 1
fi
trap 'rm -R -- "$fixture_tmp"' EXIT

if ! mkdir -p "$fixture_tmp/qml" "$fixture_tmp/absolute"; then
    echo "architecture fixtures: ERROR: could not prepare the temporary fixture" >&2
    exit 1
fi

relative_shared=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/style/SharedButton.qml")
relative_violations=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/qml/StyleViolations.qml")
relative_surface=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/style/SharedSurface.qml")
if ! ln -s "$relative_shared" "$fixture_tmp/qml/SharedButton.qml" \
    || ! ln -s "$relative_shared" "$fixture_tmp/qml/RenamedButton.qml" \
    || ! ln -s "$relative_violations" "$fixture_tmp/qml/StyleViolationsLink.qml" \
    || ! ln -s "$relative_surface" "$fixture_tmp/qml/RenamedSurface.qml" \
    || ! ln -s "$fixtures/style/SharedButton.qml" \
        "$fixture_tmp/absolute/SharedButton.qml"; then
    echo "architecture fixtures: ERROR: could not create the test symlinks" >&2
    exit 1
fi

output=''
if ! output=$(python3 "$scanner" qml-auto-bindings "$fixtures/qml/Clean.qml"); then
    fail "the auto-binding scanner failed on the clean fixture"
elif [[ -n $output ]]; then
    fail "the clean fixture produced a false positive: $output"
fi

if ! output=$(python3 "$scanner" qml-auto-bindings "$fixtures/qml/AutoBinding.qml"); then
    fail "the auto-binding scanner failed on the positive fixture"
elif [[ $output != *"x: x;"* ]]; then
    fail "x: x; was not detected"
fi

if ! output=$(python3 "$scanner" local-controls "$fixtures/qml/RawControl.qml"); then
    fail "the control scanner failed on the positive fixture"
elif [[ $output != *$'RawControl.qml\tButton'* ]]; then
    fail "the local Qt control was not detected"
fi

if ! output=$(python3 "$scanner" local-controls \
    "$fixtures/qml/InlineControls.qml"); then
    fail "the control scanner failed on inline/object-valued declarations"
elif [[ $(grep -c $'InlineControls.qml\tButton' <<< "$output") -ne 4 ]]; then
    fail "inline or object-valued declarations evaded the Qt control scanner"
fi

if ! output=$(python3 "$scanner" local-controls \
    "$fixture_tmp/qml/RenamedButton.qml"); then
    fail "the control scanner failed while following a symlink"
elif [[ $output != *$'RenamedButton.qml\tButton'* ]]; then
    fail "a Qt control behind a renamed symlink was not detected"
fi

if ! output=$(python3 "$scanner" local-controls \
    --style-root "$fixtures/style" "$fixture_tmp/qml/SharedButton.qml"); then
    fail "the control scanner rejected a canonical style symlink"
elif [[ -n $output ]]; then
    fail "a canonical shared control counted as a local reconstruction: $output"
fi

if ! output=$(python3 "$scanner" local-controls "$fixtures/qml/Clean.qml"); then
    fail "the control scanner failed on the clean fixture"
elif [[ -n $output ]]; then
    fail "a control inside a comment produced a false positive"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" "$fixtures/qml/StyleClean.qml"); then
    fail "the visual scanner failed on the clean fixture"
elif [[ -n $output ]]; then
    fail "the clean visual fixture produced a false positive: $output"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" \
    "$fixture_tmp/qml/StyleViolationsLink.qml"); then
    fail "the visual scanner failed while following a symlink"
elif [[ $output != *"named color"* || $output != *"direct numeric radius"* \
        || $output != *"local color transformation"* ]]; then
    fail "a symlink hid direct visual values"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" "$fixtures/qml/StyleViolations.qml"); then
    fail "the visual scanner failed on the positive fixture"
elif [[ $output != *"named color"* || $output != *"direct numeric radius"* \
        || $output != *"local color transformation"* || $output != *'"blue"'* \
        || $output != *'"green"'* ]]; then
    fail "multiline/property color visual values were not detected"
fi

if ! output=$(python3 "$scanner" style-copies \
    "$fixtures/style" "$fixtures/style-copy"); then
    fail "the style-copy comparison failed"
elif [[ $output != *"RenamedSurface.qml: structural copy"* ]]; then
    fail "a renamed style copy was not detected"
fi

if ! output=$(python3 "$scanner" style-copies \
    "$fixtures/style" "$fixture_tmp/qml/RenamedSurface.qml"); then
    fail "the style comparison failed while following a renamed symlink"
elif [[ $output != *"RenamedSurface.qml: structural copy"* ]]; then
    fail "a renamed symlink hid a structural style copy"
fi

if ! python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/qml/SharedButton.qml"; then
    fail "a relative symlink to the canonical component was rejected"
fi

if output=$(python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/qml/RenamedButton.qml" 2>&1); then
    fail "a renamed symlink evaded the target restriction"
elif [[ $output != *"sibling component"* ]]; then
    fail "the renamed symlink failed without a canonical-target diagnostic"
fi

if output=$(python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/absolute/SharedButton.qml" 2>&1); then
    fail "an absolute symlink evaded the relative-link policy"
elif [[ $output != *"must be relative"* ]]; then
    fail "the absolute symlink failed without a relative-link diagnostic"
fi

if ! python3 "$scanner" cmake-qml-registration \
    "$fixtures/cmake-valid/CMakeLists.txt" \
    "$fixtures/cmake-valid/qml" \
    celestina; then
    fail "the valid CMake registration was rejected"
fi

if output=$(python3 "$scanner" cmake-qml-registration \
    "$fixtures/cmake-invalid/CMakeLists.txt" \
    "$fixtures/cmake-invalid/qml" \
    celestina 2>&1); then
    fail "an unregistered QML file evaded CMake parity"
elif [[ $output != *"Unregistered.qml"* || $output != *"Missing.qml"* ]]; then
    fail "CMake parity did not report both sides of the mismatch"
fi

if python3 "$scanner" qml-auto-bindings "$fixtures/does-not-exist" \
    >/dev/null 2>&1; then
    fail "a missing input did not make the scanner fail"
fi

if python3 "$scanner" dependency-metadata </dev/null >/dev/null 2>&1; then
    fail "empty metadata did not make the dependency scanner fail"
fi

metadata='{"packages":[{"name":"core","dependencies":[{"name":"qt6-types","rename":"ui"}]}]}'
if ! output=$(python3 "$scanner" dependency-metadata <<< "$metadata"); then
    fail "the scanner rejected valid metadata"
elif [[ $output != "core: qt6-types (alias ui)" ]]; then
    fail "a renamed UI dependency was not detected"
fi

metadata='{"packages":[{"name":"core","dependencies":[{"name":"gtk4"},{"name":"iced"},{"name":"slint"},{"name":"smithay-client-toolkit"}]}]}'
if ! output=$(python3 "$scanner" dependency-metadata <<< "$metadata"); then
    fail "the scanner rejected valid UI metadata"
elif [[ $output != *"gtk4"* || $output != *"iced"* || $output != *"slint"* \
        || $output != *"smithay-client-toolkit"* ]]; then
    fail "a UI/compositor family was not detected"
fi

# Guard coverage: both guards enumerate projects by name, so a new QML project
# could be omitted from one list and still "pass" without ever being inspected.
# This does not test a scanner; it tests that scanners receive everything that
# exists.
repo_root=$(cd -- "$script_dir/.." && pwd)
architecture_guard="$script_dir/check-architecture-contract.sh"
style_guard="$repo_root/celestina-style/scripts/check-style-contract.sh"

for guard in "$architecture_guard" "$style_guard"; do
    [[ -f $guard ]] || fail "missing guard $guard"
done

if ! python3 - "$scanner" "$repo_root/docs/projects.toml" <<'PY'
import runpy
import sys
import tomllib

namespace = runpy.run_path(sys.argv[1], run_name="_architecture_evidence_fixture")
with open(sys.argv[2], "rb") as handle:
    registry = tomllib.load(handle)

root_for_prefix = namespace["canonical_evidence_root_for_prefix"]
roots_for_source = namespace["canonical_evidence_roots_for_source"]
is_evidence = namespace["is_canonical_evidence_path"]

assert root_for_prefix(registry, "suite") == "docs/evidence"
assert root_for_prefix(registry, "siderita") == "siderita/docs/evidence"
assert root_for_prefix(registry, "siderita-core") is None

roots = roots_for_source(
    registry, "celestina-rs/crates/siderita-core/src/lib.rs"
)
assert "docs/evidence" in roots
assert "siderita/docs/evidence" in roots
assert "celestina-rs/docs/evidence" in roots
assert is_evidence("siderita/docs/evidence/refactor.md", roots)
assert is_evidence("docs/evidence/refactor.md", roots)
assert not is_evidence(
    "celestina-rs/crates/siderita-core/docs/evidence/refactor.md", roots
)
assert not is_evidence("siderita/qml/docs/evidence/refactor.md", roots)
PY
then
    fail "canonical architecture evidence ownership rules were inconsistent"
fi

# Exercise the real history command in an isolated Git repository. Helper-only
# assertions cannot prove that changed-path discovery and evidence reads are
# wired to the canonical roots.
history_tmp="$fixture_tmp/history"
if ! mkdir -p "$history_tmp/docs" "$history_tmp/scripts" "$history_tmp/app/src"; then
    fail "could not prepare the architecture history fixture"
else
    printf '%s\n' \
        'schema_version = 1' \
        '' \
        '[suite]' \
        'commit_prefix = "suite"' \
        '' \
        '[[projects]]' \
        'path = "app"' \
        'commit_prefix = "app"' \
        'commit_roots = ["app/"]' \
        > "$history_tmp/docs/projects.toml"
    printf '%s\n' '# Temporary architecture debt.' \
        'lines	app/src/Coordinator.qml	3' \
        > "$history_tmp/scripts/architecture-baseline.tsv"
    printf '%s\n' 'line one' 'line two' 'line three' \
        > "$history_tmp/app/src/Coordinator.qml"
    git -C "$history_tmp" init -q
    git -C "$history_tmp" config user.name Fixture
    git -C "$history_tmp" config user.email fixture@example.invalid
    git -C "$history_tmp" config core.hooksPath /dev/null
    git -C "$history_tmp" add .
    git -C "$history_tmp" commit -qm 'fixture: establish architecture debt'

    reset_history_fixture() {
        git -C "$history_tmp" reset --hard -q HEAD
        git -C "$history_tmp" clean -fdq
    }
    check_history_fixture() {
        python3 "$scanner" baseline-history HEAD \
            "$history_tmp/scripts/architecture-baseline.tsv" \
            "$history_tmp/docs/projects.toml" \
            --root "$history_tmp" >/dev/null 2>&1
    }
    remove_history_debt() {
        printf '%s\n' 'line one' 'line two' \
            > "$history_tmp/app/src/Coordinator.qml"
        printf '%s\n' '# Temporary architecture debt.' \
            > "$history_tmp/scripts/architecture-baseline.tsv"
    }

    remove_history_debt
    mkdir -p "$history_tmp/app/src/docs/evidence"
    printf '%s\n' '# Fake nested evidence' '' \
        '- **Resolved architecture debt:** `app/src/Coordinator.qml`' \
        > "$history_tmp/app/src/docs/evidence/resolution.md"
    if check_history_fixture; then
        fail "nested fake evidence passed the real baseline-history command"
    fi

    reset_history_fixture
    remove_history_debt
    mkdir -p "$history_tmp/app/docs/evidence"
    printf '%s\n' '# Owner architecture resolution' '' \
        '- **Resolved architecture debt:** `app/src/Coordinator.qml`' \
        > "$history_tmp/app/docs/evidence/resolution.md"
    check_history_fixture \
        || fail "canonical owner evidence was rejected by baseline-history"

    reset_history_fixture
    remove_history_debt
    mkdir -p "$history_tmp/docs/evidence"
    printf '%s\n' '# Suite architecture resolution' '' \
        '- **Resolved architecture debt:** `app/src/Coordinator.qml`' \
        > "$history_tmp/docs/evidence/resolution.md"
    check_history_fixture \
        || fail "canonical suite evidence was rejected by baseline-history"
fi

baseline_fixture="$fixture_tmp/architecture-baseline.tsv"
if ! awk -F '\t' \
    '$1 != "lines" || $2 != "celestina-rs/crates/magnetitad/src/main.rs"' \
    "$script_dir/architecture-baseline.tsv" > "$baseline_fixture"; then
    fail "could not prepare the missing-baseline-row fixture"
elif output=$(ARCHITECTURE_BASELINE_FILE="$baseline_fixture" \
    ARCHITECTURE_COMPARE_REF=HEAD bash "$architecture_guard" 2>&1); then
    fail "removing a lines baseline row did not make the architecture guard fail"
elif [[ $output != *"baseline row removed without a changed source and canonical resolution evidence: "* \
        || $output != *"celestina-rs/crates/magnetitad/src/main.rs"* ]]; then
    fail "the removed baseline row failed without the source-presence diagnostic"
fi

for candidate in "$repo_root"/*/; do
    project=$(basename -- "$candidate")
    [[ $project == celestina-style ]] && continue
    [[ -d $candidate/qml ]] || continue

    # Check per line, not per file: each scanner invocation lists its inputs on
    # one line, and appearing in three of four lists still leaves one scanner
    # blind. `siderita/qml` appears in every list and therefore serves as the
    # template for what each list must contain.
    for guard in "$architecture_guard" "$style_guard"; do
        while IFS= read -r line; do
            if [[ $line != *"$project/qml"* ]]; then
                fail "$(basename -- "$guard"): an input list omits $project/qml -> $line"
            fi
        done < <(grep -F 'siderita/qml' "$guard")
    done

    # Projects with build.rs also pass through the QML-registration and style-
    # symlink loops, which enumerate bare project names.
    if [[ -f $candidate/build.rs ]]; then
        while IFS= read -r line; do
            if [[ $line != *" $project"* ]]; then
                fail "architecture guard: a 'for app in' loop omits $project -> $line"
            fi
        done < <(grep -E 'for app in .*siderita' "$architecture_guard")
    fi
done

if ((failures)); then
    exit 1
fi

echo "Architecture fixtures: OK"
