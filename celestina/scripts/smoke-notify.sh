#!/bin/sh
set -eu

# The smoke's client, run by `dev-session.sh --probe` inside the nest: once
# the shell holds the notifications name on the nest's own bus it sends one
# Notify with a long body and a one second timeout, and prints the id the
# server answered. It refuses to run on the desktop's bus: the nest's address
# must differ from CELESTINA_HOST_BUS, the address the smoke was started with.
host_bus=${CELESTINA_HOST_BUS:-}
bus=${DBUS_SESSION_BUS_ADDRESS:-}
echo "probe: bus $bus"
if [ -z "$bus" ] || [ "$bus" = "$host_bus" ]; then
    echo "probe: refusing to use a bus that is not the nest's own" >&2
    exit 3
fi

name=org.freedesktop.Notifications
if command -v busctl >/dev/null 2>&1; then
    tool=busctl
elif command -v gdbus >/dev/null 2>&1; then
    tool=gdbus
else
    echo "probe: neither busctl nor gdbus is installed" >&2
    exit 4
fi

has_name() {
    case "$tool" in
        busctl) busctl --user list --no-legend 2>/dev/null | grep -q "^$name " ;;
        gdbus) gdbus call --session --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.NameHasOwner "$name" 2>/dev/null |
            grep -q true ;;
    esac
}

attempt=0
until has_name; do
    attempt=$((attempt + 1))
    if [ "$attempt" -gt 50 ]; then
        echo "probe: $name never appeared on $bus" >&2
        exit 5
    fi
    sleep 0.1
done
# The banners' surface needs the shell's first outputs; give it a moment.
sleep 1

# A body several times the machine's bound, to exercise it.
body=$(awk 'BEGIN { for (i = 0; i < 400; i++) printf "smoke body %d ", i }')
path=/org/freedesktop/Notifications
case "$tool" in
    busctl)
        reply=$(busctl --user call "$name" "$path" "$name" Notify \
            susssasa{sv}i smoke 0 "" "Smoke test" "$body" 0 0 1000) ;;
    gdbus)
        reply=$(gdbus call --session --dest "$name" --object-path "$path" \
            --method "$name.Notify" smoke 0 "" "Smoke test" "$body" "[]" "{}" 1000) ;;
esac
echo "probe: reply $reply"
# `u 7` (busctl) or `(uint32 7,)` (gdbus).
id=$(printf '%s\n' "$reply" | sed -n 's/[^0-9]*\([0-9][0-9]*\).*/\1/p')
if [ -z "$id" ] || [ "$id" -lt 1 ]; then
    echo "probe: no usable id in the reply" >&2
    exit 6
fi
echo "probe: id=$id"
