#!/bin/sh

set -eu

# diagnostic-report.sh — collect Celestina's own journals and the kernel lines
# around them, after a freeze.
#
# Read-only, by construction and on purpose. It starts nothing, activates
# nothing, changes no service, runs no DDC and touches no hardware. It reads
# files this shell already wrote, asks `journalctl` for lines it already holds,
# redacts what should not travel, and writes one bounded directory somewhere you
# named.
#
#   diagnostic-report.sh                     the previous boot, into /tmp
#   diagnostic-report.sh --boot 0            this boot instead
#   diagnostic-report.sh --run <run_id>      only that invocation's journals
#   diagnostic-report.sh --output DIR        somewhere other than /tmp
#   diagnostic-report.sh --list              what is on disk, collecting nothing
#
# The default is `--boot -1`, the boot before this one, because the reason to
# run this is that the machine had to be reset and the interesting boot is the
# one that ended.
#
# What it cannot do: prove anything. A journal says what this shell did. It does
# not say what caused a GPU to leave the PCIe bus, and a correlation between the
# two is not a cause. See docs/diagnostics.md.

boot=-1
run=
output=
list=0

usage() {
    echo "usage: diagnostic-report.sh [--boot N] [--run RUN_ID] [--output DIR] [--list]" >&2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --boot) boot=${2:?--boot needs a value}; shift 2 ;;
        --run) run=${2:?--run needs a value}; shift 2 ;;
        --output) output=${2:?--output needs a value}; shift 2 ;;
        --list) list=1; shift ;;
        --help|-h) usage; exit 0 ;;
        *) usage; exit 2 ;;
    esac
done

state=${XDG_STATE_HOME:-$HOME/.local/state}
journals=$state/celestina/diagnostics

if [ ! -d "$journals" ]; then
    echo "diagnostic-report: no Celestina journal directory at $journals" >&2
    echo "  Celestina has not run since the journal was added, or it ran as another user." >&2
    exit 1
fi

if [ "$list" -eq 1 ]; then
    echo "Journals under $journals:"
    # Size and modification time only. The contents are the point of the bundle,
    # not of the listing.
    ls -l -- "$journals" | sed 1d
    echo
    echo "Runs present:"
    # The run_id is the segment between the component and the extension.
    for file in "$journals"/*.jsonl; do
        [ -e "$file" ] || continue
        base=${file##*/}
        echo "${base%%.*}" | sed 's/^[a-z-]*-//'
    done | sort -u
    exit 0
fi

if [ -z "$output" ]; then
    output=/tmp/celestina-diagnostic-$(date -u +%Y%m%dT%H%M%SZ)
fi

mkdir -p "$output"
chmod 0700 "$output"
read_files=$output/READ-FILES.txt
: > "$read_files"

note() {
    echo "$1" >>"$read_files"
}

echo "diagnostic-report: writing to $output"

# ── Celestina's own journals ──────────────────────────────────────────────────
#
# The primary evidence. Copied rather than filtered: they are already bounded,
# already redacted at the point of writing, and a filter here could only remove
# the line somebody needed.
mkdir -p "$output/journals"
copied=0
for file in "$journals"/*.jsonl; do
    [ -e "$file" ] || continue
    base=${file##*/}
    if [ -n "$run" ]; then
        case "$base" in
            *"$run"*) ;;
            *) continue ;;
        esac
    fi
    cp -- "$file" "$output/journals/$base"
    note "read: $file"
    copied=$((copied + 1))
done
echo "diagnostic-report: copied $copied journal file(s)"

# The run identifiers actually present in what was copied, so the reader knows
# which invocations the bundle covers without parsing every line.
if command -v grep >/dev/null 2>&1; then
    grep -ho '"run_id":"[^"]*"' "$output"/journals/*.jsonl 2>/dev/null \
        | sort -u >"$output/RUNS.txt" || : >"$output/RUNS.txt"
fi

# ── The kernel's side ─────────────────────────────────────────────────────────
#
# Only the lines that describe the graphics card and the bus it sits on. The
# whole kernel log is not collected: it is large, it is not ours, and it carries
# device and network identities that have nothing to do with this question.
if command -v journalctl >/dev/null 2>&1; then
    journalctl --boot "$boot" --dmesg --no-pager --output=short-iso 2>/dev/null \
        | grep -Ei 'amdgpu|drm|pcie|gpu|vcn|i2c|reset|hang|timeout|device lost' \
        >"$output/kernel-amdgpu.txt" 2>/dev/null || : >"$output/kernel-amdgpu.txt"
    note "read: journalctl --boot $boot --dmesg (filtered to graphics and bus lines)"

    # Celestina's own stderr, when the way it was launched happened to reach
    # journald. This is the mirror, not the evidence: the file above is the
    # evidence precisely because this may be empty.
    journalctl --boot "$boot" --user --no-pager --output=short-iso 2>/dev/null \
        | grep -Ei 'celestina' \
        >"$output/user-celestina.txt" 2>/dev/null || : >"$output/user-celestina.txt"
    note "read: journalctl --boot $boot --user (filtered to celestina lines)"
else
    echo "diagnostic-report: journalctl is not available; kernel lines were not collected" >&2
    : >"$output/kernel-amdgpu.txt"
    : >"$output/user-celestina.txt"
fi

# ── Redaction pass ────────────────────────────────────────────────────────────
#
# Celestina's own lines are redacted where they are written and need nothing
# here. `journalctl` output is somebody else's text: it can carry a home path, a
# hostname or an SSID, and none of those help. This removes the ones a bundle
# should not carry out of the machine.
if command -v sed >/dev/null 2>&1; then
    for file in "$output/kernel-amdgpu.txt" "$output/user-celestina.txt"; do
        [ -s "$file" ] || continue
        sed -i \
            -e "s#$HOME#\$HOME#g" \
            -e "s#/home/[A-Za-z0-9._-]*#/home/USER#g" \
            -e "s#\\b\\([A-Fa-f0-9]\\{2\\}:\\)\\{5\\}[A-Fa-f0-9]\\{2\\}\\b#MAC-REDACTED#g" \
            -e "s#\\bSSID[= ][^ ]*#SSID=REDACTED#g" \
            "$file"
    done
fi

# ── What this bundle is, in the bundle ────────────────────────────────────────
{
    echo "Celestina diagnostic bundle"
    echo
    echo "Collected (UTC): $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Boot selected:   $boot"
    echo "Run filter:      ${run:-<all runs>}"
    echo "Journal source:  $journals"
    echo "Journal files:   $copied"
    echo
    echo "Contents:"
    echo "  journals/            Celestina's own JSONL, one file per process per run"
    echo "  RUNS.txt             the run identifiers present"
    echo "  kernel-amdgpu.txt    journalctl --dmesg, filtered to graphics and bus lines"
    echo "  user-celestina.txt   journalctl --user, filtered to celestina lines"
    echo "  READ-FILES.txt       every source this script read"
    echo
    echo "This bundle records what Celestina did. It does not establish what"
    echo "caused anything. A journal ending mid-operation shows where the record"
    echo "stops, which is not the same as where a fault began, and the last line"
    echo "written before a power cut may be missing entirely."
} >"$output/README.txt"

chmod -R go-rwx "$output" 2>/dev/null || :

echo "diagnostic-report: done"
echo
echo "Read:"
sed 's/^/  /' "$read_files"
echo
echo "Wrote:"
find "$output" -type f | sed "s#^#  #"
