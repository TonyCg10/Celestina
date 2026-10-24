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
# toggle` and `session suspend` (refused: suspend goes through the lock). `session reboot` is
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
# the test strings. Two more nested runs place the four connectivity pieces at
# the bar's end (CEL-2-F): one on a private system bus served by
# `examples/fake_system_bus` (fake NetworkManager, BlueZ, UPower and
# power-profiles-daemon, ended by closing its input), which must show the
# network, Bluetooth, battery and profiles workers ready; one on a socket that
# does not exist, which must show them all unavailable. Neither touches the
# real system bus. Two more nested runs cover the OSD duty (CEL-2-C) on fake
# `wpctl`, `pactl`, `ddcutil` and `brightnessctl` placed first on `PATH`
# (`CELESTINA_DDC=0`): with the duty on there is no `celestina-osd` surface at
# rest, an audio event raises it and it is destroyed again, and `volume up`
# reaches the fake `wpctl`; with the duty off the verbs are rejected and no
# fake is called. Three more nested runs turn the phone and bubbles
# duties on against `fake-melibea.py` in a private runtime directory
# (CEL-2-J): one must draw no frame at rest, one opens and closes the bubbles
# selector and answers the phone verbs, and one must log the missing Melibea
# socket as absent exactly once. Before any of them the artifact's other two
# binaries, `celestina-lock` and `celestina-lock-verify`, must sit beside the
# shell and refuse a bad request (CEL-2-H). Two more nested runs (CEL-2-D) turn
# the media duty on against a fake MPRIS player (see the media block below).
# An overview run (CEL-2-L) starts two coloured `kitty` windows, one
# under a `block-out-from "screen-capture"` rule of a throwaway niri profile,
# opens the overview and then the shortcuts, and checks the capture's one
# preview of the panel's output (painted overall, the half with the blocked
# window black, taken before the panel's surface existed), the shortcut groups
# read from that profile, and no frame at rest after the panel closes.
# A rest-frame check that fails while the host throttles the nest's window is
# inconclusive and the whole smoke runs again, up to three times (the head of
# the script, `rest_fail`). A last nested run (CEL-2-I) turns the polkit duty
# on over the same dead system bus: it must exit 0, report the agent
# unavailable, never register with a polkitd and not panic (the duty is never
# activated against the real bus here). Without niri the nested session is
# skipped with a notice; without busctl and gdbus the notification part is.
usage() {
    echo "usage: scripts/smoke.sh --binary PATH" >&2
    exit 2
}
[ "$#" -eq 2 ] && [ "$1" = "--binary" ] || usage
bin=$2
# The host compositor withholds frame callbacks from a window it does not show
# (niri sends one about every second instead of every refresh), and a nest on
# such a window serves its clients at that pace: start-up frames then land
# after the first second and the rest-frame counts mean nothing. A rest-frame
# check that fails in a run whose journal shows such a wait (`rest_fail`
# below) is therefore inconclusive, not a verdict: the attempt exits 75 and
# the smoke is started again, up to three attempts. A run that was not
# throttled and fails still fails, and so does the third throttled one.
if [ -z "${CELESTINA_SMOKE_ATTEMPT:-}" ]; then
    attempt=1
    while :; do
        attempt_status=0
        CELESTINA_SMOKE_ATTEMPT=$attempt sh "$0" "$@" || attempt_status=$?
        [ "$attempt_status" -eq 75 ] || exit "$attempt_status"
        if [ "$attempt" -ge 3 ]; then
            echo "smoke: the nest was throttled by the host in 3 attempts; show its window" >&2
            exit 1
        fi
        attempt=$((attempt + 1))
        echo "smoke: the nest was throttled by the host; starting attempt $attempt of 3" >&2
    done
fi
[ -x "$bin" ] || { echo "smoke: $bin is not an executable" >&2; exit 1; }
case "$bin" in
    /*) ;;
    *) bin=$(pwd)/$bin ;;
esac
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# The artifact is three binaries (CEL-2-H): the shell finds the lock and its
# verifier beside itself, so a smoke of a lone shell proves nothing about the
# lock it would spawn. Assertions only: neither is started against a
# compositor here. A bad argument answers 2 from the lock and 6 from the
# verifier before either touches Wayland, PAM or stdin, which shows the right
# programs are there and speak their contracts.
bin_dir=$(dirname -- "$bin")
for lock_binary in celestina-lock celestina-lock-verify; do
    [ -x "$bin_dir/$lock_binary" ] || {
        echo "smoke: $bin_dir/$lock_binary is missing or not an executable" >&2
        exit 1
    }
done
set +e
"$bin_dir/celestina-lock" --not-an-option >/dev/null 2>&1
lock_status=$?
"$bin_dir/celestina-lock-verify" --protocol 0 --service production </dev/null >/dev/null 2>&1
verify_status=$?
set -e
[ "$lock_status" -eq 2 ] || {
    echo "smoke: celestina-lock answered $lock_status to a bad option, not 2" >&2
    exit 1
}
[ "$verify_status" -eq 6 ] || {
    echo "smoke: celestina-lock-verify answered $verify_status to a bad protocol, not 6" >&2
    exit 1
}
echo "smoke: celestina-lock and celestina-lock-verify are beside the shell and refuse a bad request"

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
# The niri the nest runs, resolved as dev-session.sh resolves it, so a probe
# that talks to the nest (`overlay` below) uses the same binary and the same
# protocol as the nest and never the PATH's.
niri_binary=${CELESTINA_NEST_NIRI:-}
if [ -z "$niri_binary" ]; then
    patched=${CELESTINA_NIRI_PREFIX:-$HOME/.local/lib/celestina}/niri
    if [ -x "$patched" ]; then
        niri_binary=$patched
    else
        niri_binary=niri
    fi
fi
case "$niri_binary" in
    *\'*)
        echo "smoke: the niri path $niri_binary contains a single quote" >&2
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
# The runs that start the shell without a wrapper of their own start it through
# this one, which turns the runtime's debug lines on (`frame drawn` carries the
# callback flag and the time `nest_throttled` reads); the nest's own niri keeps
# its environment.
shell_run=$work/shell-run
cat > "$shell_run" <<WRAPPER
#!/bin/sh
RUST_LOG=info,celestina_wayland::runtime=debug exec "$bin" "\$@"
WRAPPER
chmod +x "$shell_run"
run=$work/run
status=0
# shellcheck disable=SC2086 # probe_args is two words or none
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$shell_run" \
    $probe_args --once "$run" -- --exit-after 4 --metrics \
    > "$work/niri.log" 2>&1 || status=$?

# tails: the end of the run's files, for a failure's reader.
tails() {
    for f in "$run.out" "$run.err" "$run.probe" "$work/niri.log"; do
        [ -f "$f" ] || continue
        echo "--- $(basename -- "$f")" >&2
        tail -n 40 "$f" >&2
    done
}
fail() {
    echo "smoke: $1" >&2
    tails
    exit 1
}
# nest_throttled JOURNAL: whether the nest withheld a frame callback from the
# shell for 500 ms or more. A frame drawn with `next_callback=true` asked for
# one, and the next frame on that surface is drawn when it arrives: in a nest
# the host shows that is the next refresh (about 16 ms), in one it does not
# show it is the host's once-a-second tick. Prints the longest wait in ms.
nest_throttled() {
    awk '
    function secs(line) {
        # 2026-10-23T17:21:05.780542Z: the time of day in seconds.
        return substr(line, 12, 2) * 3600 + substr(line, 15, 2) * 60 + substr(line, 18, 9)
    }
    /frame drawn/ && /surface=[0-9]/ {
        match($0, /surface=[0-9]+/)
        id = substr($0, RSTART, RLENGTH)
        now = secs($0)
        if (id in asked && now - asked[id] > longest) longest = now - asked[id]
        if ($0 ~ /next_callback=true/) asked[id] = now; else delete asked[id]
    }
    END { printf "%d\n", longest * 1000; exit !(longest >= 0.5) }' "$1"
}
# rest_fail JOURNAL MESSAGE: a rest-frame budget was exceeded. In a throttled
# nest the start-up frames land late and the budget means nothing, so that is
# inconclusive: exit 75, and the head of this script starts the run again,
# up to three times; on the last attempt, and in a nest that was not
# throttled, it is a failure.
rest_fail() {
    if wait_ms=$(nest_throttled "$1"); then
        [ "${CELESTINA_SMOKE_ATTEMPT:-1}" -lt 3 ] ||
            fail "$2 (inconclusive: the nest withheld a frame callback for $wait_ms ms, its window is hidden)"
        echo "smoke: inconclusive (the nest withheld a frame callback for $wait_ms ms): $2" >&2
        tails
        exit 75
    fi
    fail "$2"
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
    --monitor none --binary "$shell_run" --probe "$here/wallpaper-layers-probe.sh" \
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
    rest_fail "$run.err" "frames were drawn at rest with the wallpaper present ($wp_rest, at most 2): $wp_lines"
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
# bus call because suspend goes through the lock (`lock suspend`).
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
grep -q 'session Suspend: refused: suspend goes through the lock' "$run.err" ||
    fail "session suspend was not refused (suspend goes through the lock)"
echo "smoke: $(grep -Eo 'idle: (armed|unavailable)' "$run.err" | head -n 1), $(grep -Eo 'night light: (applied|not applied)' "$run.err" | head -n 1), suspend refused (it goes through the lock)"

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
RUST_LOG=info,celestina_wayland::runtime=debug exec "$bin" "\$@"
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
    # The rest budget is what the design owes a placed system monitor: one
    # frame when its first reading appears and at most one more when the
    # ten-second cadence copies a changed reading to the bar. The run lasts
    # 14 s, so that second frame lands near the exit and either side of it is
    # a legitimate outcome; a bar that redrew on every two-second sample would
    # draw about six.
    [ "$g_rest" -le 2 ] || rest_fail "$run.err" "frames were drawn at rest ($g_name run, $g_rest, at most 2): $g_lines"
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
    rest_fail "$run.err" "frames were drawn at rest beyond one per duty switch ($clip_rest, at most $clip_switches; $clip_events history changes): $clip_lines"
echo "smoke: clipboard: history 0 -> 1 -> 2 (hinted copy skipped, image accepted, re-selection not duplicated); state file mode 600, no image, deleted when the duty went off, empty when it came back; journal free of the test strings (changes: $changes)"
echo "smoke: clipboard: $(printf '%s\n' "$clip_lines" | head -n 1) (rest_frames=$clip_rest, duty switches=$clip_switches, history changes=$clip_events)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: clipboard: /' || true
# The connectivity pieces (CEL-2-F): a bar with the four pieces at its end, on
# a private system bus that examples/fake_system_bus serves (fake
# NetworkManager, BlueZ, UPower and power-profiles-daemon in one dbus-daemon of
# its own, ended by closing the example's input, never killed). It must exit 0,
# draw on every metrics line, draw at most one frame at rest (the bar run's
# rule), not panic, and the journal must say the network, Bluetooth, battery and
# profiles workers answered. A second run points the shell at the dead socket:
# the same exit and no panic, and every service must report unavailable. Nothing
# here reaches the real system bus, NetworkManager, BlueZ, UPower or
# power-profiles-daemon.
case "$bin" in
    */debug/celestina) profile=debug; profile_flag= ;;
    */release/celestina) profile=release; profile_flag=--release ;;
    *) fail "the binary is not under a debug or release target directory: the fake system bus cannot be built beside it" ;;
esac
command -v cargo >/dev/null 2>&1 || fail "cargo is absent: the fake system bus example cannot be built"
target_dir=$(dirname -- "$(dirname -- "$bin")")
# shellcheck disable=SC2086 # profile_flag is one word or none
CARGO_TARGET_DIR=$target_dir cargo build --example fake_system_bus --locked --offline \
    $profile_flag --manifest-path "$here/../Cargo.toml" > "$work/fake-build.log" 2>&1 ||
    { tail -n 40 "$work/fake-build.log" >&2; fail "the fake system bus example did not build"; }
fake=$target_dir/$profile/examples/fake_system_bus
[ -x "$fake" ] || fail "the fake system bus example is not at $fake"
conn_cfg=$work/xdg-connectivity
mkdir -p "$conn_cfg/celestina"
cat > "$conn_cfg/celestina/shell.json" <<'JSON'
{
  "schema_version": 1,
  "duties": { "bar": "on" },
  "bar": { "end": ["network", "bluetooth", "battery", "power-profile"] }
}
JSON
fake_wrapper=$work/connectivity-run
cat > "$fake_wrapper" <<WRAPPER
#!/bin/sh
# Starts the fake system services on their private bus, runs the shell with
# DBUS_SYSTEM_BUS_ADDRESS at them, then closes the example's input so its
# dbus-daemon exits, and exits with the shell's status.
fifo=$work/fake-input
out=$work/fake-output
mkfifo "\$fifo"
"$fake" < "\$fifo" > "\$out" 2> $work/fake-error &
fake=\$!
exec 3> "\$fifo"
tries=0
until addr=\$(sed -n 's/^DBUS_SYSTEM_BUS_ADDRESS=//p' "\$out" 2>/dev/null | head -n 1) && [ -n "\$addr" ]; do
    tries=\$((tries + 1))
    if [ "\$tries" -ge 50 ]; then
        echo "smoke-fake-bus-failed" >&2
        exec 3>&-
        wait "\$fake"
        exit 1
    fi
    sleep 0.2
done
DBUS_SYSTEM_BUS_ADDRESS=\$addr RUST_LOG=info,celestina_wayland::runtime=debug "$bin" "\$@"
status=\$?
exec 3>&-
wait "\$fake"
exit "\$status"
WRAPPER
chmod +x "$fake_wrapper"
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
run=$work/connectivity
status=0
XDG_CONFIG_HOME=$conn_cfg timeout 90 "$here/dev-session.sh" --own-bus --monitor none \
    --binary "$fake_wrapper" --once "$run" -- --exit-after 6 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (connectivity run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (connectivity run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (connectivity run)"
if grep -q 'smoke-fake-bus-failed' "$run.err"; then
    fail "the fake system services did not start"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the connectivity pieces on"
fi
lines=$(grep '^winit ' "$run.out" || true)
[ -n "$lines" ] || fail "no metrics line for the winit output (connectivity run)"
printf '%s\n' "$lines" | while IFS= read -r l; do
    n=$(printf '%s\n' "$l" | tr ' ' '\n' | sed -n 's/^frames=//p')
    [ "${n:-0}" -ge 1 ] || exit 1
done || fail "a surface drew no frame with the connectivity pieces on: $lines"
rest=$(sum rest_frames)
[ "$rest" -le 1 ] || rest_fail "$run.err" "frames were drawn at rest with the connectivity pieces on ($rest): $lines"
for want in 'network: ready' 'bluetooth: ready' 'power: battery ready' 'power: profiles ready'; do
    grep -q "$want" "$run.err" || fail "the journal does not say '$want' (connectivity run)"
done
echo "smoke: connectivity on the fake bus: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$rest)"
# The same bar against a bus that is not there: every service reports
# unavailable and the shell still runs and draws.
run=$work/connectivity-absent
# A wrapper sets the dead socket for this run itself, as the energy run does,
# instead of relying on the script-wide export reaching the shell through
# dev-session and niri: were that inheritance to break, this run would read
# the real system bus.
absent_wrapper=$work/connectivity-absent-run
cat > "$absent_wrapper" <<WRAPPER
#!/bin/sh
DBUS_SYSTEM_BUS_ADDRESS='unix:path=$work/no-system-bus'
export DBUS_SYSTEM_BUS_ADDRESS
exec "$bin" "\$@"
WRAPPER
chmod +x "$absent_wrapper"
status=0
XDG_CONFIG_HOME=$conn_cfg timeout 60 "$here/dev-session.sh" --own-bus --monitor none \
    --binary "$absent_wrapper" --once "$run" -- --exit-after 4 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (absent bus run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (absent bus run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (absent bus run)"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with no system bus"
fi
for want in 'network: unavailable' 'bluetooth: unavailable' 'power: battery unavailable' 'power: profiles unavailable'; do
    grep -q "$want" "$run.err" || fail "the journal does not say '$want' (absent bus run)"
done
echo "smoke: connectivity with no system bus: every service unavailable, no panic"

# The OSD duty (CEL-2-C): two more nested runs, on fake tools. A directory of
# stand-ins for `wpctl`, `pactl`, `ddcutil` and `brightnessctl` comes first on
# the shell's `PATH`; each only appends its arguments to a calls file and
# answers a fixed reading, so the author's real volume, backlight and monitors
# are never touched, and `CELESTINA_DDC=0` forbids every ddcutil call besides.
# The fake `pactl subscribe` waits for a trigger file, then moves the fake
# volume and emits one event line. Run A has `duties.osd` on: the shell must
# hold no `celestina-osd` surface and draw nothing for it at rest (the wrapper
# writes a checkpoint line to stderr before the trigger, and no surface may
# exist before it), the event must create the surface and the surface must be
# destroyed again, and `celestina volume up` must reach the fake `wpctl` as
# `set-volume @DEFAULT_AUDIO_SINK@ <fraction> -l 1.0`. Run B has the duty off
# (the default): both verbs are rejected and no fake is called at all.
# Run A holds the OSD for 4000 ms and runs for 16 s, not the 500 ms and 9 s a
# quick desktop would do: when the host compositor throttles the nest window
# the nest's loop runs only every 1-2 s (measured), so a new layer surface gets
# no configure, and so no frame, before a 500 ms OSD is already destroyed. A
# hold of 4000 ms spans at least two such ticks, and the verbs re-show the
# live surface, which leaves it three to six. The last show is at about 5 s
# (the verbs, 2 s after the checkpoint at 3 s), so it expires at about 9 s and
# retires before 16 s.
osd_fakes=$work/osd-fakes
osd_state=$work/osd-state
mkdir -p "$osd_fakes" "$osd_state"
cat > "$osd_fakes/wpctl" <<'FAKE'
#!/bin/sh
# Fake wpctl: records its arguments and keeps the volume in a file.
echo "wpctl $*" >> "$CELESTINA_SMOKE_FAKE/calls"
case "$1" in
    get-volume) echo "Volume: $(cat "$CELESTINA_SMOKE_FAKE/volume")" ;;
    set-volume) echo "$3" > "$CELESTINA_SMOKE_FAKE/volume" ;;
esac
FAKE
cat > "$osd_fakes/pactl" <<'FAKE'
#!/bin/sh
# Fake pactl: records its arguments; `subscribe` waits for the trigger, moves
# the volume, emits one event and then stays alive until the shell kills it.
echo "pactl $*" >> "$CELESTINA_SMOKE_FAKE/calls"
[ "$1" = subscribe ] || exit 0
while [ ! -e "$CELESTINA_SMOKE_FAKE/trigger" ]; do sleep 0.1; done
echo 0.55 > "$CELESTINA_SMOKE_FAKE/volume"
echo "Event 'change' on sink #50"
exec sleep 600
FAKE
for tool in ddcutil brightnessctl; do
    cat > "$osd_fakes/$tool" <<'FAKE'
#!/bin/sh
# Fake: records its arguments and reads as nothing.
echo "$(basename "$0") $*" >> "$CELESTINA_SMOKE_FAKE/calls"
exit 1
FAKE
done
chmod +x "$osd_fakes/wpctl" "$osd_fakes/pactl" "$osd_fakes/ddcutil" "$osd_fakes/brightnessctl"
mkdir -p "$work/xdg-osd/celestina" "$work/xdg-osd-off/celestina"
printf '{"schema_version":1,"duties":{"osd":"on"},"osd":{"timeout_ms":4000}}\n' \
    > "$work/xdg-osd/celestina/shell.json"
printf '{"schema_version":1}\n' > "$work/xdg-osd-off/celestina/shell.json"
# $1 is the configuration home, $2 whether the wrapper pulls the trigger.
osd_wrapper() {
    cat <<WRAPPER
#!/bin/sh
# Runs the shell on the fake tools, sends the OSD verbs once it answers, and
# exits with the shell's status.
XDG_CONFIG_HOME='$1'
CELESTINA_SMOKE_FAKE='$osd_state'
CELESTINA_DDC=0
PATH='$osd_fakes':\$PATH
export XDG_CONFIG_HOME CELESTINA_SMOKE_FAKE CELESTINA_DDC PATH
RUST_LOG=info,celestina_wayland::runtime=debug,celestina_wayland::handlers=debug "$bin" "\$@" &
shell=\$!
send() {
    tries=0
    until "$bin" "\$@"; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-verb-failed \$*" >&2; return 1; }
        sleep 0.2
    done
}
if [ '$2' = trigger ]; then
    sleep 3
    echo "smoke-osd-rest-checkpoint" >&2
    : > "\$CELESTINA_SMOKE_FAKE/trigger"
    sleep 2
fi
send volume up
send brightness up
wait "\$shell"
WRAPPER
}
osd_run() {
    # $1 name, $2 configuration home, $3 trigger|none, $4 --exit-after seconds.
    rm -f "$osd_state/calls" "$osd_state/trigger"
    echo 0.40 > "$osd_state/volume"
    : > "$osd_state/calls"
    osd_wrapper "$2" "$3" > "$work/$1-run"
    chmod +x "$work/$1-run"
    run=$work/$1
    status=0
    timeout 60 "$here/dev-session.sh" --own-bus --monitor none \
        --binary "$work/$1-run" --once "$run" -- --exit-after "$4" \
        > "$work/niri.log" 2>&1 || status=$?
    [ "$status" -eq 0 ] || fail "the nested niri exited with $status ($1 run)"
    [ -f "$run.status" ] || fail "the shell never ran inside the nest ($1 run)"
    shell_status=$(cat "$run.status")
    [ "$shell_status" = 0 ] || fail "the shell exited with $shell_status ($1 run)"
    if grep -q 'panicked' "$run.err"; then
        fail "the shell panicked ($1 run)"
    fi
}
osd_run osd "$work/xdg-osd" trigger 16
grep -q 'osd duty on: workers started' "$run.err" ||
    fail "the OSD workers did not start with the duty on"
if grep -q 'smoke-verb-failed' "$run.err"; then
    fail "an OSD verb did not reach the shell"
fi
# Nothing of the OSD exists before the checkpoint: no surface and so no frame.
if awk '/smoke-osd-rest-checkpoint/ { exit } /osd surface created/ { found = 1 } END { exit !found }' "$run.err"; then
    fail "an OSD surface existed at rest, before any event"
fi
grep -q 'smoke-osd-rest-checkpoint' "$run.err" || fail "the OSD rest checkpoint was never reached"
grep -q 'osd surface created' "$run.err" || fail "the audio event did not raise the OSD surface"
grep -q 'osd surface destroyed' "$run.err" || fail "the OSD surface was not destroyed after its timeout"
created=$(grep -n 'osd surface created' "$run.err" | head -n 1 | cut -d: -f1)
destroyed=$(grep -n 'osd surface destroyed' "$run.err" | head -n 1 | cut -d: -f1)
[ "$created" -lt "$destroyed" ] || fail "the OSD surface was destroyed before it was created"
# The OSD's own surfaces are named by the runtime's creation line, so a late
# first frame of one of the shell's surfaces is never counted for the OSD. The
# nest must also have configured one of them (a draw follows the first
# configure at once); neither is a product fault when the nest's loop stalls.
osd_counts=$(awk '
    /layer surface created/ && /namespace="celestina-osd"/ {
        match($0, /surface=[0-9]+/)
        osd[substr($0, RSTART, RLENGTH)] = 1
    }
    /configure surface=/ || /frame drawn/ {
        match($0, /surface=[0-9]+/)
        id = substr($0, RSTART, RLENGTH)
        if (id in osd) { if ($0 ~ /frame drawn/) frames++; else configured++ }
    }
    END { n = 0; for (id in osd) n++; print n + 0, configured + 0, frames + 0 }' "$run.err")
set -- $osd_counts
[ "$1" -ge 1 ] || fail "the runtime never created a celestina-osd layer surface"
[ "$2" -ge 1 ] ||
    fail "the nest never configured the OSD surface within its life (its loop is stalled: host throttling?)"
osd_frames=$3
[ "$osd_frames" -ge 1 ] || fail "no frame was drawn on the OSD surface"
grep -q '^wpctl get-volume @DEFAULT_AUDIO_SINK@$' "$osd_state/calls" ||
    fail "the fake wpctl was never read: $(cat "$osd_state/calls")"
grep -q '^wpctl set-volume @DEFAULT_AUDIO_SINK@ 0\.60 -l 1\.0$' "$osd_state/calls" ||
    fail "celestina volume up did not reach wpctl as set-volume 0.60: $(cat "$osd_state/calls")"
grep -q '^pactl subscribe$' "$osd_state/calls" || fail "pactl subscribe was never started"
if grep -q '^ddcutil' "$osd_state/calls"; then
    fail "ddcutil was called with CELESTINA_DDC=0"
fi
grep -q 'volume up: queued' "$run.err" || fail "the volume verb was not queued"
echo "smoke: osd on: surface created and destroyed, osd_frames=$osd_frames, volume up reached wpctl as set-volume 0.60"

osd_run osd-off "$work/xdg-osd-off" none 9
grep -q 'volume up: rejected: the osd duty is off' "$run.err" ||
    fail "volume up was not rejected with the duty off"
grep -q 'brightness up: rejected: the osd duty is off' "$run.err" ||
    fail "brightness up was not rejected with the duty off"
if grep -q 'osd surface created' "$run.err"; then
    fail "an OSD surface appeared with the duty off"
fi
[ ! -s "$osd_state/calls" ] || fail "a fake tool was called with the duty off: $(cat "$osd_state/calls")"
echo "smoke: osd off: both verbs rejected, no tool called"

# The phone and bubbles duties (CEL-2-J): both `on` from a private
# XDG_CONFIG_HOME, against `fake-melibea.py` (stdlib, one fixture window on a
# socket inside a private 0700 runtime directory). The shell's runtime is that
# directory; its Wayland display is named by an absolute path into the host's,
# so the nest is still reached and no socket of the desktop is. The system bus
# points at a socket that does not exist, and the session bus is the nest's
# own, where no Magnetita runs, so the phone piece must read absent. Three
# nested runs, because the run with verbs draws frames that the rest check
# must not count: the metrics run sends no verb; the verbs run opens the
# bubbles selector with `bubbles select`, closes it with a second one, and
# answers `island open phone` and `phone media PlayPause`; the absent run
# points MELIBEA_SOCKET at a file that does not exist.
stop_fake() {
    [ -z "${fake_pid:-}" ] || kill "$fake_pid" 2>/dev/null || true
}
phone_home=$work/phone
phone_runtime=$phone_home/runtime
phone_socket=$phone_runtime/melibea.sock
mkdir -p "$phone_home/config/celestina" "$phone_runtime"
chmod 700 "$phone_home" "$phone_runtime"
printf '%s\n' '{"schema_version":1,"duties":{"phone":"on","bubbles":"on"}}' \
    > "$phone_home/config/celestina/shell.json"
# The head of every phone wrapper: the environment the shell gets. The nest's
# own runtime is not replaced until its Wayland display is absolute, so the
# shell can still connect to the nest.
phone_prelude() {
    cat <<PRELUDE
#!/bin/sh
# Runs the shell with the phone and bubbles duties on (CEL-2-J smoke).
host_runtime=\${XDG_RUNTIME_DIR:-}
case "\$WAYLAND_DISPLAY" in
    /*) ;;
    *) WAYLAND_DISPLAY=\$host_runtime/\$WAYLAND_DISPLAY ;;
esac
XDG_CONFIG_HOME='$phone_home/config'
XDG_RUNTIME_DIR='$phone_runtime'
MELIBEA_SOCKET='$1'
DBUS_SYSTEM_BUS_ADDRESS='unix:path=$work/no-system-bus'
RUST_LOG=info,celestina::bubbles=debug,celestina_wayland::runtime=debug
export WAYLAND_DISPLAY XDG_CONFIG_HOME XDG_RUNTIME_DIR MELIBEA_SOCKET DBUS_SYSTEM_BUS_ADDRESS RUST_LOG
PRELUDE
}
phone_metrics=$work/phone-metrics
{
    phone_prelude "$phone_socket"
    printf "exec '%s' \"\$@\"\n" "$bin"
} > "$phone_metrics"
chmod +x "$phone_metrics"
phone_verbs=$work/phone-verbs
{
    phone_prelude "$phone_socket"
    cat <<VERBS
'$bin' "\$@" &
shell=\$!
send() {
    tries=0
    until '$bin' "\$@"; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-verb-failed \$*" >&2; return 1; }
        sleep 0.2
    done
}
overlay() { '$niri_binary' msg --json layers 2>/dev/null | grep -q celestina-overlay; }
send bubbles select || true
sleep 1
overlay || echo "smoke-bubbles-not-open" >&2
send bubbles select || true
sleep 1
overlay && echo "smoke-bubbles-not-closed" >&2
send island open phone || true
send phone media PlayPause || true
wait "\$shell"
VERBS
} > "$phone_verbs"
chmod +x "$phone_verbs"
phone_absent=$work/phone-absent
{
    phone_prelude "$phone_runtime/absent.sock"
    printf "exec '%s' \"\$@\"\n" "$bin"
} > "$phone_absent"
chmod +x "$phone_absent"

# The fake serves until SIGTERM and removes its socket then; the trap makes
# sure no failing exit leaves it running.
run=$phone_home/fake
fake_pid=
python3 -I "$here/fake-melibea.py" "$phone_socket" > "$phone_home/fake.log" 2>&1 &
fake_pid=$!
trap 'stop_fake; rm -rf "$work"' EXIT INT TERM
tries=0
until [ -S "$phone_socket" ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 50 ] || fail "the fake Melibea never listened on $phone_socket"
    sleep 0.1
done

run=$work/phone-metrics
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_metrics" \
    --once "$run" -- --exit-after 4 --metrics > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (phone metrics run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (phone metrics run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (phone metrics run)"
lines=$(grep '^winit ' "$run.out" || true)
[ -n "$lines" ] || fail "no metrics line for the winit output (phone metrics run)"
frames=$(sum frames)
rest=$(sum rest_frames)
[ "$frames" -ge 1 ] || fail "no frame was drawn (phone metrics run): $lines"
[ "$rest" -eq 0 ] ||
    rest_fail "$run.err" "frames were drawn at rest with the phone and bubbles duties on ($rest): $lines"
if ! grep -q 'melibea: subscribed' "$run.err"; then
    fail "the shell did not subscribe to the fake Melibea (phone metrics run)"
fi
grep -q 'bubbles: windows count=1' "$run.err" ||
    fail "the fake's one window did not reach the bubbles (phone metrics run)"
grep -q 'magnetita: absent: nobody owns' "$run.err" ||
    fail "the phone piece was not reported absent (phone metrics run)"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the phone and bubbles duties on"
fi
echo "smoke: phone and bubbles on: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$rest); Melibea subscribed with one window, Magnetita absent"

run=$work/phone-verbs
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_verbs" \
    --once "$run" -- --exit-after 12 > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (phone verbs run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (phone verbs run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (phone verbs run)"
for marker in smoke-verb-failed smoke-bubbles-not-open smoke-bubbles-not-closed; do
    if grep -q "$marker" "$run.err"; then
        fail "a phone or bubbles check failed: $marker (phone verbs run)"
    fi
done
grep -q 'bubbles select: closed by the command' "$run.err" ||
    fail "the second bubbles select did not close the selector (phone verbs run)"
grep -q 'bubbles: windows count=1' "$run.err" ||
    fail "the fake's one window did not reach the bubbles (phone verbs run)"
grep -q 'phone media: no phone is connected' "$run.err" ||
    fail "phone media was not answered by the shell (phone verbs run)"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the bubbles selector open"
fi
grep -q 'request subscribe' "$phone_home/fake.log" ||
    fail "the shell never subscribed to the fake Melibea"
echo "smoke: bubbles selector opened and closed; island open phone and phone media answered"

# The fake stops on SIGTERM and removes its socket. The check after the
# verbs run proves the shell never reached any other socket: the fake logged
# every request it answered.
kill -TERM "$fake_pid" 2>/dev/null || true
wait "$fake_pid" 2>/dev/null || true
fake_pid=
[ ! -e "$phone_socket" ] || fail "the fake Melibea did not remove its socket on SIGTERM"
echo "smoke: fake Melibea answered $(grep -c '^fake-melibea: request' "$phone_home/fake.log") requests and removed its socket"

absent_socket=$phone_runtime/absent.sock
run=$work/phone-absent
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_absent" \
    --once "$run" -- --exit-after 3 > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (phone absent run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (phone absent run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (phone absent run)"
absent_count=$(grep -c 'melibea: absent' "$run.err" || true)
[ "$absent_count" = 1 ] ||
    fail "the missing Melibea socket was logged absent $absent_count times, not once (phone absent run)"
[ ! -e "$absent_socket" ] || fail "the absent run created $absent_socket"
echo "smoke: missing Melibea socket logged absent once"

# The media duty (CEL-2-D): two nested runs, each on the nest's own session
# bus (the desktop's is never reached) with a throwaway XDG_CONFIG_HOME
# turning `duties.media` on, and a fake MPRIS player owning
# `org.mpris.MediaPlayer2.fake` there (`examples/fake-mpris-player.rs`, a
# zbus example with no dependency of its own; `scripts/smoke-media.sh` is the
# program the nest runs). Run 1: a paused player with a valid generated PNG
# cover, `island open media` once the shell answers and `island close` a moment
# later (an open page draws a frame a second for the seconds clock in
# its title row, so rest is measured with the Island closed); the shell must
# exit 0, not panic, refuse no cover, and draw at most two frames at rest (the
# player's first scan and its cover are each a frame somebody needed, though
# nothing animates in them). Run 2: the same player with an `http://` cover, which
# must be refused (logged once, no fetch), the shell must claim no bus name
# while a player is present (the list of well-known names is identical
# before and after), and the duty is flipped off and on through `shell.json`
# (the worker is joined and restarted) without a panic. Without the example
# built next to the binary (`cargo build --example fake-mpris-player`) the
# phase is skipped with a notice.
player=$(dirname "$bin")/examples/fake-mpris-player
if [ ! -x "$player" ]; then
    echo "smoke: media phase skipped: $player is not built (cargo build --locked --example fake-mpris-player)"
elif ! command -v busctl >/dev/null 2>&1 && ! command -v gdbus >/dev/null 2>&1; then
    echo "smoke: media phase skipped: neither busctl nor gdbus is installed"
else
    media_home=$work/xdg-media
    mkdir -p "$media_home/celestina"
    printf '{"schema_version":1,"duties":{"media":"on"}}\n' > "$media_home/celestina/shell.json"
    python3 - "$work/cover.png" <<'PNG' || fail "python3 could not generate the cover fixture"
import struct, sys, zlib

# A 32x32 gradient, 8-bit RGB: stdlib only.
width, height = 32, 32
rows = b"".join(
    b"\x00" + bytes(c for x in range(width) for c in (x * 8, y * 8, 120))
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
    media_run() {
        # $1 run prefix, $2 mode, $3 art url, $4 seconds
        while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
            sleep 1
        done
        media_status=0
        XDG_CONFIG_HOME=$media_home SMOKE_MEDIA_SHELL=$bin SMOKE_MEDIA_PLAYER=$player \
            SMOKE_MEDIA_MODE=$2 SMOKE_MEDIA_ART=$3 \
            SMOKE_RUST_LOG=info,celestina::island=debug,celestina_wayland::runtime=debug \
            SMOKE_MEDIA_CONFIG=$media_home/celestina/shell.json \
            timeout 60 "$here/dev-session.sh" --own-bus --monitor none \
            --binary "$here/smoke-media.sh" --once "$1" -- --exit-after "$4" --metrics \
            > "$work/niri.log" 2>&1 || media_status=$?
        [ "$media_status" -eq 0 ] || fail "the nested niri exited with $media_status (media $2 run)"
        [ -f "$1.status" ] || fail "the shell never ran inside the nest (media $2 run)"
        [ "$(cat "$1.status")" = 0 ] || fail "the shell exited with $(cat "$1.status") (media $2 run)"
        if grep -q 'smoke-media-failed' "$1.err"; then
            fail "the media client failed (media $2 run)"
        fi
        if grep -q 'panicked' "$1.err"; then
            fail "the shell panicked (media $2 run)"
        fi
        grep -q 'smoke-media: page requested' "$1.err" ||
            fail "the page was never requested (media $2 run)"
        grep -q 'island: command applied.*page=PageId("media") open=true' "$1.err" ||
            fail "the Island did not open the media page (media $2 run)"
    }
    run=$work/media-steady
    media_run "$run" steady "file://$work/cover.png" 7
    if grep -q 'media: cover refused' "$run.err"; then
        fail "a valid cover was refused (media steady run)"
    fi
    grep -q 'media: cover decoded' "$run.err" || fail "a valid cover was not decoded (media steady run)"
    md_lines=$(grep '^winit ' "$run.out" || true)
    [ -n "$md_lines" ] || fail "no metrics line for the winit output (media steady run)"
    md_rest=$(printf '%s\n' "$md_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
        awk '{ n += $1 } END { print n + 0 }')
    [ "$md_rest" -le 2 ] ||
        rest_fail "$run.err" "frames were drawn at rest with a paused player ($md_rest, at most 2): $md_lines"
    echo "smoke: media: paused player with a cover, page open, rest_frames=$md_rest"

    run=$work/media-variant
    media_run "$run" variant "http://127.0.0.1:9/cover.png" 9
    grep -q 'media: cover refused' "$run.err" || fail "the http cover was not refused (media variant run)"
    [ "$(grep -c 'media: cover refused' "$run.err")" -le 2 ] ||
        fail "the refused http cover was logged more than the worker's restart allows (media variant run)"
    grep -q 'smoke-media: shell names unchanged' "$run.err" ||
        fail "the shell's bus names changed with a player present (media variant run)"
    grep -q 'smoke-media: duty flipped on' "$run.err" ||
        fail "the duty flip did not complete (media variant run)"
    echo "smoke: media: http cover refused, no bus name claimed, duty flipped off and on"
fi
# The overview and the shortcuts (CEL-2-L): a nested run with its own niri
# profile (the reference one plus a `block-out-from "screen-capture"` rule for
# app-id `smoke-blocked`) and a throwaway XDG_CONFIG_HOME; NIRI_CONFIG names the
# profile so the Shortcuts mode reads a known file, never the author's. Two
# plain `kitty` windows with distinct flat backgrounds (orange `smoke-open`,
# blue `smoke-blocked`) are started by the wrapper, which waits up to
# $ov_window_wait seconds for both; `celestina overview open` is held two
# seconds, `shortcuts open` one second, then `shortcuts close`, a rest, and
# `quit`. `--exit-after` is only the backstop, computed so the slowest allowed
# sequence still ends by `quit` first.
# The capture worker logs one line per output and, at debug level, the mean
# luminance of the whole-output preview and of its left and right halves. niri
# 26.04 gives no on-screen tile position, so there are no per-window crops: the
# two windows are two columns side by side, and the half that holds the blocked
# window must be black (at most a fifth of the other half: it still holds the
# compositor's own backdrop and gaps, which are not blacked out) while the other
# half and the whole preview are painted.
# The windows get a dead session bus so kitty activates no portal. Only pids
# started here are killed. Without kitty this part is skipped.
overview_run() {
# The longest wait for the two test windows, and the worst case of the rest:
# the sleeps of the sequence (1.5 + 2 + 1 + 1 + 2) and four verbs that may
# each retry for 4 s. The shell's own backstop and the nest's timeout follow.
ov_window_wait=20
ov_exit_after=$((ov_window_wait + 8 + 4 * 4 + 6))
ov_timeout=$((ov_exit_after + 20))
if ! command -v kitty >/dev/null 2>&1; then
    echo "smoke: overview run skipped: kitty is absent (the test windows)"
    return 0
fi
while [ "$(date +%S)" -gt 34 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
ov_xdg=$work/xdg-overview
mkdir -p "$ov_xdg/celestina"
printf '{"schema_version":1}\n' > "$ov_xdg/celestina/shell.json"
ov_niri=$work/overview-niri.kdl
{
    cat "$here/dev-session.kdl"
    cat <<'KDL'

window-rule {
    match app-id="^smoke-blocked$"
    block-out-from "screen-capture"
}
KDL
} > "$ov_niri"
ov_wrapper=$work/overview-run
cat > "$ov_wrapper" <<WRAPPER
#!/bin/sh
# Runs the shell, starts the two test windows, drives the overview and exits
# with the shell's status. Everything it prints goes to the run's stderr file,
# in order with the shell's own lines.
# NIRI_CONFIG is named again here: in the first run the shell inside the nest
# did not see the value given to dev-session.sh (it reported the config
# missing), so the profile is pinned where the shell starts.
NIRI_CONFIG="$ov_niri" RUST_LOG=info,celestina_wayland::runtime=debug,celestina::capture=debug "$bin" "\$@" &
shell=\$!
DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent kitty --config NONE --class smoke-open -o background='#ff8000' \\
    -o confirm_os_window_close=0 -o hide_window_decorations=yes sleep 40 &
k1=\$!
DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent kitty --config NONE --class smoke-blocked -o background='#0080ff' \\
    -o confirm_os_window_close=0 -o hide_window_decorations=yes sleep 40 &
k2=\$!
tries=0
until [ "\$(niri msg -j windows | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')" -ge 2 ]; do
    tries=\$((tries + 1))
    [ "\$tries" -lt $((ov_window_wait * 5)) ] || { echo "smoke-windows-missing" >&2; break; }
    sleep 0.2
done
niri msg -j windows | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    print("smoke-window app_id=%s id=%s" % (w.get("app_id"), w["id"]), file=sys.stderr)
    t = w.get("layout", {})
    print("smoke-window-layout id=%s scrolling=%s pos=%s size=%s" % (w["id"], t.get("pos_in_scrolling_layout"), t.get("tile_pos_in_workspace_view"), t.get("tile_size")), file=sys.stderr)
'
sleep 1.5
niri msg -j windows | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    t = w.get("layout", {})
    print("smoke-window-layout-late id=%s scrolling=%s pos=%s size=%s" % (w["id"], t.get("pos_in_scrolling_layout"), t.get("tile_pos_in_workspace_view"), t.get("tile_size")), file=sys.stderr)
'
send() {
    tries=0
    until "$bin" "\$@"; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-verb-failed \$*" >&2; return 1; }
        sleep 0.2
    done
}
send overview open
sleep 2
send shortcuts open
sleep 1
send shortcuts close
sleep 1
echo "smoke-rest-begin" >&2
sleep 2
echo "smoke-rest-end" >&2
# One try: the shell may stop before it answers; the backstop covers a miss.
"$bin" quit >/dev/null 2>&1 || echo "smoke-quit-unanswered" >&2
kill "\$k1" "\$k2" 2>/dev/null
wait "\$shell"
WRAPPER
chmod +x "$ov_wrapper"
run=$work/overview
status=0
XDG_CONFIG_HOME=$ov_xdg NIRI_CONFIG=$ov_niri CELESTINA_NEST_CONFIG=$ov_niri \
    timeout "$ov_timeout" "$here/dev-session.sh" --own-bus --monitor none --binary "$ov_wrapper" \
    --once "$run" -- --exit-after "$ov_exit_after" \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (overview run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (overview run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (overview run)"
if grep -q 'smoke-verb-failed\|smoke-windows-missing' "$run.err"; then
    fail "a verb did not reach the shell or a test window never appeared (overview run)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked in the overview run"
fi
grep -q 'overview: opened.*shortcuts=false' "$run.err" || fail "the overview did not open in Desktop mode"
grep -q 'overview: closed' "$run.err" || fail "the overview did not close"
echo "smoke: overview and shortcuts verbs answered and the panel closed"
# The overview's surface id is on the runtime's "layer surface created" line.
ov_surface=$(sed -n 's/.*layer surface created surface=\([0-9]*\) .*namespace="celestina-overview".*/\1/p' "$run.err" | head -n 1)
[ -n "$ov_surface" ] || fail "no celestina-overview surface was created"
grep -q "frame drawn frame=[0-9]* surface=$ov_surface " "$run.err" ||
    fail "no frame was drawn on the overview's surface ($ov_surface)"
echo "smoke: overview surface $ov_surface drew frames"
# The panel is never in its own capture: the frame was copied (the worker's
# "preview" line, logged once the compositor said `ready`) before the loop
# created the overview's surface. Both lines come from the one process, in
# order, into the one stderr file.
preview_line=$(grep -n 'capture: output winit: 1 preview(s)' "$run.err" | head -n 1 | cut -d: -f1)
surface_line=$(grep -n 'layer surface created surface=[0-9]* .*namespace="celestina-overview"' "$run.err" | head -n 1 | cut -d: -f1)
[ -n "$preview_line" ] && [ -n "$surface_line" ] ||
    fail "the capture's preview line or the overview's surface line is missing"
[ "$preview_line" -lt "$surface_line" ] ||
    fail "the overview's surface (line $surface_line) existed before its capture was taken (line $preview_line)"
echo "smoke: capture taken before the overview's surface existed (lines $preview_line < $surface_line)"
# The Shortcuts mode read the throwaway profile: one group for its `binds`
# blocks (merged) plus one per distinct binding mode.
expected_groups=$(python3 -c '
import re, sys
text = open(sys.argv[1]).read()
binds = 1 if re.search(r"^binds\s*\{", text, re.M) else 0
modes = set(re.findall(r"^binding-mode\s+\"([^\"]*)\"", text, re.M))
print(binds + len(modes))
' "$ov_niri")
loaded_groups=$(sed -n 's/.*overview: shortcuts loaded: \([0-9]*\) group(s).*/\1/p' "$run.err" | head -n 1)
[ -n "$loaded_groups" ] || fail "the shortcuts were never loaded"
[ "$loaded_groups" -eq "$expected_groups" ] ||
    fail "the shortcuts loaded $loaded_groups group(s), the profile has $expected_groups"
echo "smoke: shortcuts loaded $loaded_groups group(s), as the profile declares"
outputs=$(grep -c 'capture: output winit: 1 preview(s)' "$run.err" || true)
[ "$outputs" -eq 1 ] || fail "the capture was logged $outputs times as one preview of output winit, exactly one expected"
mean_luma=$(sed -n 's/.*capture: workspace [0-9]*: [0-9]*x[0-9]* mean luma //p' "$run.err" | head -n 1)
[ -n "$mean_luma" ] || fail "the capture logged no mean luminance for the preview"
[ "$mean_luma" -gt 10 ] || fail "the preview's mean luma is $mean_luma, expected above 10 (a black capture)"
echo "smoke: capture: preview mean luma $mean_luma"
open_id=$(sed -n 's/^smoke-window app_id=smoke-open id=//p' "$run.err" | head -n 1)
blocked_id=$(sed -n 's/^smoke-window app_id=smoke-blocked id=//p' "$run.err" | head -n 1)
[ -n "$open_id" ] && [ -n "$blocked_id" ] || fail "the test windows' ids were not logged"
# The windows are two columns side by side: the column index of each comes
# from niri's pos_in_scrolling_layout, "Some((column, row))" as printed.
column_of() {
    sed -n "s/^smoke-window-layout-late id=$1 scrolling=[^0-9]*\([0-9][0-9]*\),.*/\1/p" "$run.err" | head -n 1
}
open_col=$(column_of "$open_id")
blocked_col=$(column_of "$blocked_id")
[ -n "$open_col" ] && [ -n "$blocked_col" ] || fail "the test windows' columns were not logged"
[ "$open_col" != "$blocked_col" ] || fail "the test windows share column $open_col, expected two columns"
if [ "$blocked_col" -lt "$open_col" ]; then
    blocked_side=left
else
    blocked_side=right
fi
left_luma=$(sed -n 's/.*capture: workspace [0-9]*: left luma \([0-9]*\) right luma [0-9]*.*/\1/p' "$run.err" | head -n 1)
right_luma=$(sed -n 's/.*capture: workspace [0-9]*: left luma [0-9]* right luma \([0-9]*\).*/\1/p' "$run.err" | head -n 1)
[ -n "$left_luma" ] && [ -n "$right_luma" ] || fail "the capture logged no half luminance"
if [ "$blocked_side" = left ]; then
    blocked_luma=$left_luma
    open_luma=$right_luma
else
    blocked_luma=$right_luma
    open_luma=$left_luma
fi
[ "$open_luma" -gt 10 ] ||
    fail "the half with the open window ($open_id, column $open_col) has luma $open_luma, expected above 10 (the window is not in the capture)"
[ $((blocked_luma * 5)) -le "$open_luma" ] ||
    fail "the half with the blocked window ($blocked_id, column $blocked_col, $blocked_side) has luma $blocked_luma, expected at most a fifth of $open_luma"
echo "smoke: capture: left luma $left_luma right luma $right_luma; open window $open_id column $open_col, blocked window $blocked_id column $blocked_col ($blocked_side half black)"
rest_frames=$(awk '/smoke-rest-begin/ { on = 1; next } /smoke-rest-end/ { on = 0 }
    on && /frame drawn frame=/ { n++ } END { print n + 0 }' "$run.err")
[ "$rest_frames" -eq 0 ] || rest_fail "$run.err" "frames were drawn at rest after the overview closed ($rest_frames)"
echo "smoke: no frame at rest after the overview closed"
}
overview_run


# The polkit duty (CEL-2-I): the shell starts with `duties.polkit` on from a
# throwaway configuration home and the system bus pointed at a socket that is
# not there, so the agent can never reach a real polkitd or logind. It must
# start the duty, report itself unavailable once, register nothing and not
# panic. The wrapper sets the dead bus itself, as the energy run does, and the
# run is refused if that socket exists.
xdg_polkit=$work/xdg-polkit
mkdir -p "$xdg_polkit/celestina"
printf '{"schema_version":1,"duties":{"polkit":"on"}}\n' > "$xdg_polkit/celestina/shell.json"
[ ! -e "$work/no-system-bus" ] || fail "the dead system bus socket exists (polkit run)"
polkit=$work/polkit-run
cat > "$polkit" <<WRAPPER
#!/bin/sh
# Runs the shell with the polkit duty on over a system bus that is not there.
XDG_CONFIG_HOME='$xdg_polkit'
DBUS_SYSTEM_BUS_ADDRESS='unix:path=$work/no-system-bus'
export XDG_CONFIG_HOME DBUS_SYSTEM_BUS_ADDRESS
exec env RUST_LOG=info "$bin" "\$@"
WRAPPER
chmod +x "$polkit"
run=$work/polkit
status=0
timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$polkit" \
    --once "$run" -- --exit-after 5 \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (polkit run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (polkit run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (polkit run)"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the polkit duty on"
fi
grep -q 'polkit: the duty is on' "$run.err" ||
    fail "the polkit duty did not start"
grep -q 'polkit: the agent is unavailable' "$run.err" ||
    fail "the polkit agent did not report itself unavailable over the dead system bus"
if grep -Eq 'polkit: registration (Registered|Refused)' "$run.err"; then
    fail "the polkit agent registered or was answered: a system bus other than the dead one was reached"
fi
echo "smoke: polkit on over the dead system bus: the agent is unavailable and the shell did not panic"
echo "smoke: OK"
