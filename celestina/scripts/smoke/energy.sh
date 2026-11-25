#!/bin/sh
set -eu

# Smoke phase `energy`: a nested run starts the shell with the three energy
# duties `on` (a throwaway XDG_CONFIG_HOME holding `celestina/shell.json`) and
# the system bus pointed at a socket that does not exist, so no real logind is
# ever reached; it must exit 0, report the idle worker armed or unavailable
# and the night light applied or not applied (the nest has no DRM gamma), and
# answer `caffeine toggle` and `session suspend` (refused: suspend goes
# through the lock). `session reboot` is deliberately not sent: it would be a
# real logind call on a bus the nest does not own.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only energy`.
SMOKE_PHASE=energy
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

# The energy duties: the shell starts with idle, caffeine and night light all
# `on` from a throwaway configuration home, and its own client sends two verbs.
# logind lives on the system bus, which a nest does not own: the wrapper points
# DBUS_SYSTEM_BUS_ADDRESS at a socket that is not there, so the shell can only
# report logind unavailable and nothing reaches the real one. `session reboot`
# is never sent for the same reason; `session suspend` is refused before any
# bus call because suspend goes through the lock (`lock suspend`).
xdg=$work/xdg
mkdir -p "$xdg/celestina"
cat > "$xdg/celestina/shell.json" <<JSON
{
  "schema_version": 1,
  "duties": $(duties idle caffeine nightlight),
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
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$energy" \
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
    fail "the shell panicked with idle, caffeine and the night light on"
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
