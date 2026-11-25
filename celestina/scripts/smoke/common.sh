# smoke/common.sh: what every phase of the smoke shares (POSIX sh, sourced by
# `scripts/smoke/ID.sh` once it has set SMOKE_PHASE; never run by itself).
#
# The driver, `scripts/smoke.sh`, starts each phase with SMOKE_PHASE_WORK (an
# empty directory of the attempt's own), SMOKE_BIN (the shell, an absolute
# path), SMOKE_SCRIPTS (`scripts/`), SMOKE_NIRI (the niri the nest runs) and
# CELESTINA_SMOKE_ATTEMPT (1 to 3). Without SMOKE_PHASE_WORK a phase refuses
# to run (status 2): it runs only through the driver.
#
# Here the phase takes the nest lock (`nest_lock_acquire` returns at once: the
# driver holds it and the marker says so), installs its traps
# (`nest_signals phase_cleanup`: TERM ends the running nest and then the fake
# Melibea of the phone phase, when it runs) and gets the helpers the phases
# share: the default configuration and state homes inside its own directory,
# the dead system bus, the helper that prints the duty object, the failure
# helpers (`fail`, `rest_fail` and what they read), and the fixtures more than
# one phase needs, each made under the phase's directory when the phase asks
# (`make_shell_run`, `make_wallpaper_fixture`, `make_osd_fakes`), and
# `need_python3`. The driver owns the work directory: nothing here removes it.
if [ -z "${SMOKE_PHASE_WORK:-}" ]; then
    echo "smoke: $SMOKE_PHASE: run it through smoke.sh --only $SMOKE_PHASE" >&2
    exit 2
fi
# Test hook, for testing the driver's retry only: with
# CELESTINA_SMOKE_FORCE_RETRY set to this phase's id, its first attempt exits
# 75 at once, as an inconclusive rest-frame check would, and the driver must
# start a second one. Never set it for a real verdict.
if [ "${CELESTINA_SMOKE_FORCE_RETRY:-}" = "$SMOKE_PHASE" ] && [ "${CELESTINA_SMOKE_ATTEMPT:-}" = 1 ]; then
    echo "smoke: forced inconclusive attempt (test hook)" >&2
    exit 75
fi
bin=$SMOKE_BIN
here=$SMOKE_SCRIPTS
work=$SMOKE_PHASE_WORK
niri_binary=$SMOKE_NIRI
run=
fake_pid=
# shellcheck source=scripts/nest-run.sh
. "$here/nest-run.sh"
nest_lock_acquire "smoke.sh phase $SMOKE_PHASE"
# The phase's cleanup, run once on every way out (EXIT; TERM and INT exit 143
# and 130 after stopping the running nest): the fake Melibea when it was
# started. The driver removes the work directory and releases the lock.
phase_cleanup() {
    [ -z "$fake_pid" ] || stop_fake
}
nest_signals phase_cleanup
# The probe refuses the desktop's own bus: it must find a different address.
CELESTINA_HOST_BUS=${DBUS_SESSION_BUS_ADDRESS:-}
export CELESTINA_HOST_BUS
# Every nested run gets a throwaway configuration home and a system bus that
# does not exist, so a landing's smoke never acts through the author's real
# settings, their clipboard history or their real logind. Since the cutover
# (CEL-2-M) a `shell.json` that chooses nothing has every shipped duty on, so
# a run that wants a duty off must say so: `duties` prints the `duties` object
# with every duty off except those named. The list below is every key of the
# domain's `Duties`; `tests/duty_files.rs` fails when a duty is added there
# and not here, so no run can start a worker it did not name. The bar is not
# a duty (every run draws it). Runs 1 and 2 start with every duty off; the
# wallpaper and energy runs replace the home with their own, and every run
# keeps the dead system bus.
duties() {
    d_out=
    for d_name in bubbles caffeine clipboard idle lock media nightlight osd phone polkit wallpaper; do
        d_state=off
        case " $* " in
            *" $d_name "*) d_state=on ;;
        esac
        d_out="$d_out${d_out:+,}\"$d_name\":\"$d_state\""
    done
    printf '{%s}' "$d_out"
}
xdg_default=$work/xdg-default
mkdir -p "$xdg_default/celestina"
printf '{"schema_version":1,"duties":%s}\n' "$(duties)" > "$xdg_default/celestina/shell.json"
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
make_shell_run() {
shell_run=$work/shell-run
cat > "$shell_run" <<WRAPPER
#!/bin/sh
RUST_LOG=info,celestina_wayland::runtime=debug exec "$bin" "\$@"
WRAPPER
chmod +x "$shell_run"
}

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
# rest_context JOURNAL: the journal's lines before each rest frame, without
# the frames themselves. The runtime writes `metrics: rest frame` when it
# counts one, so what asked for the frame (an output change, a duty switch, a
# worker's news) is the line just above it; the journal is deleted with the
# run, and its last forty lines are mostly frames.
rest_context() {
    echo "--- what preceded each rest frame ($(basename -- "$1"))" >&2
    grep -v 'frame drawn frame=' "$1" | grep -B 4 'metrics: rest frame' | tail -n 60 >&2 || true
}
# output_changes JOURNAL [AFTER [UNTIL]]: how many times the nest's output
# changed in the journal, after the first line holding AFTER and before the
# first one holding UNTIL, when given. The nest's output is its window on the
# host, so each is the host resizing that window under the run: nothing this
# script or the shell did. With the wallpaper's surface up, each costs that
# surface two frames at rest (the wallpaper run's comment says which).
output_changes() {
    awk -v after="${2:-}" -v until="${3:-}" '
    after != "" && !seen { if (index($0, after)) seen = 1; next }
    until != "" && index($0, until) { exit }
    /output changed output=/ { n++ }
    END { print n + 0 }' "$1"
}
# rest_fail JOURNAL MESSAGE: a rest-frame budget was exceeded. In a throttled
# nest the start-up frames land late and the budget means nothing, so that is
# inconclusive: exit 75, and the driver starts the phase again in a new
# directory, up to three attempts; on the last attempt, and in a nest that
# was not throttled, it is a failure.
rest_fail() {
    rest_context "$1"
    if wait_ms=$(nest_throttled "$1"); then
        [ "${CELESTINA_SMOKE_ATTEMPT:-1}" -lt 3 ] ||
            fail "$2 (inconclusive: the nest withheld a frame callback for $wait_ms ms, its window is hidden)"
        echo "smoke: inconclusive (the nest withheld a frame callback for $wait_ms ms): $2" >&2
        tails
        exit 75
    fi
    fail "$2"
}
# sum KEY: KEY=N summed over the metrics lines a phase read into `lines`.
sum() {
    printf '%s\n' "$lines" | tr ' ' '\n' | sed -n "s/^$1=//p" |
        awk '{ n += $1 } END { print n + 0 }'
}
# stop_fake: end the fake Melibea the phone phase started (fake_pid).
stop_fake() {
    [ -z "${fake_pid:-}" ] || kill "$fake_pid" 2>/dev/null || true
}
# need_python3: every phase that runs python3 (fixtures, probes, readers of
# the nest's JSON) refuses without it instead of failing halfway. The message
# is the wallpaper gate's, unchanged.
need_python3() {
if ! command -v python3 >/dev/null 2>&1; then
    fail "python3 is absent: the wallpaper gate cannot run (it generates its fixture and reads the layers with it), so this smoke is not OK"
fi
}
# make_wallpaper_fixture: the wallpaper gate's fixture picture,
# `$fixture/default.png` (a 64x36 PNG), and the wallpaper run's configuration
# home `$config_home`, both under the phase's directory. The clipboard and
# cutover phases use the picture too.
make_wallpaper_fixture() {
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
}
# make_osd_fakes: the OSD run's stand-ins for `wpctl`, `pactl`, `ddcutil` and
# `brightnessctl` in `$osd_fakes` (see the osd phase), and its state directory
# `$osd_state`. The cutover phase puts the same fakes first on PATH.
make_osd_fakes() {
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
}
