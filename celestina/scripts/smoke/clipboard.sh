#!/bin/sh
set -eu

# Smoke phase `clipboard`: a nested run starts the shell with the clipboard
# duty `on` (CEL-2-G) and drives it with `wl-copy`, a client that demonstrably
# speaks `ext-data-control` to the nest (checked below with WAYLAND_DEBUG) and
# that only ever talks to the nest's own compositor: the run refuses to start
# a client when its display is the host's. Fixed test strings are copied, a
# selection carrying the password-manager hint, and a PNG (the wallpaper
# gate's fixture picture, made in this phase's directory by
# `make_wallpaper_fixture`); the history, the state file (mode, content) and
# the shell's own re-selection are checked, the duty is switched off and on
# in `shell.json`, and the journal must hold none of the test strings.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only clipboard`.
SMOKE_PHASE=clipboard
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

need_python3
make_wallpaper_fixture
# The clipboard duty (CEL-2-G). Inside the nest, the shell runs with
# `duties.clipboard` on, a throwaway configuration home and state home, and the
# wrapper below plays the part of a few applications with `wl-copy`/`wl-paste`
# (wl-clipboard 2.3 speaks ext-data-control; the WAYLAND_DEBUG capture proves
# it for this nest). The author's real clipboard is never reachable: the
# wrapper refuses to start any client unless WAYLAND_DISPLAY is the nest's own,
# different from the host's. Only fixed test strings are copied. The shell is
# observed through its journal (counts, sizes, reasons: never content) and
# through the state file; there is no verb that returns content to read.
for tool in wl-copy wl-paste; do
    command -v "$tool" >/dev/null 2>&1 ||
        fail "$tool is absent: the clipboard gate cannot run (wl-clipboard), so this smoke is not OK"
done
clip_alpha=clipsmoke-visible-7f3a91
clip_beta=clipsmoke-secret-b44e02
clip_state=$work/clip-state
clip_config=$work/clip-config
mkdir -p "$clip_state" "$clip_config/celestina"
printf '{"schema_version":1,"duties":%s}\n' "$(duties clipboard)" > "$clip_config/celestina/shell.json"
# The PNG the wallpaper gate generated: a real, small, decodable picture.
clip_png=$fixture/default.png
[ -f "$clip_png" ] || fail "the fixture picture of the wallpaper gate is missing"
run=$work/clipboard
clip=$work/clipboard-run
{
    printf '#!/bin/sh\n'
    printf "BIN='%s'\nRUN='%s'\nXDG_CONFIG_HOME='%s'\nXDG_STATE_HOME='%s'\n" \
        "$bin" "$run" "$clip_config" "$clip_state"
    printf "HOST_DISPLAY='%s'\nPNG='%s'\nALPHA='%s'\nBETA='%s'\n" \
        "${WAYLAND_DISPLAY:-}" "$clip_png" "$clip_alpha" "$clip_beta"
    # The two files the driver writes: the duty off and on again, every other
    # duty off throughout.
    printf "CLIP_OFF='%s'\nCLIP_ON='%s'\n" \
        "{\"schema_version\":1,\"duties\":$(duties)}" \
        "{\"schema_version\":1,\"duties\":$(duties clipboard)}"
    cat <<'WRAPPER'
# Runs the shell with the clipboard duty on and drives it with wl-copy and the
# shell's own verbs, writing what it saw to $RUN.clip; exits with the shell's
# status.
export XDG_CONFIG_HOME XDG_STATE_HOME
if [ -z "${NIRI_SOCKET:-}" ] || [ -z "${WAYLAND_DISPLAY:-}" ] ||
    [ "$WAYLAND_DISPLAY" = "$HOST_DISPLAY" ]; then
    echo "smoke-clipboard-unsafe-display" >&2
    exit 1
fi
STATE_FILE=$XDG_STATE_HOME/celestina/clipboard.json
pids=
RUST_LOG=debug "$BIN" "$@" &
shell=$!
count() { grep -c -e "$1" "$RUN.err" || true; }
# wait_count PATTERN N: until PATTERN has N lines in the journal (20 s).
wait_count() {
    n=0
    while [ "$(count "$1")" -lt "$2" ]; do
        n=$((n + 1))
        [ "$n" -lt 200 ] || { echo "smoke-clipboard-timeout $1 $2" >&2; return 1; }
        sleep 0.1
    done
}
note() { printf '%s\n' "$*" >> "$RUN.clip"; }
: > "$RUN.clip"
wait_count 'clipboard duty on' 1 || exit 1
sleep 1

# 1. A text selection grows the history to one.
WAYLAND_DEBUG=1 wl-copy -f -- "$ALPHA" 2> "$RUN.wldebug.text" &
pids="$pids $!"
wait_count 'the history changed.*total=1' 1 || exit 1

# 2. A selection carrying the password-manager hint adds nothing.
WAYLAND_DEBUG=1 wl-copy -f --sensitive -- "$BETA" 2> "$RUN.wldebug.hint" &
pids="$pids $!"
wait_count 'carries an exclusion hint' 1 || exit 1
sleep 1

# 3. A PNG selection is accepted.
wl-copy -f --type image/png < "$PNG" &
pids="$pids $!"
wait_count 'the history changed.*total=2' 1 || exit 1

# 4. The state file: mode, owner directory mode, content.
note "file-mode=$(stat -c %a "$STATE_FILE" 2>/dev/null)"
note "dir-mode=$(stat -c %a "$XDG_STATE_HOME/celestina" 2>/dev/null)"
note "dir-entries=$(ls -A "$XDG_STATE_HOME/celestina" 2>/dev/null | wc -l)"
grep -q -F -e "$ALPHA" "$STATE_FILE" && note "text-in-file=yes" || note "text-in-file=no"
grep -q -F -e "$BETA" "$STATE_FILE" && note "hinted-in-file=yes" || note "hinted-in-file=no"
if grep -q -a -e iVBOR -e PNG -e image/png "$STATE_FILE"; then
    note "image-in-file=yes"
else
    note "image-in-file=no"
fi

# 5. The shell re-selects an entry: no duplicate, and what comes back is
#    byte-identical. The order is [image, text]; index 1 is the text.
"$BIN" clipboard select 1 || note "verb-failed select 1"
wait_count 'clipboard select: requested' 1 || exit 1
sleep 1
if [ "$(timeout 5 wl-paste --no-newline 2>/dev/null)" = "$ALPHA" ]; then
    note "paste-text=match"
else
    note "paste-text=mismatch"
fi
# Now the order is [text, image]; index 1 is the image.
"$BIN" clipboard select 1 || note "verb-failed select 1 (image)"
wait_count 'clipboard select: requested' 2 || exit 1
sleep 1
timeout 5 wl-paste --type image/png > "$RUN.paste.png" 2>/dev/null || true
if cmp -s "$RUN.paste.png" "$PNG"; then
    note "paste-image=match"
else
    note "paste-image=mismatch"
fi
rm -f "$RUN.paste.png"
sleep 1

# 6. Switching the duty off in shell.json deletes the file and empties memory.
printf '%s\n' "$CLIP_OFF" > "$XDG_CONFIG_HOME/celestina/shell.json.tmp"
mv "$XDG_CONFIG_HOME/celestina/shell.json.tmp" "$XDG_CONFIG_HOME/celestina/shell.json"
wait_count 'clipboard duty off' 1 || exit 1
# The reaper deletes after the duty reports off: wait for its own word.
wait_count 'clipboard: the history file was deleted' 1 || exit 1
if [ -e "$STATE_FILE" ]; then note "file-after-off=present"; else note "file-after-off=absent"; fi
"$BIN" clipboard select 0 || note "verb-failed select 0 (off)"
wait_count 'the duty is off' 1 || exit 1

# 7. Switching it on again starts empty.
printf '%s\n' "$CLIP_ON" > "$XDG_CONFIG_HOME/celestina/shell.json.tmp"
mv "$XDG_CONFIG_HOME/celestina/shell.json.tmp" "$XDG_CONFIG_HOME/celestina/shell.json"
wait_count 'clipboard duty on' 2 || exit 1
sleep 1
if [ -e "$STATE_FILE" ]; then note "file-after-on=present"; else note "file-after-on=absent"; fi

for p in $pids; do kill "$p" 2>/dev/null || true; done
wait "$shell"
WRAPPER
} > "$clip"
chmod +x "$clip"
status=0
# The shell lives 30 s (`--exit-after 30`) plus the nest's start: it starts
# early in the minute so the clock's minute tick, a frame at rest of its own,
# cannot fall inside the run and be counted against the duty's allowance.
while [ "$(date +%S)" -gt 22 ] || [ "$(date +%S)" -lt 2 ]; do
    sleep 1
done
nest 90 "$here/dev-session.sh" --own-bus --monitor none --binary "$clip" \
    --once "$run" -- --exit-after 30 --metrics \
    > "$work/niri.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || fail "the nested niri exited with $status (clipboard run)"
[ -f "$run.status" ] || fail "the shell never ran inside the nest (clipboard run)"
shell_status=$(cat "$run.status")
[ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (clipboard run)"
if grep -q 'smoke-clipboard-' "$run.err"; then
    fail "the clipboard driver failed: $(grep 'smoke-clipboard-' "$run.err" | head -n 3)"
fi
if grep -q 'panicked' "$run.err"; then
    fail "the shell panicked with the clipboard duty on"
fi
if grep -q 'verb-failed' "$run.clip"; then
    fail "a clipboard verb did not reach the shell: $(grep verb-failed "$run.clip")"
fi
# wl-copy must have spoken ext-data-control, and the hinted copy must have
# offered the hint, or the rest of the gate proves nothing.
grep -q 'ext_data_control_source_v1' "$run.wldebug.text" ||
    fail "wl-copy did not speak ext-data-control to the nest"
grep -q 'x-kde-passwordManagerHint' "$run.wldebug.hint" ||
    fail "wl-copy --sensitive did not offer x-kde-passwordManagerHint"
want() {
    grep -qx "$1" "$run.clip" || fail "clipboard: expected '$1' in: $(tr '\n' ' ' < "$run.clip")"
}
want file-mode=600
want dir-mode=700
want dir-entries=1
want text-in-file=yes
want hinted-in-file=no
want image-in-file=no
want paste-text=match
want paste-image=match
want file-after-off=absent
want file-after-on=absent
# The history: one text, then the hinted copy added nothing, then the image,
# and the shell's own re-selections (twice) added no entry.
changes=$(grep 'the history changed' "$run.err" | sed -n 's/.*total=\([0-9]*\).*/\1/p' | tr '\n' ' ')
case "$changes" in
    *'3 '*) fail "the history grew past two entries ($changes)" ;;
esac
grep -q 'the history changed.*total=1' "$run.err" || fail "a text selection did not grow the history"
grep -q 'the history changed.*total=2' "$run.err" || fail "an image selection was not accepted"
[ "$(grep -c 'skipped a selection that carries an exclusion hint' "$run.err")" -eq 1 ] ||
    fail "the hinted selection was not skipped exactly once"
[ "$(grep -c 'skipped the echo of its own selection' "$run.err")" -ge 2 ] ||
    fail "the shell's own re-selections were not recognised as echoes"
grep -q 'clipboard duty off: memory emptied' "$run.err" ||
    fail "switching the duty off did not report that memory was emptied"
grep -q 'clipboard: the history file was deleted' "$run.err" ||
    fail "switching the duty off did not report the deletion of the history file"
# After an off the worker starts without reading the history file (`load=false`),
# and the first start of the run did read it; with the file absent below, the
# history starts empty.
grep 'clipboard duty on' "$run.err" | tail -n 1 | grep -q 'load=false' ||
    fail "switching the duty on again did not start without loading a history"
grep 'clipboard duty on' "$run.err" | head -n 1 | grep -q 'load=true' ||
    fail "the first start did not load the history"
grep -q 'the duty is off' "$run.err" || fail "a verb with the duty off was not refused"
# Privacy: the journal, the metrics, the nest's log and the driver's notes
# hold none of the fixed strings, at debug level for the whole shell.
for f in "$run.err" "$run.out" "$work/niri.log" "$run.clip"; do
    if grep -q -F -e "$clip_alpha" -e "$clip_beta" "$f"; then
        fail "a clipboard test string reached $(basename "$f")"
    fi
done
clip_lines=$(grep '^winit ' "$run.out" || true)
[ -n "$clip_lines" ] || fail "no metrics line for the winit output (clipboard run)"
# With no page open, a history change redraws nothing: the bar's clipboard
# piece draws no preview or count, the rows are the Clipboard page's and only
# an Island showing that page redraws for them. What redraws every surface
# is the duty itself being switched in
# `shell.json` (the bar's piece appears or goes even over an empty history):
# the allowance is one frame at rest per switch the journal reports (`off` once
# and `on` again after the start), so any other frame at rest (a history change,
# a tick, a loop) still fails.
clip_rest=$(printf '%s\n' "$clip_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
    awk '{ n += $1 } END { print n + 0 }')
clip_events=$(grep -c 'the history changed' "$run.err" || true)
clip_on=$(grep -c 'clipboard duty on' "$run.err" || true)
clip_off=$(grep -c 'clipboard duty off' "$run.err" || true)
clip_switches=$((clip_off + clip_on - 1))
[ "$clip_rest" -le "$clip_switches" ] ||
    rest_fail "$run.err" "frames were drawn at rest beyond one per duty switch ($clip_rest, at most $clip_switches; $clip_events history changes): $clip_lines"
echo "smoke: clipboard: history 0 -> 1 -> 2 (hinted copy skipped, image accepted, re-selection not duplicated); state file mode 600, no image, deleted when the duty went off, empty when it came back; journal free of the test strings (changes: $changes)"
echo "smoke: clipboard: $(printf '%s\n' "$clip_lines" | head -n 1) (rest_frames=$clip_rest, duty switches=$clip_switches, history changes=$clip_events)"
grep '^rss_mib=' "$run.out" | sed 's/^/smoke: clipboard: /' || true
