#!/bin/sh
set -eu

# The wallpaper checks' client, run by `dev-session.sh --probe` inside the
# nest: it polls the nest's compositor (`niri msg`, on the NIRI_SOCKET the
# nest exports) for up to eight seconds until a layer surface with the
# namespace `celestina-wallpaper` is listed, then prints the nest's layers as
# text and as JSON. It only reads; it never acts on the nest.
tries=0
until niri msg --json layers 2>/dev/null | grep -q 'celestina-wallpaper'; do
    tries=$((tries + 1))
    [ "$tries" -lt 80 ] || break
    sleep 0.1
done
echo "probe: layers (text)"
niri msg layers
echo "probe: layers (json)"
niri msg --json layers
echo "probe: outputs (json)"
niri msg --json outputs
