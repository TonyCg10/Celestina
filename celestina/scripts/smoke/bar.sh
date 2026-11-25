#!/bin/sh
set -eu

# Smoke phase `bar`: a nested run starts the shell on the bar's default layout
# (its own throwaway XDG_CONFIG_HOME with every duty off; the bar is not a
# duty since the cutover, CEL-2-M) and must draw on the wide top surface with
# no frame at rest.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only bar`.
SMOKE_PHASE=bar
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

make_shell_run
# The bar (CEL-2-A; always drawn since the cutover, CEL-2-M): a third nested
# run with a temporary XDG_CONFIG_HOME whose `celestina/shell.json` has every
# duty off and the default bar layout, so the Island's surface is the wide
# top one. It must exit 0, draw at least one frame on every metrics line of
# the output, draw none at rest, and log the surface it asked for with the
# wide anchor. The real configuration directory is never read or written.
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
mkdir -p "$work/xdg-bar/celestina"
printf '{"schema_version":1,"duties":%s}\n' "$(duties)" > "$work/xdg-bar/celestina/shell.json"
run=$work/bar
status=0
nest 60 XDG_CONFIG_HOME="$work/xdg-bar" "$here/dev-session.sh" --own-bus --monitor none \
    --binary "$shell_run" --once "$run" -- --exit-after 4 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (bar run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (bar run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (bar run)"
lines=$(grep '^winit ' "$run.out" || true)
[ -n "$lines" ] || fail "no metrics line for the winit output (bar run)"
printf '%s\n' "$lines" | while IFS= read -r l; do
    n=$(printf '%s\n' "$l" | tr ' ' '\n' | sed -n 's/^frames=//p')
    [ "${n:-0}" -ge 1 ] || exit 1
done || fail "a surface drew no frame with the bar on: $lines"
rest=$(sum rest_frames)
# The nest sometimes delivers niri's first events about two seconds after
# start (the nest's window size varies with the host); that one model update
# repaints the bar legitimately and counts as a rest frame. A shell that
# keeps redrawing (a stray one-second tick, for example) draws one per
# second, so more than one fails.
[ "$rest" -le 1 ] || rest_fail "$run.err" "frames were drawn at rest with the bar on ($rest): $lines"
grep 'island surface requested' "$run.err" | grep -q 'TopWide' ||
    fail "the Island's surface was not anchored wide"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the bar on"
fi
echo "smoke: bar on: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$rest)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: bar on: /' || true
