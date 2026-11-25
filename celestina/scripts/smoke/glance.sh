#!/bin/sh
set -eu

# Smoke phase `glance`: two nested runs (CEL-2-K) put `sysmon`, `clock` and
# `tray` (and, in the second, `weather` with a Dublin location) in the bar of
# a private XDG_CONFIG_HOME, with a fake `curl` first on PATH that logs its
# argv and never reaches the network: the first, with no location, must make
# no request at all; the second must make the one documented forecast
# request.
#
# Run it through the driver: `scripts/smoke.sh --binary PATH --only glance`.
SMOKE_PHASE=glance
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"

# The glance workers (CEL-2-K): the system monitor and the weather. Each run
# starts the shell through a wrapper that puts a fake `curl` first on PATH (a
# directory of the run's own), so the real curl is never reachable from the
# shell: the fake appends its arguments, one per line, to a log and answers
# from the file `curl-body` when there is one (exit 0) or fails (exit 1). The
# wrapper also records which curl PATH resolves to, and the run fails unless
# it is the fake. The bar holds `sysmon`, `clock` and `tray`: without a
# location nothing may ask for the weather; with one, the one request is the
# documented forecast URL, fetched with `--disable` and carrying neither the
# label nor anything else the person typed.
glance_run() {
    # $1 = run name, $2 = shell.json body, $3 = canned curl body or empty
    g_name=$1
    g_home=$work/xdg-$g_name
    g_bin=$work/fakebin-$g_name
    g_log=$work/$g_name.curl-log
    mkdir -p "$g_home/celestina" "$g_bin"
    printf '%s\n' "$2" > "$g_home/celestina/shell.json"
    cat > "$g_bin/curl" <<FAKE
#!/bin/sh
for a in "\$@"; do printf '%s\n' "\$a" >> '$g_log'; done
printf -- '--call--\n' >> '$g_log'
if [ -f '$g_bin/curl-body' ]; then cat '$g_bin/curl-body'; exit 0; fi
exit 1
FAKE
    chmod +x "$g_bin/curl"
    [ -z "$3" ] || printf '%s' "$3" > "$g_bin/curl-body"
    cat > "$work/$g_name-run" <<WRAPPER
#!/bin/sh
XDG_CONFIG_HOME='$g_home'
PATH='$g_bin':\$PATH
export XDG_CONFIG_HOME PATH
echo "smoke-curl=\$(command -v curl)" >&2
RUST_LOG=info,celestina_wayland::runtime=debug exec "$bin" "\$@"
WRAPPER
    chmod +x "$work/$g_name-run"
    run=$work/$g_name
    status=0
    while [ "$(date +%S)" -gt 40 ] || [ "$(date +%S)" -lt 2 ]; do
        sleep 1
    done
    nest 90 "$here/dev-session.sh" --own-bus --monitor none --binary "$work/$g_name-run" \
        --once "$run" -- --exit-after 14 --metrics \
        > "$work/niri.log" 2>&1 || status=$?
    [ "$status" -eq 0 ] || fail "the nested niri exited with $status ($g_name run)"
    [ -f "$run.status" ] || fail "the shell never ran inside the nest ($g_name run)"
    shell_status=$(cat "$run.status")
    [ "$shell_status" = 0 ] || fail "the shell exited with $shell_status ($g_name run)"
    grep -qx "smoke-curl=$g_bin/curl" "$run.err" ||
        fail "curl did not resolve to the fake one ($g_name run): $(grep 'smoke-curl=' "$run.err")"
    if grep -q 'panicked' "$run.err"; then
        fail "the shell panicked ($g_name run)"
    fi
    grep -q 'system monitor on' "$run.err" ||
        fail "the system monitor worker did not start ($g_name run)"
    g_lines=$(grep '^winit ' "$run.out" || true)
    [ -n "$g_lines" ] || fail "no metrics line for the winit output ($g_name run)"
    g_frames=$(printf '%s\n' "$g_lines" | tr ' ' '\n' | sed -n 's/^frames=//p' |
        awk '{ n += $1 } END { print n + 0 }')
    g_rest=$(printf '%s\n' "$g_lines" | tr ' ' '\n' | sed -n 's/^rest_frames=//p' |
        awk '{ n += $1 } END { print n + 0 }')
    [ "$g_frames" -ge 1 ] || fail "no frame was drawn ($g_name run): $g_lines"
    # The rest budget is what the design owes a placed system monitor: one
    # frame when its first reading appears and at most one more when the
    # ten-second cadence copies a changed reading to the bar. The run lasts
    # 14 s, so that second frame lands near the exit and either side of it is
    # a legitimate outcome; a bar that redrew on every two-second sample would
    # draw about six.
    [ "$g_rest" -le 2 ] || rest_fail "$run.err" "frames were drawn at rest ($g_name run, $g_rest, at most 2): $g_lines"
    g_calls=0
    [ ! -f "$g_log" ] || g_calls=$(grep -c '^--call--$' "$g_log" || true)
    echo "smoke: $g_name: frames=$g_frames rest_frames=$g_rest curl_calls=$g_calls"
    grep '^rss_mib=' "$run.out" | sed "s/^/smoke: $g_name: /" || true
}
glance_run glance '{"schema_version":1,"duties":'"$(duties)"',"bar":{"end":["sysmon","clock","tray"]}}' ''
[ ! -s "$work/glance.curl-log" ] ||
    fail "the shell ran curl with no location set: $(tr '\n' ' ' < "$work/glance.curl-log")"
echo "smoke: glance: no location, no request"

glance_run glance-weather '{"schema_version":1,"duties":'"$(duties)"',"bar":{"end":["weather","sysmon","clock","tray"]},"weather":{"latitude":53.35,"longitude":-6.26,"label":"Dublin"}}' '{"current":{"temperature_2m":12.4,"weather_code":3,"is_day":1}}'
wlog=$work/glance-weather.curl-log
[ -s "$wlog" ] || fail "the weather worker never ran curl (glance-weather run)"
wurl='https://api.open-meteo.com/v1/forecast?latitude=53.35&longitude=-6.26&current=temperature_2m,weather_code,is_day'
awk '/^--call--$/ { if (n) print last; n = 0; next } { n++; last = $0 } END { if (n) print last }' "$wlog" |
    while IFS= read -r l; do [ "$l" = "$wurl" ] || exit 1; done ||
    fail "a curl call did not end in the forecast URL (glance-weather run): $(tr '\n' ' ' < "$wlog")"
grep -qx -- '--disable' "$wlog" || fail "curl ran without --disable (glance-weather run)"
if grep -qi 'dublin' "$wlog"; then
    fail "the label reached curl's arguments (glance-weather run)"
fi
grep -q 'weather on' "$work/glance-weather.err" || fail "the weather worker did not start (glance-weather run)"
echo "smoke: glance-weather: $(grep -c '^--call--$' "$wlog") request(s), forecast URL, --disable, no label"
