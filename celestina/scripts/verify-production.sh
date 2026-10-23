#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/celestina

# --- lock artifact checks (begin) ---------------------------------------------
# Assertions about the lock's two shipped binaries (CEL-2-H, design §4.13,
# §4.14, §9.3). Read-only: they inspect files, run nothing of the lock.
check_lock_artifacts() {
    lock_dir=$1
    lock=$lock_dir/celestina-lock
    verifier=$lock_dir/celestina-lock-verify
    for file in "$lock" "$verifier"; do
        [ -f "$file" ] && [ -x "$file" ] || {
            echo "verify-production: $file is missing or not executable" >&2
            return 1
        }
    done
    # The rehearsal twin is built only with `fake-verifier`. Its binary must
    # not be beside the lock, and none of its marks may be in the shipped one:
    # its name, its fixed fake verifier path and its auto-typing field.
    [ ! -e "$lock_dir/unlock_probe" ] || {
        echo "verify-production: unlock_probe is in the release directory" >&2
        return 1
    }
    for mark in unlock_probe /usr/bin/true auto_type_after; do
        if grep -aqF -- "$mark" "$lock"; then
            echo "verify-production: the shipped celestina-lock holds the probe mark '$mark'" >&2
            return 1
        fi
    done
    # The lock draws on the CPU and links no GPU, font-configuration or
    # toolkit library; the verifier links PAM and the C runtime and nothing
    # else.
    lock_libs=$(ldd "$lock") || return 1
    if printf '%s\n' "$lock_libs" | grep -Eiq 'libEGL|libGL|libGLES|libOpenGL|libvulkan|libfontconfig|libfreetype|libwayland|libX11|libxcb'; then
        echo "verify-production: celestina-lock links a library it must not:" >&2
        printf '%s\n' "$lock_libs" >&2
        return 1
    fi
    verifier_libs=$(ldd "$verifier") || return 1
    printf '%s\n' "$verifier_libs" | grep -q 'libpam\.so' || {
        echo "verify-production: celestina-lock-verify does not link libpam" >&2
        return 1
    }
    stray=$(printf '%s\n' "$verifier_libs" | awk '{print $1}' | \
        grep -Ev '^(linux-vdso\.so|libpam\.so|libc\.so|libm\.so|libgcc_s\.so|libaudit\.so|libcap-ng\.so|/lib64/ld-linux|/lib/ld-linux|ld-linux)' || true)
    [ -z "$stray" ] || {
        echo "verify-production: celestina-lock-verify links more than PAM and libc:" >&2
        printf '%s\n' "$stray" >&2
        return 1
    }
    echo ">> lock artifacts: no probe marks, no GPU or font libraries in celestina-lock, PAM only in celestina-lock-verify"
}
# --- lock artifact checks (end) -----------------------------------------------

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification celestina
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
# Release profile here so clippy and tests share the release artifact cache
# built above.
(cd "$project_root" && cargo clippy --workspace --release --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --workspace --release --locked)
# `cargo test --workspace` unifies features across the whole workspace, so it
# rebuilds the shell's binary and the lock's with the feature set of the whole
# workspace (the toolkit's `gl` feature on for the lock, every crate feature
# any member asks for on the shell) and puts those builds at their artifact
# paths. The artifacts are the ones the build step made: put each back with
# its production invocation (cached, so this is a relink at most) before
# anything below inspects the bytes. The shell is restored with the exact
# command of build-production.sh, then the lock pair.
(cd "$project_root" && cargo build --release --locked --bin celestina)
(cd "$project_root" && cargo build --release --locked -p celestina-lock -p celestina-lock-verify)
# The closure of exactly that command against the checked-in allowlist (T-D1),
# by name, so a verification that skipped it would say so.
(cd "$project_root" && cargo test --release --locked -p celestina-lock -p celestina-lock-verify --test closure)
check_lock_artifacts "$project_root/target/release"
# The media phase of the smoke drives the shell with a fake MPRIS player, an
# example next to the release binary; it is test support, not part of the
# artifact.
(cd "$project_root" && cargo build --release --locked --example fake-mpris-player)
"$project_root/scripts/smoke.sh" --binary "$binary"

echo ">> Celestina verification steps completed; awaiting the runner seal"
