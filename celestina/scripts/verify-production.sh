#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
artifact_tool=$suite_root/scripts/production_artifact.py
style_root=$suite_root/celestina-style

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification celestina
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

"$style_root/scripts/verify-production.sh" --production-runner-internal
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && cargo clippy --locked \
    -p celestina-shell-core -p celestina-core -p magnetita-core \
    --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && \
    cargo test --locked -p celestina-shell-core -p celestina-core -p magnetita-core)
"$project_root/scripts/qmllint-production.sh"
ctest --test-dir "$build_dir" --output-on-failure
"$project_root/scripts/smoke-production.sh"

# The handover report, run for the one thing verification can prove about it:
# that it works and changes nothing. Its exit code says whether the session
# could do without Noctalia yet — 2 means not yet, which is a state, not a
# failure of this build.
"$project_root/scripts/handover-status.sh" || handover_state=$?
if [[ ${handover_state:-0} -gt 2 ]]; then
    echo "verify: the handover report itself failed" >&2
    exit 1
fi

missing=$(
    ldd "$build_dir/celestina" \
        "$style_root/build/libcelestina-style.so" \
        "$style_root/build/CelestinaStyle/libcelestina-style-plugin.so" \
        | grep 'not found' || true
)
if [ -n "$missing" ]; then
    echo "verify-production: missing dynamic dependencies:" >&2
    echo "$missing" >&2
    exit 1
fi

echo ">> Celestina bundle verification steps completed; the session was not activated"
