#!/bin/sh
set -eu

# Smoke phase `wallpaper`: a nested run turns the wallpaper duty on over a
# generated fixture picture (its own throwaway XDG_CONFIG_HOME) and must show
# a `celestina-wallpaper` surface in the Background layer with at most two
# frames at rest, and two more for each time the host resized the nest's
# window once that surface exists.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only wallpaper`.
SMOKE_PHASE=wallpaper
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

make_shell_run
# The wallpaper duty (CEL-2-E): the shell runs ten seconds with a private
# XDG_CONFIG_HOME whose `celestina/shell.json` turns `duties.wallpaper` on and
# points `wallpaper.folder` at a fixture folder holding one generated PNG
# (the author's own configuration and pictures are never read). A probe inside
# the nest lists the compositor's layers: (a) a `celestina-wallpaper` surface
# must sit in `Background` on the nested output, (b) the metrics show no frame
# at rest beyond an allowance of the same kind as the notification's, frames
# somebody needed though nothing animates in them. Two for the surface
# learning its real scale after its first frame: the frame at that scale and
# the picture decoded again for it. The surface is created when the first
# picture arrives, so that picture is its first frame and never a rest frame,
# and the Island's frames say the real scale before then; the two are owed
# only when the first decode beats the Island's first frame and the answer
# then takes over a second (never seen in the runs of the evidence record
# 2026-11-24-smoke-rest.md, which measured none at rest). And two for each
# change of the nest's output after the wallpaper's surface exists, which is
# the host resizing the nest's window under the run: the surface at its new
# size with the old picture stretched, and the picture decoded for that size
# (measured: exactly two per resize, and none from a loaded machine alone).
# The landing of 2026-11-23 that failed here with four had them all on the
# wallpaper's surface. Any other frame at rest fails: a shell that redrew by
# itself every two seconds would draw four in the ten-second run. (c) is not
# run: no layer-shell client that could hold the Background
# layer exists in the repository, and none is invented here; the owner
# detection that makes the shell refuse such an output is covered by the unit
# tests of `src/wallpaper` and `src/providers/wallpaper.rs`.
need_python3
make_wallpaper_fixture
cat > "$config_home/celestina/shell.json" <<JSON
{"schema_version":1,"duties":$(duties wallpaper),"wallpaper":{"folder":"$fixture"}}
JSON
run=$work/wallpaper
status=0
while [ "$(date +%S)" -gt 48 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
nest 60 XDG_CONFIG_HOME="$config_home" "$here/dev-session.sh" --own-bus \
    --monitor none --binary "$shell_run" --probe "$here/wallpaper-layers-probe.sh" \
    --once "$run" -- --exit-after 10 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (wallpaper run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (wallpaper run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (wallpaper run)"
[ "$(cat "$run.probe.status" 2>/dev/null)" = 0 ] ||
    fail "the layers probe failed (wallpaper run)"
python3 - "$run.probe" <<'LAYERS' || fail "no celestina-wallpaper surface in the Background layer of the nested output"
import json, sys

lines = open(sys.argv[1]).read().split("\n")
at = lines.index("probe: layers (json)") + 1
layers = json.loads(lines[at])
found = [l for l in layers
         if l["namespace"] == "celestina-wallpaper"
         and l["layer"] == "Background" and l["output"] == "winit"]
if not found:
    sys.exit(1)
print("smoke: celestina-wallpaper is in Background on winit")
LAYERS
wp_lines=$(grep '^winit ' "$run.out" || true)
[ -n "$wp_lines" ] || fail "no metrics line for the winit output (wallpaper run)"
wp_frames=$(printf '%s\n' "$wp_lines" | tr ' ' '\n' | sed -n 's/^frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
wp_rest=$(printf '%s\n' "$wp_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
[ "$wp_frames" -ge 1 ] || fail "no frame was drawn (wallpaper run): $wp_lines"
wp_changes=$(output_changes "$run.err" 'wallpaper surface created')
wp_allowed=$((2 + 2 * wp_changes))
[ "$wp_rest" -le "$wp_allowed" ] ||
    rest_fail "$run.err" "frames were drawn at rest with the wallpaper present ($wp_rest, at most $wp_allowed: 2 for the real scale, 2 for each of $wp_changes output changes): $wp_lines"
if grep -q 'wallpaper: output held by another client' "$run.err"; then
    fail "the shell found the nested output held (wallpaper run)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the wallpaper on"
fi
echo "smoke: wallpaper present, frames=$wp_frames rest_frames=$wp_rest (at most $wp_allowed, $wp_changes output changes) over $(printf '%s\n' "$wp_lines" | wc -l) surfaces"
echo "smoke: wallpaper held-output variant skipped: no layer-shell client in the repository (covered by unit tests)"
