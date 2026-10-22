#!/bin/sh
set -eu

# The smoke's media client, run by `dev-session.sh` as the nest's program (the
# way the energy phase's wrapper is): it starts the fake MPRIS player of
# `examples/fake-mpris-player.rs` and the shell on the nest's own session bus,
# drives the shell through its own verbs, and exits with the shell's status.
# It refuses to run on the desktop's bus. Everything it needs comes from the
# environment `scripts/smoke.sh` sets:
#   SMOKE_MEDIA_SHELL   the shell binary under test
#   SMOKE_MEDIA_PLAYER  the fake player binary
#   SMOKE_MEDIA_ART     the player's `mpris:artUrl`
#   SMOKE_MEDIA_MODE    `steady` (player first, page opened, then closed, then
#                       left at rest) or
#                       `variant` (names compared, the duty flipped off and on)
#   SMOKE_MEDIA_CONFIG  the `shell.json` the mode rewrites (variant only)
# Its own notes go to stderr, which the smoke keeps as the run's log.
host_bus=${CELESTINA_HOST_BUS:-}
bus=${DBUS_SESSION_BUS_ADDRESS:-}
if [ -z "$bus" ] || [ "$bus" = "$host_bus" ]; then
    echo "smoke-media-failed: not the nest's own bus" >&2
    exit 3
fi

# Well-known names on the bus, one per line, sorted.
names() {
    if command -v busctl >/dev/null 2>&1; then
        busctl --user list --no-legend 2>/dev/null | awk '$1 !~ /^:/ { print $1 }'
    else
        gdbus call --session --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.ListNames 2>/dev/null |
            tr -d "[]()'," | tr ' ' '\n' | grep -v '^:' | grep .
    fi | sort
}
send() {
    tries=0
    until "$SMOKE_MEDIA_SHELL" "$@"; do
        tries=$((tries + 1))
        [ "$tries" -lt 25 ] || { echo "smoke-media-failed verb $*" >&2; return 1; }
        sleep 0.2
    done
}

players=
cleanup() {
    for p in $players; do kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT INT TERM

player() {
    "$SMOKE_MEDIA_PLAYER" "$bus" "$SMOKE_MEDIA_ART" "$1" >&2 &
    players="$players $!"
    tries=0
    until names | grep -qx 'org.mpris.MediaPlayer2.fake'; do
        tries=$((tries + 1))
        [ "$tries" -lt 50 ] || { echo "smoke-media-failed: no player on the bus" >&2; exit 4; }
        sleep 0.1
    done
}

case "${SMOKE_MEDIA_MODE:-}" in
    steady)
        # Player first: the shell finds it with its first scan, the cover is
        # decoded early, and the page opens on a settled shell.
        player 12
        RUST_LOG=${SMOKE_RUST_LOG:-info,celestina::island=debug} "$SMOKE_MEDIA_SHELL" "$@" &
        shell=$!
        send island open media
        echo "smoke-media: page requested" >&2
        # An open page draws a frame a second whatever it shows (its title row
        # carries the seconds clock: a frame somebody needs), so the rest the
        # run measures is the closed Island's: the page stays up long enough
        # for the cover to reach it, then closes.
        sleep 0.8
        send island close
        echo "smoke-media: page closed" >&2
        wait "$shell"
        ;;
    variant)
        RUST_LOG=${SMOKE_RUST_LOG:-info,celestina::island=debug} "$SMOKE_MEDIA_SHELL" "$@" &
        shell=$!
        send island open media
        echo "smoke-media: page requested" >&2
        sleep 0.5
        before=$(names | grep -v '^org.mpris.MediaPlayer2.fake$' | grep -v '^org.freedesktop.DBus$')
        player 6
        sleep 1.5
        after=$(names | grep -v '^org.mpris.MediaPlayer2.fake$' | grep -v '^org.freedesktop.DBus$')
        if [ "$before" = "$after" ]; then
            echo "smoke-media: shell names unchanged: $(printf '%s' "$before" | tr '\n' ' ')" >&2
        else
            echo "smoke-media-failed: the bus names changed with a player present" >&2
            printf 'before:\n%s\nafter:\n%s\n' "$before" "$after" >&2
        fi
        # The duty follows shell.json live: off joins the worker, on starts a
        # new one that finds the player again.
        printf '{"schema_version":1,"duties":{"media":"off"}}\n' > "$SMOKE_MEDIA_CONFIG.tmp"
        mv "$SMOKE_MEDIA_CONFIG.tmp" "$SMOKE_MEDIA_CONFIG"
        sleep 1.5
        echo "smoke-media: duty flipped off" >&2
        printf '{"schema_version":1,"duties":{"media":"on"}}\n' > "$SMOKE_MEDIA_CONFIG.tmp"
        mv "$SMOKE_MEDIA_CONFIG.tmp" "$SMOKE_MEDIA_CONFIG"
        sleep 1.5
        echo "smoke-media: duty flipped on" >&2
        wait "$shell"
        ;;
    *)
        echo "smoke-media-failed: unknown mode" >&2
        exit 2
        ;;
esac
