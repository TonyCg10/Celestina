#!/bin/sh
set -eu

# Celestina smoke (design §8): with niri installed, a nested niri runs the
# shell for three seconds with `--metrics` (`dev-session.sh --once`, on a
# session bus of its own so no name of the desktop's bus is touched), and the
# run must exit 0, draw at least one frame on the nested `winit` output,
# report its late frames, and draw no frame at rest (`rest_frames=0`: no
# frame while nothing animated, after the first second). Without niri the
# nested session is skipped with a notice.
usage() {
    echo "usage: scripts/smoke.sh --binary PATH" >&2
    exit 2
}
[ "$#" -eq 2 ] && [ "$1" = "--binary" ] || usage
bin=$2
[ -x "$bin" ] || { echo "smoke: $bin is not an executable" >&2; exit 1; }
case "$bin" in
    /*) ;;
    *) bin=$(pwd)/$bin ;;
esac
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if ! "$bin" --check; then
    echo "smoke: $bin --check failed" >&2
    exit 1
fi

if ! command -v niri >/dev/null 2>&1; then
    echo "smoke: niri is absent; the nested session was not run"
    exit 0
fi
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "smoke: niri is installed but WAYLAND_DISPLAY is empty: the nested" >&2
    echo "  niri needs a host compositor to open its window in" >&2
    exit 1
fi
if ! command -v dbus-run-session >/dev/null 2>&1; then
    echo "smoke: dbus-run-session is absent; the nest needs a bus of its own" >&2
    exit 1
fi

# The clock's minute tick is the one frame a shell at rest draws. The run
# takes a few seconds, so it starts well clear of a minute boundary.
while [ "$(date +%S)" -gt 50 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done

work=$(mktemp -d "${TMPDIR:-/tmp}/celestina-smoke.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
run=$work/run
status=0
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$bin" \
    --once "$run" -- --exit-after 3 --metrics \
    > "$work/niri.log" 2>&1 || status=$?

fail() {
    echo "smoke: $1" >&2
    for f in "$run.out" "$run.err" "$work/niri.log"; do
        [ -f "$f" ] || continue
        echo "--- $(basename "$f")" >&2
        tail -n 40 "$f" >&2
    done
    exit 1
}

[ "$status" -eq 0 ] || fail "the nested niri exited with $status"
[ -f "$run.status" ] || fail "the shell never ran inside the nest"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status"

line=$(grep '^winit ' "$run.out" || true)
[ -n "$line" ] || fail "no metrics line for the winit output"
field() {
    printf '%s\n' "$line" | tr ' ' '\n' | sed -n "s/^$1=//p"
}
frames=$(field frames)
rest=$(field rest_frames)
late=$(field late)
[ -n "$frames" ] && [ "$frames" -ge 1 ] || fail "no frame was drawn: $line"
[ -n "$late" ] || fail "late frames are not reported: $line"
[ "$rest" = 0 ] || fail "frames were drawn at rest: $line"

echo "smoke: $line"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: /' || true
grep 'glyph atlas at exit' "$run.err" | sed 's/^.*metrics: /smoke: /' || true
echo "smoke: OK"
