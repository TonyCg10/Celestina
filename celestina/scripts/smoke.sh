#!/bin/sh
set -eu

# Celestina smoke (design §8): with niri installed, a nested niri runs the
# shell for four seconds with `--metrics` (`dev-session.sh --once`, on a
# session bus of its own so no name of the desktop's bus is touched), and the
# run must exit 0, draw at least one frame on the nested `winit` output,
# report its late frames, and draw no frame at rest (`rest_frames=0`: no
# frame while nothing animated, after the first second). Inside the same nest
# `smoke-notify.sh` sends one Notify (long body, one second timeout) to the
# shell's server on the nest's own bus; the shell must answer with an id and
# raise the banners' surface, and the banner is gone again before the run
# ends. A second nested run then opens the launcher with `celestina launcher
# toggle`, keeps it open for three seconds (the nest answers an idle client
# about once a second), closes it the same way, and must still exit 0 with
# both toggles logged and a frame drawn on the launcher's surface. A third
# nested run turns the bar duty on (its own throwaway XDG_CONFIG_HOME) and
# must draw on the wide top surface with no frame at rest. A fourth nested
# run turns the wallpaper duty on over a generated fixture picture (its own
# throwaway XDG_CONFIG_HOME) and must show a `celestina-wallpaper` surface in
# the Background layer with at most two frames at rest. A fifth nested
# run starts the shell with the three energy duties `on` (a throwaway
# XDG_CONFIG_HOME holding `celestina/shell.json`) and the system bus pointed
# at a socket that does not exist, so no real logind is ever reached; it must
# exit 0, report the idle worker armed or unavailable and the night light
# applied or not applied (the nest has no DRM gamma), and answer `caffeine
# toggle` and `session suspend` (refused: no locker yet). `session reboot` is
# deliberately not sent: it would be a real logind call on a bus the nest does
# not own. Without niri the nested session is skipped with a notice; without busctl and gdbus
# the notification part is.
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
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done

probe_args=
if command -v busctl >/dev/null 2>&1 || command -v gdbus >/dev/null 2>&1; then
    probe_args="--probe $here/smoke-notify.sh"
else
    echo "smoke: neither busctl nor gdbus is installed; no notification sent"
fi
# The probe refuses the desktop's own bus: it must find a different address.
CELESTINA_HOST_BUS=${DBUS_SESSION_BUS_ADDRESS:-}
export CELESTINA_HOST_BUS

work=$(mktemp -d "${TMPDIR:-/tmp}/celestina-smoke.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
# The generated wrapper below quotes paths with single quotes, so a path that
# holds one cannot be written safely: refuse it rather than build a script
# that means something else.
case "$work" in
    *\'*)
        echo "smoke: the temporary directory $work contains a single quote;" >&2
        echo "  set TMPDIR to a path without one" >&2
        exit 1
        ;;
esac
# Every nested run gets a throwaway configuration home and a system bus that
# does not exist, so a landing's smoke never acts through the author's real
# settings or their real logind. Runs 1 and 2 start with the default
# `shell.json` (every duty off); the wallpaper and energy runs replace the
# home with their own, and every run keeps the dead system bus.
xdg_default=$work/xdg-default
mkdir -p "$xdg_default/celestina"
cat > "$xdg_default/celestina/shell.json" <<'JSON'
{
  "schema_version": 1
}
JSON
XDG_CONFIG_HOME=$xdg_default
DBUS_SYSTEM_BUS_ADDRESS=unix:path=$work/no-system-bus
export XDG_CONFIG_HOME DBUS_SYSTEM_BUS_ADDRESS
run=$work/run
status=0
# shellcheck disable=SC2086 # probe_args is two words or none
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$bin" \
    $probe_args --once "$run" -- --exit-after 4 --metrics \
    > "$work/niri.log" 2>&1 || status=$?

fail() {
    echo "smoke: $1" >&2
    for f in "$run.out" "$run.err" "$run.probe" "$work/niri.log"; do
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

# One metrics line per surface of the output (the panel's, and the banners'
# once one was raised): frames and rest frames are summed, the rest is read
# from the first.
lines=$(grep '^winit ' "$run.out" || true)
[ -n "$lines" ] || fail "no metrics line for the winit output"
line=$(printf '%s\n' "$lines" | head -n 1)
sum() {
    printf '%s\n' "$lines" | tr ' ' '\n' | sed -n "s/^$1=//p" |
        awk '{ n += $1 } END { print n + 0 }'
}
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
[ "$rest" -le "$allowed" ] || fail "frames were drawn at rest ($rest, at most $allowed): $lines"

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
# The bar duty (CEL-2-A): a third nested run with a temporary
# XDG_CONFIG_HOME whose `celestina/shell.json` turns `duties.bar` on, so the
# Island's surface is the wide top one. It must exit 0, draw at least one
# frame on every metrics line of the output, draw none at rest, and log the
# surface it asked for with the bar on and the wide anchor. The real
# configuration directory is never read or written.
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
mkdir -p "$work/xdg-bar/celestina"
printf '{"schema_version":1,"duties":{"bar":"on"}}\n' > "$work/xdg-bar/celestina/shell.json"
run=$work/bar
status=0
XDG_CONFIG_HOME=$work/xdg-bar timeout 60 "$here/dev-session.sh" --own-bus --monitor none \
    --binary "$bin" --once "$run" -- --exit-after 4 --metrics \
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
[ "$rest" -le 1 ] || fail "frames were drawn at rest with the bar on ($rest): $lines"
grep 'island surface requested' "$run.err" | grep -q 'bar=true' ||
    fail "the Island did not ask for the bar's surface"
grep 'island surface requested' "$run.err" | grep -q 'TopWide' ||
    fail "the Island's surface was not anchored wide"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the bar on"
fi
echo "smoke: bar on: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$rest)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: bar on: /' || true
# The wallpaper duty (CEL-2-E): the shell runs ten seconds with a private
# XDG_CONFIG_HOME whose `celestina/shell.json` turns `duties.wallpaper` on and
# points `wallpaper.folder` at a fixture folder holding one generated PNG
# (the author's own configuration and pictures are never read). A probe inside
# the nest lists the compositor's layers: (a) a `celestina-wallpaper` surface
# must sit in `Background` on the nested output, (b) the metrics show no frame
# at rest beyond an allowance of two, the same kind as the notification's: the
# picture arriving (decoded off the loop, so after the first second) and its
# redecode when the surface scale changes after the first configure are each a
# frame somebody needed, though nothing animates in it; any other frame at
# rest fails, and (c) is not run: no layer-shell client that could hold the Background
# layer exists in the repository, and none is invented here; the owner
# detection that makes the shell refuse such an output is covered by the unit
# tests of `src/wallpaper` and `src/providers/wallpaper.rs`.
if ! command -v python3 >/dev/null 2>&1; then
    fail "python3 is absent: the wallpaper gate cannot run (it generates its fixture and reads the layers with it), so this smoke is not OK"
fi
fixture=$work/wallpapers
config_home=$work/config
mkdir -p "$fixture" "$config_home/celestina"
python3 - "$fixture/default.png" <<'PNG'
import struct, sys, zlib

# A 64x36 gradient, 8-bit RGB: stdlib only.
width, height = 64, 36
rows = b"".join(
    b"\x00" + bytes(c for x in range(width) for c in (x * 4, y * 7, 160))
    for y in range(height)
)

def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows))
    + chunk(b"IEND", b"")
)
open(sys.argv[1], "wb").write(png)
PNG
cat > "$config_home/celestina/shell.json" <<JSON
{"schema_version":1,"duties":{"wallpaper":"on"},"wallpaper":{"folder":"$fixture"}}
JSON
run=$work/wallpaper
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
XDG_CONFIG_HOME=$config_home timeout 60 "$here/dev-session.sh" --own-bus \
    --monitor none --binary "$bin" --probe "$here/wallpaper-layers-probe.sh" \
    --once "$run" -- --exit-after 10 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (wallpaper run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (wallpaper run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (wallpaper run)"
[ "$(cat "$run.probe.status" 2>/dev/null)" = 0 ] ||
    fail "the layers probe failed (wallpaper run)"
python3 - "$run.probe" <<'LAYERS' || fail "no celestina-wallpaper surface in the Background layer of the nested output"
import json, sys

lines = open(sys.argv[1]).read().split("\n")
at = lines.index("probe: layers (json)") + 1
layers = json.loads(lines[at])
found = [l for l in layers
         if l["namespace"] == "celestina-wallpaper"
         and l["layer"] == "Background" and l["output"] == "winit"]
if not found:
    sys.exit(1)
print("smoke: celestina-wallpaper is in Background on winit")
LAYERS
wp_lines=$(grep '^winit ' "$run.out" || true)
[ -n "$wp_lines" ] || fail "no metrics line for the winit output (wallpaper run)"
wp_frames=$(printf '%s\n' "$wp_lines" | tr ' ' '\n' | sed -n 's/^frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
wp_rest=$(printf '%s\n' "$wp_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
[ "$wp_frames" -ge 1 ] || fail "no frame was drawn (wallpaper run): $wp_lines"
[ "$wp_rest" -le 2 ] ||
    fail "frames were drawn at rest with the wallpaper present ($wp_rest, at most 2): $wp_lines"
if grep -q 'wallpaper: output held by another client' "$run.err"; then
    fail "the shell found the nested output held (wallpaper run)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the wallpaper on"
fi
echo "smoke: wallpaper present, frames=$wp_frames rest_frames=$wp_rest over $(printf '%s\n' "$wp_lines" | wc -l) surfaces"
echo "smoke: wallpaper held-output variant skipped: no layer-shell client in the repository (covered by unit tests)"

# The energy duties: the shell starts with idle, caffeine and night light all
# `on` from a throwaway configuration home, and its own client sends two verbs.
# logind lives on the system bus, which a nest does not own: the wrapper points
# DBUS_SYSTEM_BUS_ADDRESS at a socket that is not there, so the shell can only
# report logind unavailable and nothing reaches the real one. `session reboot`
# is never sent for the same reason; `session suspend` is refused before any
# bus call because no locker exists yet.
xdg=$work/xdg
mkdir -p "$xdg/celestina"
cat > "$xdg/celestina/shell.json" <<'JSON'
{
  "schema_version": 1,
  "duties": { "idle": "on", "caffeine": "on", "nightlight": "on" },
  "energy": { "caffeine": true, "night_light": true }
}
JSON
energy=$work/energy-run
cat > "$energy" <<WRAPPER
#!/bin/sh
# Runs the shell with the energy duties on, sends two verbs once it answers,
# and exits with the shell's status.
XDG_CONFIG_HOME='$xdg'
DBUS_SYSTEM_BUS_ADDRESS='unix:path=$work/no-system-bus'
export XDG_CONFIG_HOME DBUS_SYSTEM_BUS_ADDRESS
RUST_LOG=info "$bin" "\$@" &
shell=\$!
send() {
    tries=0
    until "$bin" "\$@"; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-verb-failed \$*" >&2; return 1; }
        sleep 0.2
    done
}
send caffeine toggle
send session suspend
wait "\$shell"
WRAPPER
chmod +x "$energy"
run=$work/energy
status=0
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$energy" \
    --once "$run" -- --exit-after 6 \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (energy run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (energy run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (energy run)"
if grep -q 'smoke-verb-failed' "$run.err"; then
    fail "an energy verb did not reach the shell"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the energy duties on"
fi
grep -Eq 'idle: (armed|unavailable)' "$run.err" ||
    fail "the idle worker reported neither armed nor unavailable"
grep -Eq 'night light: (applied|not applied)' "$run.err" ||
    fail "the night light reported neither applied nor not applied"
grep -q 'caffeine: now off' "$run.err" || fail "caffeine toggle was not answered"
if grep -q 'caffeine: held' "$run.err"; then
    fail "caffeine was held: a logind other than the nest's was reached"
fi
grep -q 'session suspend: requested' "$run.err" ||
    fail "session suspend was not requested"
grep -q 'session Suspend: refused: no locker yet' "$run.err" ||
    fail "session suspend was not refused for want of a locker"
echo "smoke: $(grep -Eo 'idle: (armed|unavailable)' "$run.err" | head -n 1), $(grep -Eo 'night light: (applied|not applied)' "$run.err" | head -n 1), suspend refused without a locker"
echo "smoke: OK"
