#!/bin/sh
set -eu

# Smoke phase `media`: two nested runs (CEL-2-D) turn the media duty on
# against a fake MPRIS player (see the media block below). Without the
# example built beside the binary, or without busctl and gdbus, the phase is
# skipped with a notice and exits 0.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only media`.
SMOKE_PHASE=media
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

need_python3
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
    printf '{"schema_version":1,"duties":%s}\n' "$(duties media)" > "$media_home/celestina/shell.json"
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
        nest 60 XDG_CONFIG_HOME="$media_home" SMOKE_MEDIA_SHELL="$bin" SMOKE_MEDIA_PLAYER="$player" \
            SMOKE_MEDIA_MODE="$2" SMOKE_MEDIA_ART="$3" \
            SMOKE_RUST_LOG=info,celestina::island=debug,celestina_wayland::runtime=debug \
            SMOKE_MEDIA_CONFIG="$media_home/celestina/shell.json" \
            SMOKE_MEDIA_OFF="{\"schema_version\":1,\"duties\":$(duties)}" \
            SMOKE_MEDIA_ON="{\"schema_version\":1,\"duties\":$(duties media)}" \
            "$here/dev-session.sh" --own-bus --monitor none \
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
