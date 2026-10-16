#!/bin/sh
set -eu

# Celestina smoke (design §8): with niri installed, a nested niri runs the
# shell for three seconds with `--metrics` (`dev-session.sh --once`, on a
# session bus of its own so no name of the desktop's bus is touched), and the
# run must exit 0, draw at least one frame on the nested `winit` output,
# report its late frames, and draw no frame at rest (`rest_frames=0`: no
# frame while nothing animated, after the first second). A second nested run
# then opens the launcher with `celestina launcher toggle`, keeps it open for
# three seconds (the nest answers an idle client about once a second), closes
# it the same way, and must still exit 0 with both toggles logged and a frame
# drawn on the launcher's surface. Without niri the nested session is skipped
# with a notice.
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

# The launcher: the shell runs six seconds while its own client, on the
# nest's bus, opens the launcher and closes it again. The runtime's debug
# lines name every surface a frame was drawn on: the Island's is surface 1,
# the launcher's surface 2. The handlers' debug lines carry each configure:
# the launcher fills the output, so its size is the nest's, never the
# card's 560×400 nor 0×0.
toggles=$work/launcher-run
cat > "$toggles" <<WRAPPER
#!/bin/sh
# Runs the shell with the given arguments, toggles the launcher open and
# closed through its client once the shell answers, and exits with the
# shell's status.
RUST_LOG=info,celestina_wayland::runtime=debug,celestina_wayland::handlers=debug "$bin" "\$@" &
shell=\$!
toggle() {
    tries=0
    until "$bin" launcher toggle; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-toggle-failed" >&2; return 1; }
        sleep 0.2
    done
}
toggle && sleep 3 && toggle
wait "\$shell"
WRAPPER
chmod +x "$toggles"
run=$work/launcher
status=0
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$toggles" \
    --once "$run" -- --exit-after 6 \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (launcher run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (launcher run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (launcher run)"
if grep -q 'smoke-toggle-failed' "$run.err"; then
    fail "the launcher toggle did not reach the shell"
fi
grep -q 'launcher: opened' "$run.err" || fail "the launcher did not open"
grep -q 'frame drawn frame=[0-9]* surface=2 ' "$run.err" ||
    fail "no frame was drawn on the launcher's surface"
launcher_size=$(sed -n 's/.*configure surface=2 size=(\([0-9]*\), \([0-9]*\)).*/\1x\2/p' "$run.err" | head -n 1)
case "$launcher_size" in
    '' ) fail "the launcher's surface was never configured" ;;
    0x* | *x0 | 560x400) fail "the launcher does not fill the output (configure $launcher_size)" ;;
esac
echo "smoke: launcher configured at ${launcher_size} (the whole output)"
grep -q 'launcher: closed by the command' "$run.err" ||
    fail "the launcher did not close"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the launcher open"
fi
echo "smoke: launcher opened and closed"
echo "smoke: OK"
