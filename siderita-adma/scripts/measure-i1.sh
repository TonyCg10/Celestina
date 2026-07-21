#!/bin/sh

set -eu

usage() {
    echo "uso: $0 BINARY [PID [SEGUNDOS]]" >&2
    echo "variables: SIDERITA_SCENARIO SIDERITA_APP_STAGE SIDERITA_CLOSURE_STAGE SIDERITA_QT_ROOT" >&2
}

if [ "$#" -lt 1 ] || [ "$#" -gt 3 ]; then
    usage
    exit 2
fi

binary=$1
pid=${2:-}
sample_seconds=${3:-60}

if [ ! -f "$binary" ]; then
    echo "error: no existe el binario: $binary" >&2
    exit 2
fi

case "$sample_seconds" in
    ''|*[!0-9]*|0)
        echo "error: SEGUNDOS debe ser un entero positivo" >&2
        exit 2
        ;;
esac

scratch_dir=$(mktemp -d "${TMPDIR:-/tmp}/siderita-measure.XXXXXX")
ldd_file=$scratch_dir/ldd.txt
resolved_file=$scratch_dir/resolved.txt
identity_file=$scratch_dir/identities.txt
mapped_file=$scratch_dir/mapped.txt

cleanup() {
    rm -f "$ldd_file" "$resolved_file" "$identity_file" "$mapped_file"
    rmdir "$scratch_dir" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

bytes_to_mib() {
    awk -v bytes="$1" 'BEGIN { printf "%.2f", bytes / 1048576 }'
}

print_stage() {
    label=$1
    path=$2

    if [ -z "$path" ]; then
        return
    fi
    if [ ! -e "$path" ]; then
        printf '%s_missing %s\n' "$label" "$path"
        return
    fi

    apparent=$(du -sb -- "$path" | awk '{print $1}')
    allocated=$(du -sB1 -- "$path" | awk '{print $1}')
    printf '%s_path %s\n' "$label" "$path"
    printf '%s_apparent_bytes %s\n' "$label" "$apparent"
    printf '%s_allocated_bytes %s\n' "$label" "$allocated"
}

read_cpu_ticks() {
    sed 's/^[^)]*) //' "/proc/$pid/stat" | awk '{print $12 + $13}'
}

read_context_switches() {
    awk '
        /^voluntary_ctxt_switches:/ { voluntary = $2 }
        /^nonvoluntary_ctxt_switches:/ { involuntary = $2 }
        END { print voluntary + involuntary }
    ' "/proc/$pid/status"
}

scenario=${SIDERITA_SCENARIO:-unspecified}
binary_bytes=$(stat -Lc %s "$binary")
binary_type=$(file -b "$binary" 2>/dev/null || true)
[ -n "$binary_type" ] || binary_type=unavailable

printf 'measured_at %s\n' "$(date -Iseconds)"
printf 'scenario %s\n' "$scenario"
printf 'kernel %s\n' "$(uname -srmo)"
printf 'binary %s\n' "$binary"
printf 'binary_bytes %s\n' "$binary_bytes"
printf 'binary_mib %s\n' "$(bytes_to_mib "$binary_bytes")"
printf 'binary_sha256 %s\n' "$(sha256sum "$binary" | awk '{print $1}')"
printf 'binary_type %s\n' "$binary_type"

echo "elf_dynamic_entries_begin"
readelf -d "$binary" | awk '/NEEDED|RPATH|RUNPATH/ { print }'
echo "elf_dynamic_entries_end"

if command -v ldd >/dev/null 2>&1; then
    ldd "$binary" > "$ldd_file" 2>&1 || true
    echo "ldd_begin"
    sed -n '1,240p' "$ldd_file"
    echo "ldd_end"

    awk '
        /=> \// { print $3 }
        /^[[:space:]]*\// { print $1 }
    ' "$ldd_file" | sort -u > "$resolved_file"

    : > "$identity_file"
    while IFS= read -r resolved; do
        [ -f "$resolved" ] || continue
        stat -Lc '%d:%i %s' "$resolved" >> "$identity_file"
    done < "$resolved_file"

    awk '
        !seen[$1]++ { files += 1; bytes += $2 }
        END {
            printf "resolved_elf_files %d\n", files
            printf "resolved_elf_bytes %d\n", bytes
        }
    ' "$identity_file"
fi

print_stage app_stage "${SIDERITA_APP_STAGE:-}"
print_stage closure_stage "${SIDERITA_CLOSURE_STAGE:-}"

if [ -z "$pid" ]; then
    echo "process_metrics skipped_no_pid"
    echo "frame_p95 external_wayland_trace_required"
    exit 0
fi

case "$pid" in
    *[!0-9]*|'')
        echo "error: PID debe ser numérico" >&2
        exit 2
        ;;
esac

if [ ! -r "/proc/$pid/stat" ] || [ ! -r "/proc/$pid/smaps_rollup" ]; then
    echo "error: PID inexistente o sin permisos: $pid" >&2
    exit 2
fi

ticks_start=$(read_cpu_ticks)
context_start=$(read_context_switches)
pss_sum=0
pss_min=999999999
pss_max=0
rss_sum=0
rss_min=999999999
rss_max=0
sample=0

while [ "$sample" -lt "$sample_seconds" ]; do
    sleep 1
    if [ ! -r "/proc/$pid/smaps_rollup" ]; then
        echo "error: el proceso terminó durante la medición" >&2
        exit 3
    fi

    pss_now=$(awk '/^Pss:/ { print $2 }' "/proc/$pid/smaps_rollup")
    rss_now=$(awk '/^Rss:/ { print $2 }' "/proc/$pid/smaps_rollup")
    pss_sum=$((pss_sum + pss_now))
    rss_sum=$((rss_sum + rss_now))
    [ "$pss_now" -lt "$pss_min" ] && pss_min=$pss_now
    [ "$pss_now" -gt "$pss_max" ] && pss_max=$pss_now
    [ "$rss_now" -lt "$rss_min" ] && rss_min=$rss_now
    [ "$rss_now" -gt "$rss_max" ] && rss_max=$rss_now
    sample=$((sample + 1))
done

ticks_end=$(read_cpu_ticks)
context_end=$(read_context_switches)
clock_ticks=$(getconf CLK_TCK)
pss_mean=$((pss_sum / sample_seconds))
rss_mean=$((rss_sum / sample_seconds))

printf 'pid %s\n' "$pid"
printf 'sample_seconds %s\n' "$sample_seconds"
awk -v start="$ticks_start" -v end="$ticks_end" \
    -v hz="$clock_ticks" -v seconds="$sample_seconds" \
    'BEGIN { printf "cpu_one_core_percent %.3f\n", ((end - start) / hz / seconds) * 100 }'
printf 'pss_kb_mean %s\n' "$pss_mean"
printf 'pss_kb_min %s\n' "$pss_min"
printf 'pss_kb_max %s\n' "$pss_max"
printf 'rss_kb_mean %s\n' "$rss_mean"
printf 'rss_kb_min %s\n' "$rss_min"
printf 'rss_kb_max %s\n' "$rss_max"
printf 'context_switch_delta_proxy %s\n' "$((context_end - context_start))"
awk '/^Threads:/ { printf "threads %s\n", $2 }' "/proc/$pid/status"

echo "smaps_rollup_final_begin"
awk '/^(Pss|Rss|Private_Clean|Private_Dirty|Shared_Clean|Shared_Dirty|Swap):/ { print }' \
    "/proc/$pid/smaps_rollup"
echo "smaps_rollup_final_end"

awk '$NF ~ /^\// { print $NF }' "/proc/$pid/maps" | sort -u > "$mapped_file"
: > "$identity_file"
qt_root=${SIDERITA_QT_ROOT:-}

echo "mapped_files_begin"
while IFS= read -r mapped; do
    [ -f "$mapped" ] || continue
    printf '%s\n' "$mapped"
    identity=$(stat -Lc '%d:%i %s' "$mapped")
    qt_marker=other
    if [ -n "$qt_root" ]; then
        case "$mapped" in
            "$qt_root"/*) qt_marker=qt ;;
        esac
    fi
    printf '%s %s\n' "$identity" "$qt_marker" >> "$identity_file"
done < "$mapped_file"
echo "mapped_files_end"

awk '
    !seen[$1]++ {
        files += 1
        bytes += $2
        if ($3 == "qt") {
            qt_files += 1
            qt_bytes += $2
        }
    }
    END {
        printf "mapped_unique_files %d\n", files
        printf "mapped_unique_bytes %d\n", bytes
        printf "mapped_qt_unique_files %d\n", qt_files
        printf "mapped_qt_unique_bytes %d\n", qt_bytes
    }
' "$identity_file"

echo "context_switch_delta_proxy is_not_a_wakeup_count"
echo "frame_p95 external_wayland_trace_required"
