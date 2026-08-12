#!/bin/sh

# Source-only. Refuses to rewrite the binaries a live Celestina is executing.
#
# Two GPU losses have been recorded, and both ended the same way: a shell was
# running, its files were replaced underneath it, its helper channel broke, and
# the host restarted the provider adapter several times in the same second.
# Every one of those restarts opens the graphics card's I²C buses through
# `ddcutil`, so the machine ends up with a handful of them contending for one
# bus lock — `flock() for /dev/i2c-7 failed` — and seconds later the kernel
# reports `amdgpu: device lost from bus!`.
#
# `PANEL-1-M` took the release smoke off that hardware, which was real but was
# only ever half the exposure: the smoke runs a shell in a scratch home, while
# build and deploy rewrite the files a *real* session already has open. That
# half was governed by nothing but the author and the assistant remembering,
# and it has now failed twice. This is the same rule with a latch on it.
#
# Both roots matter. The development nest runs straight out of the build tree,
# so a build alone is enough to pull the ground out from under it; an installed
# session runs from the bundle, which is what deploy overwrites.

# The test is what a process is *executing*, read from `/proc/PID/exe`, not what
# its command line says. A command line is text, and this file's own path — and
# the path of anything else that mentions the build tree — is text that matches;
# an interlock that stops a release because a grep was open is an interlock that
# gets commented out. `/proc/PID/exe` is the kernel's own answer, and it stays
# correct even for a binary that has already been unlinked and replaced, which is
# exactly the case this exists to catch.
celestina_refuse_if_running() {
    celestina_interlock_project_root=$1
    celestina_interlock_prefix=$2
    celestina_interlock_pids=

    for celestina_interlock_entry in /proc/[0-9]*; do
        celestina_interlock_pid=${celestina_interlock_entry#/proc/}
        celestina_interlock_exe=$(readlink "$celestina_interlock_entry/exe" 2>/dev/null) || continue
        # A replaced binary reads back as "/path/to/celestina (deleted)", which
        # is the state this interlock exists for. The suffix is quoted because
        # an unquoted `(deleted)` is a glob group, not two literal brackets, in
        # some of the shells this file is sourced from — and stripping nothing
        # would silently let exactly the dangerous case through.
        celestina_interlock_exe=${celestina_interlock_exe%" (deleted)"}
        case $celestina_interlock_exe in
            "$celestina_interlock_project_root"/build/celestina | \
            "$celestina_interlock_project_root"/build/celestina-* | \
            "$celestina_interlock_prefix"/libexec/celestina/celestina | \
            "$celestina_interlock_prefix"/libexec/celestina/celestina-*)
                celestina_interlock_pids="$celestina_interlock_pids $celestina_interlock_pid"
                ;;
        esac
    done
    celestina_interlock_pids=${celestina_interlock_pids# }

    if [ -z "$celestina_interlock_pids" ]; then
        return 0
    fi

    echo "celestina: refusing to rewrite binaries a running shell is executing" >&2
    echo "  live process ids: $celestina_interlock_pids" >&2
    for celestina_interlock_pid in $celestina_interlock_pids; do
        if [ -r "/proc/$celestina_interlock_pid/cmdline" ]; then
            echo "    $celestina_interlock_pid: $(tr '\0' ' ' \
                <"/proc/$celestina_interlock_pid/cmdline")" >&2
        fi
    done
    cat >&2 <<'EOF'
  Replacing these files under a live shell restarts its provider adapter, and
  each restart probes the graphics card over DDC. Concurrent probes on one I2C
  bus have twice ended in `amdgpu: device lost from bus!`.
  Close the development nest (`celestina/scripts/dev-session.sh --stop`) or the
  installed session first, then run this again.
EOF
    return 1
}
