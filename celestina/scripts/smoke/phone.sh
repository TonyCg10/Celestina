#!/bin/sh
set -eu

# Smoke phase `phone`: three nested runs turn the phone and bubbles duties on
# against `fake-melibea.py` in a private runtime directory (CEL-2-J): one must
# draw no frame at rest, one opens and closes the bubbles selector and answers
# the phone verbs, and one must log the missing Melibea socket as absent
# exactly once. The fake is started without `nest`: `phase_cleanup`
# (`smoke/common.sh`) stops it on every way out, TERM included.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only phone`.
SMOKE_PHASE=phone
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

need_python3
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
phone_home=$work/phone
phone_runtime=$phone_home/runtime
phone_socket=$phone_runtime/melibea.sock
mkdir -p "$phone_home/config/celestina" "$phone_runtime"
chmod 700 "$phone_home" "$phone_runtime"
printf '{"schema_version":1,"duties":%s}\n' "$(duties phone bubbles)" \
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

# The fake serves until SIGTERM and removes its socket then; `phase_cleanup`
# makes sure no failing exit leaves it running.
run=$phone_home/fake
python3 -I "$here/fake-melibea.py" "$phone_socket" > "$phone_home/fake.log" 2>&1 &
fake_pid=$!
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
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_metrics" \
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
    rest_fail "$run.err" "frames were drawn at rest with the phone and the bubbles on ($rest): $lines"
if ! grep -q 'melibea: subscribed' "$run.err"; then
    fail "the shell did not subscribe to the fake Melibea (phone metrics run)"
fi
grep -q 'bubbles: windows count=1' "$run.err" ||
    fail "the fake's one window did not reach the bubbles (phone metrics run)"
grep -q 'magnetita: absent: nobody owns' "$run.err" ||
    fail "the phone piece was not reported absent (phone metrics run)"
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the phone and the bubbles on"
fi
echo "smoke: phone and bubbles on: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$rest); Melibea subscribed with one window, Magnetita absent"

run=$work/phone-verbs
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_verbs" \
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
nest 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$phone_absent" \
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
