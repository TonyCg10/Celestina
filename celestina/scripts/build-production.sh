#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-build celestina
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "build" ]; then
    echo "build-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

(cd "$project_root" && cargo build --release --locked --bin celestina)
# The session lock's two binaries are built in an invocation of their own
# (CEL-2-H, design §4.14): with the shell in the same command, feature
# unification would switch the toolkit's `gl` feature on for the one process
# that can end the lock. `cargo test --workspace` unifies the same way, which
# is why verify-production.sh rebuilds this pair after it.
(cd "$project_root" && cargo build --release --locked -p celestina-lock -p celestina-lock-verify)

echo ">> Celestina release build steps completed (not installed)"
