#!/bin/sh
set -u

# Calcita smoke: the quick gate without a visible window.
#
#  1) The shared static check for `x: x` auto-bindings, where an injected
#     property shadows the id, resolves to itself and stays undefined. The
#     engine and qmllint both accept it, so it is caught by pattern.
#  2) An 8-second offscreen start in a scratch home with no session bus: the
#     binary must still be running when the timeout ends it (status 124) and
#     the QML runtime must report no errors, including the ones that mean an
#     object never constructed.
#  3) The window's own report (CALCITA_SMOKE_REPORT) after 3 s: the empty state
#     is shown and the appearance reached the theme.
#  4) The same start with tests/fixtures/three-pages.pdf on the command line:
#     a document window opens, QtPdf reads three pages and the empty window
#     has retired.
#  5) The same start with tests/fixtures/annotated.pdf: the window reports the
#     four annotations the model read on its worker, and the first page as
#     drawn (CALCITA_SMOKE_GRAB, a PPM) shows another program's highlight in
#     yellow and its ink in red where the file puts them, over paper where
#     it puts nothing: the page provider renders annotations.
#
# Startup only. Keyboard, focus and assistive technology need a real Wayland
# session.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/calcita
scanner=$root/../scripts/architecture_scanners.py
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary requires a path}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "usage: scripts/smoke.sh [--binary PATH]" >&2
    exit 2
fi

if ! autos=$(python3 "$scanner" qml-auto-bindings "$root/qml"); then
    echo "smoke: the auto-binding scanner could not complete" >&2
    exit 1
fi
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (the property shadows the id):" >&2
    echo "$autos" >&2
    exit 1
fi

if [ ! -x "$bin" ]; then
    echo "smoke: the binary is missing: $bin" >&2
    exit 1
fi

fixture=$root/tests/fixtures/three-pages.pdf
annotated=$root/tests/fixtures/annotated.pdf
for required in "$fixture" "$annotated"; do
    if [ ! -f "$required" ]; then
        echo "smoke: the fixture is missing: $required" >&2
        exit 1
    fi
done

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

# One 8-second offscreen start in a fresh scratch home, with the arguments
# given; the window's last report is left in $report.
run() {
    rm -rf "$scratch/home"
    mkdir -p "$scratch/home/config" "$scratch/home/data" "$scratch/home/cache" \
        "$scratch/home/state" "$scratch/home/run"
    chmod 0700 "$scratch/home/run"
    log=$scratch/output.log

    XDG_CONFIG_HOME=$scratch/home/config \
    XDG_DATA_HOME=$scratch/home/data \
    XDG_CACHE_HOME=$scratch/home/cache \
    XDG_STATE_HOME=$scratch/home/state \
    XDG_RUNTIME_DIR=$scratch/home/run \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/home/run/no-session-bus \
    CALCITA_FAKE=1 \
    CALCITA_SMOKE_REPORT=1 \
    CALCITA_SMOKE_GRAB=${grab:-} \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        timeout 8 "$bin" "$@" >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 124 ]; then
        echo "smoke: the binary exited on its own (rc=$rc); last lines:" >&2
        tail -20 "$log" >&2
        exit 1
    fi

    errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected|QQmlApplicationEngine failed|QML [A-Za-z]*: ' "$log" || true)
    if [ -n "$errors" ]; then
        echo "smoke: QML errors at startup:" >&2
        echo "$errors" | sort | uniq -c | sort -rn >&2
        exit 1
    fi

    report=$(grep -o 'calcita-smoke:.*' "$log" | tail -1)
}

run
case $report in
    *" empty=true pageCount=0 textScale=1 fontBody=13 annotations=-1") ;;
    *)
        echo "smoke: the window did not report the empty state: ${report:-no report}" >&2
        exit 1
        ;;
esac
empty_report=$report

run "$fixture"
case $report in
    *" empty=false pageCount=3 textScale=1 fontBody=13 annotations=0") ;;
    *)
        echo "smoke: the fixture did not open with three pages: ${report:-no report}" >&2
        exit 1
        ;;
esac
pages_report=$report

grab=$scratch/annotated-page.ppm
run "$annotated"
case $report in
    *" empty=false pageCount=1 textScale=1 fontBody=13 annotations=4") ;;
    *)
        echo "smoke: the annotated fixture did not report its four annotations: ${report:-no report}" >&2
        exit 1
        ;;
esac
if [ ! -s "$grab" ]; then
    echo "smoke: the annotated page was not grabbed: $grab" >&2
    exit 1
fi
# The page is A4 (595 x 842 points) at whatever scale the window fitted it
# to; each region is the annotation's /Rect in the fixture, inset by a tenth.
if ! pixels=$(python3 -I - "$grab" <<'PY'
import sys

data = open(sys.argv[1], "rb").read()
fields = []
at = 0
while len(fields) < 4:
    while data[at:at + 1].isspace():
        at += 1
    if data[at:at + 1] == b"#":
        at = data.index(b"\n", at)
        continue
    end = at
    while not data[end:end + 1].isspace():
        end += 1
    fields.append(data[at:end])
    at = end
magic, width, height, depth = fields[0], int(fields[1]), int(fields[2]), int(fields[3])
if magic != b"P6" or depth != 255:
    sys.exit(f"not a binary 8-bit PPM: {magic!r} depth {depth}")
pixels = data[at + 1:]


def region(x0, y0, x1, y1):
    inset_x, inset_y = (x1 - x0) / 10, (y1 - y0) / 10
    left = round((x0 + inset_x) / 595 * width)
    right = round((x1 - inset_x) / 595 * width)
    top = round((842 - y1 + inset_y) / 842 * height)
    bottom = round((842 - y0 - inset_y) / 842 * height)
    for y in range(top, bottom):
        for x in range(left, right):
            offset = (y * width + x) * 3
            yield pixels[offset], pixels[offset + 1], pixels[offset + 2]


def share(colours, test):
    colours = list(colours)
    return sum(1 for colour in colours if test(*colour)) / max(1, len(colours))


yellow = share(region(70, 752, 290, 786), lambda r, g, b: r > 200 and g > 200 and b < 120)
red = share(region(300, 480, 520, 640), lambda r, g, b: r > 180 and g < 90 and b < 90)
paper = share(region(300, 200, 520, 300), lambda r, g, b: min(r, g, b) > 200)
print(f"{width}x{height} highlight={yellow:.2f} ink={red:.3f} paper={paper:.2f}")
if yellow < 0.5 or red < 0.01 or paper < 0.99:
    sys.exit(1)
PY
); then
    echo "smoke: the annotated page does not show its annotations: ${pixels:-no measure}" >&2
    exit 1
fi

echo "smoke: OK — binary alive for 8 s three times, no QML errors, no auto-bindings ($empty_report; $pages_report; $report; $pixels)"
