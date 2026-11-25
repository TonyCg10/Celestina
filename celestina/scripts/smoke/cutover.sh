#!/bin/sh
set -eu

# Smoke phase `cutover` (CEL-2-M): `cutover_run` starts the shell with the
# design's §5.1 file: every duty on, the lock included, the wallpaper folder at
# the fixture picture (`make_wallpaper_fixture`), the bar's three zones, the
# fake `wpctl`/`pactl`/`ddcutil`/`brightnessctl` first on PATH
# (`make_osd_fakes`), a Melibea socket path that does not exist, a throwaway
# state home and the dead system bus. It must exit 0 with one
# `celestina-panel` surface per output in the Top layer and one
# `celestina-wallpaper` per output in Background, the idle worker armed or
# unavailable, the night light applied or not, caffeine not held, the lock
# duty on with its sleep inhibitor refused by the dead bus (and its warning
# that the PAM service is missing, on a host without it), the polkit duty on
# with its agent unavailable over the dead bus and nothing registered (as the
# polkit phase checks), the OSD workers started with no `celestina-osd`
# surface, Melibea absent once, Magnetita absent, the clipboard loaded, and
# frames at rest within the budgets documented at the run: the wallpaper's
# surface by itself, and every other surface before the switch below and
# after it. Then, with the shell still running, the file is rewritten with
# every duty off: the journal must report each duty off, the
# `celestina-wallpaper` surface must be gone while the panel's surface is the
# same one, and the clipboard's history file must be deleted. Since the
# cutover a `shell.json` that chooses nothing has every duty on, so every
# other run writes the duties it wants off (`duties` in `smoke/common.sh`).
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only cutover`.
SMOKE_PHASE=cutover
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

need_python3
make_wallpaper_fixture
make_osd_fakes
# The cutover (CEL-2-M, design §5.1 and §8): the shell started with the file
# the author's session will run, every duty on, the lock and the polkit agent
# included (both on by default since CEL-2-M-LOCK-POLKIT, and named on here
# all the same; here the agent starts
# over the dead system bus, so the agent must report itself unavailable and
# register nothing, as the polkit phase checks, and the switch below
# must turn it off), the wallpaper folder at the fixture picture the wallpaper run
# generated, the bar's three zones with the pieces this tree has, the OSD
# run's fakes first on PATH (`CELESTINA_DDC=0`, a calls file of this run's
# own), MELIBEA_SOCKET at a path that does not exist (the shell then never
# looks in the real runtime directory, as the phone absent run shows), a
# throwaway state home and the dead system bus, all set in the wrapper
# itself. The wallpaper run's probe lists the nest's layers once the wallpaper
# surface is up: that is the shell at rest. The wrapper waits for the picture,
# rests nine seconds, rewrites the file with every duty off (`bar` kept on: the
# panel's one wide surface is not what the switch tests), waits for the
# wallpaper duty to report off and lists the layers again itself.
# Frames at rest: the runtime writes `metrics: rest frame` with the surface
# for each one it counts, so the journal says which surface drew it and on
# which side of the wrapper's `smoke-cutover-rewrite` line; the three counts
# below must add up to the metrics' own total. One budget over the whole run
# (thirteen, the sum of every earlier run's allowance) let a shell that
# redrew every two seconds pass. Now each surface is held to what it can owe,
# over a time long enough (eight seconds or more) for such a shell to draw
# four.
# The wallpaper's surface, whenever it draws (none in fourteen runs without a
# resize): the wallpaper run's two rules. Two frames for learning its real
# scale after its first frame, and two for each change of the nest's output
# between `wallpaper surface created` and `wallpaper duty off`, which is the
# host resizing the nest's window (measured here too: two per resize). They
# are counted by surface and not by side because the surface outlives the
# wrapper's line by the 300 ms the shell waits before it follows a rewritten
# file, and a resize there would land its frames after the line.
# Every other surface before the switch, every duty on (none in each of the
# fourteen runs, nine resting five seconds and five resting nine): one frame
# for the nest's late first events repainting the bar (the bar run's rule),
# and one for the system monitor's held-back copy described next, which falls
# on this side when the picture, and so the switch, is three seconds late.
# The system monitor's first reading and the pieces arriving animate, so
# they are not rest frames.
# Every other surface after the switch, every duty off: one for the switch
# itself, a single rewrite of the file, should no piece animate as it goes
# (the clipboard run's rule: one per duty switch; here the pieces going
# animate and it has not been seen). The system monitor's held-back copy,
# due ten seconds after its first reading (twelve seconds into the run), is
# taken back when the switch withdraws the reading, so no frame is drawn for
# it. A resize after the wallpaper is gone costs no rest frame (measured over
# three): the panel animates to its new width.
# Only when the lock warned that the PAM service is missing, its desktop
# notice, which the shell's own server shows and expires after ten seconds,
# adds one frame on each side: its posting when that falls after the panel's
# first second, and its expiry, a second after the switch or, when the switch
# is late, before it. A panel that redrew every two seconds would still draw
# four on either side, more than either side is allowed.
cutover_run() {
    cutover_home=$work/xdg-cutover
    cutover_state=$work/cutover-state
    cutover_osd=$work/cutover-osd-state
    mkdir -p "$cutover_home/celestina" "$cutover_state" "$cutover_osd"
    echo 0.40 > "$cutover_osd/volume"
    : > "$cutover_osd/calls"
    cat > "$cutover_home/celestina/shell.json" <<JSON
{
  "schema_version": 1,
  "do_not_disturb": false,
  "duties": {
    "idle": "on", "caffeine": "on", "nightlight": "on",
    "wallpaper": "on", "osd": "on", "media": "on", "clipboard": "on",
    "lock": "on", "polkit": "on", "phone": "on", "bubbles": "on"
  },
  "energy": { "caffeine": true, "night_light": true },
  "wallpaper": { "folder": "$fixture", "mode": "fill" },
  "bar": {
    "start": ["workspaces", "phone", "notifications"],
    "centre": ["weather", "clock"],
    "end": ["bubbles", "sysmon", "tray", "network", "bluetooth",
            "volume", "brightness", "clipboard", "media", "battery",
            "power-profile"],
    "tray_hidden": ["nm-applet", "blueman", "blueman-tray"]
  }
}
JSON
    # The off file names every duty off; the bar is drawn whatever the file
    # says, so the surface under test stays the panel's one wide surface.
    cutover_off="{\"schema_version\":1,\"duties\":$(duties)}"
    run=$work/cutover
    cutover_wrapper=$work/cutover-run
    cat > "$cutover_wrapper" <<WRAPPER
#!/bin/sh
# Runs the shell with the cutover file, rests, switches every duty off in the
# file while the shell runs, lists the nest's layers and exits with the
# shell's status. Its notes go to the run's stderr, in order with the shell's.
XDG_CONFIG_HOME='$cutover_home'
XDG_STATE_HOME='$cutover_state'
DBUS_SYSTEM_BUS_ADDRESS='unix:path=$work/no-system-bus'
MELIBEA_SOCKET='$work/cutover-absent-melibea.sock'
CELESTINA_SMOKE_FAKE='$cutover_osd'
CELESTINA_DDC=0
PATH='$osd_fakes':\$PATH
RUST_LOG=info,celestina_wayland::runtime=debug,celestina_wayland::handlers=debug
export XDG_CONFIG_HOME XDG_STATE_HOME DBUS_SYSTEM_BUS_ADDRESS MELIBEA_SOCKET
export CELESTINA_SMOKE_FAKE CELESTINA_DDC PATH RUST_LOG
"$bin" "\$@" &
shell=\$!
# wait_for PATTERN: until the journal holds it (20 s).
wait_for() {
    n=0
    until grep -q "\$1" '$run.err'; do
        n=\$((n + 1))
        [ "\$n" -lt 200 ] || { echo "smoke-cutover-failed timeout \$1" >&2; return 1; }
        sleep 0.1
    done
}
wait_for 'wallpaper uploaded' || exit 1
sleep 9
echo "smoke-cutover-rewrite" >&2
printf '%s\n' '$cutover_off' > "\$XDG_CONFIG_HOME/celestina/shell.json.tmp"
mv "\$XDG_CONFIG_HOME/celestina/shell.json.tmp" "\$XDG_CONFIG_HOME/celestina/shell.json"
wait_for 'wallpaper duty off' || exit 1
wait_for 'clipboard: the history file was deleted' || exit 1
sleep 2
kill -0 "\$shell" 2>/dev/null || echo "smoke-cutover-failed the shell exited before the layers after the switch were listed" >&2
'$niri_binary' msg --json layers > '$run.layers' 2>/dev/null || echo "smoke-cutover-failed layers" >&2
if [ -e '$cutover_state/celestina/clipboard.json' ]; then
    echo "smoke-cutover-failed history-present" >&2
fi
wait "\$shell"
WRAPPER
    chmod +x "$cutover_wrapper"
    # Eighteen seconds plus the nest's start, clear of the minute boundary
    # (the clock's tick is a frame at rest of its own).
    while [ "$(date +%S)" -gt 26 ] || [ "$(date +%S)" -lt 2 ]; do
        sleep 1
    done
    status=0
    nest 90 "$here/dev-session.sh" --own-bus --monitor none --binary "$cutover_wrapper" \
        --probe "$here/wallpaper-layers-probe.sh" --once "$run" -- --exit-after 18 --metrics \
        > "$work/niri.log" 2>&1 || status=$?
    [ "$status" -eq 0 ] || fail "the nested niri exited with $status (cutover run)"
    [ -f "$run.status" ] || fail "the shell never ran inside the nest (cutover run)"
    shell_status=$(cat "$run.status")
    [ "$shell_status" = 0 ] || fail "the shell exited with $shell_status (cutover run)"
    [ "$(cat "$run.probe.status" 2>/dev/null)" = 0 ] || fail "the layers probe failed (cutover run)"
    if grep -q 'panicked' "$run.err"; then
        fail "the shell panicked with every duty on (cutover run)"
    fi
    if grep -q 'smoke-cutover-failed' "$run.err"; then
        fail "the cutover driver failed: $(grep 'smoke-cutover-failed' "$run.err" | head -n 3)"
    fi
    grep -q 'smoke-cutover-rewrite' "$run.err" || fail "the cutover driver never rewrote the file"
    # The layers: at rest, one panel in Top and one wallpaper in Background on
    # every output, no OSD; after the switch, the wallpaper gone and the panel
    # still there.
    [ -s "$run.layers" ] || fail "the layers after the switch were not listed (cutover run)"
    python3 - "$run.probe" "$run.layers" <<'LAYERS' || fail "the layers are not the cutover's (cutover run)"
import json, sys

lines = open(sys.argv[1]).read().split("\n")
layers = json.loads(lines[lines.index("probe: layers (json)") + 1])
outputs = json.loads(lines[lines.index("probe: outputs (json)") + 1])
names = sorted(outputs) if isinstance(outputs, dict) else sorted(o["name"] for o in outputs)
after = json.loads(open(sys.argv[2]).read())

def count(listed, namespace, layer, output):
    return sum(1 for l in listed
               if l["namespace"] == namespace and l["layer"] == layer and l["output"] == output)

problems = []
if not names:
    problems.append("no output listed")
for output in names:
    panels = count(layers, "celestina-panel", "Top", output)
    if panels != 1:
        problems.append(f"{output}: {panels} celestina-panel surface(s) in Top at rest, not one")
    papers = count(layers, "celestina-wallpaper", "Background", output)
    if papers != 1:
        problems.append(f"{output}: {papers} celestina-wallpaper surface(s) in Background at rest, not one")
    panels_after = count(after, "celestina-panel", "Top", output)
    if panels_after != 1:
        problems.append(f"{output}: {panels_after} celestina-panel surface(s) in Top after the switch, not one")
if any(l["namespace"] == "celestina-osd" for l in layers):
    problems.append("a celestina-osd surface existed at rest")
if any(l["namespace"] == "celestina-wallpaper" for l in after):
    problems.append("a celestina-wallpaper surface is still listed after the switch")
if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"smoke: cutover: {len(names)} output(s); at rest one celestina-panel (Top) and one celestina-wallpaper (Background) each, no celestina-osd; after the switch the wallpaper is gone and the panel stays")
LAYERS
    # The panel's surface is the same one before and after the switch: the
    # runtime created exactly one per output in the whole run.
    cutover_outputs=$(grep -c 'output added output=' "$run.err" || true)
    cutover_panels=$(grep -c 'layer surface created.*namespace="celestina-panel"' "$run.err" || true)
    [ "$cutover_outputs" -ge 1 ] || fail "the nest added no output (cutover run)"
    [ "$cutover_panels" -eq "$cutover_outputs" ] ||
        fail "the runtime created $cutover_panels celestina-panel surface(s) for $cutover_outputs output(s) (cutover run)"
    if grep -q 'osd surface created' "$run.err"; then
        fail "an OSD surface appeared with nothing to show (cutover run)"
    fi
    # Every duty's own word at start.
    for want in 'idle duty on' 'night-light duty on' 'lock: the duty is on' 'system monitor on' \
        'clipboard duty on' 'osd duty on: workers started' 'phone duty on' 'bubbles duty on' \
        'polkit: the duty is on' 'polkit: the agent is unavailable' \
        'magnetita: absent: nobody owns' 'wallpaper surface created' 'wallpaper uploaded'; do
        grep -q "$want" "$run.err" || fail "the journal does not say '$want' (cutover run)"
    done
    # The polkit agent over the dead bus: unavailable, and never registered
    # or answered by a polkitd (the polkit run's own check).
    if grep -Eq 'polkit: registration (Registered|Refused)' "$run.err"; then
        fail "the polkit agent registered or was answered: a system bus other than the dead one was reached (cutover run)"
    fi
    grep -Eq 'idle: (armed|unavailable)' "$run.err" ||
        fail "the idle worker reported neither armed nor unavailable (cutover run)"
    grep -Eq 'night light: (applied|not applied)' "$run.err" ||
        fail "the night light reported neither applied nor not applied (cutover run)"
    if grep -q 'caffeine: held' "$run.err"; then
        fail "caffeine was held: a logind other than the nest's was reached (cutover run)"
    fi
    for want in 'network: unavailable' 'bluetooth: unavailable' 'power: battery unavailable' 'power: profiles unavailable'; do
        grep -q "$want" "$run.err" || fail "the journal does not say '$want' (cutover run)"
    done
    cutover_absent=$(grep -c 'melibea: absent' "$run.err" || true)
    [ "$cutover_absent" = 1 ] ||
        fail "the missing Melibea socket was logged absent $cutover_absent times, not once (cutover run)"
    # The lock: on, pinned to the binaries beside the shell, and its logind
    # worker reaching only the dead socket (availability is the duty and the
    # pinned binaries, `celestina_shell::lock`; the bus only costs it the
    # sleep inhibitor, which it says). The PAM service file is the host's
    # and this script never looks at it (`tests/lock_scripts.rs`): a host
    # without it makes the duty warn exactly once, a host with it never, and
    # the journal alone says which host this is.
    grep -q 'lock: the sleep inhibitor was not taken: Bus' "$run.err" ||
        fail "the lock's sleep inhibitor was not refused by the dead system bus (cutover run)"
    cutover_pam=$(grep -c 'the PAM service celestina-lock is not installed' "$run.err" || true)
    cutover_notice=0
    case "$cutover_pam" in
        0) cutover_pam_note="the PAM service is installed on this host and the lock did not warn" ;;
        1)
            cutover_notice=1
            cutover_pam_note="the lock warned once that the PAM service is missing"
            ;;
        *) fail "the lock warned $cutover_pam times that the PAM service is missing, not once (cutover run)" ;;
    esac
    if grep -q 'the lock process' "$run.err"; then
        fail "a lock child was started with no lock verb sent (cutover run)"
    fi
    # The OSD workers read the fakes and nothing else.
    grep -q '^pactl subscribe$' "$cutover_osd/calls" || fail "pactl subscribe was never started (cutover run)"
    if grep -Eq '^ddcutil|set-volume' "$cutover_osd/calls"; then
        fail "a fake tool was asked to change something with no verb sent (cutover run): $(cat "$cutover_osd/calls")"
    fi
    # The switch: every duty reports off, in the journal after the rewrite.
    cutover_switches=0
    for duty in wallpaper idle night-light clipboard osd phone bubbles; do
        awk '/smoke-cutover-rewrite/ { on = 1; next } on' "$run.err" | grep -q "$duty duty off" ||
            fail "the journal does not report '$duty duty off' after the rewrite (cutover run)"
        cutover_switches=$((cutover_switches + 1))
    done
    # The polkit duty says it in its own words (`polkit/duty.rs`).
    awk '/smoke-cutover-rewrite/ { on = 1; next } on' "$run.err" | grep -q 'polkit: the duty is off' ||
        fail "the journal does not report 'polkit: the duty is off' after the rewrite (cutover run)"
    cutover_switches=$((cutover_switches + 1))
    grep -q 'system monitor off' "$run.err" || fail "the system monitor did not stop with the bar's pieces gone (cutover run)"
    grep -q 'shell settings changed on disk' "$run.err" || fail "the shell did not follow the rewritten file (cutover run)"
    # Frames: every surface drew, and the rest within the two budgets above.
    lines=$(grep '^winit ' "$run.out" || true)
    [ -n "$lines" ] || fail "no metrics line for the winit output (cutover run)"
    printf '%s\n' "$lines" | while IFS= read -r l; do
        n=$(printf '%s\n' "$l" | tr ' ' '\n' | sed -n 's/^frames=//p')
        [ "${n:-0}" -ge 1 ] || exit 1
    done || fail "a surface drew no frame with every duty on: $lines"
    cutover_rest=$(sum rest_frames)
    # "PAPER BEFORE AFTER": the wallpaper surfaces' rest frames, and every
    # other surface's on each side of the wrapper's line.
    cutover_counts=$(awk '
        /layer surface created/ && /namespace="celestina-wallpaper"/ {
            match($0, /surface=[0-9]+/)
            paper[substr($0, RSTART, RLENGTH)] = 1
        }
        /smoke-cutover-rewrite/ { after = 1 }
        /metrics: rest frame/ {
            match($0, /surface=[0-9]+/)
            if (substr($0, RSTART, RLENGTH) in paper) p++
            else if (after) a++
            else b++
        }
        END { print p + 0, b + 0, a + 0 }' "$run.err")
    cutover_paper=${cutover_counts%% *}
    cutover_after=${cutover_counts##* }
    cutover_before=${cutover_counts#* }
    cutover_before=${cutover_before% *}
    [ $((cutover_paper + cutover_before + cutover_after)) -eq "$cutover_rest" ] ||
        fail "the journal names $cutover_counts rest frames (wallpaper, before, after) and the metrics count $cutover_rest (cutover run): $lines"
    cutover_changes=$(output_changes "$run.err" 'wallpaper surface created' 'wallpaper duty off')
    cutover_paper_allowed=$((2 + 2 * cutover_changes))
    cutover_before_allowed=$((1 + 1 + cutover_notice))
    cutover_after_allowed=$((1 + cutover_notice))
    [ "$cutover_paper" -le "$cutover_paper_allowed" ] ||
        rest_fail "$run.err" "the wallpaper drew frames at rest with every duty on ($cutover_paper, at most $cutover_paper_allowed: 2 for the real scale, 2 for each of $cutover_changes output changes): $lines"
    [ "$cutover_before" -le "$cutover_before_allowed" ] ||
        rest_fail "$run.err" "frames were drawn at rest with every duty on, before the switch ($cutover_before, at most $cutover_before_allowed: bar 1, a late switch 1, notice $cutover_notice): $lines"
    [ "$cutover_after" -le "$cutover_after_allowed" ] ||
        rest_fail "$run.err" "frames were drawn at rest after every duty went off ($cutover_after, at most $cutover_after_allowed: the switch 1, notice $cutover_notice): $lines"
    echo "smoke: cutover: every duty on then off: $(printf '%s\n' "$lines" | head -n 1) (rest_frames=$cutover_rest: the wallpaper $cutover_paper, at most $cutover_paper_allowed with $cutover_changes output changes; the rest $cutover_before before the switch, at most $cutover_before_allowed, and $cutover_after after it, at most $cutover_after_allowed; over $(printf '%s\n' "$lines" | wc -l) surfaces)"
    echo "smoke: cutover: $(grep -Eo 'idle: (armed|unavailable)' "$run.err" | head -n 1), $(grep -Eo 'night light: (applied|not applied)' "$run.err" | head -n 1), caffeine not held, lock on with the inhibitor refused by the dead bus ($cutover_pam_note), polkit on with the agent unavailable over the dead bus and nothing registered, Melibea absent once, Magnetita absent, $cutover_switches duty switches reported off and the history file deleted"
    grep '^rss_mib=' "$run.out" | sed 's/^/smoke: cutover: /' || true
}
cutover_run
