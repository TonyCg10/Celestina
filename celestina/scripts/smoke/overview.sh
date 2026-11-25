#!/bin/sh
set -eu

# Smoke phase `overview`: a nested run (CEL-2-L) starts two coloured `kitty`
# windows, one under a `block-out-from "screen-capture"` rule of a throwaway
# niri profile, opens the overview and then the shortcuts, and checks the
# capture's one preview of the panel's output (painted overall, the half with
# the blocked window black, taken before the panel's surface existed), the
# shortcut groups read from that profile, and no frame at rest after the panel
# closes. Without kitty the phase is skipped with a notice and exits 0.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only overview`.
SMOKE_PHASE=overview
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

need_python3
# The overview and the shortcuts (CEL-2-L): a nested run with its own niri
# profile (the reference one plus a `block-out-from "screen-capture"` rule for
# app-id `smoke-blocked`) and a throwaway XDG_CONFIG_HOME; NIRI_CONFIG names the
# profile so the Shortcuts mode reads a known file, never the author's. Two
# plain `kitty` windows with distinct flat backgrounds (orange `smoke-open`,
# blue `smoke-blocked`) are started by the wrapper, which waits up to
# $ov_window_wait seconds for both; `celestina overview open` is held two
# seconds, `shortcuts open` one second, then `shortcuts close`, a rest, and
# `quit`. `--exit-after` is only the backstop, computed so the slowest allowed
# sequence still ends by `quit` first.
# The capture worker logs one line per output and, at debug level, the mean
# luminance of the whole-output preview and of its left and right halves. niri
# 26.04 gives no on-screen tile position, so there are no per-window crops: the
# two windows are two columns side by side, and the half that holds the blocked
# window must be black (at most a fifth of the other half: it still holds the
# compositor's own backdrop and gaps, which are not blacked out) while the other
# half and the whole preview are painted.
# The windows get a dead session bus so kitty activates no portal. Only pids
# started here are killed. Without kitty this part is skipped.
overview_run() {
# The longest wait for the two test windows, and the worst case of the rest:
# the sleeps of the sequence (1.5 + 2 + 1 + 1 + 2) and four verbs that may
# each retry for 4 s. The shell's own backstop and the nest's timeout follow.
ov_window_wait=20
ov_exit_after=$((ov_window_wait + 8 + 4 * 4 + 6))
ov_timeout=$((ov_exit_after + 20))
if ! command -v kitty >/dev/null 2>&1; then
    echo "smoke: overview run skipped: kitty is absent (the test windows)"
    return 0
fi
while [ "$(date +%S)" -gt 34 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
ov_xdg=$work/xdg-overview
mkdir -p "$ov_xdg/celestina"
printf '{"schema_version":1,"duties":%s}\n' "$(duties)" > "$ov_xdg/celestina/shell.json"
ov_niri=$work/overview-niri.kdl
{
    cat "$here/dev-session.kdl"
    cat <<'KDL'

window-rule {
    match app-id="^smoke-blocked$"
    block-out-from "screen-capture"
}
KDL
} > "$ov_niri"
ov_wrapper=$work/overview-run
cat > "$ov_wrapper" <<WRAPPER
#!/bin/sh
# Runs the shell, starts the two test windows, drives the overview and exits
# with the shell's status. Everything it prints goes to the run's stderr file,
# in order with the shell's own lines.
# NIRI_CONFIG is named again here: in the first run the shell inside the nest
# did not see the value given to dev-session.sh (it reported the config
# missing), so the profile is pinned where the shell starts.
NIRI_CONFIG="$ov_niri" RUST_LOG=info,celestina_wayland::runtime=debug,celestina::capture=debug "$bin" "\$@" &
shell=\$!
DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent kitty --config NONE --class smoke-open -o background='#ff8000' \\
    -o confirm_os_window_close=0 -o hide_window_decorations=yes sleep 40 &
k1=\$!
DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent kitty --config NONE --class smoke-blocked -o background='#0080ff' \\
    -o confirm_os_window_close=0 -o hide_window_decorations=yes sleep 40 &
k2=\$!
tries=0
until [ "\$(niri msg -j windows | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')" -ge 2 ]; do
    tries=\$((tries + 1))
    [ "\$tries" -lt $((ov_window_wait * 5)) ] || { echo "smoke-windows-missing" >&2; break; }
    sleep 0.2
done
niri msg -j windows | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    print("smoke-window app_id=%s id=%s" % (w.get("app_id"), w["id"]), file=sys.stderr)
    t = w.get("layout", {})
    print("smoke-window-layout id=%s scrolling=%s pos=%s size=%s" % (w["id"], t.get("pos_in_scrolling_layout"), t.get("tile_pos_in_workspace_view"), t.get("tile_size")), file=sys.stderr)
'
sleep 1.5
niri msg -j windows | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    t = w.get("layout", {})
    print("smoke-window-layout-late id=%s scrolling=%s pos=%s size=%s" % (w["id"], t.get("pos_in_scrolling_layout"), t.get("tile_pos_in_workspace_view"), t.get("tile_size")), file=sys.stderr)
'
send() {
    tries=0
    until "$bin" "\$@"; do
        tries=\$((tries + 1))
        [ "\$tries" -lt 20 ] || { echo "smoke-verb-failed \$*" >&2; return 1; }
        sleep 0.2
    done
}
send overview open
sleep 2
send shortcuts open
sleep 1
send shortcuts close
sleep 1
echo "smoke-rest-begin" >&2
sleep 2
echo "smoke-rest-end" >&2
# One try: the shell may stop before it answers; the backstop covers a miss.
"$bin" quit >/dev/null 2>&1 || echo "smoke-quit-unanswered" >&2
kill "\$k1" "\$k2" 2>/dev/null
wait "\$shell"
WRAPPER
chmod +x "$ov_wrapper"
run=$work/overview
status=0
nest "$ov_timeout" XDG_CONFIG_HOME="$ov_xdg" NIRI_CONFIG="$ov_niri" CELESTINA_NEST_CONFIG="$ov_niri" \
    "$here/dev-session.sh" --own-bus --monitor none --binary "$ov_wrapper" \
    --once "$run" -- --exit-after "$ov_exit_after" \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (overview run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (overview run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (overview run)"
if grep -q 'smoke-verb-failed\|smoke-windows-missing' "$run.err"; then
    fail "a verb did not reach the shell or a test window never appeared (overview run)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked in the overview run"
fi
grep -q 'overview: opened.*shortcuts=false' "$run.err" || fail "the overview did not open in Desktop mode"
grep -q 'overview: closed' "$run.err" || fail "the overview did not close"
echo "smoke: overview and shortcuts verbs answered and the panel closed"
# The overview's surface id is on the runtime's "layer surface created" line.
ov_surface=$(sed -n 's/.*layer surface created surface=\([0-9]*\) .*namespace="celestina-overview".*/\1/p' "$run.err" | head -n 1)
[ -n "$ov_surface" ] || fail "no celestina-overview surface was created"
grep -q "frame drawn frame=[0-9]* surface=$ov_surface " "$run.err" ||
    fail "no frame was drawn on the overview's surface ($ov_surface)"
echo "smoke: overview surface $ov_surface drew frames"
# The panel is never in its own capture: the frame was copied (the worker's
# "preview" line, logged once the compositor said `ready`) before the loop
# created the overview's surface. Both lines come from the one process, in
# order, into the one stderr file.
preview_line=$(grep -n 'capture: output winit: 1 preview(s)' "$run.err" | head -n 1 | cut -d: -f1)
surface_line=$(grep -n 'layer surface created surface=[0-9]* .*namespace="celestina-overview"' "$run.err" | head -n 1 | cut -d: -f1)
[ -n "$preview_line" ] && [ -n "$surface_line" ] ||
    fail "the capture's preview line or the overview's surface line is missing"
[ "$preview_line" -lt "$surface_line" ] ||
    fail "the overview's surface (line $surface_line) existed before its capture was taken (line $preview_line)"
echo "smoke: capture taken before the overview's surface existed (lines $preview_line < $surface_line)"
# The Shortcuts mode read the throwaway profile: one group for its `binds`
# blocks (merged) plus one per distinct binding mode.
expected_groups=$(python3 -c '
import re, sys
text = open(sys.argv[1]).read()
binds = 1 if re.search(r"^binds\s*\{", text, re.M) else 0
modes = set(re.findall(r"^binding-mode\s+\"([^\"]*)\"", text, re.M))
print(binds + len(modes))
' "$ov_niri")
loaded_groups=$(sed -n 's/.*overview: shortcuts loaded: \([0-9]*\) group(s).*/\1/p' "$run.err" | head -n 1)
[ -n "$loaded_groups" ] || fail "the shortcuts were never loaded"
[ "$loaded_groups" -eq "$expected_groups" ] ||
    fail "the shortcuts loaded $loaded_groups group(s), the profile has $expected_groups"
echo "smoke: shortcuts loaded $loaded_groups group(s), as the profile declares"
outputs=$(grep -c 'capture: output winit: 1 preview(s)' "$run.err" || true)
[ "$outputs" -eq 1 ] || fail "the capture was logged $outputs times as one preview of output winit, exactly one expected"
mean_luma=$(sed -n 's/.*capture: workspace [0-9]*: [0-9]*x[0-9]* mean luma //p' "$run.err" | head -n 1)
[ -n "$mean_luma" ] || fail "the capture logged no mean luminance for the preview"
[ "$mean_luma" -gt 10 ] || fail "the preview's mean luma is $mean_luma, expected above 10 (a black capture)"
echo "smoke: capture: preview mean luma $mean_luma"
open_id=$(sed -n 's/^smoke-window app_id=smoke-open id=//p' "$run.err" | head -n 1)
blocked_id=$(sed -n 's/^smoke-window app_id=smoke-blocked id=//p' "$run.err" | head -n 1)
[ -n "$open_id" ] && [ -n "$blocked_id" ] || fail "the test windows' ids were not logged"
# The windows are two columns side by side: the column index of each comes
# from niri's pos_in_scrolling_layout, "Some((column, row))" as printed.
column_of() {
    sed -n "s/^smoke-window-layout-late id=$1 scrolling=[^0-9]*\([0-9][0-9]*\),.*/\1/p" "$run.err" | head -n 1
}
open_col=$(column_of "$open_id")
blocked_col=$(column_of "$blocked_id")
[ -n "$open_col" ] && [ -n "$blocked_col" ] || fail "the test windows' columns were not logged"
[ "$open_col" != "$blocked_col" ] || fail "the test windows share column $open_col, expected two columns"
if [ "$blocked_col" -lt "$open_col" ]; then
    blocked_side=left
else
    blocked_side=right
fi
left_luma=$(sed -n 's/.*capture: workspace [0-9]*: left luma \([0-9]*\) right luma [0-9]*.*/\1/p' "$run.err" | head -n 1)
right_luma=$(sed -n 's/.*capture: workspace [0-9]*: left luma [0-9]* right luma \([0-9]*\).*/\1/p' "$run.err" | head -n 1)
[ -n "$left_luma" ] && [ -n "$right_luma" ] || fail "the capture logged no half luminance"
if [ "$blocked_side" = left ]; then
    blocked_luma=$left_luma
    open_luma=$right_luma
else
    blocked_luma=$right_luma
    open_luma=$left_luma
fi
[ "$open_luma" -gt 10 ] ||
    fail "the half with the open window ($open_id, column $open_col) has luma $open_luma, expected above 10 (the window is not in the capture)"
[ $((blocked_luma * 5)) -le "$open_luma" ] ||
    fail "the half with the blocked window ($blocked_id, column $blocked_col, $blocked_side) has luma $blocked_luma, expected at most a fifth of $open_luma"
echo "smoke: capture: left luma $left_luma right luma $right_luma; open window $open_id column $open_col, blocked window $blocked_id column $blocked_col ($blocked_side half black)"
rest_frames=$(awk '/smoke-rest-begin/ { on = 1; next } /smoke-rest-end/ { on = 0 }
    on && /frame drawn frame=/ { n++ } END { print n + 0 }' "$run.err")
[ "$rest_frames" -eq 0 ] || rest_fail "$run.err" "frames were drawn at rest after the overview closed ($rest_frames)"
echo "smoke: no frame at rest after the overview closed"
}
overview_run
