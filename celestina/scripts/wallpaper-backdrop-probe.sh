#!/bin/sh
set -eu

# One-shot evidence, not a gate (CEL-2-E, plan decision 1): does niri place
# the shell's wallpaper surface within the backdrop when the nest's profile
# carries `layer-rule { match namespace="wallpaper"; place-within-backdrop
# true }`? The nest runs a COPY of dev-session.kdl with that rule appended
# (the reference profile and the live session are never touched), on its own
# bus and a private XDG_CONFIG_HOME with the duty on and a generated PNG.
# niri's IPC does not say where a layer surface is placed, so the script
# reports two things: what `niri msg layers` lists, and a measurement. The
# fixture picture has a blue channel of exactly 160 in every pixel, which
# nothing else in the nest has; the nest takes a screenshot of its output in
# the normal view and in the overview, once with the rule and once with the
# unmodified profile, each also with the author's `layout { background-color
# "transparent" }`, and counts the pixels of that picture in each. A surface
# placed within the backdrop is drawn only behind the workspaces. It exits 0
# whatever the answer is.
#
#   usage: scripts/wallpaper-backdrop-probe.sh --binary PATH
usage() {
    echo "usage: scripts/wallpaper-backdrop-probe.sh --binary PATH" >&2
    exit 2
}
[ "$#" -eq 2 ] && [ "$1" = "--binary" ] || usage
bin=$2
[ -x "$bin" ] || { echo "backdrop-probe: $bin is not an executable" >&2; exit 1; }
case "$bin" in
    /*) ;;
    *) bin=$(pwd)/$bin ;;
esac
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
for tool in niri python3 dbus-run-session; do
    command -v "$tool" >/dev/null 2>&1 ||
        { echo "backdrop-probe: $tool is absent" >&2; exit 1; }
done
[ -n "${WAYLAND_DISPLAY:-}" ] ||
    { echo "backdrop-probe: WAYLAND_DISPLAY is empty; the nest needs a host" >&2; exit 1; }

work=$(mktemp -d "${TMPDIR:-/tmp}/celestina-backdrop.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM

# Four profile copies: with and without the rule, each on the reference
# profile's opaque workspace background and with the transparent one the
# author's real config sets (`layout { background-color "transparent" }`).
cp "$here/dev-session.kdl" "$work/profile-plain.kdl"
cp "$work/profile-plain.kdl" "$work/profile.kdl"
cat >> "$work/profile.kdl" <<'KDL'

layer-rule {
    match namespace="wallpaper"
    place-within-backdrop true
}
KDL
for base in profile-plain profile; do
    cp "$work/$base.kdl" "$work/$base-transparent.kdl"
    cat >> "$work/$base-transparent.kdl" <<'KDL'

layout {
    background-color "transparent"
}
KDL
done

# The fixture and the private configuration home.
fixture=$work/wallpapers
mkdir -p "$fixture" "$work/config/celestina"
python3 - "$fixture/default.png" <<'PNG'
import struct, sys, zlib

width, height = 64, 36
rows = b"".join(
    b"\x00" + bytes(c for x in range(width) for c in (x * 4, y * 7, 160))
    for y in range(height)
)

def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

open(sys.argv[1], "wb").write(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(rows))
    + chunk(b"IEND", b"")
)
PNG
cat > "$work/config/celestina/shell.json" <<JSON
{"schema_version":1,"duties":{"wallpaper":"on"},"wallpaper":{"folder":"$fixture"}}
JSON

echo "backdrop-probe: $(niri --version)"
for profile in profile profile-transparent; do
    if niri validate -c "$work/$profile.kdl" >"$work/validate.out" 2>&1; then
        echo "backdrop-probe: niri validate accepts $profile"
    else
        echo "backdrop-probe: niri validate REFUSES $profile:"
        cat "$work/validate.out"
    fi
done

# Runs inside the nest: waits for the wallpaper, lists the layers, then
# captures the output in the normal view and in the overview (the nest's own
# compositor; nothing of the live session).
cat > "$work/shots.sh" <<'SHOTS'
#!/bin/sh
tries=0
until niri msg --json layers 2>/dev/null | grep -q 'celestina-wallpaper'; do
    tries=$((tries + 1))
    [ "$tries" -lt 80 ] || break
    sleep 0.1
done
sleep 3
echo "probe: layers (json)"
niri msg --json layers
niri msg action screenshot-screen --write-to-disk true --show-pointer false \
    --path "$SHOT_DIR/normal.png"
niri msg action toggle-overview
sleep 3
niri msg action screenshot-screen --write-to-disk true --show-pointer false \
    --path "$SHOT_DIR/overview.png"
sleep 1
SHOTS
chmod +x "$work/shots.sh"

# Counts, in a PNG screenshot, the share of sampled pixels with blue == 160.
cat > "$work/count.py" <<'COUNT'
import struct, sys, zlib

data = open(sys.argv[1], "rb").read()
pos, idat = 8, b""
while pos < len(data):
    size, kind = struct.unpack(">I4s", data[pos:pos + 8])
    body = data[pos + 8:pos + 8 + size]
    if kind == b"IHDR":
        width, height, _depth, color = struct.unpack(">IIBB", body[:10])
    elif kind == b"IDAT":
        idat += body
    pos += 12 + size
step = {2: 3, 6: 4}[color]
raw, stride, prev, hits, total = zlib.decompress(idat), width * step, bytearray(width * step), 0, 0
at = 0
for y in range(height):
    kind, line, at = raw[at], bytearray(raw[at + 1:at + 1 + stride]), at + 1 + stride
    for i in range(stride):
        a = line[i - step] if i >= step else 0
        b, c = prev[i], (prev[i - step] if i >= step else 0)
        if kind == 1:
            line[i] = (line[i] + a) & 255
        elif kind == 2:
            line[i] = (line[i] + b) & 255
        elif kind == 3:
            line[i] = (line[i] + ((a + b) >> 1)) & 255
        elif kind == 4:
            pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
            line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
    prev = line
    if y % 4 == 0:
        for x in range(0, width, 4):
            total += 1
            hits += line[x * step + 2] == 160
print("%s: %.1f%% of sampled pixels are the wallpaper's" % (sys.argv[2], 100.0 * hits / total))
COUNT

# One nest per variant: the profile (with or without the rule), its layer
# listing, and the pixel counts.
variant() {
    name=$1
    profile=$2
    shots=$work/shots-$name
    mkdir -p "$shots"
    status=0
    SHOT_DIR=$shots CELESTINA_NEST_CONFIG=$profile XDG_CONFIG_HOME=$work/config \
        timeout 60 "$here/dev-session.sh" --own-bus --monitor none --binary "$bin" \
        --probe "$work/shots.sh" --once "$work/run-$name" -- --exit-after 14 \
        > "$work/niri-$name.log" 2>&1 || status=$?
    echo "backdrop-probe: ==== $name: nest exit=$status shell exit=$(cat "$work/run-$name.status" 2>/dev/null || echo none)"
    sed -n '/^probe: layers (json)/{n;p}' "$work/run-$name.probe" 2>/dev/null |
        sed 's/^/backdrop-probe: layers: /'
    for view in normal overview; do
        if [ -f "$shots/$view.png" ]; then
            python3 "$work/count.py" "$shots/$view.png" "backdrop-probe: $name, $view view"
        else
            echo "backdrop-probe: $name, $view view: no screenshot was written"
        fi
    done
    grep 'wallpaper surface created\|another client holds' "$work/run-$name.err" 2>/dev/null |
        sed 's/^[^ ]* /backdrop-probe: shell: /' | head -n 3 || true
}
variant with-rule "$work/profile.kdl"
variant without-rule "$work/profile-plain.kdl"
variant with-rule-transparent "$work/profile-transparent.kdl"
variant without-rule-transparent "$work/profile-plain-transparent.kdl"
echo "backdrop-probe: niri's IPC lists no placement field (layers carry namespace, output, layer, keyboard_interactivity only);"
echo "backdrop-probe: how to read the shares: with the rule, a surface in the backdrop is covered by the workspace background in the normal view"
echo "backdrop-probe: (about 0%) and shows in the overview; it shows in the normal view only when the workspace background is transparent."
echo "backdrop-probe: Measured once on 2026-10-16 in the nest (niri 26.04): with-rule 0.0% normal / 47.1% overview; without-rule 97.7% / 34.8%;"
echo "backdrop-probe: with-rule-transparent 97.7% / 81.9%; without-rule-transparent 97.7% / 34.8%. So the rule hides the wallpaper outside the overview"
echo "backdrop-probe: unless the real config's transparent workspace background is also set; this script never touches the real config or session."
