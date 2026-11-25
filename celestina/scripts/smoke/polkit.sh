#!/bin/sh
set -eu

# Smoke phase `polkit`: a nested run (CEL-2-I) turns the polkit duty on over
# the dead system bus: it must exit 0, report the agent unavailable, never
# register with a polkitd and not panic (the duty is never activated against
# the real bus here).
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only polkit`.
SMOKE_PHASE=polkit
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

# The polkit duty (CEL-2-I): the shell starts with `duties.polkit` on from a
# throwaway configuration home and the system bus pointed at a socket that is
# not there, so the agent can never reach a real polkitd or logind. It must
# start the duty, report itself unavailable once, register nothing and not
# panic. The wrapper sets the dead bus itself, as the energy run does, and the
# run is refused if that socket exists.
xdg_polkit=$work/xdg-polkit
mkdir -p "$xdg_polkit/celestina"
printf '{"schema_version":1,"duties":%s}\n' "$(duties polkit)" > "$xdg_polkit/celestina/shell.json"
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
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$polkit" \
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
