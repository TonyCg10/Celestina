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
# not own. A sixth and a seventh nested run (CEL-2-K) put `sysmon`, `clock`
# and `tray` (and, in the seventh, `weather` with a Dublin location) in the
# bar of a private XDG_CONFIG_HOME, with a fake `curl` first on PATH that logs
# its argv and never reaches the network: the sixth, with no location, must
# make no request at all; the seventh must make the one documented forecast
# request. An eighth nested run starts the shell with the clipboard duty `on`
# (CEL-2-G) and drives it with `wl-copy`, a client that demonstrably speaks
# `ext-data-control` to the nest (checked below with WAYLAND_DEBUG) and that
# only ever talks to the nest's own compositor: the run refuses to start a
# client when its display is the host's. Fixed test strings are copied, a
# selection carrying the password-manager hint, and a PNG; the history, the
# state file (mode, content) and the shell's own re-selection are checked, the
# duty is switched off and on in `shell.json`, and the journal must hold none of
# the test strings. Without niri the nested session is skipped with a notice;
# without busctl and gdbus the notification part is.
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
# settings, their clipboard history or their real logind. Runs 1 and 2 start with the default
# `shell.json` (every duty off); the wallpaper and energy runs replace the
# home with their own, and every run keeps the dead system bus.
xdg_default=$work/xdg-default
mkdir -p "$xdg_default/celestina"
cat > "$xdg_default/celestina/shell.json" <<'JSON'
{
  "schema_version": 1
}
JSON
# A shell whose clipboard duty is off deletes the history file under
# `$XDG_STATE_HOME`: every run therefore gets a throwaway state home, so no run
# can touch the author's real file.
xdg_state=$work/xdg-state
mkdir -p "$xdg_state"
XDG_CONFIG_HOME=$xdg_default
XDG_STATE_HOME=$xdg_state
DBUS_SYSTEM_BUS_ADDRESS=unix:path=$work/no-system-bus
export XDG_CONFIG_HOME XDG_STATE_HOME DBUS_SYSTEM_BUS_ADDRESS
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

# The glance workers (CEL-2-K): the system monitor and the weather. Each run
# starts the shell through a wrapper that puts a fake `curl` first on PATH (a
# directory of the run's own), so the real curl is never reachable from the
# shell: the fake appends its arguments, one per line, to a log and answers
# from the file `curl-body` when there is one (exit 0) or fails (exit 1). The
# wrapper also records which curl PATH resolves to, and the run fails unless
# it is the fake. The bar holds `sysmon`, `clock` and `tray`: without a
# location nothing may ask for the weather; with one, the one request is the
# documented forecast URL, fetched with `--disable` and carrying neither the
# label nor anything else the person typed.
glance_run() {
    # $1 = run name, $2 = shell.json body, $3 = canned curl body or empty
    g_name=$1
    g_home=$work/xdg-$g_name
    g_bin=$work/fakebin-$g_name
    g_log=$work/$g_name.curl-log
    mkdir -p "$g_home/celestina" "$g_bin"
    printf '%s\n' "$2" > "$g_home/celestina/shell.json"
    cat > "$g_bin/curl" <<FAKE
#!/bin/sh
for a in "\$@"; do printf '%s\n' "\$a" >> '$g_log'; done
printf -- '--call--\n' >> '$g_log'
if [ -f '$g_bin/curl-body' ]; then cat '$g_bin/curl-body'; exit 0; fi
exit 1
FAKE
    chmod +x "$g_bin/curl"
    [ -z "$3" ] || printf '%s' "$3" > "$g_bin/curl-body"
    cat > "$work/$g_name-run" <<WRAPPER
#!/bin/sh
XDG_CONFIG_HOME='$g_home'
PATH='$g_bin':\$PATH
export XDG_CONFIG_HOME PATH
echo "smoke-curl=\$(command -v curl)" >&2
RUST_LOG=info exec "$bin" "\$@"
WRAPPER
    chmod +x "$work/$g_name-run"
    run=$work/$g_name
    status=0
    while [ "$(date +%S)" -gt 40 ] || [ "$(date +%S)" -lt 2 ]; do
        sleep 1
    done
    timeout 90 "$here/dev-session.sh" --own-bus --monitor none --binary "$work/$g_name-run" \
        --once "$run" -- --exit-after 14 --metrics \
        > "$work/niri.log" 2>&1 || status=$?
    [ "$status" -eq 0 ] || fail "the nested niri exited with $status ($g_name run)"
    [ -f "$run.status" ] || fail "the shell never ran inside the nest ($g_name run)"
    shell_status=$(cat "$run.status")
    [ "$shell_status" = 0 ] || fail "the shell exited with $shell_status ($g_name run)"
    grep -qx "smoke-curl=$g_bin/curl" "$run.err" ||
        fail "curl did not resolve to the fake one ($g_name run): $(grep 'smoke-curl=' "$run.err")"
    if grep -q 'panicked' "$run.err"; then
        fail "the shell panicked ($g_name run)"
    fi
    grep -q 'system monitor on' "$run.err" ||
        fail "the system monitor worker did not start ($g_name run)"
    g_lines=$(grep '^winit ' "$run.out" || true)
    [ -n "$g_lines" ] || fail "no metrics line for the winit output ($g_name run)"
    g_frames=$(printf '%s\n' "$g_lines" | tr ' ' '\n' | sed -n 's/^frames=//p' |
        awk '{ n += $1 } END { print n + 0 }')
    g_rest=$(printf '%s\n' "$g_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
        awk '{ n += $1 } END { print n + 0 }')
    [ "$g_frames" -ge 1 ] || fail "no frame was drawn ($g_name run): $g_lines"
    [ "$g_rest" -le 1 ] || fail "frames were drawn at rest ($g_name run, $g_rest): $g_lines"
    g_calls=0
    [ ! -f "$g_log" ] || g_calls=$(grep -c '^--call--$' "$g_log" || true)
    echo "smoke: $g_name: frames=$g_frames rest_frames=$g_rest curl_calls=$g_calls"
    grep '^rss_mib=' "$run.out" | sed "s/^/smoke: $g_name: /" || true
}
glance_run glance '{"schema_version":1,"duties":{"bar":"on"},"bar":{"end":["sysmon","clock","tray"]}}' ''
[ ! -s "$work/glance.curl-log" ] ||
    fail "the shell ran curl with no location set: $(tr '\n' ' ' < "$work/glance.curl-log")"
echo "smoke: glance: no location, no request"

glance_run glance-weather '{"schema_version":1,"duties":{"bar":"on"},"bar":{"end":["weather","sysmon","clock","tray"]},"weather":{"latitude":53.35,"longitude":-6.26,"label":"Dublin"}}' '{"current":{"temperature_2m":12.4,"weather_code":3,"is_day":1}}'
wlog=$work/glance-weather.curl-log
[ -s "$wlog" ] || fail "the weather worker never ran curl (glance-weather run)"
wurl='https://api.open-meteo.com/v1/forecast?latitude=53.35&longitude=-6.26&current=temperature_2m,weather_code,is_day'
awk '/^--call--$/ { if (n) print last; n = 0; next } { n++; last = $0 } END { if (n) print last }' "$wlog" |
    while IFS= read -r l; do [ "$l" = "$wurl" ] || exit 1; done ||
    fail "a curl call did not end in the forecast URL (glance-weather run): $(tr '\n' ' ' < "$wlog")"
grep -qx -- '--disable' "$wlog" || fail "curl ran without --disable (glance-weather run)"
if grep -qi 'dublin' "$wlog"; then
    fail "the label reached curl's arguments (glance-weather run)"
fi
grep -q 'weather on' "$work/glance-weather.err" || fail "the weather worker did not start (glance-weather run)"
echo "smoke: glance-weather: $(grep -c '^--call--$' "$wlog") request(s), forecast URL, --disable, no label"

# The clipboard duty (CEL-2-G). Inside the nest, the shell runs with
# `duties.clipboard` on, a throwaway configuration home and state home, and the
# wrapper below plays the part of a few applications with `wl-copy`/`wl-paste`
# (wl-clipboard 2.3 speaks ext-data-control; the WAYLAND_DEBUG capture proves
# it for this nest). The author's real clipboard is never reachable: the
# wrapper refuses to start any client unless WAYLAND_DISPLAY is the nest's own,
# different from the host's. Only fixed test strings are copied. The shell is
# observed through its journal (counts, sizes, reasons: never content) and
# through the state file; there is no verb that returns content to read.
for tool in wl-copy wl-paste; do
    command -v "$tool" >/dev/null 2>&1 ||
        fail "$tool is absent: the clipboard gate cannot run (wl-clipboard), so this smoke is not OK"
done
clip_alpha=clipsmoke-visible-7f3a91
clip_beta=clipsmoke-secret-b44e02
clip_state=$work/clip-state
clip_config=$work/clip-config
mkdir -p "$clip_state" "$clip_config/celestina"
cat > "$clip_config/celestina/shell.json" <<'JSON'
{"schema_version":1,"duties":{"clipboard":"on"}}
JSON
# The PNG the wallpaper gate generated: a real, small, decodable picture.
clip_png=$fixture/default.png
[ -f "$clip_png" ] || fail "the fixture picture of the wallpaper gate is missing"
run=$work/clipboard
clip=$work/clipboard-run
{
    printf '#!/bin/sh\n'
    printf "BIN='%s'\nRUN='%s'\nXDG_CONFIG_HOME='%s'\nXDG_STATE_HOME='%s'\n" \
        "$bin" "$run" "$clip_config" "$clip_state"
    printf "HOST_DISPLAY='%s'\nPNG='%s'\nALPHA='%s'\nBETA='%s'\n" \
        "${WAYLAND_DISPLAY:-}" "$clip_png" "$clip_alpha" "$clip_beta"
    cat <<'WRAPPER'
# Runs the shell with the clipboard duty on and drives it with wl-copy and the
# shell's own verbs, writing what it saw to $RUN.clip; exits with the shell's
# status.
export XDG_CONFIG_HOME XDG_STATE_HOME
if [ -z "${NIRI_SOCKET:-}" ] || [ -z "${WAYLAND_DISPLAY:-}" ] ||
    [ "$WAYLAND_DISPLAY" = "$HOST_DISPLAY" ]; then
    echo "smoke-clipboard-unsafe-display" >&2
    exit 1
fi
STATE_FILE=$XDG_STATE_HOME/celestina/clipboard.json
pids=
RUST_LOG=debug "$BIN" "$@" &
shell=$!
count() { grep -c -e "$1" "$RUN.err" || true; }
# wait_count PATTERN N: until PATTERN has N lines in the journal (20 s).
wait_count() {
    n=0
    while [ "$(count "$1")" -lt "$2" ]; do
        n=$((n + 1))
        [ "$n" -lt 200 ] || { echo "smoke-clipboard-timeout $1 $2" >&2; return 1; }
        sleep 0.1
    done
}
note() { printf '%s\n' "$*" >> "$RUN.clip"; }
: > "$RUN.clip"
wait_count 'clipboard duty on' 1 || exit 1
sleep 1

# 1. A text selection grows the history to one.
WAYLAND_DEBUG=1 wl-copy -f -- "$ALPHA" 2> "$RUN.wldebug.text" &
pids="$pids $!"
wait_count 'the history changed.*total=1' 1 || exit 1

# 2. A selection carrying the password-manager hint adds nothing.
WAYLAND_DEBUG=1 wl-copy -f --sensitive -- "$BETA" 2> "$RUN.wldebug.hint" &
pids="$pids $!"
wait_count 'carries an exclusion hint' 1 || exit 1
sleep 1

# 3. A PNG selection is accepted.
wl-copy -f --type image/png < "$PNG" &
pids="$pids $!"
wait_count 'the history changed.*total=2' 1 || exit 1

# 4. The state file: mode, owner directory mode, content.
note "file-mode=$(stat -c %a "$STATE_FILE" 2>/dev/null)"
note "dir-mode=$(stat -c %a "$XDG_STATE_HOME/celestina" 2>/dev/null)"
note "dir-entries=$(ls -A "$XDG_STATE_HOME/celestina" 2>/dev/null | wc -l)"
grep -q -F -e "$ALPHA" "$STATE_FILE" && note "text-in-file=yes" || note "text-in-file=no"
grep -q -F -e "$BETA" "$STATE_FILE" && note "hinted-in-file=yes" || note "hinted-in-file=no"
if grep -q -a -e iVBOR -e PNG -e image/png "$STATE_FILE"; then
    note "image-in-file=yes"
else
    note "image-in-file=no"
fi

# 5. The shell re-selects an entry: no duplicate, and what comes back is
#    byte-identical. The order is [image, text]; index 1 is the text.
"$BIN" clipboard select 1 || note "verb-failed select 1"
wait_count 'clipboard select: requested' 1 || exit 1
sleep 1
if [ "$(timeout 5 wl-paste --no-newline 2>/dev/null)" = "$ALPHA" ]; then
    note "paste-text=match"
else
    note "paste-text=mismatch"
fi
# Now the order is [text, image]; index 1 is the image.
"$BIN" clipboard select 1 || note "verb-failed select 1 (image)"
wait_count 'clipboard select: requested' 2 || exit 1
sleep 1
timeout 5 wl-paste --type image/png > "$RUN.paste.png" 2>/dev/null || true
if cmp -s "$RUN.paste.png" "$PNG"; then
    note "paste-image=match"
else
    note "paste-image=mismatch"
fi
rm -f "$RUN.paste.png"
sleep 1

# 6. Switching the duty off in shell.json deletes the file and empties memory.
printf '{"schema_version":1,"duties":{"clipboard":"off"}}\n' > "$XDG_CONFIG_HOME/celestina/shell.json.tmp"
mv "$XDG_CONFIG_HOME/celestina/shell.json.tmp" "$XDG_CONFIG_HOME/celestina/shell.json"
wait_count 'clipboard duty off' 1 || exit 1
# The reaper deletes after the duty reports off: wait for its own word.
wait_count 'clipboard: the history file was deleted' 1 || exit 1
if [ -e "$STATE_FILE" ]; then note "file-after-off=present"; else note "file-after-off=absent"; fi
"$BIN" clipboard select 0 || note "verb-failed select 0 (off)"
wait_count 'the duty is off' 1 || exit 1

# 7. Switching it on again starts empty.
printf '{"schema_version":1,"duties":{"clipboard":"on"}}\n' > "$XDG_CONFIG_HOME/celestina/shell.json.tmp"
mv "$XDG_CONFIG_HOME/celestina/shell.json.tmp" "$XDG_CONFIG_HOME/celestina/shell.json"
wait_count 'clipboard duty on' 2 || exit 1
sleep 1
if [ -e "$STATE_FILE" ]; then note "file-after-on=present"; else note "file-after-on=absent"; fi

for p in $pids; do kill "$p" 2>/dev/null || true; done
wait "$shell"
WRAPPER
} > "$clip"
chmod +x "$clip"
status=0
# The shell lives 30 s (`--exit-after 30`) plus the nest's start: it starts
# early in the minute so the clock's minute tick, a frame at rest of its own,
# cannot fall inside the run and be counted against the duty's allowance.
while [ "$(date +%S)" -gt 22 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
timeout 90 "$here/dev-session.sh" --own-bus --monitor none --binary "$clip" \
    --once "$run" -- --exit-after 30 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (clipboard run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (clipboard run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (clipboard run)"
if grep -q 'smoke-clipboard-' "$run.err"; then
    fail "the clipboard driver failed: $(grep 'smoke-clipboard-' "$run.err" | head -n 3)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the clipboard duty on"
fi
if grep -q 'verb-failed' "$run.clip"; then
    fail "a clipboard verb did not reach the shell: $(grep verb-failed "$run.clip")"
fi
# wl-copy must have spoken ext-data-control, and the hinted copy must have
# offered the hint, or the rest of the gate proves nothing.
grep -q 'ext_data_control_source_v1' "$run.wldebug.text" ||
    fail "wl-copy did not speak ext-data-control to the nest"
grep -q 'x-kde-passwordManagerHint' "$run.wldebug.hint" ||
    fail "wl-copy --sensitive did not offer x-kde-passwordManagerHint"
want() {
    grep -qx "$1" "$run.clip" || fail "clipboard: expected '$1' in: $(tr '\n' ' ' < "$run.clip")"
}
want file-mode=600
want dir-mode=700
want dir-entries=1
want text-in-file=yes
want hinted-in-file=no
want image-in-file=no
want paste-text=match
want paste-image=match
want file-after-off=absent
want file-after-on=absent
# The history: one text, then the hinted copy added nothing, then the image,
# and the shell's own re-selections (twice) added no entry.
changes=$(grep 'the history changed' "$run.err" | sed -n 's/.*total=\([0-9]*\).*/\1/p' | tr '\n' ' ')
case "$changes" in
    *'3 '*) fail "the history grew past two entries ($changes)" ;;
esac
grep -q 'the history changed.*total=1' "$run.err" || fail "a text selection did not grow the history"
grep -q 'the history changed.*total=2' "$run.err" || fail "an image selection was not accepted"
[ "$(grep -c 'skipped a selection that carries an exclusion hint' "$run.err")" -eq 1 ] ||
    fail "the hinted selection was not skipped exactly once"
[ "$(grep -c 'skipped the echo of its own selection' "$run.err")" -ge 2 ] ||
    fail "the shell's own re-selections were not recognised as echoes"
grep -q 'clipboard duty off: memory emptied' "$run.err" ||
    fail "switching the duty off did not report that memory was emptied"
grep -q 'clipboard: the history file was deleted' "$run.err" ||
    fail "switching the duty off did not report the deletion of the history file"
# After an off the worker starts without reading the history file (`load=false`),
# and the first start of the run did read it; with the file absent below, the
# history starts empty.
grep 'clipboard duty on' "$run.err" | tail -n 1 | grep -q 'load=false' ||
    fail "switching the duty on again did not start without loading a history"
grep 'clipboard duty on' "$run.err" | head -n 1 | grep -q 'load=true' ||
    fail "the first start did not load the history"
grep -q 'the duty is off' "$run.err" || fail "a verb with the duty off was not refused"
# Privacy: the journal, the metrics, the nest's log and the driver's notes
# hold none of the fixed strings, at debug level for the whole shell.
for f in "$run.err" "$run.out" "$work/niri.log" "$run.clip"; do
    if grep -q -F -e "$clip_alpha" -e "$clip_beta" "$f"; then
        fail "a clipboard test string reached $(basename "$f")"
    fi
done
clip_lines=$(grep '^winit ' "$run.out" || true)
[ -n "$clip_lines" ] || fail "no metrics line for the winit output (clipboard run)"
# With the bar off and no page open, a history change redraws nothing: the
# rows are the Clipboard page's and only an Island showing that page redraws
# for them. What redraws every surface is the duty itself being switched in
# `shell.json` (the bar's piece appears or goes even over an empty history):
# the allowance is one frame at rest per switch the journal reports (`off` once
# and `on` again after the start), so any other frame at rest (a history change,
# a tick, a loop) still fails.
clip_rest=$(printf '%s\n' "$clip_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
clip_events=$(grep -c 'the history changed' "$run.err" || true)
clip_on=$(grep -c 'clipboard duty on' "$run.err" || true)
clip_off=$(grep -c 'clipboard duty off' "$run.err" || true)
clip_switches=$((clip_off + clip_on - 1))
[ "$clip_rest" -le "$clip_switches" ] ||
    fail "frames were drawn at rest beyond one per duty switch ($clip_rest, at most $clip_switches; $clip_events history changes): $clip_lines"
echo "smoke: clipboard: history 0 -> 1 -> 2 (hinted copy skipped, image accepted, re-selection not duplicated); state file mode 600, no image, deleted when the duty went off, empty when it came back; journal free of the test strings (changes: $changes)"
echo "smoke: clipboard: $(printf '%s\n' "$clip_lines" | head -n 1) (rest_frames=$clip_rest, duty switches=$clip_switches, history changes=$clip_events)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: clipboard: /' || true
echo "smoke: OK"
