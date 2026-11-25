#!/bin/sh
set -eu

# Smoke phase `notify` (design §8): a nested niri runs the shell for four
# seconds with `--metrics` (`dev-session.sh --once`, on a session bus of its
# own so no name of the desktop's bus is touched), and the run must exit 0,
# draw at least one frame on the nested `winit` output, report its late
# frames, and draw no frame at rest (`rest_frames=0`: no frame while nothing
# animated, after the first second). Inside the same nest `smoke-notify.sh`
# sends one Notify (long body, one second timeout) to the shell's server on
# the nest's own bus; the shell must answer with an id and raise the banners'
# surface, and the banner is gone again before the run ends. Without busctl
# and gdbus the notification part is skipped with a notice.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only notify`.
SMOKE_PHASE=notify
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

# The clock's minute tick is the one frame a shell at rest draws. The run
# takes a few seconds, so it starts well clear of a minute boundary.
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done

probe_args=
if command -v busctl >/dev/null 2>&1 || command -v gdbus >/dev/null 2>&1; then
    probe_args="--probe $here/smoke-notify.sh"
else
    echo "smoke: neither busctl nor gdbus is installed; no notification sent"
fi
make_shell_run
run=$work/run
status=0
# shellcheck disable=SC2086 # probe_args is two words or none
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$shell_run" \
    $probe_args --once "$run" -- --exit-after 4 --metrics \
    > "$work/niri.log" 2>&1 || status=$?

[ "$status" -eq 0 ] || fail "the nested niri exited with $status"
[ -f "$run.status" ] || fail "the shell never ran inside the nest"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status"

# One metrics line per surface of the output (the panel's, and the banners'
# once one was raised): frames and rest frames are summed, the rest is read
# from the first.
lines=$(grep '^winit ' "$run.out" || true)
[ -n "$lines" ] || fail "no metrics line for the winit output"
line=$(printf '%s\n' "$lines" | head -n 1)
frames=$(sum frames)
rest=$(sum rest_frames)
late=$(printf '%s\n' "$line" | tr ' ' '\n' | sed -n 's/^late=//p')
[ "$frames" -ge 1 ] || fail "no frame was drawn: $lines"
[ -n "$late" ] || fail "late frames are not reported: $line"
# A shell at rest draws nothing after the first second. The notification
# changes the Island's state twice after it (posted, then expired): each is a
# frame somebody needed, though nothing animates in it, so a run that sent one
# allows exactly those two and any other frame at rest still fails.
allowed=0
[ -z "$probe_args" ] || allowed=2
[ "$rest" -le "$allowed" ] || rest_fail "$run.err" "frames were drawn at rest ($rest, at most $allowed): $lines"

if [ -n "$probe_args" ]; then
    [ "$(cat "$run.probe.status" 2>/dev/null)" = 0 ] ||
        fail "the notification probe failed (it prints the nest's bus address)"
    id=$(sed -n 's/^probe: id=//p' "$run.probe")
    [ -n "$id" ] && [ "$id" -gt 0 ] || fail "Notify returned no id"
    grep -q 'banners surface requested.*created=true' "$run.err" ||
        fail "the shell did not raise the banners' surface for notification $id"
    grep -q 'banners surface released' "$run.err" ||
        fail "the banner of notification $id did not expire inside the run"
    echo "smoke: notification $id raised the banners' surface and it expired"
fi
echo "smoke: $line (frames=$frames rest_frames=$rest over $(printf '%s\n' "$lines" | wc -l) surfaces)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: /' || true
grep 'glyph atlas at exit' "$run.err" | sed 's/^.*metrics: /smoke: /' || true
