#!/bin/sh
set -eu

# Smoke phase `osd`: two nested runs cover the OSD duty (CEL-2-C) on fake
# `wpctl`, `pactl`, `ddcutil` and `brightnessctl` placed first on `PATH`
# (`CELESTINA_DDC=0`, made by `make_osd_fakes`): with the duty on there is no
# `celestina-osd` surface at rest, an audio event raises it and it is
# destroyed again, and `volume up` reaches the fake `wpctl`; with the duty off
# the verbs are rejected and no fake is called.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only osd`.
SMOKE_PHASE=osd
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

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
# (every duty off, what a file before the cutover meant): both verbs are
# rejected and no fake is called at all.
# Run A holds the OSD for 4000 ms and runs for 16 s, not the 500 ms and 9 s a
# quick desktop would do: when the host compositor throttles the nest window
# the nest's loop runs only every 1-2 s (measured), so a new layer surface gets
# no configure, and so no frame, before a 500 ms OSD is already destroyed. A
# hold of 4000 ms spans at least two such ticks, and the verbs re-show the
# live surface, which leaves it three to six. The last show is at about 5 s
# (the verbs, 2 s after the checkpoint at 3 s), so it expires at about 9 s and
# retires before 16 s.
make_osd_fakes
mkdir -p "$work/xdg-osd/celestina" "$work/xdg-osd-off/celestina"
printf '{"schema_version":1,"duties":%s,"osd":{"timeout_ms":4000}}\n' "$(duties osd)" \
    > "$work/xdg-osd/celestina/shell.json"
printf '{"schema_version":1,"duties":%s}\n' "$(duties)" > "$work/xdg-osd-off/celestina/shell.json"
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
    nest 60 "$here/dev-session.sh" --own-bus --monitor none \
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
