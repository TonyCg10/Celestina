#!/bin/sh
set -eu

# Smoke phase `connectivity`: two nested runs place the four connectivity
# pieces at the bar's end (CEL-2-F): one on a private system bus served by
# `examples/fake_system_bus` (fake NetworkManager, BlueZ, UPower and
# power-profiles-daemon, ended by closing its input), which must show the
# network, Bluetooth, battery and profiles workers ready; one on a socket that
# does not exist, which must show them all unavailable. Neither touches the
# real system bus.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only connectivity`.
SMOKE_PHASE=connectivity
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

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
# The build runs through `nest` (a process group of its own, an hour at most:
# it may wait on the shared target directory's lock) so that TERM ends it too.
nest 3600 CARGO_TARGET_DIR="$target_dir" cargo build --example fake_system_bus --locked --offline \
    $profile_flag --manifest-path "$here/../Cargo.toml" > "$work/fake-build.log" 2>&1 ||
    { tail -n 40 "$work/fake-build.log" >&2; fail "the fake system bus example did not build"; }
fake=$target_dir/$profile/examples/fake_system_bus
[ -x "$fake" ] || fail "the fake system bus example is not at $fake"
conn_cfg=$work/xdg-connectivity
mkdir -p "$conn_cfg/celestina"
cat > "$conn_cfg/celestina/shell.json" <<JSON
{
  "schema_version": 1,
  "duties": $(duties),
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
nest 90 XDG_CONFIG_HOME="$conn_cfg" "$here/dev-session.sh" --own-bus --monitor none \
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
nest 60 XDG_CONFIG_HOME="$conn_cfg" "$here/dev-session.sh" --own-bus --monitor none \
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
