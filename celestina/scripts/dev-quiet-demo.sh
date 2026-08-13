#!/bin/sh

set -eu

# dev-quiet-demo.sh — exercise every quiet-surface behaviour in the running
# nest, one step at a time, saying what to look at before each one.
#
# It drives only the routes a key or a provider would: notifications over the
# nest's own bus, volume through wpctl, brightness through the shell's session
# verb (one DDC worker, never a bare ddcutil). Volume and brightness are
# restored at the end. The one behaviour it cannot drive is the real-time
# retreat under a *clicked* panel menu — injecting input into the live nest is
# off the table — so it ends by asking for that click.
#
#   dev-quiet-demo.sh                 run the whole sequence
#   CELESTINA_DEMO_OUTPUT=DP-2 ...    name the DDC monitor (default DP-1)

runtime=${XDG_RUNTIME_DIR:-/tmp}
env_file=$runtime/celestina-dev-session.env
output=${CELESTINA_DEMO_OUTPUT:-DP-1}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
celestina=$here/../build/celestina

if [ ! -f "$env_file" ]; then
    echo "dev-quiet-demo: no nested session is recorded; start one with dev-session.sh" >&2
    exit 1
fi
# shellcheck source=/dev/null
. "$env_file"
export WAYLAND_DISPLAY NIRI_SOCKET
[ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && export DBUS_SESSION_BUS_ADDRESS

if ! busctl --user --no-pager status org.freedesktop.Notifications >/dev/null 2>&1; then
    echo "dev-quiet-demo: no notification server on the nest's bus yet" >&2
    exit 1
fi
if [ ! -x "$celestina" ]; then
    echo "dev-quiet-demo: $celestina is missing" >&2
    exit 1
fi

step() {
    echo
    echo ">> $1"
    sleep 1
}

step "1/7 Toast: one drop from the bell, top right"
notify-send -t 6000 -a 'Celestina' 'One alone' 'It hangs from the bell by its membrane.'
sleep 6

step "2/7 Toast pile: only the first grips the bar, the rest hang below"
for n in One Two Three; do
    notify-send -t 8000 -a 'Celestina' "Message $n" "Body of message $n."
done
sleep 9

step "3/7 Critical toast: red stripe, action buttons, never expires — dismiss it with its X"
notify-send -u critical -a 'Magnetita' 'Critical' 'Red stripe and buttons over the glass.' \
    -A open=Open -A mute=Mute &
sleep 6

step "4/7 Volume display: a card out of the speaker icon, gone in about two seconds"
wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%+
sleep 5

step "5/7 Brightness display: same drop, out of the sun, named after $output"
"$celestina" msg brightness-step by=5 output="$output" >/dev/null 2>&1 \
    || echo "   (no DDC answer from $output; skipping)"
sleep 5

step "6/7 The card file: brightness first, volume slides in front, the other peeks behind — hover raises it"
"$celestina" msg brightness-step by=5 output="$output" >/dev/null 2>&1 || true
sleep 1
wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%+
sleep 6

step "7/7 Fallback: a toast holds the corner, so the volume card retreats to the bottom right"
notify-send -t 10000 -a 'Celestina' 'Holding the corner' 'The display arriving now yields to the bottom right.'
sleep 2.5
wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%-
sleep 6

echo
echo ">> restoring levels"
wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%-
"$celestina" msg brightness-step by=-10 output="$output" >/dev/null 2>&1 || true

echo
echo ">> done. One check needs your pointer: raise a volume card with the"
echo "   wheel, then click a panel menu in that corner — the card must jump"
echo "   to the bottom right at once, and come back on the next reading."
